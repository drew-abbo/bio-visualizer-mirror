//! Exports [SeekInfo] and utilities for caching it on the disk.
//!
//! # Disk Caching
//!
//! `local_data::video_cache_path()` is a global directory (for all projects) of
//! cached data. In this directory are entries with hashed file names. Each name
//! is the 16-character hexadecimal hash of a cached video files paths.
//!
//! Each entry is either a file (with the hashed file name), or a directory of
//! files. A directory should only used when there are multiple entries with the
//! same hash. When it's a directory, each file in the directory just has a hex
//! number as it's name.
//!
//! There's a lock file in the root directory that is used to allow multiple
//! processes and threads to access the system at once without it all blowing
//! up. The lock file stores a `u16` ID that can be fetch-incremented.
//!
//! ```txt
//! xor_checksum: u64                   <- XOR of other header fields (words)
//! video_file_path_len: usize          <- the length of the video file's path
//! video_file_path_hash: [u8; 16]      <- the file's hash (for redundancy)
//! frame_count: NonZeroUsize           <- number of frames
//! keyframe_count: NonZeroUsize        <- number of keyframes
//! last_hit: u128                      <- when cache was last hit (nanoseconds since UNIX epoch)
//!
//! [video_file_path]*                  <- the video file path
//!                  ^ `video_file_path_len` bytes
//!
//! (padding up to multiple of 256 bytes)
//!
//! [{
//!     keyframe_idx: usize             <- frame index of keyframe
//!     timestamp: i64                  <- timestamp of keyframe
//! }]*
//!   ^ `keyframe_count` elements
//! ```

use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::iter;
use std::mem;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use util::cast_slice;
use util::local_data;
use util::strn::StrN;

/// Info needed to seek around in an FFmpeg video.
#[derive(Debug, Clone)]
pub struct SeekInfo {
    /// The number of frames in a video.
    pub frame_count: NonZeroUsize,
    /// The frame index and timestamp of every keyframe in a video. This array
    /// is always at least 1 in length with the 1st element's frame index always
    /// being 0.
    pub keyframe_timestamps: Vec<(usize, i64)>,
}

impl SeekInfo {
    /// Load cached [SeekInfo] about a video file `path`.
    #[inline(always)]
    pub fn from_cached(path: impl AsRef<Path>) -> Result<SeekInfoCacheEntry, io::Error> {
        SeekInfoCacheEntry::new(path.as_ref())
    }
}

#[derive(Debug, Clone)]
pub struct SeekInfoCacheEntry {
    seek_info: Option<SeekInfo>,
    video_file_path: PathBuf, // canonicalized
    video_file_path_hash: StrN<16>,
    video_file_timestamp: SystemTime,
}

impl SeekInfoCacheEntry {
    /// Uses the [SeekInfo] from the cache if it was present or returns the
    /// entry again in an [Err].
    pub fn cached(mut self) -> Result<SeekInfo, Self> {
        match self.seek_info.take() {
            Some(seek_info) => Ok(seek_info),
            None => Err(self),
        }
    }

    /// Returns the cached [SeekInfo] if there is one or fills the cache entry
    /// with the result of `f` (if it's [Ok]) and returns it.
    ///
    /// [LoadCacheError::GetErr] indicates that `f` failed.
    /// [LoadCacheError::LoadErr] indicates that disk serialization failed (the
    /// [SeekInfo] `f` returned can be extraced from this variant).
    pub fn or_insert_with<F, E>(mut self, f: F) -> Result<SeekInfo, InsertCacheError<E>>
    where
        F: FnOnce() -> Result<SeekInfo, E>,
        E: Error,
    {
        self = match self.cached() {
            Ok(seek_info) => return Ok(seek_info),
            Err(slf) => slf,
        };

        // Keep the cache directory write-locked while we work here.
        let mut write_lock = local_data::video_cache_write_lock();

        let fetch_result = self.try_fetch().map_err(InsertCacheError::Fetch)?;

        // If another thread or process beat us to it.
        if let Lookup::Hit(seek_info) = fetch_result {
            return Ok(seek_info);
        }

        let seek_info = f().map_err(|e| InsertCacheError::Create(e))?;

        let cache_file_path = match fetch_result {
            Lookup::Hit(_) => unreachable!(), // handled above

            // Path of file to make/overwrite.
            Lookup::MissNotFound(new_file_path) => new_file_path,
            Lookup::MissOutdated(old_file_path) => old_file_path,

            // There's a file w/ the same hash. Turn it into a dir, move the
            // original into it, and make a new file inside.
            Lookup::MissFileCollision(existing_file_path) => {
                debug_assert!(existing_file_path.is_file());

                let write_lock_ref = &mut write_lock;
                let setup = move || -> Result<PathBuf, io::Error> {
                    // tmp_path = .../cache_dir/tmp
                    let mut tmp_path =
                        PathBuf::with_capacity(existing_file_path.as_os_str().len() + 32);
                    existing_file_path.clone_into(&mut tmp_path);
                    tmp_path.pop();
                    tmp_path.push("temp");

                    fs::rename(&existing_file_path, &tmp_path)?;

                    // parent_dir_path = .../cache_dir/hash
                    let parent_dir_path = existing_file_path;

                    fs::create_dir(&parent_dir_path)?;

                    // existing_file_new_path = .../cache_dir/hash/new_id_1
                    let mut existing_file_new_path = parent_dir_path;
                    existing_file_new_path.push(u64_hex(write_lock_ref.next_id()).as_str());

                    fs::rename(&tmp_path, &existing_file_new_path)?;

                    // new_file_path = .../cache_dir/hash/new_id_2
                    let mut new_file_path = existing_file_new_path;
                    new_file_path.pop();
                    new_file_path.push(u64_hex(write_lock_ref.next_id()).as_str());

                    Ok(new_file_path)
                };

                match setup() {
                    Ok(new_file_path) => new_file_path,
                    Err(e) => return Err(InsertCacheError::Insert(e, seek_info)),
                }
            }

            // There's a dir with the same hash. Make a file inside.
            Lookup::MissDirCollision(mut existing_dir_path) => {
                debug_assert!(existing_dir_path.is_dir());

                existing_dir_path.push(u64_hex(write_lock.next_id()).as_str());
                existing_dir_path
            }
        };

        if let Err(e) = self.save_to_cache_file(&cache_file_path, &seek_info) {
            return Err(InsertCacheError::Insert(e, seek_info));
        }

        drop(write_lock);

        #[cfg(debug_assertions)]
        {
            let _read_lock = local_data::video_cache_read_lock();

            let double_check = self.try_fetch_from_cache_file(&cache_file_path).unwrap();
            assert!(
                matches!(double_check, Lookup::Hit(_)),
                "Double check failed: {double_check:?}",
            );
        }

        Ok(seek_info)
    }

    fn save_to_cache_file(
        &self,
        cache_file_path: &Path,
        seek_info: &SeekInfo,
    ) -> Result<(), io::Error> {
        let mut cache_file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(cache_file_path)
            .map(BufWriter::new)?;

        let header = CacheFileHeader::from_seek_info(seek_info, &self.video_file_path);
        let video_file_path_bytes = self.video_file_path.as_os_str().as_encoded_bytes();

        const PADDING_BUFFER: [u8; 256] = [0; 256];
        let video_file_path_end =
            size_of::<CacheFileHeader>() + self.video_file_path.as_os_str().len();
        let keyframes_start = (video_file_path_end + 255) & !255; // rounded up to multiple of 256
        let padding_bytes = &PADDING_BUFFER[..(keyframes_start - video_file_path_end)];

        const _: () = assert!(size_of::<(usize, i64)>() == size_of::<usize>() + size_of::<i64>());
        // Doing this lets us skip an allocation and a memcpy.
        // SAFETY: Reinterpret is safe when there's no padding in the tuples.
        let keyframe_timestamps_as_bytes =
            unsafe { cast_slice::cast_slice::<(usize, i64), u8>(&seek_info.keyframe_timestamps) };

        cache_file.write_all(header.as_bytes())?;
        cache_file.write_all(video_file_path_bytes)?;
        cache_file.write_all(padding_bytes)?;
        cache_file.write_all(keyframe_timestamps_as_bytes)?;
        cache_file.flush()?;

        Ok(())
    }

    fn new(video_file_path: &Path) -> Result<Self, io::Error> {
        let video_file_path = video_file_path.canonicalize()?;
        let video_file_path_hash = hashed_path(&video_file_path);
        let video_file_timestamp = video_file_path.metadata()?.modified()?;
        let mut ret = Self {
            seek_info: None,
            video_file_path,
            video_file_path_hash,
            video_file_timestamp,
        };

        // Keep the cache directory read-locked while we work here.
        let _read_lock_guard = local_data::video_cache_read_lock();

        if let Lookup::Hit(seek_info) = ret.try_fetch()? {
            ret.seek_info = Some(seek_info);
        }
        Ok(ret)
    }

    /// Tries to extract seek info for this entry from the cache directory. In
    /// case of a cache miss, the path of the file that would need to be written
    /// to is returned.
    fn try_fetch(&self) -> Result<Lookup<SeekInfo, PathBuf>, io::Error> {
        let target_cache_file_path = self.target_cache_file_path();

        let is_dir = match target_cache_file_path.metadata() {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Lookup::MissNotFound(target_cache_file_path));
            }

            Ok(metadata) if metadata.is_dir() => true, // directory
            Ok(_) => false,                            // file

            Err(e) => return Err(e),
        };

        if !is_dir {
            return Ok(
                match self.try_fetch_from_cache_file(&target_cache_file_path)? {
                    Lookup::Hit(seek_info) => Lookup::Hit(seek_info),
                    Lookup::MissNotFound(()) => Lookup::MissFileCollision(target_cache_file_path),
                    Lookup::MissOutdated(()) => Lookup::MissOutdated(target_cache_file_path),
                    _ => unreachable!(),
                },
            );
        }

        let dir = fs::read_dir(&target_cache_file_path)?;

        // Reuse path buffer (cheaper than allocating every iteration).
        let mut dir_entry_path = target_cache_file_path;
        for dir_entry in dir {
            let dir_entry = dir_entry?;
            dir_entry_path.push(dir_entry.file_name());

            match self.try_fetch_from_cache_file(&dir_entry_path)? {
                Lookup::Hit(seek_info) => return Ok(Lookup::Hit(seek_info)),
                Lookup::MissOutdated(()) => {
                    return Ok(Lookup::MissOutdated(dir_entry_path));
                }
                Lookup::MissNotFound(()) => {}
                _ => unreachable!(),
            }

            dir_entry_path.pop();
        }
        let target_cache_file_path = dir_entry_path;

        Ok(Lookup::MissDirCollision(target_cache_file_path))
    }

    /// Tries to extract seek info for this entry from a specific cache file
    /// given that cache file's path.
    ///
    /// The only kinds of misses that can be returned are [Lookup::MissNotFound]
    /// and [Lookup::MissOutdated].
    fn try_fetch_from_cache_file(
        &self,
        cache_file_path: &Path,
    ) -> Result<Lookup<SeekInfo>, io::Error> {
        let mut cache_file = File::options()
            .read(true)
            .write(true)
            .open(cache_file_path)?;
        cache_file.lock_shared()?;

        let mut start_buf = [0u8; 256];
        cache_file.read_exact(&mut start_buf)?;
        let header = CacheFileHeader::from_bytes(slice_to_arr(&start_buf[0..64]))?;

        // Validate hash with expected hash
        if cfg!(debug_assertions)
            && header.video_file_path_hash != *self.video_file_path_hash.as_buffer()
        {
            util::debug_log_error!("Hash in cache file doesn't match (ignoring).");
        }

        // Path length mismatch (cache miss).
        if header.video_file_path_len != self.video_file_path.as_os_str().len() {
            return Ok(Lookup::MissNotFound(()));
        }

        let mut large_video_file_path_buf: Vec<u8>;
        let video_file_path_bytes = if header.video_file_path_len <= 192 {
            // If the path fits within the 1st 256 byte read we don't need to
            // allocate any memory.
            &start_buf[64..(64 + header.video_file_path_len)]
        } else {
            // If the path doesn't fit entirely within the 1st 256 byte read, we
            // need to read more. The path ends with padding up to a byte
            // multiple of 256.
            let bytes_read = 256;
            let byte_to_read_to = ((64 + header.video_file_path_len) + 255) & !255;
            let bytes_to_read = byte_to_read_to - bytes_read;

            large_video_file_path_buf = vec![];
            large_video_file_path_buf.reserve_exact(bytes_to_read);
            large_video_file_path_buf.extend(iter::repeat_n(0, bytes_to_read));
            cache_file.read_exact(&mut large_video_file_path_buf)?;

            &large_video_file_path_buf[..header.video_file_path_len]
        };

        // Validate hash with extracted path
        if cfg!(debug_assertions)
            && hashed_bytes(video_file_path_bytes) != self.video_file_path_hash
        {
            util::debug_log_error!("Path in cache file doesn't match its hash (ignoring).");
        }

        // Path contents mismatch (cache miss).
        if video_file_path_bytes != self.video_file_path.as_os_str().as_encoded_bytes() {
            return Ok(Lookup::MissNotFound(()));
        }

        // Cache out of date (cache miss).
        let is_outdated = self.video_file_timestamp > cache_file.metadata()?.modified()?;
        if is_outdated {
            return Ok(Lookup::MissOutdated(()));
        }

        let keyframe_count = header.keyframe_count.get();
        let mut keyframe_timestamps = Vec::<(usize, i64)>::new();
        keyframe_timestamps.reserve_exact(keyframe_count);
        keyframe_timestamps.extend(iter::repeat_n((0, 0), keyframe_count));

        const _: () = assert!(size_of::<(usize, i64)>() == size_of::<usize>() + size_of::<i64>());
        // Doing this lets us skip an allocation and a memcpy.
        // SAFETY: Reinterpret is safe when there's no padding in the tuples.
        let keyframe_timestamps_as_bytes =
            unsafe { cast_slice::cast_slice_mut::<(usize, i64), u8>(&mut keyframe_timestamps) };

        cache_file.read_exact(keyframe_timestamps_as_bytes)?;

        // Validate timestamps
        if cfg!(debug_assertions) {
            if keyframe_timestamps[0].0 != 0 {
                util::debug_log_error!("Cached timestamps dont start at index 0 (ignoring).");
            }
            for window in keyframe_timestamps.windows(2) {
                let ((frame_idx_l, timestamp_l), (frame_idx_r, timestamp_r)) =
                    (window[0], window[1]);
                if frame_idx_l >= frame_idx_r || timestamp_l >= timestamp_r {
                    util::debug_log_error!("Cached timestamps out of order (ignoring).");
                    break;
                }
            }
        }

        // log the cache hit
        let mut new_header = header;
        new_header.last_hit = nanos_since_unix_epoch();
        new_header.xor_checksum = new_header.new_xor_checksum();
        cache_file.unlock()?; // shared unlock
        cache_file.lock()?;
        cache_file.seek(SeekFrom::Start(0))?;
        cache_file.write_all(new_header.as_bytes())?;
        cache_file.sync_all()?;
        cache_file.unlock()?; // exclusive unlock

        Ok(Lookup::Hit(SeekInfo {
            frame_count: new_header.frame_count,
            keyframe_timestamps,
        }))
    }

    fn target_cache_file_path(&self) -> PathBuf {
        let cache_dir_path = local_data::video_cache_path();
        let mut ret = PathBuf::with_capacity(cache_dir_path.as_os_str().len() + 64);
        cache_dir_path.clone_into(&mut ret);
        ret.push(self.video_file_path_hash.as_str());
        ret
    }
}

/// Indicates [SeekInfoCacheEntry::or_insert_with] failed.
#[derive(thiserror::Error, Debug)]
pub enum InsertCacheError<E: Error> {
    #[error("Failed to fetch cache: {0}")]
    Fetch(io::Error),
    #[error(transparent)]
    Create(#[from] E),
    #[error("Failed to insert into cache: {0}")]
    Insert(io::Error, SeekInfo),
}

impl From<InsertCacheError<io::Error>> for io::Error {
    fn from(err: InsertCacheError<io::Error>) -> Self {
        match err {
            InsertCacheError::Fetch(io_err) => io_err,
            InsertCacheError::Create(io_err) => io_err,
            InsertCacheError::Insert(io_err, _seek_info) => io_err,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(C)]
struct CacheFileHeader {
    pub xor_checksum: u64,              // XOR of other header fields (words)
    pub video_file_path_len: usize,     // the length of the video file's path
    pub video_file_path_hash: [u8; 16], // the file's hash (for redundancy)
    pub frame_count: NonZeroUsize,      // number of frames
    pub keyframe_count: NonZeroUsize,   // number of keyframes
    pub last_hit: u128,                 // when cache was last hit (nanoseconds sine UNIX epoch)
}
const _: () = assert!(size_of::<CacheFileHeader>() == 64);
const _: () = assert!(align_of::<CacheFileHeader>() >= align_of::<[u8; 64]>());
const _: () = assert!(size_of::<usize>() == 8);

impl CacheFileHeader {
    /// Create a [CacheFileHeader] from [SeekInfo].
    ///
    /// # Panics
    ///
    /// This function may panic if the [SeekInfo] is invalid.
    #[inline(always)]
    pub fn from_seek_info(seek_info: &SeekInfo, path: impl AsRef<Path>) -> Self {
        Self::from_seek_info_impl(seek_info, path.as_ref())
    }

    /// Validate that some bytes represent a valid [CacheFileHeader].
    pub fn from_bytes(bytes: &[u8; 64]) -> Result<Self, io::Error> {
        // Extracts the bytes for a field given only the field's name.
        macro_rules! field_bytes {
            ($field:ident) => {{
                const OFFSET: usize = mem::offset_of!(CacheFileHeader, $field);
                const SIZE: usize = {
                    const fn size_of_field<F, T, U>(f: F) -> usize
                    where
                        F: FnOnce(T) -> U,
                    {
                        mem::forget(f);
                        mem::size_of::<U>()
                    }
                    size_of_field(|header: CacheFileHeader| header.$field)
                };
                *slice_to_arr::<SIZE>(&bytes[OFFSET..(OFFSET + SIZE)])
            }};
        }

        let xor_checksum = u64::from_ne_bytes(field_bytes!(xor_checksum));
        let video_file_path_len = usize::from_ne_bytes(field_bytes!(video_file_path_len));
        let video_file_path_hash = field_bytes!(video_file_path_hash);
        let frame_count = usize::from_ne_bytes(field_bytes!(frame_count));
        let keyframe_count = usize::from_ne_bytes(field_bytes!(keyframe_count));
        let last_hit = u128::from_ne_bytes(field_bytes!(last_hit));

        fn invalid_data_err(msg: &str) -> io::Error {
            io::Error::new(io::ErrorKind::InvalidData, msg)
        }

        if xor_checksum != Self::new_xor_checksum_from_bytes(bytes) {
            return Err(invalid_data_err("Invalid XOR checksum"));
        }

        let Some(frame_count) = NonZeroUsize::new(frame_count) else {
            return Err(invalid_data_err("Frame count cannot be 0"));
        };
        let Some(keyframe_count) = NonZeroUsize::new(keyframe_count) else {
            return Err(invalid_data_err("Keyframe count cannot be 0"));
        };
        if keyframe_count > frame_count {
            return Err(invalid_data_err("Keyframe count cannot exceed frame count"));
        }

        if xor_checksum != Self::new_xor_checksum_from_bytes(bytes) {
            return Err(invalid_data_err("Invalid XOR checksum"));
        }

        Ok(Self {
            xor_checksum,
            video_file_path_len,
            video_file_path_hash,
            frame_count,
            keyframe_count,
            last_hit,
        })
    }

    /// Convert this [CacheFileHeader] to bytes.
    #[inline(always)]
    pub const fn as_bytes(&self) -> &[u8; 64] {
        // SAFETY: It's fine to reinterpret as bytes since all struct members
        // are just plain old data (`NonZeroUsize`s are kinda an exception, but
        // they're fine as long as they aren't set to 0, something that can't
        // happen since we're not returning a mutable reference).
        unsafe { mem::transmute::<&Self, &[u8; 64]>(self) }
    }

    /// A new [Self::xor_checksum] value given the header's other values.
    #[must_use]
    #[inline]
    pub fn new_xor_checksum(&self) -> u64 {
        Self::new_xor_checksum_from_bytes(self.as_bytes())
    }

    /// A new [Self::xor_checksum] value given the header's other values, from
    /// the header's bytes.
    #[must_use]
    pub fn new_xor_checksum_from_bytes(bytes: &[u8]) -> u64 {
        assert_eq!(bytes.len(), 64);
        bytes[8..] // skip the checksum itself
            .as_chunks::<8>()
            .0
            .iter()
            .map(|word| u64::from_ne_bytes(*word))
            .reduce(|a, b| a ^ b)
            .expect("1+ word")
    }

    fn from_seek_info_impl(seek_info: &SeekInfo, video_file_path: &Path) -> Self {
        let mut ret = CacheFileHeader {
            xor_checksum: 0, // temporary
            video_file_path_len: video_file_path.as_os_str().len(),
            video_file_path_hash: *hashed_path(video_file_path).as_buffer(),
            frame_count: seek_info.frame_count,
            keyframe_count: NonZeroUsize::new(seek_info.keyframe_timestamps.len())
                .expect("at least 1 keyframe"),
            last_hit: nanos_since_unix_epoch(),
        };
        ret.xor_checksum = ret.new_xor_checksum();
        ret
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lookup<H, M = ()> {
    /// An up-to-date item was found in the cache.
    Hit(H),
    /// No item was found in the cache.
    MissNotFound(M),
    /// An item was found in the cache but was out of date.
    MissOutdated(M),
    /// A file was found with the same hash.
    MissFileCollision(M),
    /// A directory was found with the same hash.
    MissDirCollision(M),
}

/// Hashes a path into a 16 character ASCII string. See [hashed_bytes].
#[inline]
fn hashed_path(path: &Path) -> StrN<16> {
    // NOTE: If the Rust version changes, the internal representation may
    // change here, invalidating all cache entries.
    let bytes = path.as_os_str().as_encoded_bytes();
    hashed_bytes(bytes)
}

/// Hashes some bytes into a 16 character ASCII string.
///
/// This function uses a custom implementation of the FNV-1a hashing algorithm
/// instead of using [std::hash::DefaultHasher] so that it's stable across Rust
/// versions.
fn hashed_bytes(bytes: &[u8]) -> StrN<16> {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    u64_hex(hash)
}

#[inline(always)]
fn u64_hex(n: u64) -> StrN<16> {
    fn u64_hex_impl(mut n: u64) -> StrN<16> {
        let mut ret = [b'0'; 16];
        for i in (0..16).rev() {
            ret[i] = b"0123456789abcdef"[(n & 0xF) as usize];
            n >>= 4;
        }
        // SAFETY: Just built string from 16 ASCII characters.
        StrN::from_str(unsafe { str::from_utf8_unchecked(&ret) })
            .expect("StrN<16> fits a str of len 16")
    }
    let ret = u64_hex_impl(n);

    debug_assert!(ret.len() == 16);
    // SAFETY: Just created from 16 chars.
    #[cfg(not(debug_assertions))]
    unsafe {
        std::hint::assert_unchecked(ret.len() == 16)
    };

    ret
}

#[inline(always)]
fn slice_to_arr<const N: usize>(bytes: &[u8]) -> &[u8; N] {
    bytes.try_into().expect("bytes len mismatch")
}

fn nanos_since_unix_epoch() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("it's well after January 1, 1970")
        .as_nanos()
}

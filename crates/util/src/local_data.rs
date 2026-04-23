//! For finding and dealing with a user's local data (e.g. OS-specific paths to
//! local app data, and handling project data). See the [project] submodule.

pub mod project;

use std::env;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, LazyLock, Mutex, MutexGuard};

use crate::read_write_at::{ReadAt, WriteAt};
use crate::saved_file::open_file_with_create_info;
use crate::version;

/// The path to the root of the app's data directory, unique for each user.
///
/// This value will only be computed the first time this function is called.
/// Once computed, subsequent calls are significantly cheaper.
///
/// The directory will be created if it doesn't exist.
pub fn root_path() -> &'static Path {
    static PATH: LazyLock<PathBuf> = LazyLock::new(|| {
        let mut path = PathBuf::from(env::var_os(LOCAL_DATA_ROOT_ENV_VAR).unwrap_or_else(|| {
            panic!("Environment variable `{LOCAL_DATA_ROOT_ENV_VAR}` should be set.")
        }));

        path.reserve_exact(
            LOCAL_APP_DATA_SUFFIX
                .iter()
                .cloned()
                .map(str::len)
                .sum::<usize>()
                + LOCAL_APP_DATA_SUFFIX.len() * 2,
        );
        for dir in LOCAL_APP_DATA_SUFFIX {
            path.push(dir);
        }

        ensure_dirs_exist(&path);
        path
    });

    &PATH
}

/// The path to the directory where the app stores project data, unique for each
/// user.
///
/// This value will only be computed the first time this function is called.
/// Once computed, subsequent calls are significantly cheaper.
///
/// The directory will be created if it doesn't exist.
pub fn projects_path() -> &'static Path {
    static PATH: LazyLock<PathBuf> = LazyLock::new(|| {
        let path = join_paths(root_path(), PROJECTS_DIR_NAME);
        ensure_dirs_exist(&path);
        path
    });
    &PATH
}

/// The path to the directory where the app stores node definitions, unique for each
/// user.
///
/// This value will only be computed the first time this function is called.
/// Once computed, subsequent calls are significantly cheaper.
///
/// The directory will be created if it doesn't exist.
pub fn nodes_path() -> &'static Path {
    static PATH: LazyLock<PathBuf> = LazyLock::new(|| {
        let path = join_paths(root_path(), NODES_DIR_NAME);
        ensure_dirs_exist(&path);
        path
    });
    &PATH
}

/// The path to the directory where the app stores crash reports.
///
/// This value will only be computed the first time this function is called.
/// Once computed, subsequent calls are significantly cheaper.
///
/// The directory will be created if it doesn't exist.
pub fn crash_reports_path() -> &'static Path {
    static PATH: LazyLock<PathBuf> = LazyLock::new(|| {
        let path = join_paths(root_path(), CRASH_REPORTS_DIR_NAME);
        ensure_dirs_exist(&path);
        path
    });
    &PATH
}

/// The path to the directory where cached video information is stored, unique
/// for each user.
///
/// This value will only be computed the first time this function is called.
/// Once computed, subsequent calls are significantly cheaper.
///
/// The directory will be created if it doesn't exist.
pub fn video_cache_path() -> &'static Path {
    static PATH: LazyLock<PathBuf> = LazyLock::new(|| {
        let path = join_paths(root_path(), VIDEO_CACHE_NAME);
        ensure_dirs_exist(&path);
        path
    });
    &PATH
}

/// Returns a guard for a shared advisory read-lock on the
/// [video cache directory](video_cache_path).
///
/// With a read-lock it is okay to read *or write* to files so long as you are
/// locking them (with [File::lock]/[File::lock_shared]). It is *not okay* to
/// delete or move files/directories.
pub fn video_cache_read_lock() -> VideoCacheReadLockGuard {
    let (file, refs_lock, refs_condvar) = video_cache_lock();

    let mut refs_lock_guard = refs_lock.lock().expect(LOCK_NOT_POISONED);
    *refs_lock_guard += 1;

    if *refs_lock_guard == 1 {
        _ = file.lock_shared().inspect_err(|e| {
            crate::debug_log_error!("Failed to read-lock video cache (ignoring): {e}");
        });
    }

    drop(refs_lock_guard);

    VideoCacheReadLockGuard {
        file,
        refs_lock,
        refs_condvar,
    }
}

/// A lock guard returned by [video_cache_read_lock] that unlocks when dropped.
#[derive(Debug)]
pub struct VideoCacheReadLockGuard {
    file: &'static File,
    refs_lock: &'static Mutex<usize>,
    refs_condvar: &'static Condvar,
}

impl Drop for VideoCacheReadLockGuard {
    fn drop(&mut self) {
        let mut refs_lock_guard = self.refs_lock.lock().expect(LOCK_NOT_POISONED);
        *refs_lock_guard -= 1;

        if *refs_lock_guard == 0 {
            _ = self.file.unlock().inspect_err(|e| {
                crate::debug_log_error!("Failed to read-unlock video cache (ignoring): {e}");
            });
        }

        self.refs_condvar.notify_all();
    }
}

impl Clone for VideoCacheReadLockGuard {
    fn clone(&self) -> Self {
        *self.refs_lock.lock().expect(LOCK_NOT_POISONED) += 1;
        Self {
            file: self.file,
            refs_lock: self.refs_lock,
            refs_condvar: self.refs_condvar,
        }
    }
}

/// Returns a guard for an exclusive advisory write-lock on the
/// [video cache directory](video_cache_path).
///
/// A write-lock indicates you are the only one with access to the cache
/// directory. This blocks all readers completely.
pub fn video_cache_write_lock() -> VideoCacheWriteLockGuard {
    let (file, refs_lock, refs_condvar) = video_cache_lock();

    let refs_lock_guard = refs_condvar
        .wait_while(refs_lock.lock().expect(LOCK_NOT_POISONED), |refs| {
            *refs != 0
        })
        .expect(LOCK_NOT_POISONED);

    _ = file.lock().inspect_err(|e| {
        crate::debug_log_error!("Failed to write-lock video cache (ignoring): {e}");
    });

    VideoCacheWriteLockGuard {
        file,
        next_id: None,
        _refs_lock_guard: refs_lock_guard,
    }
}

/// A lock guard returned by [video_cache_write_lock] that unlocks when dropped.
#[derive(Debug)]
pub struct VideoCacheWriteLockGuard {
    file: &'static File,
    next_id: Option<u64>,
    _refs_lock_guard: MutexGuard<'static, usize>,
}

impl VideoCacheWriteLockGuard {
    /// Generate an ID with this lock file.
    pub fn next_id(&mut self) -> u64 {
        let ret = match self.next_id {
            Some(next_id) => next_id,
            None => {
                let mut buf = [0u8; 8];
                self.file
                    .read_exact_at(&mut buf, 0)
                    .expect("Reading an ID from video cache lock shouldn't fail.");
                u64::from_ne_bytes(buf)
            }
        };
        self.next_id = Some(ret + 1);
        ret
    }
}

impl Drop for VideoCacheWriteLockGuard {
    fn drop(&mut self) {
        if let Some(next_id) = self.next_id {
            self.file
                .write_all_at(&next_id.to_ne_bytes(), 0)
                .expect("Writing an ID to video cache lock shouldn't fail.");
        }

        _ = self.file.unlock().inspect_err(|e| {
            crate::debug_log_error!("Failed to write-unlock video cache (ignoring): {e}");
        });
    }
}

const ROOT_DIR_NAME: &str = version::APP_NAME;
const PROJECTS_DIR_NAME: &str = "Projects";
const NODES_DIR_NAME: &str = "Nodes";
const CRASH_REPORTS_DIR_NAME: &str = "CrashReports";
const VIDEO_CACHE_NAME: &str = "VideoCache";
const VIDEO_CACHE_LOCK_NAME: &str = "VideoCacheLock";

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!("Unsupported platform.");

#[cfg(target_os = "windows")]
const LOCAL_DATA_ROOT_ENV_VAR: &str = "LOCALAPPDATA";

#[cfg(any(target_os = "macos", target_os = "linux"))]
const LOCAL_DATA_ROOT_ENV_VAR: &str = "HOME";

#[cfg(target_os = "windows")]
const LOCAL_APP_DATA_SUFFIX: &[&str] = &[ROOT_DIR_NAME];

#[cfg(target_os = "macos")]
const LOCAL_APP_DATA_SUFFIX: &[&str] = &["Library", "Application Support", ROOT_DIR_NAME];

#[cfg(target_os = "linux")]
const LOCAL_APP_DATA_SUFFIX: &[&str] = &[".local", "share", ROOT_DIR_NAME];

const LOCK_NOT_POISONED: &str = "The lock isn't poisoned.";

/// Returns a file and the number of active readers in this process.
fn video_cache_lock() -> (&'static File, &'static Mutex<usize>, &'static Condvar) {
    static LOCK_FILE: LazyLock<File> = LazyLock::new(|| {
        let (file, created) =
            open_file_with_create_info(join_paths(video_cache_path(), VIDEO_CACHE_LOCK_NAME))
                .expect("Opening/creating video cache lock file shouldn't fail.");

        if created {
            file.write_all_at(&0u64.to_ne_bytes(), 0)
                .expect("Writing an initial ID to video cache lock shouldn't fail.");
        }

        file
    });

    static LOCK_FILE_REFS: Mutex<usize> = Mutex::new(0);
    static REFS_CONDVAR: Condvar = Condvar::new();

    (&LOCK_FILE, &LOCK_FILE_REFS, &REFS_CONDVAR)
}

fn join_paths(a: impl AsRef<Path>, b: impl AsRef<Path>) -> PathBuf {
    let (a, b) = (a.as_ref(), b.as_ref());

    let mut ret = PathBuf::new();
    ret.reserve_exact(a.as_os_str().len() + 2 + b.as_os_str().len());

    a.clone_into(&mut ret);
    ret.push(b);

    ret
}

fn ensure_dirs_exist(path: &Path) {
    fs::create_dir_all(path).expect("Creating local data dirs shouldn't fail.");
}

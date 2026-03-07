//! Exports [FFmpegVideo].

use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::SystemTime;
use std::{any, mem, thread, usize};

use util::channels::request_channel::Request;

use ffmpeg::codec::Context as FFmpegCodecContext;
use ffmpeg::codec::decoder::Video as FFmpegVideoDecoder;
use ffmpeg::format::Pixel as FFmpegPixelFormat;
use ffmpeg::format::context::Input as FFmpegInputFormatContext;
use ffmpeg::media::Type as FFmpegMediaType;
use ffmpeg::software::scaling::Context as FFmpegScalingContext;
use ffmpeg::software::scaling::flag::Flags as FFmpegScalingFlags;
use ffmpeg_next::{self as ffmpeg};

use super::FFmpegResult;
use crate::fps::{Fps, FpsError};
use crate::frame::{Dimensions, FrameBuffer, Pixel, RescaleMethod};

pub type FFmpegVideoFrame = ffmpeg::frame::Video;

impl FrameBuffer for FFmpegVideoFrame {
    fn dimensions(&self) -> Dimensions {
        (self.width(), self.height())
            .try_into()
            .expect("The dimensions should be valid")
    }

    fn pixels_mut(&mut self) -> &mut [Pixel] {
        assert!(
            self.format() == FFmpegPixelFormat::RGBA,
            "Bad FFmpeg pixel format (not RGBA)."
        );

        let frame_area = self.width() as usize * self.height() as usize;
        let expected_buffer_len = frame_area * size_of::<Pixel>();
        assert!(
            self.data(0).len() == expected_buffer_len,
            "Bad FFmpeg buffer length."
        );

        let buffer_alignment = self.data(0).as_ptr() as usize % align_of::<Pixel>();
        assert!(buffer_alignment == 0, "Bad FFmpeg buffer alignment.");

        // SAFETY: The src data is RGBA, aligned properly, and has the right
        // length. It's safe to cast to pixels.
        unsafe { util::cast_slice::cast_slice_mut(self.data_mut(0)) }
    }
}

/// A video (courtesy for FFmpeg).
///
/// If any method returns an error, the object should be discarded. Its behavior
/// becomes undefined.
pub struct FFmpegVideo {
    inner: FFmpegVideoInner,
    last_frame: Option<FFmpegVideoFrame>,
    paused: bool,
    playhead: usize,
    duration: NonZeroUsize,
}

impl FFmpegVideo {
    /// The [decoder pixel format](FFmpegVideoDecoder::format) that all
    /// [FFmpeg video frames](FFmpegVideoFrame) should be using.
    pub const TARGET_PIXEL_FORMAT: FFmpegPixelFormat = FFmpegPixelFormat::RGBA;

    /// Open a video file with FFmpeg.
    ///
    /// The first time opening a video that doesn't specify its duration (in
    /// frames) in its metadata will result in the entire video being decoded to
    /// determine the video's length. The returned [Request] will resolve when
    /// the duration has been determined. This can be a computationally
    /// expensive and long process.
    ///
    /// If it needs to be computed, the file's video duration will be cached
    /// along with its last-modified timestamp.
    pub fn new(
        path: &Path,
        rescale: Option<(Dimensions, RescaleMethod)>,
        paused: bool,
    ) -> Request<FFmpegResult<Self>> {
        Self::new_mapped(path, rescale, paused, |r| r)
    }

    /// The same as [Self::new] except `f` is called on the result whenever it's
    /// done being constructed.
    pub fn new_mapped<F, R>(
        path: &Path,
        rescale: Option<(Dimensions, RescaleMethod)>,
        paused: bool,
        f: F,
    ) -> Request<R>
    where
        F: Send + FnOnce(FFmpegResult<Self>) -> R + 'static,
        R: Send + 'static,
    {
        let mut inner = match FFmpegVideoInner::new(path, rescale) {
            Ok(inner) => inner,
            Err(e) => return f(Err(e)).into(),
        };

        // Try to get duration from metadata.
        if let Some(known_duration) = inner.known_frame_count() {
            return f(Ok(Self::from_parts(inner, paused, known_duration))).into();
        }

        // See if we've cached the duration.
        let Ok(path_info) = frame_count_cache::PathInfo::new(path) else {
            return f(Err(ffmpeg::Error::Unknown)).into();
        };
        if let Some(cached_duration) = frame_count_cache::get(&path_info) {
            return f(Ok(Self::from_parts(inner, paused, cached_duration))).into();
        }

        // Otherwise we'll need to decode the entire stream to figure out the
        // duration. Instead of blocking, we'll do this on another thread and
        // return a request that will resolve eventually.
        let (req, res) = Request::new();
        thread::spawn(move || {
            let count_frames_and_construct = || {
                let duration = inner.count_frames(|| res.connection_open())?;
                inner.seek_playhead(0)?;

                frame_count_cache::insert(path_info, duration);

                Ok(Self::from_parts(inner, paused, duration))
            };

            let response = f(count_frames_and_construct());
            _ = res.respond(response);
        });
        req
    }

    /// Write the next frame to the frame buffer `dest_frame`.
    ///
    /// Calling this function when [Self::playhead] is more than or equal to
    /// [Self::duration] will result in the function panicking.
    pub fn write_next(
        &mut self,
        recycled_frame: Option<FFmpegVideoFrame>,
    ) -> FFmpegResult<FFmpegVideoFrame> {
        assert!(
            self.playhead < self.duration(),
            "Can't play past video duration."
        );

        if !self.paused {
            self.playhead += 1;

            return if let Some(last_frame) = self.last_frame.take() {
                Ok(last_frame)
            } else {
                let mut dest_frame = recycled_frame.unwrap_or_else(|| FFmpegVideoFrame::empty());
                self.inner
                    .write_next_frame_in_stream(&mut dest_frame)
                    .map(|_| dest_frame)
            };
        }

        if let Some(ref last_frame) = self.last_frame {
            if let Some(mut recycled_frame) = recycled_frame
                && recycled_frame.format() == Self::TARGET_PIXEL_FORMAT
                && recycled_frame.width() == self.inner.src_dimensions.width()
                && recycled_frame.height() == self.inner.src_dimensions.height()
            {
                // SAFETY: This can silently fail and cause U.B. if the frames
                // don't have the same pixel format or dimensions (ffmpeg-next
                // sucks). We just checked though so it's fine.
                recycled_frame.clone_from(last_frame);
                Ok(recycled_frame)
            } else {
                Ok(last_frame.clone())
            }
        } else {
            let mut dest_frame = recycled_frame.unwrap_or_else(|| FFmpegVideoFrame::empty());

            self.inner.write_next_frame_in_stream(&mut dest_frame)?;
            self.last_frame = Some(dest_frame.clone());
            Ok(dest_frame)
        }
    }

    /// Seek to a frame index so that the next frame that will be written is
    /// `new_playhead`.
    ///
    /// Calling this function with a `new_playhead` value greater than
    /// [Self::duration] will result in the function panicking. `new_playhead`
    /// can equal the duration (in this case [Self::write_next] must not be
    /// called).
    pub fn seek_playhead(&mut self, new_playhead: usize) -> FFmpegResult<()> {
        assert!(
            new_playhead <= self.duration(),
            "Can't seek past video duration."
        );

        if new_playhead == self.playhead {
            return Ok(());
        }

        self.last_frame = None;
        self.playhead = new_playhead;

        // No frames can be fetched from after the stream is over so if we're
        // seeking to the end we can skip the real work.
        if new_playhead == self.duration() {
            return Ok(());
        }

        self.inner.seek_playhead(new_playhead)
    }

    /// The index of the next frame that will be written.
    ///
    /// The returnd value will never be more than [Self::duration], but it can
    /// equal it (in this case [Self::write_next] must not be called).
    pub const fn playhead(&self) -> usize {
        self.playhead
    }

    /// The number of frames this video has.
    ///
    /// This value will never be 0. Also see [Self::duration_non_zero].
    pub const fn duration(&self) -> usize {
        self.duration.get()
    }

    /// The number of frames this video has.
    pub const fn duration_non_zero(&self) -> NonZeroUsize {
        self.duration
    }

    /// The intended (native) [Fps] playback speed of this video.
    pub const fn src_fps(&self) -> Fps {
        self.inner.src_fps
    }

    /// The intended (native) dimensions of the frames in this video.
    pub const fn src_dimensions(&self) -> Dimensions {
        self.inner.src_dimensions
    }

    /// The dimensions of the frames that will be produced.
    pub const fn dest_dimensions(&self) -> Dimensions {
        match self.rescale() {
            Some((dest_dimensions, _)) => dest_dimensions,
            None => self.inner.src_dimensions,
        }
    }

    /// Sets whether or not the stream will be paused.
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Whether or not the stream is paused.
    pub fn paused(&mut self) -> bool {
        self.paused
    }

    /// Set how frames should be rescaled if needed.
    ///
    /// If `dest_dimensions` is the same as [Self::src_dimensions],
    /// [Self::rescale] will return [None].
    pub fn set_rescale(
        &mut self,
        dest_dimensions: Dimensions,
        rescale_method: RescaleMethod,
    ) -> FFmpegResult<()> {
        self.inner.set_rescale(dest_dimensions, rescale_method)
    }

    /// Whether or not frames are being rescaled and if so to what dimensions
    /// and how.
    pub const fn rescale(&self) -> Option<(Dimensions, RescaleMethod)> {
        self.inner.rescale()
    }

    const fn from_parts(inner: FFmpegVideoInner, paused: bool, duration: NonZeroUsize) -> Self {
        Self {
            inner,
            last_frame: None,
            paused,
            playhead: 0,
            duration,
        }
    }
}

// The FFmpeg types don't implement `Debug` so we're doing it by hand.
impl Debug for FFmpegVideo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let type_name = any::type_name::<FFmpegVideo>().split("::").last().unwrap();
        f.debug_struct(type_name).finish_non_exhaustive()
    }
}

/// A basic FFmpeg video stream that can write formatted and resized frames from
/// a stream in order and can be seeked. See [FFmpegVideo].
struct FFmpegVideoInner {
    // Frame Generation:
    input_context: FFmpegInputFormatContext,
    decoder: FFmpegVideoDecoder,
    reformatter: FrameReformatter,
    draining: bool,

    // Src Info (Final):
    target_stream_index: usize,
    src_fps: Fps,
    src_dimensions: Dimensions,
}

impl<'a> FFmpegVideoInner {
    /// Create an [FFmpegVideo] with everything but the duration.
    pub fn new(path: &'a Path, rescale: Option<(Dimensions, RescaleMethod)>) -> FFmpegResult<Self> {
        // This object is a handle to the file we opened. Right now, this is
        // just the kind of container (e.g. MP4, MKV) and FFmpeg has none of the
        // actual video/audio data yet (only the file's metadata).
        let input_context = ffmpeg::format::input(path)?;

        // Video containers can have multiple video streams. This picks the one
        // FFmpeg thinks is the best.
        let best_video_stream = input_context
            .streams()
            .best(FFmpegMediaType::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;

        // When we're going through packets later, we'll need to make sure to
        // ignore all packets that aren't from the video stream we care about.
        // To do that, we'll save the video stream's index.
        let target_stream_index = best_video_stream.index();

        // This gathers the information we'll need for decoding our video
        // stream (e.g. bitrate, resolution, frames per second). We need this to
        // create a decoder.
        let decoder_context = FFmpegCodecContext::from_parameters(best_video_stream.parameters())?;

        // This is the actual object we can use to decode our video streams into
        // frames. It also provides more information about the video.
        let decoder = decoder_context.decoder().video()?;

        const BAD_STATS_ERR: ffmpeg::Error = ffmpeg::Error::InvalidData;

        // The stream should have constant frame spacing. If it doesn't we might
        // be able to detect that early here.
        if best_video_stream.avg_frame_rate() != best_video_stream.rate() {
            return Err(BAD_STATS_ERR);
        }

        let src_fps: Fps = best_video_stream
            .avg_frame_rate()
            .try_into()
            .or(Err(BAD_STATS_ERR))?;

        let src_dimensions =
            Dimensions::new(decoder.width(), decoder.height()).ok_or(BAD_STATS_ERR)?;

        let (dest_dimensions, rescale_method) =
            rescale.unwrap_or((src_dimensions, RescaleMethod::default()));

        let reformatter = FrameReformatter::new(&decoder, dest_dimensions, rescale_method)?;

        Ok(Self {
            input_context,
            decoder,
            reformatter,
            draining: false,
            target_stream_index,
            src_fps,
            src_dimensions,
        })
    }

    /// Writes the next frame in the stream to `dest_frame`.
    pub fn write_next_frame_in_stream(
        &mut self,
        dest_frame: &mut FFmpegVideoFrame,
    ) -> FFmpegResult<()> {
        // Internally, the `packets` iterator mutates our `input_context`, not
        // its own internal state. This means we can cheaply re-create the
        // packets iterator each time we want to grab a new packet.
        let mut packets = self
            .input_context
            .packets()
            .filter_map(|(packet_stream, packet)| {
                // Skip packets that aren't for our stream.
                (packet_stream.index() == self.target_stream_index).then(|| packet)
            });

        loop {
            // Try to write a new reformatted frame to the output buffer.
            let decode_result = self.reformatter.reformat(dest_frame, |dest_frame| {
                // https://ffmpeg.org/doxygen/8.0/group__lavc__decoding.html#ga11e6542c4e66d3028668788a1a74217c
                self.decoder.receive_frame(dest_frame)
            });

            let Err(decode_err) = decode_result else {
                // Congratulations, we wrote a frame to the output buffer.
                return Ok(());
            };

            // `EAGAIN` means we haven't sent enough packets for a frame yet. If
            // that happens, we have to load some packets and keep going.
            if decode_err != ffmpeg::error::EAGAIN.into() {
                // Something must be wrong w/ the file or the object's state.
                return Err(decode_err);
            }

            // If we're draining it means we're already out of packets to send.
            if self.draining {
                return Err(ffmpeg::Error::Eof);
            }

            if let Some(packet) = packets.next() {
                // Send the next packet to the decoder and try again.
                self.decoder.send_packet(&packet)?;
            } else {
                // If we're out of packets we'll tell the decoder to drain what
                // it has.
                self.decoder.send_eof()?;
                self.draining = true;
            }
        }
    }

    /// See [FFmpegVideo::seek_playhead].
    pub fn seek_playhead(&mut self, new_playhead: usize) -> FFmpegResult<()> {
        // FFmpeg seeks by timestamp, not by frame. This is going to be a pain.

        // For now we'll just handle the seek to 0 case.
        if new_playhead == 0 {
            self.input_context.seek(0, ..)?;
            self.decoder.flush();
            return Ok(());
        }

        todo!()
    }

    /// See [FFmpegVideo::rescale].
    pub const fn rescale(&self) -> Option<(Dimensions, RescaleMethod)> {
        match self.reformatter.rescale_method {
            Some(rescale_method) => Some((self.reformatter.dest_dimensions, rescale_method)),
            None => None,
        }
    }

    /// See [FFmpegVideo::set_rescale].
    pub fn set_rescale(
        &mut self,
        dest_dimensions: Dimensions,
        rescale_method: RescaleMethod,
    ) -> FFmpegResult<()> {
        let reformatter = &mut self.reformatter;

        let same_dest_dimensions = reformatter.dest_dimensions == dest_dimensions;
        let same_rescale_method = reformatter.rescale_method == Some(rescale_method);
        let rescale_needed = self.src_dimensions != dest_dimensions;

        if same_dest_dimensions && (same_rescale_method || !rescale_needed) {
            return Ok(());
        }

        *reformatter = FrameReformatter::new(&self.decoder, dest_dimensions, rescale_method)?;
        Ok(())
    }

    /// The frame count if it is known (in the metadata).
    pub fn known_frame_count(&self) -> Option<NonZeroUsize> {
        // The frame count is an optional field that some containers provide.
        // Not all video formats provide it though.
        match self
            .input_context
            .stream(self.target_stream_index)
            .expect("target stream should be present")
            .frames()
        {
            ..=0 => None,
            frames => Some(NonZeroUsize::new(frames as usize).unwrap()),
        }
    }

    /// Decode the entire rest of the video to see how many frames it has. This
    /// function will move the playhead to the end of the video. This is a very
    /// expensive function.
    ///
    /// Every once in a while `continue_predicate` to ensure the work is even
    /// still needed. If it returns `false`, the function will immediately exit
    /// with an error derived from [ffmpeg::error::ECANCELED].
    pub fn count_frames<F>(&mut self, mut continue_predicate: F) -> FFmpegResult<NonZeroUsize>
    where
        F: FnMut() -> bool,
    {
        let old_reformatter = mem::replace(
            &mut self.reformatter,
            // SAFETY: We will not use any frames `self.formatter` writes to
            // until we put the original reformatter back.
            unsafe { FrameReformatter::with_no_formatting() },
        );

        let mut frame_count: usize = 0;

        let mut dest_frame = FFmpegVideoFrame::empty();
        loop {
            // Every once if we check and it's not listening, we can stop.
            if frame_count % 64 == 0 && !continue_predicate() {
                return Err(ffmpeg::error::ECANCELED.into());
            }

            match self.write_next_frame_in_stream(&mut dest_frame) {
                Ok(_) => frame_count += 1,
                Err(ffmpeg::Error::Eof) => break,
                Err(e) => return Err(e),
            }
        }

        drop(mem::replace(&mut self.reformatter, old_reformatter));

        // We'll call a frame count of 0 an EOF error.
        frame_count.try_into().map_err(|_| ffmpeg::Error::Eof)
    }
}

/// Used to rescale and reformat video frames.
struct FrameReformatter {
    inner: Option<FrameReformatterInner>,
    dest_dimensions: Dimensions,
    rescale_method: Option<RescaleMethod>,
}

impl FrameReformatter {
    /// Create a [FrameReformatter].
    pub fn new(
        decoder: &FFmpegVideoDecoder,
        dest_dimensions: Dimensions,
        rescale_method: RescaleMethod,
    ) -> FFmpegResult<Self> {
        let dimensions_match = dest_dimensions.width() == decoder.width()
            && dest_dimensions.height() == decoder.height();

        // If the dimensions and pixel format both don't need to change, we
        // won't need to do any reformatting.
        if dimensions_match && decoder.format() == FFmpegVideo::TARGET_PIXEL_FORMAT {
            return Ok(Self {
                inner: None,
                dest_dimensions,
                rescale_method: None,
            });
        }

        let scaling_flags = if dimensions_match {
            FFmpegScalingFlags::empty()
        } else {
            match rescale_method {
                RescaleMethod::NearestNeighbor => FFmpegScalingFlags::POINT,
                RescaleMethod::Bilinear => FFmpegScalingFlags::BILINEAR,
                RescaleMethod::Bicubic => FFmpegScalingFlags::BICUBIC,
            }
        };

        let scaler = FFmpegScalingContext::get(
            // Src:
            decoder.format(),
            decoder.width(),
            decoder.height(),
            // Dest:
            FFmpegVideo::TARGET_PIXEL_FORMAT,
            dest_dimensions.width(),
            dest_dimensions.height(),
            // Rescale method:
            scaling_flags,
        )?;

        Ok(Self {
            inner: Some(FrameReformatterInner {
                scaler,
                intermediate_buffer: FFmpegVideoFrame::empty(),
            }),
            dest_dimensions,
            rescale_method: (!dimensions_match).then(|| rescale_method),
        })
    }

    /// If formating needs to happen, apply the `f` operation to an intermediate
    /// buffer and copy that buffer to an RGBA buffer `dest_frame`. If
    /// formatting doesn't need to happen `f` will just be applied directly to
    /// `dest_frame`.
    ///
    /// `dest_frame` should be RGBA and have the the same dest dimensions as was
    /// passed to the formatter. If it doesn't you'll end up with memory
    /// reallocations.
    pub fn reformat<F, R>(&mut self, dest_frame: &mut FFmpegVideoFrame, f: F) -> FFmpegResult<R>
    where
        F: FnOnce(&mut FFmpegVideoFrame) -> FFmpegResult<R>,
    {
        let Some(inner) = &mut self.inner else {
            return f(dest_frame);
        };

        let ret = f(&mut inner.intermediate_buffer)?;
        inner.scaler.run(&inner.intermediate_buffer, dest_frame)?;
        Ok(ret)
    }

    /// Whether or not rescaling is needed.
    pub const fn is_rescale_needed(&self) -> bool {
        self.inner.is_some()
    }

    /// Create a reformatter that does no formatting ever. The result of
    /// [Self::reformat] may not have the right pixel format or dimensions.
    ///
    /// # Safety
    ///
    /// Do not assume anything about the format or dimensions of what gets
    /// written by [Self::reformat].
    pub const unsafe fn with_no_formatting() -> Self {
        Self {
            inner: None,
            dest_dimensions: const { Dimensions::new(1, 1).unwrap() },
            rescale_method: None,
        }
    }
}

// SAFETY: The `ffmpeg::software::scaling::Context` type (aliased
// `FFmpegScalingContext` here) which we're storing in our `FrameReformatter`
// struct *is* safe to send/share between threads. The library authors likely
// just didn't think to mark it. I opened an issue about it here:
// <https://github.com/zmwangx/rust-ffmpeg/issues/252>
unsafe impl Send for FrameReformatter {}
unsafe impl Sync for FrameReformatter {}

struct FrameReformatterInner {
    scaler: FFmpegScalingContext,
    intermediate_buffer: FFmpegVideoFrame,
}

mod frame_count_cache {
    use std::io;

    use super::*;

    /// Info about a path.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PathInfo {
        /// The [canonicalized](Path::canonicalize) path.
        pub path: PathBuf,
        /// The last last modification time of the object at the path.
        pub modified: SystemTime,
    }

    impl PathInfo {
        /// Get some info about a path or [None] if it can't be determined.
        pub fn new(path: &Path) -> Result<Self, io::Error> {
            let path = path.canonicalize()?;
            let modified = path.metadata()?.modified()?;
            Ok(Self { path, modified })
        }
    }

    /// Get the duration of the a video file if it was cached with [put].
    pub fn get(path_info: &PathInfo) -> Option<NonZeroUsize> {
        let mut cache = CACHE.lock().expect(LOCK_UNPOISONED);

        let Some((cached_duration, cached_modified)) = cache.get(&path_info.path).cloned() else {
            return None;
        };

        if cached_modified != path_info.modified {
            cache.remove(&path_info.path);
            return None;
        }

        Some(cached_duration)
    }

    /// Cache the duration of a video file.
    pub fn insert(path: PathInfo, frames: NonZeroUsize) {
        let mut cache = CACHE.lock().expect(LOCK_UNPOISONED);
        cache.insert(path.path, (frames, path.modified));
    }

    static CACHE: LazyLock<Mutex<HashMap<PathBuf, (NonZeroUsize, SystemTime)>>> =
        LazyLock::new(Mutex::default);

    const LOCK_UNPOISONED: &str = "The lock shouldn't be poisoned.";
}

impl TryFrom<ffmpeg::Rational> for Fps {
    type Error = FpsError;

    fn try_from(ffmpeg::Rational(num, den): ffmpeg::Rational) -> Result<Self, Self::Error> {
        Fps::from_frac(
            u32::try_from(num).unwrap_or(0),
            u32::try_from(den).unwrap_or(0),
        )
    }
}

//! Exports [FFmpegVideo].

mod seek_info;
use seek_info::SeekInfo;

mod inner;
use inner::*;

use std::convert::Infallible;
use std::fmt::{self, Debug};
use std::num::NonZeroUsize;
use std::path::Path;
use std::{any, thread};

use ffmpeg::format::Pixel as FFmpegPixelFormat;
use ffmpeg::software::scaling::Context as FFmpegScalingContext;
use ffmpeg::software::scaling::flag::Flags as FFmpegScalingFlags;
use ffmpeg_next as ffmpeg;

use util::channels::request_channel::Request;

use super::FFmpegResult;
use crate::fps::Fps;
use crate::frame::{Dimensions, FrameBuffer, Pixel, RescaleMethod};

pub type FFmpegVideoFrame = ffmpeg::frame::Video;

impl FrameBuffer for FFmpegVideoFrame {
    fn dimensions(&self) -> Dimensions {
        (self.width(), self.height()).into()
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

/// The [decoder pixel format](FFmpegVideoDecoder::format) that all
/// [FFmpeg video frames](FFmpegVideoFrame) should be using.
const TARGET_PIXEL_FORMAT: FFmpegPixelFormat = FFmpegPixelFormat::RGBA;

/// A video (courtesy for FFmpeg).
///
/// If any method returns an error, the object should be discarded. Its behavior
/// becomes undefined.
pub struct FFmpegVideo {
    inner: FFmpegVideoInner,
    last_frame: Option<FFmpegVideoFrame>,
    paused: bool,
    playhead: usize,
    seek_info: SeekInfo,
    last_keyframe_array_idx: usize, // index in `SeekInfo` array not frame index
}

impl FFmpegVideo {
    /// Open a video file with FFmpeg.
    ///
    /// To enable frame accurate seeking, this struct needs to know the exact
    /// duration (in frames) of a video and the frame index of each keyframe. To
    /// Figure this out, the enture video will need to be decoded, which can
    /// take a while. That is why this function returns a [Request]. Once this
    /// information is determined, it's cached on disk, meaning subsequent loads
    /// of the same video file should be much faster (this is true across
    /// processes, projects, ect., it only invalidates when the file changes).
    ///
    /// Before the request resolves, the `f` is called on the [FFmpegVideo] so
    /// that you can get a request that resolves to something else.
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

        let cache_entry = match SeekInfo::from_cached(path) {
            Ok(cache_entry) => Some(match cache_entry.cached() {
                Ok(seek_info) => {
                    util::debug_log_info!("Video seek info cache: HIT");
                    let video = Self::from_parts(inner, paused, seek_info);
                    return f(Ok(video)).into();
                }
                Err(cache_entry) => {
                    util::debug_log_info!("Video seek info cache: MISS");
                    cache_entry
                }
            }),

            // If cache lookup fails we just won't use the cache.
            Err(e) => {
                util::debug_log_error!("Failed to fetch seek info from cache (ignoring): {e}");
                None
            }
        };

        // Since we can't immediately resolve the request, we'll spawn a thread
        // that will figure the seek info and respond to the request.
        let (req, res) = Request::new();
        thread::spawn(move || {
            let mut seek_info = match inner.determine_seek_info(|_| res.connection_open()) {
                Ok(seek_info) => seek_info,
                Err(e) => {
                    _ = res.respond(f(Err(e)));
                    return;
                }
            };

            if let Some(cache_entry) = cache_entry {
                seek_info = cache_entry
                    .or_insert_with::<_, Infallible>(|| Ok(seek_info))
                    .inspect_err(|e| util::debug_log_error!("Failed to cache seek info: {e}"))
                    .expect("reading from cache worked so writing shouldn't fail");
            }

            let video = Self::from_parts(inner, paused, seek_info);
            let response = f(Ok(video));
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

        self.rescale_last_frame_if_needed()?;

        if !self.paused {
            self.playhead += 1;

            // Keep the last keyframe updated.
            if let Some((next_keyframe_frame_idx, _)) = self
                .seek_info
                .keyframe_timestamps
                .get(self.last_keyframe_array_idx + 1)
                .cloned()
                && self.playhead == next_keyframe_frame_idx
            {
                self.last_keyframe_array_idx += 1;
            }

            return if let Some(last_frame) = self.last_frame.take() {
                Ok(last_frame)
            } else {
                self.inner.next_frame(recycled_frame)
            };
        }

        if let Some(ref last_frame) = self.last_frame {
            if let Some(mut recycled_frame) = recycled_frame
                && recycled_frame.format() == TARGET_PIXEL_FORMAT
                && recycled_frame.width() == self.inner.src_dimensions().width()
                && recycled_frame.height() == self.inner.src_dimensions().height()
            {
                // SAFETY: This can silently fail and cause U.B. if the frames
                // don't have the same pixel format or dimensions (`ffmpeg-next`
                // sucks). We just checked though so it's fine.
                recycled_frame.clone_from(last_frame);
                Ok(recycled_frame)
            } else {
                Ok(last_frame.clone())
            }
        } else {
            let new_frame = self.inner.next_frame(recycled_frame)?;
            self.last_frame = Some(new_frame.clone());
            Ok(new_frame)
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

        // No frames can be fetched from after the stream is over so if we're
        // seeking to the end we can skip the real work.
        if new_playhead == self.duration() {
            return Ok(());
        }

        if self.should_walk_instead_of_seek(new_playhead) {
            self.inner.skip_frames(new_playhead - self.playhead)?;
        } else {
            self.last_keyframe_array_idx =
                self.inner.seek_playhead(new_playhead, &self.seek_info)?;
        }
        self.last_frame = None;
        self.playhead = new_playhead;
        Ok(())
    }

    /// The index of the next frame that will be written.
    ///
    /// The returned value will never be more than [Self::duration], but it can
    /// equal it (in this case [Self::write_next] must not be called).
    #[inline(always)]
    pub const fn playhead(&self) -> usize {
        self.playhead
    }

    /// The number of frames this video has.
    ///
    /// This value will never be 0. Also see [Self::duration_non_zero].
    #[inline(always)]
    pub const fn duration(&self) -> usize {
        self.duration_non_zero().get()
    }

    /// The number of frames this video has.
    #[inline(always)]
    pub const fn duration_non_zero(&self) -> NonZeroUsize {
        self.seek_info.frame_count
    }

    /// The intended (native) [Fps] playback speed of this video.
    #[inline(always)]
    pub const fn src_fps(&self) -> Fps {
        self.inner.src_fps()
    }

    /// The intended (native) dimensions of the frames in this video.
    #[inline(always)]
    pub const fn src_dimensions(&self) -> Dimensions {
        self.inner.src_dimensions()
    }

    /// The dimensions of the frames that will be produced.
    #[inline(always)]
    pub const fn dest_dimensions(&self) -> Dimensions {
        self.inner.dest_dimensions()
    }

    /// Sets whether or not the stream will be paused.
    #[inline(always)]
    pub const fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Whether or not the stream is paused.
    #[inline(always)]
    pub const fn paused(&self) -> bool {
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

    /// Just the [RescaleMethod] from [Self::rescale].
    #[inline(always)]
    pub const fn rescale_method(&self) -> Option<RescaleMethod> {
        self.inner.rescale_method()
    }

    const fn from_parts(inner: FFmpegVideoInner, paused: bool, seek_info: SeekInfo) -> Self {
        Self {
            inner,
            last_frame: None,
            paused,
            playhead: 0,
            seek_info,
            last_keyframe_array_idx: 0,
        }
    }

    fn rescale_last_frame_if_needed(&mut self) -> FFmpegResult<()> {
        let Some(last_frame) = self.last_frame.take() else {
            return Ok(());
        };
        if last_frame.dimensions() == self.dest_dimensions() {
            self.last_frame = Some(last_frame);
            return Ok(());
        }

        let new_last_frame = rescale_ffmpeg_frame(
            last_frame,
            self.dest_dimensions(),
            self.rescale_method().unwrap_or(RescaleMethod::best()),
        )?;
        self.last_frame = Some(new_last_frame);
        Ok(())
    }

    /// A best guess as to whether it's cheaper to decode frames forward to get
    /// to `new_playhead` instead of seeking.
    fn should_walk_instead_of_seek(&self, new_playhead: usize) -> bool {
        match new_playhead.checked_sub(self.playhead) {
            Some(0) | None => return false, // We can't walk backwards.
            Some(1) => return true,         // Always walk if it's just 1 frame.
            _ => {}
        };

        // If we've already passed the last keyframe we have to walk.
        let Some((next_keyframe_frame_idx, _)) = self
            .seek_info
            .keyframe_timestamps
            .get(self.last_keyframe_array_idx + 1)
            .copied()
        else {
            return true;
        };

        // Walk if we're seeking to before the next keyframe.
        new_playhead < next_keyframe_frame_idx
    }
}

// The FFmpeg types don't implement `Debug` so we're doing it by hand.
impl Debug for FFmpegVideo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let type_name = any::type_name::<FFmpegVideoInner>()
            .split("::")
            .last()
            .unwrap();
        f.debug_struct(type_name).finish_non_exhaustive()
    }
}

struct FrameScaler {
    scaler: FFmpegScalingContext,

    #[cfg(debug_assertions)]
    src_format_debug: FFmpegPixelFormat,
    #[cfg(debug_assertions)]
    src_dimensions_debug: Dimensions,

    dest_dimensions: Dimensions,
    rescale_method: RescaleMethod,
}

impl FrameScaler {
    /// Create a new [FrameScaler].
    pub fn new(
        src_format: FFmpegPixelFormat,
        src_dimensions: Dimensions,
        dest_dimensions: Dimensions,
        rescale_method: RescaleMethod,
    ) -> FFmpegResult<Self> {
        let scaling_flags = if src_dimensions == dest_dimensions {
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
            src_format,
            src_dimensions.width(),
            src_dimensions.height(),
            // Dest:
            TARGET_PIXEL_FORMAT,
            dest_dimensions.width(),
            dest_dimensions.height(),
            // Rescale method:
            scaling_flags,
        )?;

        Ok(Self {
            scaler,

            #[cfg(debug_assertions)]
            src_format_debug: src_format,
            #[cfg(debug_assertions)]
            src_dimensions_debug: src_dimensions,

            dest_dimensions,
            rescale_method,
        })
    }

    /// Like [Self::new] but [None] is returned if no reformatting is needed.
    pub fn new_if_needed(
        src_format: FFmpegPixelFormat,
        src_dimensions: Dimensions,
        dest_dimensions: Dimensions,
        rescale_method: RescaleMethod,
    ) -> FFmpegResult<Option<Self>> {
        if src_format == TARGET_PIXEL_FORMAT && src_dimensions == dest_dimensions {
            Ok(None)
        } else {
            Self::new(src_format, src_dimensions, dest_dimensions, rescale_method).map(Some)
        }
    }

    /// Rescale `src` onto `dest`.
    pub fn rescale(
        &mut self,
        src: &FFmpegVideoFrame,
        dest: &mut FFmpegVideoFrame,
    ) -> FFmpegResult<()> {
        #[cfg(debug_assertions)]
        {
            debug_assert_eq!(src.format(), self.src_format_debug);
            debug_assert_eq!(
                Dimensions::new(src.width(), src.height()),
                Some(self.src_dimensions_debug)
            );
        }

        debug_assert_eq!(dest.format(), TARGET_PIXEL_FORMAT);
        debug_assert_eq!(
            Dimensions::new(dest.width(), dest.height()),
            Some(self.dest_dimensions)
        );

        self.scaler.run(src, dest)
    }

    /// This rescaler's [Dimensions].
    pub const fn dest_dimensions(&self) -> Dimensions {
        self.dest_dimensions
    }

    /// This rescaler's [RescaleMethod].
    pub const fn rescale_method(&self) -> RescaleMethod {
        self.rescale_method
    }
}

// SAFETY: The `ffmpeg::software::scaling::Context` type (aliased
// `FFmpegScalingContext` here) which we're storing in `FrameScaler` *is* safe
// to send/share between threads. The library authors likely just didn't think
// to mark it. I opened an issue about it here:
// <https://github.com/zmwangx/rust-ffmpeg/issues/252>
unsafe impl Send for FrameScaler {}
unsafe impl Sync for FrameScaler {}

/// Rescale a frame ([FFmpegVideoFrame]) to a new size.
pub fn rescale_ffmpeg_frame(
    frame: FFmpegVideoFrame,
    new_dimensions: Dimensions,
    rescale_method: RescaleMethod,
) -> FFmpegResult<FFmpegVideoFrame> {
    if frame.dimensions() == new_dimensions {
        return Ok(frame);
    }

    let mut scaler = FrameScaler::new(
        frame.format(),
        frame.dimensions(),
        new_dimensions,
        rescale_method,
    )?;

    let mut rescaled_frame = FFmpegVideoFrame::new(
        TARGET_PIXEL_FORMAT,
        new_dimensions.width(),
        new_dimensions.height(),
    );
    scaler.rescale(&frame, &mut rescaled_frame)?;
    Ok(rescaled_frame)
}

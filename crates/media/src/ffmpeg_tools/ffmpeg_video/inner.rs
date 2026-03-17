//! Exports [FFmpegVideoInner].

use std::ffi::c_int;
use std::fmt::Debug;
use std::num::NonZeroUsize;
use std::path::Path;

use ffmpeg::codec::Context as FFmpegCodecContext;
use ffmpeg::codec::decoder::Video as FFmpegVideoDecoder;
use ffmpeg::format::context::Input as FFmpegInputFormatContext;
use ffmpeg::format::stream::Stream as FFmpegStream;
use ffmpeg::media::Type as FFmpegMediaType;
use ffmpeg::{Rational, Rescale};
use ffmpeg_next as ffmpeg;

use super::{FFmpegResult, FFmpegVideoFrame, FrameScaler, TARGET_PIXEL_FORMAT};
use crate::fps::Fps;
use crate::frame::{Dimensions, RescaleMethod};

/// A basic FFmpeg video stream that can write formatted and resized frames from
/// a stream in order and can be seeked. See [FFmpegVideo].
pub struct FFmpegVideoInner {
    // Frame Generation:
    input_context: FFmpegInputFormatContext,
    decoder: FFmpegVideoDecoder,
    scaler: Option<FrameScaler>,
    src_frame_buffer: Option<FFmpegVideoFrame>,
    draining: bool,

    // Seeking:
    next_frame_min_timestamp: Option<i64>,
    last_frame_timestamp: Option<i64>,
    frame_timestamp_delta: Range1,

    // Src Info (Final):
    target_stream_index: usize,
    src_fps: Fps,
    src_dimensions: Dimensions,

    // Optimization:
    frames_since_keyframe: Option<NonZeroUsize>,
    max_frames_between_keyframes: Option<NonZeroUsize>,
    seek_by_frame_supported: Option<bool>,
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

        // The stream should have constant frame spacing. If it doesn't we might
        // be able to detect that early here.
        if best_video_stream.avg_frame_rate() != best_video_stream.rate() {
            return Err(Self::UNSUPPORTED_FORMAT);
        }

        let src_fps: Fps = best_video_stream
            .avg_frame_rate()
            .try_into()
            .or(Err(Self::UNSUPPORTED_FORMAT))?;

        let src_dimensions =
            Dimensions::new(decoder.width(), decoder.height()).ok_or(Self::UNSUPPORTED_FORMAT)?;

        let scaler = match rescale {
            Some((dest_dimensions, rescale_method)) => FrameScaler::new_if_needed(
                decoder.format(),
                src_dimensions,
                dest_dimensions,
                rescale_method,
            )?,
            None => None,
        };

        Ok(Self {
            // Frame Generation:
            input_context,
            decoder,
            scaler,
            src_frame_buffer: None,
            draining: false,

            // Seeking:
            next_frame_min_timestamp: None,
            last_frame_timestamp: None,
            frame_timestamp_delta: None.into(),

            // Src Info (Final):
            target_stream_index,
            src_fps,
            src_dimensions,

            // Optimization:
            frames_since_keyframe: None,
            max_frames_between_keyframes: None,
            seek_by_frame_supported: None,
        })
    }

    /// Decodes the next frame and returns it.
    pub fn next_frame(
        &mut self,
        recycled_frame: Option<FFmpegVideoFrame>,
    ) -> FFmpegResult<FFmpegVideoFrame> {
        self.write_next_frame_in_stream(recycled_frame, false)
            .map(|frame| frame.expect("return not skipped"))
    }

    /// Decodes the next frame without returning it.
    pub fn skip_frame(&mut self) -> FFmpegResult<()> {
        self.write_next_frame_in_stream(None, true)
            .map(|frame| assert!(frame.is_none(), "return skipped"))
    }

    /// See [FFmpegVideo::seek_playhead].
    pub fn seek_playhead(&mut self, new_playhead: usize) -> FFmpegResult<()> {
        self.next_frame_min_timestamp = None;
        self.last_frame_timestamp = None;
        self.frames_since_keyframe = None;

        // Try to seek directly to the target frame (not always supported).
        if matches!(self.seek_by_frame_supported, Some(true) | None) {
            let frame_idx = i64::try_from(new_playhead).expect("valid frame index");
            let seek_result = self.avformat_seek_file(
                frame_idx,
                frame_idx,
                frame_idx,
                ffmpeg::sys::AVSEEK_FLAG_ANY | ffmpeg::sys::AVSEEK_FLAG_FRAME,
            );

            if seek_result.is_ok() {
                self.seek_by_frame_supported = Some(true);
                self.decoder.flush();
                return Ok(());
            }

            self.seek_by_frame_supported = Some(false);
        }

        // Otherwise we need to seek to the nearest keyframe behind the target
        // frame and next time we have to decode we'll walk up to the right
        // frame.
        let next_frame_timestamp = self.frame_idx_to_timestamp(new_playhead);
        self.avformat_seek_file(
            self.start_timestamp(),
            next_frame_timestamp,
            next_frame_timestamp,
            ffmpeg::sys::AVSEEK_FLAG_BACKWARD,
        )?;
        self.decoder.flush();
        self.next_frame_min_timestamp = Some(next_frame_timestamp);

        Ok(())
    }

    /// See [FFmpegVideo::rescale].
    pub const fn rescale(&self) -> Option<(Dimensions, RescaleMethod)> {
        match &self.scaler {
            Some(scaler) => Some((scaler.dest_dimensions(), scaler.rescale_method())),
            None => None,
        }
    }

    /// See [FFmpegVideo::set_rescale].
    pub fn set_rescale(
        &mut self,
        new_dest_dimensions: Dimensions,
        new_rescale_method: RescaleMethod,
    ) -> FFmpegResult<()> {
        let (old_dest_dimensions, old_rescale_nethod) = match &self.scaler {
            Some(scaler) => (scaler.dest_dimensions(), Some(scaler.rescale_method())),
            None => (self.src_dimensions, None),
        };

        let same_dest_dimensions = new_dest_dimensions == old_dest_dimensions;
        let same_rescale_method = Some(new_rescale_method) == old_rescale_nethod;
        let rescale_needed = self.src_dimensions != old_dest_dimensions;

        if same_dest_dimensions && (same_rescale_method || !rescale_needed) {
            return Ok(());
        }

        self.scaler = Some(FrameScaler::new(
            self.decoder.format(),
            self.src_dimensions,
            new_dest_dimensions,
            new_rescale_method,
        )?);
        Ok(())
    }

    /// The frame count if it is known (in the metadata).
    pub fn known_frame_count(&self) -> Option<NonZeroUsize> {
        // The frame count is an optional field that some containers provide.
        // Not all video formats provide it though.
        match self.target_stream().frames() {
            ..=0 => None,
            frames => Some(NonZeroUsize::new(frames as usize).unwrap()),
        }
    }

    /// Continually skips the next frame while `continue_predicate` returns
    /// `true` (or until the stream's end is found). The number of frames
    /// skipped is returned.
    ///
    /// `continue_predicate` is called after every each frame is decoded. The
    /// number of the frames decoded so far is passed as an argument. If the
    /// predicate returns `false`, the function will immediately exit.
    ///
    /// [ffmpeg::Error::Eof] is returned if no frames could be skipped.
    pub fn skip_frames_while<F>(&mut self, mut continue_predicate: F) -> FFmpegResult<NonZeroUsize>
    where
        F: FnMut(NonZeroUsize) -> bool,
    {
        let mut frame_count: usize = 0;
        loop {
            match self.skip_frame() {
                Ok(_) => frame_count += 1,
                Err(ffmpeg::Error::Eof) => break,
                Err(e) => return Err(e),
            }

            let non_zero_frame_count = frame_count.try_into().expect("Just incremented");
            if !continue_predicate(non_zero_frame_count) {
                break;
            }
        }

        // We'll call a frame count of 0 an EOF error.
        frame_count.try_into().map_err(|_| ffmpeg::Error::Eof)
    }

    /// Skips the next `n` frames.
    pub fn skip_frames(&mut self, n: usize) -> FFmpegResult<()> {
        for _ in 0..n {
            self.skip_frame()?;
        }
        Ok(())
    }

    /// The target dimensions of this video.
    pub const fn dest_dimensions(&self) -> Dimensions {
        match &self.scaler {
            Some(scaler) => scaler.dest_dimensions,
            None => self.src_dimensions,
        }
    }

    /// The native dimensions of this video.
    pub const fn src_dimensions(&self) -> Dimensions {
        self.src_dimensions
    }

    /// The native [Fps] of this video.
    pub const fn src_fps(&self) -> Fps {
        self.src_fps
    }

    /// Whether or not this stream supports jumping directly to a frame without
    /// extra decoding. [None] is returned if it's unknown.
    pub const fn seek_by_frame_supported(&self) -> Option<bool> {
        self.seek_by_frame_supported
    }

    /// The max number of frames we've decoded between 2 keyframes. [None] is
    /// returned if we haven't decoded 2 keyframes in one sequence.
    pub const fn max_frames_between_keyframes(&self) -> Option<NonZeroUsize> {
        self.max_frames_between_keyframes
    }

    /// The number of frames we've decoded since the last keyframe. [None] is
    /// returned if we haven't decoded a keyframes in this sequence.
    pub const fn frames_since_keyframe(&self) -> Option<NonZeroUsize> {
        self.frames_since_keyframe
    }

    /// This video's format is not supported.
    const UNSUPPORTED_FORMAT: ffmpeg::Error = ffmpeg::Error::InvalidData;

    fn write_next_frame_in_stream(
        &mut self,
        recycled_frame: Option<FFmpegVideoFrame>,
        skip_return: bool,
    ) -> FFmpegResult<Option<FFmpegVideoFrame>> {
        let mut src_frame = self
            .src_frame_buffer
            .take()
            .unwrap_or_else(|| self.new_src_frame_buffer(None));

        let ret =
            self.write_next_frame_in_stream_impl(recycled_frame, &mut src_frame, skip_return)?;

        if src_frame.format() != self.decoder.format()
            || src_frame.width() != self.decoder.width()
            || src_frame.height() != self.decoder.height()
        {
            return Err(ffmpeg::Error::InputChanged);
        }
        self.src_frame_buffer = Some(src_frame);

        if let Some(ret_frame) = &ret {
            debug_assert!(!skip_return);

            assert_eq!(ret_frame.format(), TARGET_PIXEL_FORMAT);
            assert_eq!(ret_frame.width(), self.dest_dimensions().width());
            assert_eq!(ret_frame.height(), self.dest_dimensions().height());
        } else {
            debug_assert!(skip_return);
        }

        Ok(ret)
    }

    fn write_next_frame_in_stream_impl(
        &mut self,
        recycled_frame: Option<FFmpegVideoFrame>,
        src_frame: &mut FFmpegVideoFrame,
        skip_return: bool,
    ) -> FFmpegResult<Option<FFmpegVideoFrame>> {
        debug_assert_eq!(src_frame.format(), self.decoder.format());
        debug_assert_eq!(src_frame.width(), self.decoder.width());
        debug_assert_eq!(src_frame.height(), self.decoder.height());

        let mut ret_frame = (!skip_return).then(|| self.new_dest_frame_buffer(recycled_frame));

        loop {
            // We'll write directly to the frame we'll return if reformatting
            // isn't needed.
            let intermediate_frame = match (&self.scaler, &mut ret_frame) {
                (None, Some(ret_frame)) => ret_frame,
                _ => &mut *src_frame,
            };

            // https://ffmpeg.org/doxygen/8.0/group__lavc__decoding.html#ga11e6542c4e66d3028668788a1a74217c
            let decode_err = match self.decoder.receive_frame(intermediate_frame) {
                Ok(()) => {
                    let frame_timestamp = intermediate_frame.timestamp();
                    let frame_timestamp = self.validate_new_frame_timestamp(frame_timestamp)?;

                    let frame_is_keyframe = intermediate_frame.is_key();
                    self.track_new_frame_is_keyframe(frame_is_keyframe);

                    // If we seeked and aren't at the right frame we need to
                    // continue to advance.
                    if let Some(min_timestamp) = self.next_frame_min_timestamp
                        && frame_timestamp < min_timestamp
                    {
                        continue;
                    }

                    if !skip_return {
                        let ret_frame = ret_frame
                            .as_mut()
                            .expect("should be created if we're returning a frame");

                        // We're going to return this frame. Reformat the
                        // intermediate frame onto the return frame (if we didn't
                        // already write directly to it).
                        if let Some(scaler) = &mut self.scaler {
                            scaler.rescale(src_frame, ret_frame)?;
                        }
                    }
                    return Ok(ret_frame);
                }

                Err(e) => e,
            };

            // `EAGAIN` means we haven't sent enough packets for a frame yet. If
            // that happens, we have to load some packets and keep going.
            if decode_err != EAGAIN {
                // Something must be wrong w/ the file or the object's state.
                return Err(decode_err);
            }

            // If we're draining it means we're already out of packets to send.
            if self.draining {
                return Err(ffmpeg::Error::Eof);
            }

            // Internally, the `packets` iterator mutates our `input_context`,
            // not its own internal state. This means we can cheaply re-create
            // the packets iterator each time we want to grab a new packet.
            let mut packets = self
                .input_context
                .packets()
                .filter_map(|(packet_stream, packet)| {
                    // Skip packets that aren't for our stream.
                    (packet_stream.index() == self.target_stream_index).then_some(packet)
                });

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

    fn new_dest_frame_buffer(&self, recycled_buffer: Option<FFmpegVideoFrame>) -> FFmpegVideoFrame {
        let Some(scaler) = &self.scaler else {
            return self.new_src_frame_buffer(recycled_buffer);
        };

        let dest_width = scaler.dest_dimensions.width();
        let dest_height = scaler.dest_dimensions.height();

        if let Some(recycled_buffer) = recycled_buffer
            && recycled_buffer.format() == TARGET_PIXEL_FORMAT
            && recycled_buffer.width() == dest_width
            && recycled_buffer.height() == dest_height
        {
            recycled_buffer
        } else {
            FFmpegVideoFrame::new(TARGET_PIXEL_FORMAT, dest_width, dest_height)
        }
    }

    fn new_src_frame_buffer(&self, recycled_buffer: Option<FFmpegVideoFrame>) -> FFmpegVideoFrame {
        let src_format = self.decoder.format();
        let src_width = self.decoder.width();
        let src_height = self.decoder.height();

        if let Some(recycled_buffer) = recycled_buffer
            && recycled_buffer.format() == src_format
            && recycled_buffer.width() == src_width
            && recycled_buffer.height() == src_height
        {
            recycled_buffer
        } else {
            FFmpegVideoFrame::new(src_format, src_width, src_height)
        }
    }

    /// A more granular input context seeking function than
    /// [FFmpegInputFormatContext::seek]. This one gives you direct control over
    /// the seeking flags, the min/max timestamp, and also is hard-coded to
    /// *only* deal with the target stream.
    ///
    /// See the [FFmpeg docs](https://ffmpeg.org/doxygen/8.0/group__lavf__decoding.html#ga3b40fc8d2fda6992ae6ea2567d71ba30)
    /// for `avformat_seek_file`.
    fn avformat_seek_file(
        &mut self,
        min_ts: i64,
        ts: i64,
        max_ts: i64,
        flags: c_int,
    ) -> FFmpegResult<()> {
        // SAFETY: This FFI call is safe. See `ffmpeg-next`'s implementation for
        // `ffmpeg::format::context::Input::seek` (it's very similar).
        match unsafe {
            ffmpeg::sys::avformat_seek_file(
                self.input_context.as_mut_ptr(),
                self.target_stream_index as i32,
                min_ts,
                ts,
                max_ts,
                flags,
            )
        } {
            s if s >= 0 => Ok(()),
            e => Err(e.into()),
        }
    }

    fn validate_new_frame_timestamp(
        &mut self,
        new_frame_timestamp: Option<i64>,
    ) -> FFmpegResult<i64> {
        // We're not supporting video formats that don't provide presentation
        // timestamps for frames (without it, frame-accurate seeking is
        // impossible).
        let Some(new_timestamp) = new_frame_timestamp else {
            return Err(Self::UNSUPPORTED_FORMAT);
        };

        if new_timestamp < self.start_timestamp() {
            return Err(Self::UNSUPPORTED_FORMAT);
        }

        if let Some(last_timestamp) = self.last_frame_timestamp {
            // Compute the difference between this frame's timestamp and the
            // last frame's timestamp (timestamp delta).
            let Some(timestamp_delta) = new_timestamp
                .checked_sub(last_timestamp)
                .and_then(|delta| (delta > 0).then_some(delta))
            else {
                // The new timestamp must be after the last one.
                return Err(Self::UNSUPPORTED_FORMAT);
            };

            // If the timestamp difference is not constant between frames (with
            // an allowed ±1 tolerance) then this is a variable frame rate video
            // (which we aren't going to support).
            let timestamp_delta = self.frame_timestamp_delta.update(timestamp_delta);
            self.frame_timestamp_delta = match timestamp_delta {
                Some(delta) if !delta.has_split_range() => delta,
                _ => return Err(Self::UNSUPPORTED_FORMAT),
            };
        }

        self.last_frame_timestamp = Some(new_timestamp);
        Ok(new_timestamp)
    }

    fn track_new_frame_is_keyframe(&mut self, new_frame_is_keyframe: bool) {
        if new_frame_is_keyframe {
            if let Some(frames_since_keyframe) = self.frames_since_keyframe {
                self.max_frames_between_keyframes = match self.max_frames_between_keyframes {
                    Some(max) => Some(max.max(frames_since_keyframe)),
                    None => Some(frames_since_keyframe),
                };
            }

            self.frames_since_keyframe = Some(NonZeroUsize::MIN);
            return;
        }

        if let Some(frames_since_keyframe) = &mut self.frames_since_keyframe {
            // SAFETY: n + 1 where n > 0 is never 0.
            *frames_since_keyframe =
                unsafe { NonZeroUsize::new_unchecked(frames_since_keyframe.get() + 1) };
        }
    }

    #[inline]
    fn target_stream(&self) -> FFmpegStream<'_> {
        self.input_context
            .stream(self.target_stream_index)
            .expect("target stream should be present")
    }

    #[inline]
    fn start_timestamp(&self) -> i64 {
        match self.target_stream().start_time() {
            // If we don't have a start time, we'll just assume it's 0.
            // `ffmpeg-next` does not handle this edge case.
            ffmpeg::sys::AV_NOPTS_VALUE => 0,
            start_time => start_time,
        }
    }

    fn frame_idx_to_timestamp(&self, frame_idx: usize) -> i64 {
        let frame_idx = i64::try_from(frame_idx).expect("valid frame index");
        let start_timestamp = self.start_timestamp();
        let frame_duration =
            Rational::try_from(self.src_fps.inverse()).expect("`Fps` created from `Rational`");
        let time_base = self.target_stream().time_base();

        start_timestamp.saturating_add(frame_idx.rescale(frame_duration, time_base))
    }
}

const EAGAIN: ffmpeg::Error = ffmpeg::Error::Other {
    errno: ffmpeg::error::EAGAIN,
};

type Range1InnerT = i64;

/// For tracking [values](Range1InnerT) that all must fall within a range some
/// unknown value ±1.
#[derive(Debug, Clone, Copy)]
struct Range1 {
    n: Range1InnerT,
    min_count: usize,
    mid_count: usize,
    max_count: usize,
}

impl Range1 {
    /// Create a [RangePlusMinus1] with a starting value.
    pub const fn new(n: Range1InnerT) -> Self {
        Self {
            n,
            min_count: 0,
            mid_count: 1,
            max_count: 0,
        }
    }

    /// Create a [RangePlusMinus1] with *no starting value*. Equivalent to
    /// [Self::default].
    pub const fn without_starting_value() -> Self {
        Self {
            n: 0,
            min_count: 0,
            mid_count: 0,
            max_count: 0,
        }
    }

    /// Create a [RangePlusMinus1] with an optional starting value. Equivalent
    /// to [Self::from].
    pub const fn from_optional_starting_value(option_n: Option<Range1InnerT>) -> Self {
        match option_n {
            Some(n) => Self::new(n),
            None => Self::without_starting_value(),
        }
    }

    /// Whether or not any values have been recorded.
    pub const fn has_values(self) -> bool {
        self.min_count != 0 || self.mid_count != 0 || self.max_count != 0
    }

    /// Updates the range of `self` with `new_n` if `new_n` is within range of
    /// the previous values. If all previous values and `new_n` aren't within a
    /// range of 3 consecutive values values, [None] is returned.
    #[must_use]
    pub const fn update(mut self, new_n: Range1InnerT) -> Option<Self> {
        if !self.has_values() {
            return Some(Self::new(new_n));
        }

        // in range (middle)
        if new_n == self.n {
            self.mid_count += 1;
            return Some(self);
        }

        // in range (±1)
        if self.eq_with_self_offset(new_n, 1) {
            self.max_count += 1;
            return Some(self);
        }
        if self.eq_with_self_offset(new_n, -1) {
            self.min_count += 1;
            return Some(self);
        }

        // out of range but range can shift (±2)
        if self.eq_with_self_offset(new_n, 2) && self.min_count == 0 {
            self.n += 1;
            self.min_count = self.mid_count;
            self.mid_count = self.max_count;
            self.max_count = 1;
            return Some(self);
        }
        if self.eq_with_self_offset(new_n, -2) && self.max_count == 0 {
            self.n -= 1;
            self.max_count = self.mid_count;
            self.mid_count = self.min_count;
            self.min_count = 1;
            return Some(self);
        }

        // Fully out of range
        None
    }

    /// Whether or not both values `n-1` and `n+1` have been added more than
    /// once but `n` has never been added.
    pub const fn has_split_range(self) -> bool {
        self.min_count >= 2 && self.max_count >= 2 && self.mid_count == 0
    }

    #[inline(always)]
    const fn eq_with_self_offset(self, other_n: Range1InnerT, self_offset: i8) -> bool {
        if let Some(offset_n) = self.n.checked_add(self_offset as Range1InnerT) {
            offset_n == other_n
        } else {
            false
        }
    }
}

impl Default for Range1 {
    fn default() -> Self {
        Self::without_starting_value()
    }
}

impl From<Option<Range1InnerT>> for Range1 {
    fn from(option_n: Option<Range1InnerT>) -> Self {
        Self::from_optional_starting_value(option_n)
    }
}

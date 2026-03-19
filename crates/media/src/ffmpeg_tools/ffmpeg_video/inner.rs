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
/// a stream in order and can be seeked. See [FFmpegVideo](super::FFmpegVideo).
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
    fixed_frame_timestamp_delta: Option<i64>,

    // Src Info (Final):
    target_stream_index: usize,
    src_fps: Fps,
    src_dimensions: Dimensions,

    // Optimization:
    frames_since_keyframe: Option<NonZeroUsize>,
    max_frames_between_keyframes: Option<NonZeroUsize>,
}

impl<'a> FFmpegVideoInner {
    /// Create an [FFmpegVideo](super::FFmpegVideo) with everything but the
    /// duration.
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

        let (dest_dimensions, rescale_method) =
            rescale.unwrap_or((src_dimensions, RescaleMethod::default()));
        let scaler = FrameScaler::new_if_needed(
            decoder.format(),
            src_dimensions,
            dest_dimensions,
            rescale_method,
        )?;

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
            fixed_frame_timestamp_delta: None,

            // Src Info (Final):
            target_stream_index,
            src_fps,
            src_dimensions,

            // Optimization:
            frames_since_keyframe: None,
            max_frames_between_keyframes: None,
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

    /// See [FFmpegVideo::seek_playhead](super::FFmpegVideo::seek_playhead).
    pub fn seek_playhead(&mut self, new_playhead: usize) -> FFmpegResult<()> {
        self.seek_with_timestamp(self.frame_idx_to_timestamp(new_playhead))
    }

    /// See [FFmpegVideo::rescale](super::FFmpegVideo::rescale).
    pub const fn rescale(&self) -> Option<(Dimensions, RescaleMethod)> {
        match &self.scaler {
            Some(scaler) => Some((scaler.dest_dimensions(), scaler.rescale_method())),
            None => None,
        }
    }

    /// See [FFmpegVideo::set_rescale](super::FFmpegVideo::set_rescale).
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
        let (dest_width, dest_height) = match &self.scaler {
            Some(scaler) => scaler.dest_dimensions.into(),
            None => self.src_dimensions.into(),
        };

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

            // Once we get a fixed timestamp delta we can't let it change.
            if let Some(new_fixed_delta) = self.frame_timestamp_delta.fixed_value() {
                if let Some(old_fixed_delta) = self.fixed_frame_timestamp_delta {
                    if new_fixed_delta != old_fixed_delta {
                        return Err(Self::UNSUPPORTED_FORMAT);
                    }
                } else {
                    self.fixed_frame_timestamp_delta = Some(new_fixed_delta);
                }
            }
        };

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

    /// Like [Self::seek_playhead] but it seeks to the first frame equal to or
    /// after the timestamp (not a frame index).
    fn seek_with_timestamp(&mut self, new_timestamp: i64) -> FFmpegResult<()> {
        self.draining = false;
        self.next_frame_min_timestamp = None;
        self.last_frame_timestamp = None;
        self.frames_since_keyframe = None;

        // Seek to the nearest keyframe behind the target and next time we have
        // to decode we'll walk up to the right frame.
        self.avformat_seek_file(
            self.start_timestamp(),
            new_timestamp,
            new_timestamp,
            ffmpeg::sys::AVSEEK_FLAG_BACKWARD,
        )?;
        self.decoder.flush();
        self.next_frame_min_timestamp = Some(new_timestamp);

        Ok(())
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

    /// Try to determine the frame count.
    ///
    /// This function assumes its playhead is at frame 0 when it is called. It
    /// will have seeked back to 0 if [Ok] is returned.
    ///
    /// `continue_predicate` is called after every each frame is decoded. If the
    /// predicate returns `false`, the function will immediately exit. The
    /// predicate is passed an integer for the number of times it has been
    /// called.
    pub fn determine_frame_count<F>(
        &mut self,
        mut continue_predicate: F,
    ) -> FFmpegResult<NonZeroUsize>
    where
        F: FnMut(usize) -> bool,
    {
        const CANCELLED: ffmpeg::Error = ffmpeg::Error::Other {
            errno: ffmpeg::error::ECANCELED,
        };
        let mut predicate_calls = 0;

        let mut frames_skipped = 0;

        // We need a fixed timestamp delta to find the end with a jump so we'll
        // decode until we have one.
        while self.fixed_frame_timestamp_delta.is_none() {
            match self.skip_frame() {
                // Found end early.
                Err(ffmpeg::Error::Eof) if frames_skipped > 0 => {
                    self.seek_playhead(0)?;
                    return Ok(frames_skipped.try_into().expect("non-0"));
                }

                Ok(_) => frames_skipped += 1,
                Err(e) => return Err(e),
            }

            if !continue_predicate(predicate_calls) {
                return Err(CANCELLED);
            }
            predicate_calls += 1;
        }
        let fixed_timestamp_delta = self.fixed_frame_timestamp_delta.expect("just found");
        let frames_between_keyframes = self
            .max_frames_between_keyframes
            .map_or_else(|| frames_skipped, NonZeroUsize::get);

        let start_timestamp = self.start_timestamp();
        let end_timestamp = self.end_timestamp()?;
        let backstep_amount = fixed_timestamp_delta * frames_between_keyframes as i64;

        // We'll try to seek a few times, walking backwards each time.
        const MAX_ATTEMPTS: usize = 3;
        for attempt in 1.. {
            let near_end_timestamp = end_timestamp - backstep_amount * attempt as i64;
            if near_end_timestamp <= start_timestamp {
                if attempt > 1 {
                    self.seek_playhead(0)?;
                    frames_skipped = 0;
                }
                break;
            }

            let seek_result = self.seek_with_timestamp(near_end_timestamp);
            self.decoder.flush();
            match seek_result {
                Ok(_) => {
                    frames_skipped = ((near_end_timestamp - start_timestamp)
                        / fixed_timestamp_delta)
                        .max(0) as usize;
                    break;
                }
                Err(e) if attempt == MAX_ATTEMPTS => return Err(e),
                Err(_) => {}
            }
        }

        loop {
            match self.skip_frame() {
                Err(ffmpeg::Error::Eof) if frames_skipped > 0 => break,
                Ok(_) => frames_skipped += 1,
                Err(e) => return Err(e),
            }

            if !continue_predicate(predicate_calls) {
                return Err(CANCELLED);
            }
            predicate_calls += 1;
        }

        self.seek_playhead(0)?;
        Ok(frames_skipped.try_into().expect("non-0"))
    }

    fn end_timestamp(&self) -> FFmpegResult<i64> {
        let duration_from_stream = self.target_stream().duration();
        if duration_from_stream != ffmpeg::sys::AV_NOPTS_VALUE {
            return Ok(duration_from_stream);
        }

        let duration_from_ctx = self.input_context.duration();
        if duration_from_ctx != ffmpeg::sys::AV_NOPTS_VALUE {
            return Ok(duration_from_ctx.rescale(
                ffmpeg::sys::AV_TIME_BASE_Q,
                self.target_stream().time_base(),
            ));
        }

        Err(Self::UNSUPPORTED_FORMAT)
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

    /// The best fixed value for representing this range if one exists.
    pub const fn fixed_value(self) -> Option<Range1InnerT> {
        if !self.has_values() {
            return None;
        }

        let total = self.min_count + self.mid_count + self.max_count;

        // We want at least a few samples before we decide conclusively.
        const MIN_SAMPLES: usize = 12;
        if total < MIN_SAMPLES {
            return None;
        }

        // Order values by count.
        let mut values = [self.n - 1, self.n, self.n + 1];
        let mut counts = [self.min_count, self.mid_count, self.max_count];
        if counts[0] < counts[1] {
            counts.swap(0, 1);
            values.swap(0, 1);
        }
        if counts[1] < counts[2] {
            counts.swap(1, 2);
            values.swap(1, 2);
        }
        if counts[0] < counts[1] {
            counts.swap(0, 1);
            values.swap(0, 1);
        }

        let best_val = values[0];
        let [best_count, mid_count, worst_count] = counts;

        // Strong dominance
        if best_count >= mid_count * 2 && best_count >= worst_count * 2 {
            return Some(best_val);
        }

        // Majority agreement (at least 80% of samples agree)
        if best_count * 10 >= total * 8 {
            return Some(best_val);
        }

        // Mid + neighbors cluster tightly (all present but centered)
        if (best_count + mid_count) * 10 >= total * 9 {
            // top two bins cover at least 90%
            return Some(best_val);
        }

        // Need more data.
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

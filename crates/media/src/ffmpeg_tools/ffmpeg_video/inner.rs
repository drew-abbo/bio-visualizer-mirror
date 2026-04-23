//! Exports [FFmpegVideoInner].

use std::ffi::c_int;
use std::fmt::Debug;
use std::num::NonZeroUsize;
use std::path::Path;

use ffmpeg::codec::Context as FFmpegCodecContext;
use ffmpeg::codec::decoder::Video as FFmpegVideoDecoder;
use ffmpeg::format::context::Input as FFmpegInputFormatContext;
use ffmpeg::media::Type as FFmpegMediaType;
use ffmpeg_next as ffmpeg;

use super::{FFmpegResult, FFmpegVideoFrame, FrameScaler, TARGET_PIXEL_FORMAT};
use crate::ffmpeg_tools::ffmpeg_video::seek_info::SeekInfo;
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
    frames_until_target: usize,

    // Src Info (Final):
    target_stream_index: usize,
    src_fps: Fps,
    src_dimensions: Dimensions,
}

impl FFmpegVideoInner {
    /// Create an [FFmpegVideo](super::FFmpegVideo) with everything but the
    /// duration.
    pub fn new(path: &Path, rescale: Option<(Dimensions, RescaleMethod)>) -> FFmpegResult<Self> {
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
            return Err(UNSUPPORTED_FORMAT);
        }

        let src_fps: Fps = best_video_stream
            .avg_frame_rate()
            .try_into()
            .or(Err(UNSUPPORTED_FORMAT))?;

        let src_dimensions =
            Dimensions::new(decoder.width(), decoder.height()).ok_or(UNSUPPORTED_FORMAT)?;

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
            frames_until_target: 0,

            // Src Info (Final):
            target_stream_index,
            src_fps,
            src_dimensions,
        })
    }

    /// Decodes the next frame and returns it.
    pub fn next_frame(
        &mut self,
        recycled_frame: Option<FFmpegVideoFrame>,
    ) -> FFmpegResult<FFmpegVideoFrame> {
        self.write_next_frame_in_stream(recycled_frame, false)
            .map(|(frame, _is_keyframe)| frame.expect("return not skipped"))
    }

    /// Decodes the next frame without returning it.
    pub fn skip_frame(&mut self) -> FFmpegResult<()> {
        self.skip_frame_with_stats().map(|_| ())
    }

    /// See [FFmpegVideo::seek_playhead](super::FFmpegVideo::seek_playhead). The
    /// *keyframe* index that was seeked to is returned (index of a
    /// [SeekInfo::keyframe_timestamps] element).
    pub fn seek_playhead(
        &mut self,
        new_playhead: usize,
        seek_info: &SeekInfo,
    ) -> FFmpegResult<usize> {
        assert!(new_playhead < seek_info.frame_count.get());

        self.draining = false;
        self.frames_until_target = 0;

        let keyframe_array_idx = match seek_info
            .keyframe_timestamps
            .binary_search_by_key(&new_playhead, |&(frame_idx, _)| frame_idx)
        {
            Ok(idx) => idx,            // frame_idx == new_playhead
            Err(idx @ 1..) => idx - 1, // max frame_idx < new_playhead
            Err(0) => unreachable!(),
        };

        let (target_frame_idx, target_ts) = seek_info.keyframe_timestamps[keyframe_array_idx];

        // Find the halfway between the target and prev/next timestamps (bias
        // towards target). This should handle any "off by a few" errors.
        let min_ts = seek_info.keyframe_timestamps[..keyframe_array_idx]
            .last()
            .map(|(_, last_ts)| *last_ts + (target_ts - *last_ts + 1) / 2)
            .unwrap_or(i64::MIN);
        let max_ts = seek_info.keyframe_timestamps[(keyframe_array_idx + 1)..]
            .first()
            .map(|(_, next_ts)| target_ts + (*next_ts - target_ts) / 2)
            .unwrap_or(i64::MAX);

        self.avformat_seek_file(min_ts, target_ts, max_ts, 0)?;
        self.decoder.flush();

        debug_assert!(target_frame_idx <= new_playhead);
        self.frames_until_target = new_playhead - target_frame_idx;

        Ok(keyframe_array_idx)
    }

    /// See [FFmpegVideo::rescale](super::FFmpegVideo::rescale).
    pub const fn rescale(&self) -> Option<(Dimensions, RescaleMethod)> {
        match &self.scaler {
            Some(scaler) => Some((scaler.dest_dimensions(), scaler.rescale_method())),
            None => None,
        }
    }

    /// Just the [RescaleMethod] from [Self::rescale].
    pub const fn rescale_method(&self) -> Option<RescaleMethod> {
        if let Some((_, rescale_method)) = self.rescale() {
            Some(rescale_method)
        } else {
            None
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
    #[inline(always)]
    pub const fn src_dimensions(&self) -> Dimensions {
        self.src_dimensions
    }

    /// The native [Fps] of this video.
    #[inline(always)]
    pub const fn src_fps(&self) -> Fps {
        self.src_fps
    }

    fn skip_frame_with_stats(&mut self) -> FFmpegResult<FrameStats> {
        self.write_next_frame_in_stream(None, true)
            .map(|(frame, is_keyframe)| {
                assert!(frame.is_none(), "return skipped");
                is_keyframe
            })
    }

    fn write_next_frame_in_stream(
        &mut self,
        recycled_frame: Option<FFmpegVideoFrame>,
        skip_return: bool,
    ) -> FFmpegResult<(Option<FFmpegVideoFrame>, FrameStats)> {
        let mut src_frame = self
            .src_frame_buffer
            .take()
            .unwrap_or_else(|| self.new_src_frame_buffer(None));

        let (ret_frame, frame_is_keyframe) =
            self.write_next_frame_in_stream_impl(recycled_frame, &mut src_frame, skip_return)?;

        if src_frame.format() != self.decoder.format()
            || src_frame.width() != self.decoder.width()
            || src_frame.height() != self.decoder.height()
        {
            return Err(ffmpeg::Error::InputChanged);
        }
        self.src_frame_buffer = Some(src_frame);

        if let Some(ret_frame) = &ret_frame {
            debug_assert!(!skip_return);

            assert_eq!(ret_frame.format(), TARGET_PIXEL_FORMAT);
            assert_eq!(ret_frame.width(), self.dest_dimensions().width());
            assert_eq!(ret_frame.height(), self.dest_dimensions().height());
        } else {
            debug_assert!(skip_return);
        }

        Ok((ret_frame, frame_is_keyframe))
    }

    fn write_next_frame_in_stream_impl(
        &mut self,
        recycled_frame: Option<FFmpegVideoFrame>,
        src_frame: &mut FFmpegVideoFrame,
        skip_return: bool,
    ) -> FFmpegResult<(Option<FFmpegVideoFrame>, FrameStats)> {
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
                    // If we seeked and aren't at the right frame yet we need to
                    // skip frames.
                    if self.frames_until_target > 0 {
                        self.frames_until_target -= 1;
                        continue;
                    }

                    let frame_stats = FrameStats::from_frame(intermediate_frame)?;

                    if !skip_return {
                        let ret_frame = ret_frame
                            .as_mut()
                            .expect("should be created if we're returning a frame");

                        // We're going to return this frame. Reformat the
                        // intermediate frame onto the return frame (if we
                        // didn't already write directly to it).
                        if let Some(scaler) = &mut self.scaler {
                            scaler.rescale(src_frame, ret_frame)?;
                        }
                    }
                    return Ok((ret_frame, frame_stats));
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

    /// Try to determine the frame count and keyframe timestamps.
    ///
    /// This function assumes its playhead is at frame 0 when it is called. It
    /// will have seeked back to 0 if [Ok] is returned.
    ///
    /// `continue_predicate` is called before each frame is decoded. If the
    /// predicate returns `false`, the function will immediately exit. The
    /// predicate is passed the number of frames it has found so far.
    pub fn determine_seek_info<F>(&mut self, mut continue_predicate: F) -> FFmpegResult<SeekInfo>
    where
        F: FnMut(usize) -> bool,
    {
        let mut keyframe_timestamps = vec![];
        let mut last_timestamp = i64::MIN;

        for frame_idx in 0.. {
            if !continue_predicate(frame_idx) {
                return Err(ECANCELED);
            }

            match self.skip_frame_with_stats() {
                Ok(frame_stats) => {
                    // Double check new timestamp is after last.
                    if frame_stats.timestamp() <= last_timestamp {
                        return Err(UNSUPPORTED_FORMAT);
                    }
                    last_timestamp = frame_stats.timestamp();

                    if frame_stats.is_keyframe() {
                        keyframe_timestamps.push((frame_idx, frame_stats.timestamp()));
                    } else if frame_idx == 0 {
                        // The 1st frame always must be a keyframe.
                        return Err(UNSUPPORTED_FORMAT);
                    }

                    continue;
                }

                Err(e) if e == ffmpeg::Error::Eof && frame_idx != 0 => {
                    let frame_count = NonZeroUsize::new(frame_idx).expect("non-0");
                    debug_assert!(!keyframe_timestamps.is_empty());

                    let seek_info = SeekInfo {
                        frame_count,
                        keyframe_timestamps,
                    };
                    self.seek_playhead(0, &seek_info)?;
                    return Ok(seek_info);
                }

                Err(e) => return Err(e),
            }
        }

        unreachable!("loop is 0.. (infinite)")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FrameStats {
    is_keyframe: bool,
    timestamp: i64,
}

impl FrameStats {
    /// Create [FrameStats] from an [FFmpegVideoFrame].
    #[inline]
    pub fn from_frame(frame: &FFmpegVideoFrame) -> FFmpegResult<Self> {
        let is_keyframe = frame.is_key();
        let timestamp = frame.timestamp().ok_or(UNSUPPORTED_FORMAT)?;
        Ok(FrameStats {
            is_keyframe,
            timestamp,
        })
    }

    /// Whether an [FFmpegVideoFrame] is a keyframe.
    #[inline(always)]
    pub fn is_keyframe(&self) -> bool {
        self.is_keyframe
    }

    /// An [FFmpegVideoFrame]'s timestamp.
    #[inline(always)]
    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

const UNSUPPORTED_FORMAT: ffmpeg::Error = ffmpeg::Error::InvalidData;
const EAGAIN: ffmpeg::Error = ffmpeg::Error::Other {
    errno: ffmpeg::error::EAGAIN,
};
const ECANCELED: ffmpeg::Error = ffmpeg::Error::Other {
    errno: ffmpeg::error::ECANCELED,
};

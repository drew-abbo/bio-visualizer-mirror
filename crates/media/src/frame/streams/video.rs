//! The module contains [Video], a [FrameStream](super::FrameStream) that can
//! load and play a video in almost any kind of video file format (using
//! FFmpeg).

use std::path::Path;

use ffmpeg::codec::Context as FFmpegCodecContext;
use ffmpeg::codec::decoder::Video as FFmpegVideoDecoder;
use ffmpeg::format::context::Input as FFmpegInputFormatContext;
use ffmpeg::format::stream::Stream as FFmpegStream;
use ffmpeg::frame::Video as FFmpegVideoFrame;
use ffmpeg::media::Type as FFmpegMediaType;
use ffmpeg_next as ffmpeg;

use super::ffmpeg_tools::{VideoFrameBuffer, VideoFrameFormatter, VideoStreamError};
use crate::frame::streams::{BufferStream, StreamError, StreamStats};
use crate::frame::{Dimensions, FrameBuffer};

/// A [FrameStream](super::FrameStream) that can load and play a video in almost
/// any kind of video file format (using FFmpeg).
pub struct Video {
    input_context: FFmpegInputFormatContext,
    decoder: FFmpegVideoDecoder,
    formatter: VideoFrameFormatter,
    video_stream_index: usize,
    stats: StreamStats,
    stream_over: bool,
}

impl Video {
    /// Create a new [super::FrameStream] from a video file's file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, VideoStreamError> {
        Self::new_impl(path.as_ref())
    }

    fn new_impl(path: &Path) -> Result<Self, VideoStreamError> {
        // This object is a handle to the file we opened. Right now, this is
        // just the kind of container (e.g. MP4, MKV) and FFmpeg has none of the
        // actual video/audio data yet (only the file's metadata).
        let input_context =
            ffmpeg::format::input(path).map_err(|_| VideoStreamError::NoInputContext)?;

        // Video containers can have multiple video streams. This says "FFmpeg,
        // my friend, would you please pick the video stream you think is
        // best?".
        let video_stream = input_context
            .streams()
            .best(FFmpegMediaType::Video)
            .ok_or(VideoStreamError::NoBestVideoStream)?;

        // When we're going through packets later, we'll need to make sure to
        // ignore all packets that aren't from the video stream we care about.
        // To do that, we'll save the video stream's index.
        let video_stream_index = video_stream.index();

        // This gathers the information we'll need for decoding our video
        // stream (e.g. bitrate, resolution, frames per second). We need this to
        // create a decoder.
        let decoder_context = FFmpegCodecContext::from_parameters(video_stream.parameters())
            .map_err(|_| VideoStreamError::DecoderCreateFailure)?;

        // This is the actual object we can use to decode our video streams into
        // frames. It also provides more information about the video.
        let decoder = decoder_context
            .decoder()
            .video()
            .map_err(|_| VideoStreamError::DecoderCreateFailure)?;

        let stats = Self::get_stream_stats(&decoder, &video_stream)?;

        // If the video comes in with a pixel format we can't use (not RGBA),
        // we'll have to convert it.
        let formatter = VideoFrameFormatter::new(decoder.format(), stats.dimensions)?;

        Ok(Self {
            input_context,
            decoder,
            formatter,
            video_stream_index,
            stats,
            stream_over: false,
        })
    }

    fn get_stream_stats(
        decoder: &FFmpegVideoDecoder,
        video_stream: &FFmpegStream,
    ) -> Result<StreamStats, VideoStreamError> {
        let fps_frac = video_stream.avg_frame_rate();
        let stream_duration = video_stream.duration() as f64 * f64::from(video_stream.time_base());

        let fps: f64 = fps_frac.into();

        let stream_length =
            Some((stream_duration * fps_frac.0 as f64 / fps_frac.1 as f64) as usize);

        let dimensions = Dimensions::new(decoder.width(), decoder.height())
            .ok_or_else(|| VideoStreamError::ZeroLengthSide(decoder.width(), decoder.height()))?;

        // We'll try to stay one second ahead.
        let buffering_recommendation = fps.ceil() as usize;

        Ok(StreamStats {
            fps,
            stream_length,
            dimensions,
            buffering_recommendation,
        })
    }

    fn write_next_ffmpeg_buffer(
        &mut self,
        mut ffmpeg_buffer: FFmpegVideoFrame,
    ) -> Result<FFmpegVideoFrame, StreamError> {
        // We'll try and receive a frame. This will only work once packets have
        // been loaded into the decoder.
        let frame_written = self.formatter.format(&mut ffmpeg_buffer, |ffmpeg_buffer| {
            Ok(self.decoder.receive_frame(ffmpeg_buffer).is_ok())
        })?;

        if frame_written {
            return Ok(ffmpeg_buffer);
        }

        // If we didn't get a frame from the decoder, we need to make sure that
        // the stream didn't end.
        if self.stream_over {
            return Err(StreamError::StreamEnd);
        }

        // Internally, the `packets` iterator mutates our `input_context`, not
        // its own internal state. This means we can re-create the packets
        // iterator each time we want to grab a new packet.
        let mut packets = self.input_context.packets();

        loop {
            // We'll try and grab a packet if any are left.
            let Some((stream, packet)) = packets.next() else {
                // If no packets are left, we'll tell the decoder no more are
                // coming.
                self.decoder.send_eof()?;
                self.stream_over = true;
                break;
            };

            // If we find a packet for our stream we'll load it into the decoder
            // and then we'll take it from the top. Otherwise, just check the
            // next packet.
            if stream.index() == self.video_stream_index {
                self.decoder.send_packet(&packet)?;
                break;
            }
        }

        // If we made it here, we've now loaded the decoder with data, so we'll
        // take it from the top. This function will never be called recursively
        // more than once.
        self.write_next_ffmpeg_buffer(ffmpeg_buffer)
    }
}

impl BufferStream for Video {
    type Buffer = VideoFrameBuffer;

    fn stats(&self) -> StreamStats {
        self.stats
    }

    fn start_over(&mut self) -> Result<(), StreamError> {
        self.input_context.seek(0, ..)?;
        self.decoder.flush();
        self.stream_over = false;

        Ok(())
    }

    fn write_next_buffer(&mut self, buffer: Self::Buffer) -> Result<Self::Buffer, StreamError> {
        debug_assert_eq!(self.stats().dimensions, buffer.dimensions());

        VideoFrameBuffer::from_ffmpeg_video_frame(
            self.write_next_ffmpeg_buffer(buffer.into_ffmpeg_buffer())?,
        )
        .map_err(|err| err.into())
    }

    fn create_next_buffer(&mut self) -> Result<Self::Buffer, StreamError> {
        self.write_next_buffer(VideoFrameBuffer::new(self.stats().dimensions)?)
    }
}

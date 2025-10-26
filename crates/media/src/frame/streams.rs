//! All of the modules for the built-in [FrameStream]s that can be used to
//! construct [Producer](super::Producer)s of [Frame]s, along with the actual
//! [FrameStream] and [BufferStream] traits.

pub mod still_frame;

use std::time::Duration;

use super::{Dimensions, Frame, FrameBuffer};

/// A stream of [Frame]s. Also see [BufferStream] and [super::Producer].
///
/// There is a blanket implementation of this trait for all types that implement
/// [BufferStream].
pub trait FrameStream: Send + 'static {
    /// A collection of a few stats about this stream.
    ///
    /// # Contract
    ///
    /// Calling this function multiple times should never result in a different
    /// value being returned than what was returned from the first call.
    ///
    /// Barring any unexpected/unavoidable errors, the stream should produce
    /// the exact amount of frames before producing [StreamError::StreamEnd] as
    /// the returned [StreamStats::stream_length] value (if this value is not
    /// [None]).
    ///
    /// All frames produced by this stream should have the same dimensions as
    /// the returned [StreamStats::dimensions] value.
    fn stats(&self) -> StreamStats;

    /// Reset, starting the stream over from the beginning.
    ///
    /// For "live" streams with no start point and that don't end, this method
    /// can do nothing (without returning an error). If the stream *can't* be
    /// restarted, an error should be returned.
    ///
    /// An error being returned indicates that no frames may ever be able to be
    /// be generated again.
    fn start_over(&mut self) -> Result<(), StreamError>;

    /// Write the next frame's contents to the [Frame] `buffer`.
    ///
    /// It can safely be assumed that `frame`'s dimensions will match the
    /// dimensions from [FrameStream::stats].
    ///
    /// `frame` will usually be a frame that was recently returned by this
    /// [FrameStream], but not necessarily always. If `frame` cannot be worked
    /// with because of its internal [FrameBuffer] implementation, consume it
    /// and create a new [Frame].
    fn write_next_frame(&mut self, frame: Frame) -> Result<Frame, StreamError>;

    /// Create and return a [Frame] with the next frame's contents.
    ///
    /// An error being returned indicates that no future frames will be able to
    /// be generated until after [FrameStream::start_over] is called.
    ///
    /// # Contract
    ///
    /// The returned frame's dimensions should match the [Dimensions] from
    /// [FrameStream::stats].
    fn create_next_frame(&mut self) -> Result<Frame, StreamError>;
}

/// A stream of [FrameBuffer]s. Also see [FrameStream] and [super::Producer].
///
/// There is a blanket implementation of [FrameStream] for all types that
/// implement this trait. This trait exists solely to make implementing
/// [FrameStream] a bit easier.
pub trait BufferStream: Send + 'static {
    /// The type of [FrameBuffer] that this [BufferStream] will produce.
    type Buffer: FrameBuffer;

    /// A collection of a few stats about this stream.
    ///
    /// # Contract
    ///
    /// Calling this function multiple times should never result in a different
    /// value being returned than what was returned from the first call.
    ///
    /// Barring any unexpected/unavoidable errors, the stream should produce
    /// the exact amount of frames before producing [StreamError::StreamEnd] as
    /// the returned [StreamStats::stream_length] value (if this value is not
    /// [None]).
    ///
    /// All frames produced by this stream should have the same dimensions as
    /// the returned [StreamStats::dimensions] value.
    fn stats(&self) -> StreamStats;

    /// Reset, starting the stream over from the beginning.
    ///
    /// If it doesn't make sense for the stream to be started over (e.g. live
    /// input), this method can do nothing (without returning an error).
    ///
    /// An error being returned indicates that no buffers may ever be able to be
    /// be generated again.
    fn start_over(&mut self) -> Result<(), StreamError>;

    /// Write the next frame's contents to the [FrameBuffer] `buffer`.
    ///
    /// An error being returned indicates that no future frames will be able to
    /// be generated until after [FrameStream::start_over] is called.
    ///
    /// It can safely be assumed that `buffer`'s dimensions will match the
    /// dimensions from [BufferStream::stats].
    fn write_next_buffer(&mut self, buffer: Self::Buffer) -> Result<Self::Buffer, StreamError>;

    /// Create and return a [FrameBuffer] with the next frame's contents.
    ///
    /// An error being returned indicates that no future frames will be able to
    /// be generated until after [BufferStream::start_over] is called.
    ///
    /// # Contract
    ///
    /// The returned frame's dimensions should match the [Dimensions] from
    /// [BufferStream::stats].
    fn create_next_buffer(&mut self) -> Result<Self::Buffer, StreamError>;
}

impl<S: BufferStream + Send> FrameStream for S {
    fn stats(&self) -> StreamStats {
        <S as BufferStream>::stats(self)
    }

    fn start_over(&mut self) -> Result<(), StreamError> {
        <S as BufferStream>::start_over(self)
    }

    fn write_next_frame(&mut self, frame: Frame) -> Result<Frame, StreamError> {
        if let Ok(buffer) = frame.to_buffer() {
            self.write_next_buffer(buffer).map(Frame::from_buffer)
        } else {
            self.create_next_frame()
        }
    }

    fn create_next_frame(&mut self) -> Result<Frame, StreamError> {
        self.create_next_buffer().map(Frame::from_buffer)
    }
}

/// Stats about a stream ([FrameStream] or [BufferStream]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamStats {
    /// The intended number of frames per second to target for default-speed
    /// playback of a stream.
    ///
    /// A value of `0.0` or less indicates that the stream is just a single
    /// still frame.
    pub fps: f64,

    /// The number of frames a stream will produce before it cannot produce any
    /// more, or [None] if it can produce infinite frames.
    pub stream_length: Option<usize>,

    /// The dimensions of frames that a stream will produce.
    pub dimensions: Dimensions,

    /// A recommendation as to the number of frames that should be cached ahead
    /// of the frame that is currently needed so that frame's can be played back
    /// in real-time without stuttering.
    pub buffering_recommendation: usize,
}

impl StreamStats {
    /// The amount of time a stream will play for if played at [Self::fps]
    /// frames per second. [None] is returned if  [Self::stream_length] is
    /// [None] or [Self::fps] is not normal and positive non-zero.
    pub fn stream_duration(&self) -> Option<Duration> {
        if !self.fps.is_normal() || self.fps <= 0.0 {
            None
        } else {
            Some(Duration::from_secs_f64(
                self.stream_length? as f64 / self.fps,
            ))
        }
    }

    /// The amount of time between two frames if a stream is played at
    /// [Self::fps] frames per second. [None] is returned if [Self::fps] is not
    /// normal and positive non-zero.
    pub fn frame_interval(&self) -> Option<Duration> {
        if !self.fps.is_normal() || self.fps <= 0.0 {
            None
        } else {
            Some(Duration::from_secs(1).div_f64(self.fps))
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum StreamError {
    #[error("The stream ended.")]
    StreamEnd,
    #[error("Something went wrong with the stream.")]
    Other,
}

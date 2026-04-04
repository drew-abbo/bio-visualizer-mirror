//! Exports all kinds of [FrameStream]s ([PlaybackStream]s of [Frame]s).

mod stream_generator;
use stream_generator::StreamGenerator;

use ffmpeg_next as ffmpeg;

use util::channels::ChannelError;

use super::{Dimensions, RescaleMethod};
use crate::frame::Frame;
use crate::playback_stream::PlaybackStream;

mod still_frame_stream;
pub use still_frame_stream::*;

mod video_frame_stream;
pub use video_frame_stream::*;

/// A [PlaybackStream] of [Frame]s.
pub trait FrameStream: PlaybackStream<Frame, FrameStreamError> {
    /// Whether or not the last frame that was fetched is the same as the frame
    /// that was fetched before it.
    ///
    /// `true` should always be returned after the first call to
    /// [PlaybackStream::fetch].
    fn fetched_frame_changed(&self) -> bool;

    /// The dimensions of the [Frame]s that are produced.
    fn dimensions(&self) -> Dimensions;

    /// Change the dimensions of the [Frame]s that are produced. If rescaling
    /// will need to happen, `rescale_method` dictates how the frames should be
    /// rescaled.
    ///
    /// When `new_dimensions` matches the stream's
    /// [native dimensions](Self::native_dimensions), no rescaling should occur.
    fn set_dimensions(&mut self, new_dimensions: Dimensions, rescale_method: RescaleMethod);

    /// The native [Dimensions] of this stream. [When set](Self::set_dimensions)
    /// to this value, no rescaling is required.
    fn native_dimensions(&self) -> Dimensions;

    /// The rescale method (if any) that will be used to make fetched frames
    /// into the right dimensions.
    fn rescale_method(&self) -> Option<RescaleMethod>;

    /// Change the rescale method but keep the same dimensions. The new rescale
    /// method that is being used will be returned.
    fn set_rescale_method(&mut self, new_rescale_method: RescaleMethod) -> Option<RescaleMethod> {
        self.set_dimensions(self.dimensions(), new_rescale_method);
        self.rescale_method()
    }

    /// Set the produced [Frame]s dimensions to [Self::native_dimensions] so
    /// that no rescaling is required.
    fn reset_dimensions(&mut self) {
        self.set_dimensions(self.native_dimensions(), RescaleMethod::default());
    }

    /// Whether the last frame returned by [PlaybackStream::fetch] is visually
    /// distinct from the previously fetched frame.
    ///
    /// The default implementation returns `true` so callers remain correct even
    /// when a stream does not provide a distinctness signal.
    fn last_frame_is_distinct_from_previous(&self) -> bool {
        true
    }
}

/// Indicates something went wrong with [FrameStream] (a [PlaybackStream] of
/// [Frame]s).
#[derive(thiserror::Error, Debug, Clone)]
#[error(transparent)]
pub struct FrameStreamError(#[from] FrameStreamErrorInner);

#[derive(thiserror::Error, Debug, Clone)]
enum FrameStreamErrorInner {
    #[error("Video Error: {0}")]
    VideoError(#[from] ffmpeg::Error),
    #[error("Channel Error: {0}")]
    ChannelError(#[from] ChannelError),
}

impl From<ffmpeg::Error> for FrameStreamError {
    fn from(e: ffmpeg::Error) -> Self {
        Into::<FrameStreamErrorInner>::into(e).into()
    }
}

impl From<ChannelError> for FrameStreamError {
    fn from(e: ChannelError) -> Self {
        Into::<FrameStreamErrorInner>::into(e).into()
    }
}

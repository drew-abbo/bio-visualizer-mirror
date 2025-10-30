//! Defines [FrameStream], a simple [super::FrameStream] that just spits out the
//! same [Frame] indefinitely.

use super::{FrameStream, StreamError, StreamStats};
use crate::frame::Frame;

/// A [FrameStream] that just outputs a single frame forever.
#[derive(Debug, Clone)]
pub struct StillFrame(Frame);

impl StillFrame {
    /// Create a still frame stream from the provided `frame`.
    pub fn new(frame: Frame) -> Self {
        Self(frame)
    }
}

impl FrameStream for StillFrame {
    fn stats(&self) -> StreamStats {
        StreamStats {
            fps: 0.0,
            stream_length: None,
            dimensions: self.0.dimensions(),
            buffering_recommendation: 1,
        }
    }

    fn start_over(&mut self) -> Result<(), StreamError> {
        Ok(())
    }

    fn write_next_frame(&mut self, mut frame: Frame) -> Result<Frame, StreamError> {
        frame
            .fill_from_frame(&self.0)
            .expect("The dimensions should match.");
        Ok(frame)
    }

    fn create_next_frame(&mut self) -> Result<Frame, StreamError> {
        Ok(self.0.clone())
    }
}

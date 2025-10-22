//! This module contains the submodules [message_channel] and [request_channel],
//! 2 kinds of single producer single consumer queue-based message passing
//! systems.

pub mod message_channel;
pub mod request_channel;

use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;

/// An alias for a [Result] that has [ChannelError] as the error type.
pub type ChannelResult<T> = Result<T, ChannelError>;

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChannelError {
    #[error("One side of the connection was dropped.")]
    ConnectionDropped,
    #[error("The operation timed out after {}+ milliseconds.", timeout.as_millis())]
    Timeout { timeout: Duration },
    #[error("A response has already been received for this request.")]
    ResponseAlreadyReceived,
}

const THREAD_PANIC_MSG: &str = "Another thread panicked while holding a resource this one needs.";

fn ensure_connection_not_dropped<T>(channel: &Arc<T>) -> Result<(), ChannelError> {
    if Arc::strong_count(channel) == 2 {
        Ok(())
    } else {
        Err(ChannelError::ConnectionDropped)
    }
}

//! This module contains the submodules [message_channel] and [request_channel],
//! 2 kinds of single producer single consumer queue-based message passing
//! systems.

pub mod message_channel;
pub mod request_channel;

mod conn_n;
use conn_n::ConnN;

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

impl ChannelError {
    /// Whether this error is a [Self::ConnectionDropped] variant.
    #[inline(always)]
    pub fn is_connection_dropped_error(&self) -> bool {
        matches!(self, Self::ConnectionDropped)
    }

    /// Whether this error is a [Self::Timeout] variant.
    #[inline(always)]
    pub fn is_timeout_error(&self) -> bool {
        matches!(self, Self::Timeout { .. })
    }

    /// Whether this error is a [Self::ResponseAlreadyReceived] variant.
    #[inline(always)]
    pub fn is_response_already_received_error(&self) -> bool {
        matches!(self, Self::ResponseAlreadyReceived)
    }
}

const THREAD_PANIC_MSG: &str = "Another thread panicked while holding a resource this one needs.";

#[inline(always)]
fn connection_not_dropped<T>(channel: &ConnN<T>) -> bool {
    !channel.is_only_handle()
}

#[inline(always)]
fn ensure_connection_not_dropped<T>(channel: &ConnN<T>) -> Result<(), ChannelError> {
    if connection_not_dropped(channel) {
        Ok(())
    } else {
        Err(ChannelError::ConnectionDropped)
    }
}

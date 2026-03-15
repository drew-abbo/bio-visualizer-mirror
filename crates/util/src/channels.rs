//! This module contains the submodules [message_channel] and [request_channel],
//! 2 kinds of single producer single consumer queue-based message passing
//! systems.

pub mod message_channel;
pub mod request_channel;

mod conn_n;

use conn_n::ConnN;
use request_channel::ReqRes;

use std::convert::Infallible;
use std::time::Duration;

use thiserror::Error;

/// An alias for a [Result] that has [ChannelError] as the error type.
pub type ChannelResult<T, M = Infallible> = Result<T, ChannelError<M>>;

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChannelError<T = Infallible> {
    #[error("One side of the connection was dropped.")]
    ConnectionDropped,
    #[error("The wait operation timed out after {}+ milliseconds.", timeout.as_millis())]
    WaitTimeout { timeout: Duration },
    #[error("The send operation timed out after {}+ milliseconds.", timeout.as_millis())]
    SendTimeout { msg: T, timeout: Duration },
    #[error("The send operation is currently blocked.")]
    SendBlocked { msg: T },
    #[error("The send operation timed out after {}+ milliseconds (no msg).", timeout.as_millis())]
    SendTimeoutNoMsg { timeout: Duration },
    #[error("The send operation is currently blocked (no msg).")]
    SendBlockedNoMsg,
    #[error("A response has already been received for this request.")]
    ResponseAlreadyReceived,
}

impl<T> ChannelError<T> {
    /// Whether this error is a [Self::ConnectionDropped] variant.
    #[inline(always)]
    pub fn is_connection_dropped_error(&self) -> bool {
        matches!(self, Self::ConnectionDropped)
    }

    /// Whether this error is a [Self::ResponseAlreadyReceived] variant.
    #[inline(always)]
    pub fn is_response_already_received_error(&self) -> bool {
        matches!(self, Self::ResponseAlreadyReceived)
    }

    /// Whether this error is a [Self::WaitTimeout] variant.
    #[inline(always)]
    pub fn is_recv_timeout_error(&self) -> bool {
        matches!(self, Self::WaitTimeout { .. })
    }

    /// Whether this error is a [Self::WaitTimeout] variant.
    #[inline(always)]
    pub fn is_wait_timeout_error(&self) -> bool {
        matches!(self, Self::WaitTimeout { .. })
    }

    /// Whether this error is a [Self::SendTimeout] or [Self::SendTimeoutNoMsg]
    /// variant.
    #[inline(always)]
    pub fn is_send_timeout_error(&self) -> bool {
        matches!(
            self,
            Self::SendTimeout { .. } | Self::SendTimeoutNoMsg { .. }
        )
    }

    /// Whether this error is a timeout related error ([Self::WaitTimeout],
    /// [Self::SendTimeout], or [Self::SendTimeoutNoMsg] variant).
    #[inline(always)]
    pub fn is_any_timeout_error(&self) -> bool {
        self.is_wait_timeout_error() || self.is_send_timeout_error()
    }

    /// Whether this error is a [Self::SendBlocked] or [Self::SendBlockedNoMsg]
    /// variant.
    #[inline(always)]
    pub fn is_send_blocked_error(&self) -> bool {
        matches!(self, Self::SendBlocked { .. } | Self::SendBlockedNoMsg)
    }

    /// Maps an internal `msg` of type `T` to a new type `R` if it has one (for
    /// [Self::SendTimeout] and [Self::SendBlocked] variants).
    ///
    /// Also see [Self::unmap_msg].
    pub fn map_msg<F, R>(self, f: F) -> ChannelError<R>
    where
        F: FnOnce(T) -> R,
    {
        match self {
            Self::SendTimeout { msg, timeout } => {
                let msg = f(msg);
                ChannelError::SendTimeout { msg, timeout }
            }
            Self::SendBlocked { msg } => ChannelError::SendBlocked { msg: f(msg) },

            Self::ConnectionDropped => ChannelError::ConnectionDropped,
            Self::WaitTimeout { timeout } => ChannelError::WaitTimeout { timeout },
            Self::SendTimeoutNoMsg { timeout } => ChannelError::SendTimeoutNoMsg { timeout },
            Self::SendBlockedNoMsg => ChannelError::SendBlockedNoMsg,
            Self::ResponseAlreadyReceived => ChannelError::ResponseAlreadyReceived,
        }
    }

    /// Removes the internal `msg` of type `T` (for [Self::SendTimeout] and
    /// [Self::SendBlocked] variants).
    ///
    /// Also see [Self::map_msg].
    pub fn unmap_msg(self) -> ChannelError {
        match self {
            Self::SendTimeout { msg: _, timeout } => ChannelError::SendTimeoutNoMsg { timeout },
            Self::SendBlocked { msg: _ } => ChannelError::SendBlockedNoMsg,

            Self::ConnectionDropped => ChannelError::ConnectionDropped,
            Self::WaitTimeout { timeout } => ChannelError::WaitTimeout { timeout },
            Self::SendTimeoutNoMsg { timeout } => ChannelError::SendTimeoutNoMsg { timeout },
            Self::SendBlockedNoMsg => ChannelError::SendBlockedNoMsg,
            Self::ResponseAlreadyReceived => ChannelError::ResponseAlreadyReceived,
        }
    }

    /// Returns a reference to the internal `msg` of type `T` to a new type `R`
    /// if it has one (for [Self::SendTimeout] and [Self::SendBlocked]
    /// variants).
    ///
    /// Also see [Self::msg_mut] and [Self::into_msg].
    pub fn msg(&self) -> Option<&T> {
        match self {
            Self::SendTimeout { msg, .. } => Some(msg),
            Self::SendBlocked { msg } => Some(msg),
            _ => None,
        }
    }

    /// Returns a *mutable* reference to the internal `msg` of type `T` to a new
    /// type `R` if it has one (for [Self::SendTimeout] and [Self::SendBlocked]
    /// variants).
    ///
    /// Also see [Self::msg] and [Self::into_msg].
    pub fn msg_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::SendTimeout { msg, .. } => Some(msg),
            Self::SendBlocked { msg } => Some(msg),
            _ => None,
        }
    }

    /// Returns the internal `msg` of type `T` to a new type `R` if it has one
    /// (for [Self::SendTimeout] and [Self::SendBlocked] variants).
    ///
    /// Also see [Self::msg] and [Self::msg_mut].
    pub fn into_msg(self) -> Option<T> {
        match self {
            Self::SendTimeout { msg, .. } => Some(msg),
            Self::SendBlocked { msg } => Some(msg),
            _ => None,
        }
    }
}

impl<Q, A> ChannelError<ReqRes<Q, A>> {
    fn map_to_req(self) -> ChannelError<Q> {
        self.map_msg(|(req, _)| req)
    }
}

const THREAD_PANIC_MSG: &str = "Another thread panicked while holding a resource this one needs.";

#[inline(always)]
fn connection_not_dropped<C>(channel: &ConnN<C>) -> bool {
    !channel.is_only_handle()
}

#[inline(always)]
fn ensure_connection_not_dropped<C, T>(channel: &ConnN<C>) -> Result<(), ChannelError<T>> {
    if connection_not_dropped(channel) {
        Ok(())
    } else {
        Err(ChannelError::ConnectionDropped)
    }
}

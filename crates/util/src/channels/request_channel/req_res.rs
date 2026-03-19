//! The request and response types.

use std::mem;
use std::sync::{Condvar, Mutex, TryLockError};
use std::time::{Duration, Instant};

use super::{ChannelError, ChannelResult, ConnN, THREAD_PANIC_MSG};

/// The request data from a [Client](super::Client) (`Q`) and the handler from
/// the [Server](super::Server) that can be used to respond to the request
/// (optional because some requests are not meant to be replied to, see
/// [Client::alert](super::Client::alert)).
pub type ReqRes<Q, A> = (Q, Option<ResponseHandle<A>>);

/// A handle to use for responding to a request (usually from a
/// [Client](super::Client)).
///
/// Also see [Request].
#[derive(Debug)]
pub struct ResponseHandle<A>(ConnN<Responder<A>>);

impl<A> ResponseHandle<A> {
    /// Respond to a request from the server.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    pub fn respond(self, response: A) -> ChannelResult<()> {
        super::ensure_connection_not_dropped(&self.0)?;

        self.0
            .response
            .lock()
            .expect(THREAD_PANIC_MSG)
            .replace(response);

        // We need to notify the client that the request has been responded to
        // so that it if it's waiting.
        self.0.notifier.notify_one();

        Ok(())
    }

    /// Whether the other party still has their end of the connection alive, the
    /// inverse of [Self::connection_closed].
    pub fn connection_open(&self) -> bool {
        super::connection_not_dropped(&self.0)
    }

    /// Whether the other party has dropped their end of the connection, the
    /// inverse of [Self::connection_open].
    pub fn connection_closed(&self) -> bool {
        !self.connection_open()
    }
}

// We need a custom `Drop` implementation since the client may be waiting. We
// have to notify it that we won't reply so it doesn't just wait forever.
impl<A> Drop for ResponseHandle<A> {
    fn drop(&mut self) {
        // We need to notify the client that no response is coming.
        self.0.notifier.notify_one();
    }
}

/// A handle to use to await a response to a request (usually from a
/// [Server](super::Server), but not always).
///
/// Also see [ResponseHandle].
#[derive(Debug)]
pub struct Request<A>(RequestInner<A>);

impl<A> Request<A> {
    /// Create a new [Request] and [ResponseHandle].
    pub fn new() -> (Self, ResponseHandle<A>) {
        let [con1, con2] = ConnN::new::<2>(Responder::default());
        (Request(RequestInner::Awaiting(con1)), ResponseHandle(con2))
    }

    /// Create a [Request] that has already been responded to.
    ///
    /// No connection will be allocated since the request has already been
    /// resolved.
    pub const fn with_response(response: A) -> Self {
        Request(RequestInner::ResponseInline(response))
    }

    /// Create a [Request] that has already been received.
    ///
    /// No connection will be allocated since the request has already been
    /// resolved.
    pub const fn with_response_received() -> Self {
        Request(RequestInner::ResponseReceived)
    }

    /// Waits for a response from the server until one appears.
    ///
    /// For a version with a maximum wait time, see [Self::wait_timeout]. If you
    /// just want to check without waiting, see [Self::check].
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// A [ChannelError::ResponseAlreadyReceived] is returned if this request
    /// has already been responded to.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let request = client.request(1).unwrap();
    /// let response = request.wait().unwrap();
    /// ```
    pub fn wait(&mut self) -> ChannelResult<A> {
        if let Some(response_result) = self.0.received_response() {
            return response_result;
        }
        let responder = self.0.responder().unwrap();

        let mut response = responder.response.lock().expect(THREAD_PANIC_MSG);

        if let Some(response) = response.take() {
            return Ok(response);
        }

        // If there's no response we need to make sure the other end hasn't hung
        // up.
        super::ensure_connection_not_dropped(responder)?;

        loop {
            response = responder.notifier.wait(response).expect(THREAD_PANIC_MSG);

            if let Some(response) = response.take() {
                return Ok(response);
            }

            // No response after waking up means one of two things:
            // 1. The other end hung up.
            // 2. This was a spurious (early) wakeup (should go back to sleep).
            super::ensure_connection_not_dropped(responder)?;
        }
    }

    /// Waits for a response from the server for up to `timeout` time.
    ///
    /// After `timeout` time, a [ChannelError::WaitTimeout] error is returned.
    /// Note that this function's execution may take slightly longer than
    /// `timeout` time.
    ///
    /// For a version without a maximum waiting time, see [Self::wait]. If you
    /// just want to check without waiting, see [Self::check].
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// A [ChannelError::ResponseAlreadyReceived] is returned if this request
    /// has already been responded to.
    pub fn wait_timeout(&mut self, timeout: Duration) -> ChannelResult<A> {
        if let Some(response_result) = self.0.received_response() {
            return response_result;
        }
        let responder = self.0.responder().unwrap();

        let mut response = responder.response.lock().expect(THREAD_PANIC_MSG);

        if let Some(response) = response.take() {
            return Ok(response);
        }

        // If there's no response we need to make sure the other end hasn't hung
        // up.
        super::ensure_connection_not_dropped(responder)?;

        let deadline = Instant::now() + timeout;

        loop {
            let time_until_deadline = deadline.saturating_duration_since(Instant::now());

            let (returned_response, wait_result) = responder
                .notifier
                .wait_timeout(response, time_until_deadline)
                .expect(THREAD_PANIC_MSG);
            response = returned_response;

            match response.take() {
                Some(response) => return Ok(response),
                None if wait_result.timed_out() => {
                    return Err(ChannelError::WaitTimeout { timeout });
                }
                None => {}
            }

            // No response after waking up means one of two things:
            // 1. The other end hung up.
            // 2. This was a spurious (early) wakeup (should go back to sleep).
            super::ensure_connection_not_dropped(responder)?;
        }
    }

    /// Receives a response from the server if a response is waiting, returning
    /// [None] otherwise. This function may still block slightly.
    ///
    /// This function will block if the server is currently sending a response.
    /// For a function that will never block at all, see
    /// [Self::check_non_blocking]. If you want to wait for a request to appear,
    /// see [Self::wait] or [Self::wait_timeout].
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// A [ChannelError::ResponseAlreadyReceived] is returned if this request
    /// has already been responded to.
    pub fn check(&mut self) -> ChannelResult<Option<A>> {
        if let Some(response_result) = self.0.received_response() {
            return response_result.map(Some);
        }
        let responder = self.0.responder().unwrap();

        if let Some(response) = responder.response.lock().expect(THREAD_PANIC_MSG).take() {
            Ok(Some(response))
        } else {
            // If there's no response we need to make sure the other end hasn't
            // hung up.
            super::ensure_connection_not_dropped(responder)?;

            Ok(None)
        }
    }

    /// Receives a response from the server if the queue is not locked and a
    /// response is waiting. [None] is returned otherwise. This function will
    /// not block.
    ///
    /// Note that [None] being returned *does not* always mean there are no
    /// response waiting. If the server is currently adding an item, [None] will
    /// still be returned (even if there are items in the queue). If you don't
    /// want this behavior, see [Self::check]. If you want to wait for a
    /// response to appear, see [Self::wait] or [Self::wait_timeout].
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// A [ChannelError::ResponseAlreadyReceived] is returned if this request
    /// has already been responded to.
    pub fn check_non_blocking(&mut self) -> ChannelResult<Option<A>> {
        if let Some(response_result) = self.0.received_response() {
            return response_result.map(Some);
        }
        let responder = self.0.responder().unwrap();

        match responder.response.try_lock() {
            Ok(mut response) => Ok(response.take()),
            Err(e) => match e {
                TryLockError::WouldBlock => Ok(None),
                TryLockError::Poisoned(_) => panic!("{}", THREAD_PANIC_MSG),
            },
        }
    }

    /// Whether or not a response to this request has already been received.
    pub const fn response_received(&self) -> bool {
        matches!(self.0, RequestInner::ResponseReceived)
    }

    /// Whether or not a response could still be received (the inverse of
    /// [Self::connection_closed]).
    ///
    /// A [ChannelError::ResponseAlreadyReceived] is returned if this request
    /// has already been responded to.
    pub fn connection_open(&self) -> ChannelResult<bool> {
        match &self.0 {
            RequestInner::ResponseReceived => Err(ChannelError::ResponseAlreadyReceived),
            RequestInner::ResponseInline(_) => Ok(true),
            RequestInner::Awaiting(responder) => Ok(super::connection_not_dropped(responder)),
        }
    }

    /// Whether or not a response can no longer be received (the inverse of
    /// [Self::connection_open]).
    ///
    /// A [ChannelError::ResponseAlreadyReceived] is returned if this request
    /// has already been responded to.
    pub fn connection_closed(&self) -> ChannelResult<bool> {
        self.connection_open().map(|open| !open)
    }
}

impl<A> From<A> for Request<A> {
    fn from(response: A) -> Self {
        Self::with_response(response)
    }
}

#[derive(Debug)]
enum RequestInner<A> {
    ResponseReceived,
    ResponseInline(A),
    Awaiting(ConnN<Responder<A>>),
}

impl<A> RequestInner<A> {
    /// Returns any inline response (replacing `self` with
    /// [Self::ResponseReceived]) or an error if a response was already
    /// received. [None] is returned if the response is still being awaited.
    fn received_response(&mut self) -> Option<ChannelResult<A>> {
        match self {
            Self::ResponseReceived => Some(Err(ChannelError::ResponseAlreadyReceived)),
            Self::Awaiting(_) => None,
            Self::ResponseInline(_) => match mem::replace(self, Self::ResponseReceived) {
                Self::ResponseInline(response) => Some(Ok(response)),
                _ => unreachable!(),
            },
        }
    }

    /// Get the inner [Responder] if this an [Self::AwaitingResponse] variant.
    #[inline(always)]
    const fn responder(&self) -> Option<&ConnN<Responder<A>>> {
        match &self {
            RequestInner::Awaiting(responder) => Some(responder),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct Responder<A> {
    response: Mutex<Option<A>>,
    notifier: Condvar,
}

impl<A> Default for Responder<A> {
    fn default() -> Self {
        Self {
            response: Mutex::new(None),
            notifier: Condvar::default(),
        }
    }
}

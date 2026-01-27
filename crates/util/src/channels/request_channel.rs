//! This module defines the [Server] and [Client] types (along with
//! [Request] and [ResponseHandle]) for working with a two-way SPSC (single
//! producer single consumer) requesting system, useful in situations with a
//! single thread making requests and another single thread responding.

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::{Arc, Condvar, Mutex, TryLockError};
use std::time::{Duration, Instant};

use super::message_channel;
use super::{ChannelError, ChannelResult, THREAD_PANIC_MSG};

/// The request data from a [Client] (`Q`) and the handler from the [Server]
/// that can be used to respond to the request (optional because some requests
/// are not meant to be replied to, see [Client::alert]).
pub type ReqRes<Q, A> = (Q, Option<ResponseHandle<A>>);

/// The server (request receiver/responder) of a two-way message channel (single
/// producer single consumer). Also see [Client].
///
/// See [new] and [with_capacity] to construct.
#[derive(Debug)]
pub struct Server<Q, A> {
    channel: message_channel::Inbox<ReqRes<Q, A>>,
    _marker: PhantomData<A>,
}

impl<Q, A> Server<Q, A> {
    /// Waits for a request from the client until one appears.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// Also see:
    /// - [Self::wait_timeout]
    /// - [Self::check]
    /// - [Self::check_non_blocking]
    /// - [Self::wait_all]
    /// - [Self::wait_timeout_all]
    /// - [Self::check_all]
    /// - [Self::check_non_blocking_all]
    ///
    /// # Example
    ///
    /// ```ignore
    /// while let Ok((req, res)) = server.wait() {
    ///     // Do something with `req`...
    ///
    ///     // Respond to the request if you can.
    ///     if let Some(res) = res {
    ///         res.respond("Request handled.").unwrap();
    ///     }
    /// }
    /// ```
    pub fn wait(&self) -> ChannelResult<ReqRes<Q, A>> {
        self.channel.wait()
    }

    /// Waits for a request from the client for up to `timeout` time.
    ///
    /// After `timeout` time, a [ChannelError::Timeout] error is returned. Note
    /// that this function's execution may take slightly longer than `timeout`
    /// time.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// Also see:
    /// - [Self::wait]
    /// - [Self::check]
    /// - [Self::check_non_blocking]
    /// - [Self::wait_all]
    /// - [Self::wait_timeout_all]
    /// - [Self::check_all]
    /// - [Self::check_non_blocking_all]
    pub fn wait_timeout(&self, timeout: Duration) -> ChannelResult<ReqRes<Q, A>> {
        self.channel.wait_timeout(timeout)
    }

    /// Receives a request from the client if a request is waiting, returning
    /// [None] otherwise. This function may still block slightly.
    ///
    /// This function will block if the client is currently sending a new
    /// request.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// Also see:
    /// - [Self::wait]
    /// - [Self::wait_timeout]
    /// - [Self::check_non_blocking]
    /// - [Self::wait_all]
    /// - [Self::wait_timeout_all]
    /// - [Self::check_all]
    /// - [Self::check_non_blocking_all]
    pub fn check(&self) -> ChannelResult<Option<ReqRes<Q, A>>> {
        self.channel.check()
    }

    /// Receives a request from the client if the queue is not locked and a
    /// request is waiting. [None] is returned otherwise. This function will not
    /// block.
    ///
    /// Note that [None] being returned *does not* always mean there are no
    /// request waiting. If the client is currently adding an item, [None] will
    /// still be returned (even if there are items in the queue).
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// Also see:
    /// - [Self::wait]
    /// - [Self::wait_timeout]
    /// - [Self::check]
    /// - [Self::wait_all]
    /// - [Self::wait_timeout_all]
    /// - [Self::check_all]
    /// - [Self::check_non_blocking_all]
    pub fn check_non_blocking(&self) -> ChannelResult<Option<ReqRes<Q, A>>> {
        self.channel.check_non_blocking()
    }

    /// Waits for a request from the client until one appears, returning all
    /// requests if multiple have built up.
    ///
    /// The returned [VecDeque] is guaranteed to have at least 1 element.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// Also see:
    /// - [Self::wait]
    /// - [Self::wait_timeout]
    /// - [Self::check]
    /// - [Self::check_non_blocking]
    /// - [Self::wait_timeout_all]
    /// - [Self::check_all]
    /// - [Self::check_non_blocking_all]
    pub fn wait_all(&self) -> ChannelResult<VecDeque<ReqRes<Q, A>>> {
        self.channel.wait_all()
    }

    /// Waits for a request from the client for up to `timeout` time, returning
    /// all requests if multiple have built up.
    ///
    /// After `timeout` time, a [ChannelError::Timeout] error is returned. Note
    /// that this function's execution may take slightly longer than `timeout`
    /// time.
    ///
    /// The returned [VecDeque] is guaranteed to have at least 1 element.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// Also see:
    /// - [Self::wait]
    /// - [Self::wait_timeout]
    /// - [Self::check]
    /// - [Self::check_non_blocking]
    /// - [Self::wait_all]
    /// - [Self::check_all]
    /// - [Self::check_non_blocking_all]
    pub fn wait_timeout_all(&self, timeout: Duration) -> ChannelResult<VecDeque<ReqRes<Q, A>>> {
        self.channel.wait_timeout_all(timeout)
    }

    /// Receives all requests from the client if requests are waiting, returning
    /// [None] otherwise. This function may still block slightly.
    ///
    /// This function will block if the client is currently sending a new
    /// request.
    ///
    /// The returned [VecDeque] is guaranteed to have at least 1 element.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// Also see:
    /// - [Self::wait]
    /// - [Self::wait_timeout]
    /// - [Self::check]
    /// - [Self::check_non_blocking]
    /// - [Self::wait_all]
    /// - [Self::wait_timeout_all]
    /// - [Self::check_non_blocking_all]
    pub fn check_all(&self) -> ChannelResult<Option<VecDeque<ReqRes<Q, A>>>> {
        self.channel.check_all()
    }

    /// Receives all request from the client if the queue is not locked and a
    /// requests are waiting. [None] is returned otherwise. This function will
    /// not block.
    ///
    /// Note that [None] being returned *does not* always mean there are no
    /// request waiting. If the client is currently adding an item, [None] will
    /// still be returned (even if there are items in the queue).
    ///
    /// The returned [VecDeque] is guaranteed to have at least 1 element.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// Also see:
    /// - [Self::wait]
    /// - [Self::wait_timeout]
    /// - [Self::check]
    /// - [Self::check_non_blocking]
    /// - [Self::wait_all]
    /// - [Self::wait_timeout_all]
    /// - [Self::check_all]
    pub fn check_non_blocking_all(&self) -> ChannelResult<Option<VecDeque<ReqRes<Q, A>>>> {
        self.channel.check_non_blocking_all()
    }

    /// Whether the other party still has their end of the connection alive, the
    /// inverse of [Self::connection_closed].
    pub fn connection_open(&self) -> bool {
        self.channel.connection_open()
    }

    /// Whether the other party has dropped their end of the connection, the
    /// inverse of [Self::connection_open].
    pub fn connection_closed(&self) -> bool {
        self.channel.connection_closed()
    }
}

/// The client (request sender/receiver) of a two-way message channel (single
/// producer single consumer). Also see [Server].
///
/// See [new] and [with_capacity] to construct.
#[derive(Debug)]
pub struct Client<Q, A> {
    channel: message_channel::Outbox<ReqRes<Q, A>>,
}

impl<Q, A> Client<Q, A> {
    /// Send a request to the server.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    pub fn request(&self, request: Q) -> ChannelResult<Request<A>> {
        let responder = Arc::new(Responder {
            response: Mutex::new(None),
            notifier: Condvar::default(),
        });

        self.channel
            .send((request, Some(ResponseHandle(responder.clone()))))?;

        Ok(Request(Some(responder)))
    }

    /// Send a message to the server that it does not need to reply to.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    pub fn alert(&self, request: Q) -> ChannelResult<()> {
        self.channel.send((request, None)).map(|_| ())
    }

    /// The number of requests that have been sent but not received. Received
    /// does not necessarily mean responded to.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    pub fn messages_in_flight(&self) -> ChannelResult<usize> {
        self.channel.messages_in_flight()
    }

    /// Whether the other party still has their end of the connection alive, the
    /// inverse of [Self::connection_closed].
    pub fn connection_open(&self) -> bool {
        self.channel.connection_open()
    }

    /// Whether the other party has dropped their end of the connection, the
    /// inverse of [Self::connection_open].
    pub fn connection_closed(&self) -> bool {
        self.channel.connection_closed()
    }
}

/// A handle to use for responding to a request from a [Client].
#[derive(Debug)]
pub struct ResponseHandle<A>(Arc<Responder<A>>);

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

/// A handle to use to await a response to a request from a [Server].
#[derive(Debug)]
pub struct Request<A>(Option<Arc<Responder<A>>>);

impl<A> Request<A> {
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
        let request = self
            .0
            .as_ref()
            .ok_or(ChannelError::ResponseAlreadyReceived)?;

        let mut response = request.response.lock().expect(THREAD_PANIC_MSG);

        if let Some(response) = response.take() {
            return Ok(response);
        }

        // If there's no response we need to make sure the other end hasn't hung
        // up.
        super::ensure_connection_not_dropped(request)?;

        loop {
            response = request.notifier.wait(response).expect(THREAD_PANIC_MSG);

            if let Some(response) = response.take() {
                return Ok(response);
            }

            // No response after waking up means one of two things:
            // 1. The other end hung up.
            // 2. This was a spurious (early) wakeup (should go back to sleep).
            super::ensure_connection_not_dropped(request)?;
        }
    }

    /// Waits for a response from the server for up to `timeout` time.
    ///
    /// After `timeout` time, a [ChannelError::Timeout] error is returned. Note
    /// that this function's execution may take slightly longer than `timeout`
    /// time.
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
        let request = self
            .0
            .as_ref()
            .ok_or(ChannelError::ResponseAlreadyReceived)?;

        let mut response = request.response.lock().expect(THREAD_PANIC_MSG);

        if let Some(response) = response.take() {
            return Ok(response);
        }

        // If there's no response we need to make sure the other end hasn't hung
        // up.
        super::ensure_connection_not_dropped(request)?;

        let deadline = Instant::now() + timeout;

        loop {
            let time_until_deadline = deadline.saturating_duration_since(Instant::now());

            let (returned_response, wait_result) = request
                .notifier
                .wait_timeout(response, time_until_deadline)
                .expect(THREAD_PANIC_MSG);
            response = returned_response;

            match response.take() {
                Some(response) => return Ok(response),
                None if wait_result.timed_out() => {
                    return Err(ChannelError::Timeout { timeout });
                }
                None => {}
            }

            // No response after waking up means one of two things:
            // 1. The other end hung up.
            // 2. This was a spurious (early) wakeup (should go back to sleep).
            super::ensure_connection_not_dropped(request)?;
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
        let request = self
            .0
            .as_ref()
            .ok_or(ChannelError::ResponseAlreadyReceived)?;

        if let Some(response) = request.response.lock().expect(THREAD_PANIC_MSG).take() {
            Ok(Some(response))
        } else {
            // If there's no response we need to make sure the other end hasn't
            // hung up.
            super::ensure_connection_not_dropped(request)?;

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
        let request = self
            .0
            .as_ref()
            .ok_or(ChannelError::ResponseAlreadyReceived)?;

        match request.response.try_lock() {
            Ok(mut response) => Ok(response.take()),
            Err(e) => match e {
                TryLockError::WouldBlock => Ok(None),
                TryLockError::Poisoned(_) => panic!("{}", THREAD_PANIC_MSG),
            },
        }
    }

    /// Whether or not a response to this request has already been received.
    pub fn response_received(&self) -> bool {
        self.0.is_none()
    }

    /// Whether the other party still has their end of the connection alive, the
    /// inverse of [Self::connection_closed].
    ///
    /// A [ChannelError::ResponseAlreadyReceived] is returned if this request
    /// has already been responded to.
    pub fn connection_open(&self) -> ChannelResult<bool> {
        Ok(super::connection_not_dropped(
            self.0
                .as_ref()
                .ok_or(ChannelError::ResponseAlreadyReceived)?,
        ))
    }

    /// Whether the other party has dropped their end of the connection, the
    /// inverse of [Self::connection_open].
    ///
    /// A [ChannelError::ResponseAlreadyReceived] is returned if this request
    /// has already been responded to.
    pub fn connection_closed(&self) -> ChannelResult<bool> {
        self.connection_open().map(|open| !open)
    }
}

/// Create a two-way message channel's [Server] and [Client].
///
/// `Q` is the request type. `A` is the response type.
///
/// - The server will be able to receive requests as long as the client hasn't
///   been dropped.
/// - The client will be able to send requests as long as the server hasn't been
///   dropped.
/// - Requests will be able to be replied to as long as both the server and
///   client request objects ([ResponseHandle], and [Request] respectively) have
///   not been dropped.
pub fn new<Q, A>() -> (Server<Q, A>, Client<Q, A>) {
    let (inbox, outbox) = message_channel::new();
    (
        Server {
            channel: inbox,
            _marker: PhantomData,
        },
        Client { channel: outbox },
    )
}

/// Create a two-way message channel's [Server] and [Client] with space to store
/// `capacity` requests without reallocating memory. More requests than
/// `capacity` can still sit in the inbox at a time (the channel is not
/// bounded).
///
/// `Q` is the request type. `A` is the response type.
///
/// - The server will be able to receive requests as long as the client hasn't
///   been dropped.
/// - The client will be able to send requests as long as the server hasn't been
///   dropped.
/// - Requests will be able to be replied to as long as both the server and
///   client request objects ([ResponseHandle], and [Request] respectively) have
///   not been dropped.
pub fn with_capacity<Q, A>(capacity: usize) -> (Server<Q, A>, Client<Q, A>) {
    let (inbox, outbox) = message_channel::with_capacity(capacity);
    (
        Server {
            channel: inbox,
            _marker: PhantomData,
        },
        Client { channel: outbox },
    )
}

#[derive(Debug)]
struct Responder<A> {
    response: Mutex<Option<A>>,
    notifier: Condvar,
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    fn request_respond_works() {
        let (server, client) = new::<i32, i32>();

        let thread = thread::spawn(move || {
            let mut requests = vec![];
            for i in 1..=3 {
                requests.push(client.request(i));
            }

            for (request, i) in requests.into_iter().zip(1..=3) {
                assert!((request).is_ok());
                let mut request = request.unwrap();

                assert_eq!(request.wait(), Ok(-i));
            }
        });

        while let Ok((req, res)) = server.wait() {
            assert!(res.is_some());
            assert!(res.unwrap().respond(-req).is_ok());
        }

        thread.join().unwrap();
    }

    #[test]
    fn server_timeout_works() {
        let (server, client) = new::<i32, i32>();

        let thread = thread::spawn(move || {
            assert!(client.request(1).is_ok());

            // Do nothing (w/o dropping the client) for long enough that the
            // server times out.
            thread::sleep(Duration::from_millis(3000));
        });

        // Ensure client is up and running.
        assert!(server.wait().is_ok());

        let timeout = Duration::from_millis(500);
        assert!(matches!(
            server.wait_timeout(timeout),
            Err(ChannelError::Timeout { .. })
        ));

        thread.join().unwrap();
    }

    #[test]
    fn early_response_handler_drop_is_ok() {
        let (server, client) = new::<i32, i32>();

        let thread = thread::spawn(move || {
            assert!(client.request(1).is_ok());

            let mut req = client.request(2).unwrap();
            assert_eq!(req.wait(), Err(ChannelError::ConnectionDropped));

            // Do nothing (w/o dropping the client) for long enough that the
            // server times out.
            thread::sleep(Duration::from_millis(3000));
        });

        // Ensure client is up and running.
        assert!(server.wait().is_ok());

        let (_req, res) = server.wait().unwrap();
        drop(res);

        thread.join().unwrap();
    }
}

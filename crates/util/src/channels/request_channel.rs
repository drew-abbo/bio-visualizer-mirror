//! This module defines the [Server] and [Client] types (along with
//! [Request] and [ResponseHandle]) for working with a two-way SPSC (single
//! producer single consumer) requesting system, useful in situations with a
//! single thread making requests and another single thread responding.

mod req_res;

use std::collections::VecDeque;
use std::time::Duration;

use super::message_channel;
use super::{ChannelError, ChannelResult, ConnN, THREAD_PANIC_MSG};
use super::{connection_not_dropped, ensure_connection_not_dropped};

pub use req_res::*;

/// The server (request receiver/responder) of a two-way message channel (single
/// producer single consumer). Also see [Client].
///
/// See [new] and [with_capacity] to construct.
#[derive(Debug)]
pub struct Server<Q, A> {
    channel: message_channel::Inbox<ReqRes<Q, A>>,
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
    /// - [Self::wait_in_place]
    /// - [Self::wait_timeout_in_place]
    /// - [Self::check_in_place]
    /// - [Self::check_non_blocking_in_place]
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
    /// - [Self::wait_in_place]
    /// - [Self::wait_timeout_in_place]
    /// - [Self::check_in_place]
    /// - [Self::check_non_blocking_in_place]
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
    /// - [Self::wait_in_place]
    /// - [Self::wait_timeout_in_place]
    /// - [Self::check_in_place]
    /// - [Self::check_non_blocking_in_place]
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
    /// - [Self::wait_in_place]
    /// - [Self::wait_timeout_in_place]
    /// - [Self::check_in_place]
    /// - [Self::check_non_blocking_in_place]
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
    /// - [Self::wait_in_place]
    /// - [Self::wait_timeout_in_place]
    /// - [Self::check_in_place]
    /// - [Self::check_non_blocking_in_place]
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
    /// - [Self::wait_in_place]
    /// - [Self::wait_timeout_in_place]
    /// - [Self::check_in_place]
    /// - [Self::check_non_blocking_in_place]
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
    /// - [Self::wait_in_place]
    /// - [Self::wait_timeout_in_place]
    /// - [Self::check_in_place]
    /// - [Self::check_non_blocking_in_place]
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
    /// - [Self::wait_in_place]
    /// - [Self::wait_timeout_in_place]
    /// - [Self::check_in_place]
    /// - [Self::check_non_blocking_in_place]
    pub fn check_non_blocking_all(&self) -> ChannelResult<Option<VecDeque<ReqRes<Q, A>>>> {
        self.channel.check_non_blocking_all()
    }

    /// Waits for a request from the outbox until one appears, giving in-place
    /// access to all requests if multiple have built up.
    ///
    /// No messages can be sent while `f` is executing.
    ///
    /// The [VecDeque] is guaranteed to have at least 1 element.
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
    /// - [Self::check_non_blocking_all]
    /// - [Self::wait_timeout_in_place]
    /// - [Self::check_in_place]
    /// - [Self::check_non_blocking_in_place]
    pub fn wait_in_place<F, R>(&self, f: F) -> ChannelResult<R>
    where
        F: FnOnce(&mut VecDeque<ReqRes<Q, A>>) -> R,
    {
        self.channel.wait_in_place(f)
    }

    /// Waits for a request from the outbox for up to `timeout` time, giving
    /// in-place access to all requests if multiple have built up.
    ///
    /// After `timeout` time, a [ChannelError::Timeout] error is returned. Note
    /// that this function's execution may take slightly longer than `timeout`
    /// time.
    ///
    /// No messages can be sent while `f` is executing.
    ///
    /// The [VecDeque] is guaranteed to have at least 1 element.
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
    /// - [Self::check_non_blocking_all]
    /// - [Self::wait_in_place]
    /// - [Self::check_in_place]
    /// - [Self::check_non_blocking_in_place]
    pub fn wait_timeout_in_place<F, R>(&self, f: F, timeout: Duration) -> ChannelResult<R>
    where
        F: FnOnce(&mut VecDeque<ReqRes<Q, A>>) -> R,
    {
        self.channel.wait_timeout_in_place(f, timeout)
    }

    /// Gives in-place access to all requests from the outbox if at least one
    /// request is waiting, returning [None] otherwise. This function may still
    /// block slightly.
    ///
    /// This function will block if the outbox is currently sending a new
    /// message.
    ///
    /// No messages can be sent while `f` is executing.
    ///
    /// The [VecDeque] is guaranteed to have at least 1 element.
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
    /// - [Self::check_non_blocking_all]
    /// - [Self::wait_in_place]
    /// - [Self::wait_timeout_in_place]
    /// - [Self::check_non_blocking_in_place]
    pub fn check_in_place<F, R>(&self, f: F) -> ChannelResult<Option<R>>
    where
        F: FnOnce(&mut VecDeque<ReqRes<Q, A>>) -> R,
    {
        self.channel.check_in_place(f)
    }

    /// Gives in-place access to all requests from the outbox if the queue is
    /// not locked and at least one request is waiting, returning [None]
    /// otherwise. This function will not block.
    ///
    /// Note that [None] being returned *does not* always mean there are no
    /// messages in the inbox. If the outbox is currently adding an item, [None]
    /// will still be returned (even if there are items in the queue).
    ///
    /// No messages can be sent while `f` is executing.
    ///
    /// The [VecDeque] is guaranteed to have at least 1 element.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped and there are no more items in the queue.
    ///
    /// Also see:
    /// - [Self::wait]
    /// - [Self::wait_timeout]
    /// - [Self::check]
    /// - [Self::check_non_blocking]
    /// - [Self::check_non_blocking_all]
    /// - [Self::wait_all]
    /// - [Self::wait_timeout_all]
    /// - [Self::check_all]
    /// - [Self::wait_in_place]
    /// - [Self::wait_timeout_in_place]
    /// - [Self::check_in_place]
    pub fn check_non_blocking_in_place<F, R>(&self, f: F) -> ChannelResult<Option<R>>
    where
        F: FnOnce(&mut VecDeque<ReqRes<Q, A>>) -> R,
    {
        self.channel.check_non_blocking_in_place(f)
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

    /// Direct access to the inner request queue.
    ///
    /// No requests can be sent while `f` is executing.
    ///
    /// There are no checks for whether or not the connection has been dropped.
    pub fn with_queue_in_place<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut VecDeque<ReqRes<Q, A>>) -> R,
    {
        self.channel.with_queue_in_place(f)
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
    ///
    /// Also see [Self::request_bounded] and [Self::request_bounded].
    pub fn request(&self, request: Q) -> ChannelResult<Request<A>> {
        Self::send_template(self, request, |channel, msg| channel.send(msg))
    }

    /// Send a request to the server only once there are less than
    /// `max_in_flight` requests [in flight](Self::messages_in_flight).
    ///
    /// If `max_in_flight` is `0`, `1` will be used instead.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    ///
    /// Also see [Self::request] and [Self::request_bounded_timeout].
    pub fn request_bounded(&self, request: Q, max_in_flight: usize) -> ChannelResult<Request<A>> {
        Self::send_template(self, request, |channel, msg| {
            channel.send_bounded(msg, max_in_flight)
        })
    }

    /// Sends a request to the inbox only once there are less than
    /// `max_in_flight` requests [in flight](Self::messages_in_flight) (waiting
    /// for up to `timeout` time).
    ///
    /// After `timeout` time, a [ChannelError::Timeout] error is returned. Note
    /// that this function's execution may take slightly longer than `timeout`
    /// time.
    ///
    /// If `max_in_flight` is `0`, `1` will be used instead.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    ///
    /// Also see [Self::request] and [Self::request_bounded].
    pub fn request_bounded_timeout(
        &self,
        request: Q,
        max_in_flight: usize,
        timeout: Duration,
    ) -> ChannelResult<Request<A>> {
        Self::send_template(self, request, |channel, msg| {
            channel.send_bounded_timeout(msg, max_in_flight, timeout)
        })
    }

    /// Send a message to the server that it does not need to reply to.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    ///
    /// Also see [Self::alert_bounded] and [Self::alert_bounded_timeout].
    pub fn alert(&self, request: Q) -> ChannelResult<()> {
        self.alert_template(request, |channel, msg| channel.send(msg))
    }

    /// Send a message to the server that it does not need to reply to once
    /// there are less than `max_in_flight` requests
    /// [in flight](Self::messages_in_flight).
    ///
    /// If `max_in_flight` is `0`, `1` will be used instead.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    ///
    /// Also see [Self::alert] and [Self::alert_bounded_timeout].
    pub fn alert_bounded(&self, request: Q, max_in_flight: usize) -> ChannelResult<()> {
        Self::alert_template(self, request, |channel, msg| {
            channel.send_bounded(msg, max_in_flight)
        })
    }

    /// Send a message to the server that it does not need to reply to once
    /// there are less than `max_in_flight` requests
    /// [in flight](Self::messages_in_flight) (waiting for up to `timeout`
    /// time).
    ///
    /// After `timeout` time, a [ChannelError::Timeout] error is returned. Note
    /// that this function's execution may take slightly longer than `timeout`
    /// time.
    ///
    /// If `max_in_flight` is `0`, `1` will be used instead.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    ///
    /// Also see [Self::alert] and [Self::alert_bounded].
    pub fn alert_bounded_timeout(
        &self,
        request: Q,
        max_in_flight: usize,
        timeout: Duration,
    ) -> ChannelResult<()> {
        Self::alert_template(self, request, |channel, msg| {
            channel.send_bounded_timeout(msg, max_in_flight, timeout)
        })
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

    /// Direct access to the inner request queue.
    ///
    /// No requests can be received while `f` is executing.
    ///
    /// There are no checks for whether or not the connection has been dropped.
    pub fn with_queue_in_place<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut VecDeque<ReqRes<Q, A>>) -> R,
    {
        self.channel.with_queue_in_place(f)
    }

    #[inline]
    fn send_template<F, R>(&self, request: Q, sender: F) -> ChannelResult<Request<A>>
    where
        F: FnOnce(&message_channel::Outbox<ReqRes<Q, A>>, ReqRes<Q, A>) -> ChannelResult<R>,
    {
        let (req, res) = Request::new();
        sender(&self.channel, (request, Some(res)))?;
        Ok(req)
    }

    #[inline]
    fn alert_template<F, R>(&self, request: Q, sender: F) -> ChannelResult<()>
    where
        F: FnOnce(&message_channel::Outbox<ReqRes<Q, A>>, ReqRes<Q, A>) -> ChannelResult<R>,
    {
        sender(&self.channel, (request, None)).map(|_| ())
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
    (Server { channel: inbox }, Client { channel: outbox })
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
    (Server { channel: inbox }, Client { channel: outbox })
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

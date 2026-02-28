//! This module defines the [Inbox] and [Outbox] types for working with a
//! one-way SPSC (single producer single consumer) queue, useful in situations
//! with a single thread producing data and another single thread reading it.

use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

use super::{ChannelError, ChannelResult, THREAD_PANIC_MSG};

/// The inbox (message receiver) of a one-way message channel (single producer
/// single consumer queue). Also see [Outbox].
///
/// See [new], [with_capacity], and [with_starting_messages] to construct.
#[derive(Debug)]
pub struct Inbox<T> {
    channel: Arc<OneWayChannel<T>>,
}

impl<T> Inbox<T> {
    /// Waits for a message from the outbox until one appears.
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
    pub fn wait(&self) -> ChannelResult<T> {
        self.wait_for_queue(Self::queue_pop)
    }

    /// Waits for a message from the outbox for up to `timeout` time.
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
    pub fn wait_timeout(&self, timeout: Duration) -> ChannelResult<T> {
        self.wait_for_queue_timeout(timeout, Self::queue_pop)
    }

    /// Receives a message from the outbox if a message is waiting, returning
    /// [None] otherwise. This function may still block slightly.
    ///
    /// This function will block if the outbox is currently sending a new
    /// message.
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
    pub fn check(&self) -> ChannelResult<Option<T>> {
        self.check_for_queue(Self::queue_pop)
    }

    /// Receives a message from the outbox if the queue is not locked and a
    /// message is waiting. [None] is returned otherwise. This function will not
    /// block.
    ///
    /// Note that [None] being returned *does not* always mean there are no
    /// messages in the inbox. If the outbox is currently adding an item, [None]
    /// will still be returned (even if there are items in the queue).
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
    pub fn check_non_blocking(&self) -> ChannelResult<Option<T>> {
        self.check_for_queue_non_blocking(Self::queue_pop)
    }

    /// Waits for a message from the outbox until one appears, returning all
    /// messages if multiple have built up.
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
    pub fn wait_all(&self) -> ChannelResult<VecDeque<T>> {
        self.wait_for_queue(Self::queue_pop_all)
    }

    /// Waits for a message from the outbox for up to `timeout` time, returning
    /// all messages if multiple have built up.
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
    pub fn wait_timeout_all(&self, timeout: Duration) -> ChannelResult<VecDeque<T>> {
        self.wait_for_queue_timeout(timeout, Self::queue_pop_all)
    }

    /// Receives all messages from the outbox if at least one message is
    /// waiting, returning [None] otherwise. This function may still block
    /// slightly.
    ///
    /// This function will block if the outbox is currently sending a new
    /// message.
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
    pub fn check_all(&self) -> ChannelResult<Option<VecDeque<T>>> {
        self.check_for_queue(Self::queue_pop_all)
    }

    /// Receives all messages from the outbox if the queue is not locked and at
    /// least one message is waiting. [None] is returned otherwise. This
    /// function will not block.
    ///
    /// Note that [None] being returned *does not* always mean there are no
    /// messages in the inbox. If the outbox is currently adding an item, [None]
    /// will still be returned (even if there are items in the queue).
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
    pub fn check_non_blocking_all(&self) -> ChannelResult<Option<VecDeque<T>>> {
        self.check_for_queue_non_blocking(Self::queue_pop_all)
    }

    /// Waits for a message from the outbox until one appears, giving in-place
    /// access to all messages if multiple have built up.
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
        F: FnOnce(&mut VecDeque<T>) -> R,
    {
        self.wait_for_queue(|q| f(q))
    }

    /// Waits for a message from the outbox for up to `timeout` time, giving
    /// in-place access to all messages if multiple have built up.
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
        F: FnOnce(&mut VecDeque<T>) -> R,
    {
        self.wait_for_queue_timeout(timeout, |q| f(q))
    }

    /// Gives in-place access to all messages from the outbox if at least one
    /// message is waiting, returning [None] otherwise. This function may still
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
        F: FnOnce(&mut VecDeque<T>) -> R,
    {
        self.check_for_queue(|q| f(q))
    }

    /// Gives in-place access to all messages from the outbox if the queue is
    /// not locked and at least one message is waiting, returning [None]
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
        F: FnOnce(&mut VecDeque<T>) -> R,
    {
        self.check_for_queue_non_blocking(|q| f(q))
    }

    /// Whether the other party still has their end of the connection alive, the
    /// inverse of [Self::connection_closed].
    pub fn connection_open(&self) -> bool {
        super::connection_not_dropped(&self.channel)
    }

    /// Whether the other party has dropped their end of the connection, the
    /// inverse of [Self::connection_open].
    pub fn connection_closed(&self) -> bool {
        !self.connection_open()
    }

    /// Direct access to the inner message queue.
    ///
    /// No messages can be sent while `f` is executing.
    ///
    /// There are no checks for whether or not the connection has been dropped.
    pub fn with_queue_in_place<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut VecDeque<T>) -> R,
    {
        let mut queue = self.channel.queue.lock().expect(THREAD_PANIC_MSG);
        self.mutate_queue(&mut queue, |q| f(q))
    }

    /// Wait for the queue to have at least 1 item in it, then return the queue.
    fn wait_for_queue<F, R>(&self, f: F) -> ChannelResult<R>
    where
        F: FnOnce(&mut MutexGuard<'_, QueueAndBound<T>>) -> R,
    {
        let mut queue = self.channel.queue.lock().expect(THREAD_PANIC_MSG);

        if !queue.is_empty() {
            return Ok(self.mutate_queue(&mut queue, f));
        }

        // If there are no messages we need to make sure the other end hasn't
        // hung up.
        super::ensure_connection_not_dropped(&self.channel)?;

        loop {
            queue = self.channel.notifier.wait(queue).expect(THREAD_PANIC_MSG);

            if !queue.is_empty() {
                return Ok(self.mutate_queue(&mut queue, f));
            }

            // No messages after waking up means one of two things:
            // 1. The other end hung up.
            // 2. This was a spurious (early) wakeup (should go back to sleep).
            super::ensure_connection_not_dropped(&self.channel)?;
        }
    }

    /// Wait for the queue to have at least 1 item in it for up to `duration`,
    /// then return the queue.
    fn wait_for_queue_timeout<F, R>(&self, timeout: Duration, f: F) -> ChannelResult<R>
    where
        F: FnOnce(&mut MutexGuard<'_, QueueAndBound<T>>) -> R,
    {
        let mut queue = self.channel.queue.lock().expect(THREAD_PANIC_MSG);

        if !queue.is_empty() {
            return Ok(self.mutate_queue(&mut queue, f));
        }

        // If there are no messages we need to make sure the other end hasn't
        // hung up.
        super::ensure_connection_not_dropped(&self.channel)?;

        let deadline = Instant::now() + timeout;

        loop {
            let time_until_deadline = deadline.saturating_duration_since(Instant::now());

            let (returned_queue, wait_result) = self
                .channel
                .notifier
                .wait_timeout(queue, time_until_deadline)
                .expect(THREAD_PANIC_MSG);
            queue = returned_queue;

            if !queue.is_empty() {
                return Ok(self.mutate_queue(&mut queue, f));
            }

            if wait_result.timed_out() {
                return Err(ChannelError::Timeout { timeout });
            }

            // No messages after waking up means one of two things:
            // 1. The other end hung up.
            // 2. This was a spurious (early) wakeup (should go back to sleep).
            super::ensure_connection_not_dropped(&self.channel)?;
        }
    }

    /// See if the queue has at least 1 item in it. If it does, return the
    /// queue.
    fn check_for_queue<F, R>(&self, f: F) -> ChannelResult<Option<R>>
    where
        F: FnOnce(&mut MutexGuard<'_, QueueAndBound<T>>) -> R,
    {
        let mut queue = self.channel.queue.lock().expect(THREAD_PANIC_MSG);

        if !queue.is_empty() {
            return Ok(Some(self.mutate_queue(&mut queue, f)));
        }

        // If there are no messages we need to make sure the other end hasn't
        // hung up.
        super::ensure_connection_not_dropped(&self.channel)?;

        Ok(None)
    }

    /// See if the queue's mutex is unlocked and the queue has at least 1 item
    /// in it. If it does, return the queue.
    fn check_for_queue_non_blocking<F, R>(&self, f: F) -> ChannelResult<Option<R>>
    where
        F: FnOnce(&mut MutexGuard<'_, QueueAndBound<T>>) -> R,
    {
        match self.channel.queue.try_lock() {
            Ok(mut queue) => {
                if !queue.is_empty() {
                    Ok(Some(self.mutate_queue(&mut queue, f)))
                } else {
                    // If there are no messages we need to make sure the other
                    // end hasn't hung up.
                    super::ensure_connection_not_dropped(&self.channel)?;

                    Ok(None)
                }
            }
            Err(e) => match e {
                TryLockError::WouldBlock => Ok(None),
                TryLockError::Poisoned(_) => panic!("{}", THREAD_PANIC_MSG),
            },
        }
    }

    /// Do not mutate the queue in a way that the outbox may care about without
    /// doing it through this function.
    #[inline(always)]
    fn mutate_queue<F, R>(&self, queue: &mut MutexGuard<'_, QueueAndBound<T>>, f: F) -> R
    where
        F: FnOnce(&mut MutexGuard<'_, QueueAndBound<T>>) -> R,
    {
        let ret = f(queue);

        // We need to notify the outbox if it's waiting and the queue has been
        // freed up enough.
        if let Some(bound) = queue.bound
            && queue.len() < bound.get()
        {
            self.channel.notifier.notify_one();
        }

        ret
    }

    fn queue_pop(queue: &mut MutexGuard<'_, QueueAndBound<T>>) -> T {
        queue.pop_front().expect("The queue should be non-empty.")
    }

    fn queue_pop_all(queue: &mut MutexGuard<'_, QueueAndBound<T>>) -> VecDeque<T> {
        queue.split_off(0)
    }
}

// We need a custom `Drop` implementation since the outbox may be waiting. We
// have to notify it that no more messages will be removed from the inbox so it
// doesn't just wait forever.
impl<T> Drop for Inbox<T> {
    fn drop(&mut self) {
        self.channel.notifier.notify_one();
    }
}

/// The outbox (message sender) of a one-way message channel (single producer
/// single consumer queue). Also see [Inbox].
///
/// See [new], [with_capacity], and [with_starting_messages] to construct.
#[derive(Debug)]
pub struct Outbox<T> {
    channel: Arc<OneWayChannel<T>>,
}

impl<T> Outbox<T> {
    /// Sends a message to the inbox, returning the number of messages that have
    /// been sent but not received (after sending the message).
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    ///
    /// Also see [Self::send_bounded] and [Self::send_bounded_timeout].
    pub fn send(&self, msg: T) -> ChannelResult<usize> {
        super::ensure_connection_not_dropped(&self.channel)?;

        let mut queue = self.channel.queue.lock().expect(THREAD_PANIC_MSG);
        queue.push_back(msg);
        let in_flight = queue.len();

        // We need to notify the inbox that a message has arrived if it's
        // waiting.
        self.channel.notifier.notify_one();

        Ok(in_flight)
    }

    /// Sends a message to the inbox only once there are less than
    /// `max_in_flight` messages [in flight](Self::messages_in_flight),
    /// returning the number of messages that have been sent but not received
    /// (after sending the message).
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    ///
    /// If `max_in_flight` is `0`, `1` will be used instead.
    ///
    /// Also see [Self::send] and [Self::send_bounded_timeout].
    pub fn send_bounded(&self, msg: T, max_in_flight: usize) -> ChannelResult<usize> {
        super::ensure_connection_not_dropped(&self.channel)?;

        let mut queue = self.channel.queue.lock().expect(THREAD_PANIC_MSG);

        let max_in_flight = NonZeroUsize::new(max_in_flight).unwrap_or(NonZeroUsize::MIN);

        if queue.len() >= max_in_flight.get() {
            queue.bound = Some(max_in_flight);

            // Wait until queue is small enough.
            loop {
                queue = self.channel.notifier.wait(queue).expect(THREAD_PANIC_MSG);

                if queue.len() < max_in_flight.get() {
                    break;
                }

                super::ensure_connection_not_dropped(&self.channel)?;
            }

            queue.bound = None;
        }

        queue.push_back(msg);
        let in_flight = queue.len();

        // We need to notify the inbox that a message has arrived if it's
        // waiting.
        self.channel.notifier.notify_one();

        Ok(in_flight)
    }

    /// Sends a message to the inbox only once there are less than
    /// `max_in_flight` messages [in flight](Self::messages_in_flight) (waiting
    /// for up to `timeout` time), returning the number of messages that have
    /// been sent but not received (after sending the message).
    ///
    /// After `timeout` time, a [ChannelError::Timeout] error is returned. Note
    /// that this function's execution may take slightly longer than `timeout`
    /// time.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    ///
    /// If `max_in_flight` is `0`, `1` will be used instead.
    ///
    /// Also see [Self::send] and [Self::send_bounded].
    pub fn send_bounded_timeout(
        &self,
        msg: T,
        max_in_flight: usize,
        timeout: Duration,
    ) -> ChannelResult<usize> {
        super::ensure_connection_not_dropped(&self.channel)?;

        let mut queue = self.channel.queue.lock().expect(THREAD_PANIC_MSG);

        let max_in_flight = NonZeroUsize::new(max_in_flight).unwrap_or(NonZeroUsize::MIN);

        if queue.len() >= max_in_flight.get() {
            queue.bound = Some(max_in_flight);

            let deadline = Instant::now() + timeout;

            // Wait until queue is small enough.
            loop {
                let time_until_deadline = deadline.saturating_duration_since(Instant::now());

                let (returned_queue, wait_result) = self
                    .channel
                    .notifier
                    .wait_timeout(queue, time_until_deadline)
                    .expect(THREAD_PANIC_MSG);
                queue = returned_queue;

                if queue.len() < max_in_flight.get() {
                    break;
                }

                if wait_result.timed_out() {
                    return Err(ChannelError::Timeout { timeout });
                }

                // No messages after waking up means one of two things:
                // 1. The other end hung up.
                // 2. This was a spurious (early) wakeup (should go back to
                //    sleep).
                super::ensure_connection_not_dropped(&self.channel)?;
            }

            queue.bound = None;
        }

        queue.push_back(msg);
        let in_flight = queue.len();

        // We need to notify the inbox that a message has arrived if it's
        // waiting.
        self.channel.notifier.notify_one();

        Ok(in_flight)
    }

    /// The number of messages that have been sent but not received.
    ///
    /// A [ChannelError::ConnectionDropped] error is returned if the other end
    /// of the connection was dropped.
    pub fn messages_in_flight(&self) -> ChannelResult<usize> {
        super::ensure_connection_not_dropped(&self.channel)?;

        Ok(self.channel.queue.lock().expect(THREAD_PANIC_MSG).len())
    }

    /// Whether the other party still has their end of the connection alive, the
    /// inverse of [Self::connection_closed].
    pub fn connection_open(&self) -> bool {
        super::connection_not_dropped(&self.channel)
    }

    /// Whether the other party has dropped their end of the connection, the
    /// inverse of [Self::connection_open].
    pub fn connection_closed(&self) -> bool {
        !self.connection_open()
    }
}

// We need a custom `Drop` implementation since the inbox may be waiting. We
// have to notify it that no more messages are coming so it doesn't just wait
// forever.
impl<T> Drop for Outbox<T> {
    fn drop(&mut self) {
        self.channel.notifier.notify_one();
    }
}

/// Create a one-way message channel's [Inbox] and [Outbox].
///
/// - The inbox will be able to receive messages as long as the outbox hasn't
///   been dropped or while there are still pending messages.
/// - The outbox will be able to send messages as long as the inbox hasn't been
///   dropped.
pub fn new<T>() -> (Inbox<T>, Outbox<T>) {
    OneWayChannel {
        queue: Mutex::default(),
        notifier: Condvar::default(),
    }
    .into()
}

/// Create a one-way message channel's [Inbox] and [Outbox] with space to store
/// `capacity` messages without reallocating memory. More messages than
/// `capacity` can still sit in the inbox at a time (the channel is not
/// bounded).
///
/// - The inbox will be able to receive messages as long as the outbox hasn't
///   been dropped or while there are still pending messages.
/// - The outbox will be able to send messages as long as the inbox hasn't been
///   dropped.
pub fn with_capacity<T>(capacity: usize) -> (Inbox<T>, Outbox<T>) {
    OneWayChannel {
        queue: Mutex::new(VecDeque::with_capacity(capacity).into()),
        notifier: Condvar::default(),
    }
    .into()
}

/// Create a one-way message channel's [Inbox] and [Outbox] with starting
/// messages in the inbox.
///
/// - The inbox will be able to receive messages as long as the outbox hasn't
///   been dropped or while there are still pending messages.
/// - The outbox will be able to send messages as long as the inbox hasn't been
///   dropped.
pub fn with_starting_messages<T, I: IntoIterator<Item = T>>(msg: I) -> (Inbox<T>, Outbox<T>) {
    OneWayChannel {
        queue: Mutex::new(msg.into_iter().collect()),
        notifier: Condvar::default(),
    }
    .into()
}

#[derive(Debug)]
struct QueueAndBound<T> {
    queue: VecDeque<T>,
    bound: Option<NonZeroUsize>,
}

impl<T> QueueAndBound<T> {
    pub fn new(queue: VecDeque<T>) -> Self {
        Self { queue, bound: None }
    }
}

impl<T> Default for QueueAndBound<T> {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            bound: None,
        }
    }
}

impl<T> From<VecDeque<T>> for QueueAndBound<T> {
    fn from(queue: VecDeque<T>) -> Self {
        Self::new(queue)
    }
}

impl<T> FromIterator<T> for QueueAndBound<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::new(VecDeque::from_iter(iter))
    }
}

impl<T> AsRef<VecDeque<T>> for QueueAndBound<T> {
    fn as_ref(&self) -> &VecDeque<T> {
        &self.queue
    }
}

impl<T> AsMut<VecDeque<T>> for QueueAndBound<T> {
    fn as_mut(&mut self) -> &mut VecDeque<T> {
        &mut self.queue
    }
}

impl<T> Deref for QueueAndBound<T> {
    type Target = VecDeque<T>;

    fn deref(&self) -> &Self::Target {
        &self.queue
    }
}

impl<T> DerefMut for QueueAndBound<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.queue
    }
}

#[derive(Debug)]
struct OneWayChannel<T> {
    queue: Mutex<QueueAndBound<T>>,
    notifier: Condvar,
}

impl<T> From<OneWayChannel<T>> for (Inbox<T>, Outbox<T>) {
    fn from(channel: OneWayChannel<T>) -> Self {
        let channel = Arc::new(channel);
        (
            Inbox {
                channel: channel.clone(),
            },
            Outbox { channel },
        )
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    fn messages_can_be_received() {
        let (inbox, outbox) = new::<i32>();

        let thread = thread::spawn(move || {
            assert!(outbox.send(1).is_ok());
            assert!(outbox.send(2).is_ok());
            assert!(outbox.send(3).is_ok());
        });

        assert_eq!(inbox.wait(), Ok(1));
        assert_eq!(inbox.wait(), Ok(2));
        assert_eq!(inbox.wait(), Ok(3));

        thread.join().unwrap();
    }

    #[test]
    fn timeout_works() {
        let (inbox, outbox) = new::<i32>();

        assert!(outbox.send(1).is_ok());

        let timeout = Duration::from_millis(500);
        assert_eq!(inbox.wait_timeout(timeout), Ok(1));
        assert_eq!(
            inbox.wait_timeout(timeout),
            Err(ChannelError::Timeout { timeout })
        );
    }

    #[test]
    fn check_works() {
        let (inbox, outbox) = new::<i32>();

        assert_eq!(inbox.check(), Ok(None));
        assert_eq!(inbox.check(), Ok(None));

        assert!(outbox.send(1).is_ok());

        assert_eq!(inbox.check(), Ok(Some(1)));
        assert_eq!(inbox.check(), Ok(None));
        assert_eq!(inbox.check(), Ok(None));
    }

    #[test]
    fn lots_of_messages_are_ok() {
        let (inbox, outbox) = new::<i32>();

        let thread = thread::spawn(move || {
            for i in 1..=10_000 {
                assert!(outbox.send(i).is_ok());
            }
        });

        for i in 1..=10_000 {
            let msg = inbox.wait();
            assert!(msg.is_ok());
            assert!(msg.unwrap() == i);
        }

        thread.join().unwrap();
    }

    #[test]
    fn early_inbox_drop_is_fine() {
        let (inbox, outbox) = new::<i32>();

        let thread = thread::spawn(move || {
            let mut failed_on_1st_msg = true;
            for i in 1.. {
                if outbox.send(i).is_err() {
                    break;
                }
                failed_on_1st_msg = false;

                // Just so that we don't use too much memory...
                if i > 1_000 {
                    thread::sleep(Duration::from_millis(100));
                }
            }
            assert!(!failed_on_1st_msg)
        });

        thread::sleep(Duration::from_millis(500));
        drop(inbox);

        thread.join().unwrap();
    }

    #[test]
    fn early_outbox_drop_is_fine() {
        let (inbox, outbox) = new::<i32>();

        thread::scope(|s| {
            s.spawn(move || {
                assert!(outbox.send(1).is_ok());
                assert!(outbox.send(2).is_ok());
                assert!(outbox.send(3).is_ok());

                drop(outbox);
            });
        });

        assert_eq!(inbox.wait(), Ok(1));
        assert_eq!(inbox.wait(), Ok(2));
        assert_eq!(inbox.wait(), Ok(3));

        assert!(inbox.wait().is_err());
    }

    #[test]
    fn bounded_works() {
        let (inbox, outbox) = new::<i32>();

        const BOUND: usize = 2;

        let thread = thread::spawn(move || {
            for i in 1..=32 {
                assert!(outbox.send_bounded(i, BOUND).is_ok());
            }

            drop(outbox);
        });

        thread::sleep(Duration::from_millis(500));

        while let Ok(msgs) = inbox.wait_all() {
            assert!(msgs.len() <= BOUND);

            // Give it time to re-populate
            thread::sleep(Duration::from_millis(75));
        }

        thread.join().unwrap();
    }

    #[test]
    fn bounded_drop_works() {
        let (inbox, outbox) = new::<i32>();

        const BOUND: usize = 2;

        let thread = thread::spawn(move || {
            assert!(outbox.send_bounded(1, BOUND).is_ok());
            assert!(outbox.send_bounded(2, BOUND).is_ok());
            assert!(outbox.send_bounded(3, BOUND).is_ok());
            assert!(
                outbox
                    .send_bounded(4, BOUND)
                    .is_err_and(|e| e == ChannelError::ConnectionDropped)
            );

            drop(outbox);
        });

        assert_eq!(inbox.wait(), Ok(1));
        thread::sleep(Duration::from_millis(500));
        drop(inbox);

        thread.join().unwrap();
    }
}

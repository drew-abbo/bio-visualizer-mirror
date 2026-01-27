//! This module defines the [Inbox] and [Outbox] types for working with a
//! one-way SPSC (single producer single consumer) queue, useful in situations
//! with a single thread producing data and another single thread reading it.

use std::collections::VecDeque;
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
    pub fn check_non_blocking_all(&self) -> ChannelResult<Option<VecDeque<T>>> {
        self.check_for_queue_non_blocking(Self::queue_pop_all)
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

    /// Wait for the queue to have at least 1 item in it, then return the queue.
    fn wait_for_queue<F, R>(&self, f: F) -> ChannelResult<R>
    where
        F: FnOnce(MutexGuard<'_, VecDeque<T>>) -> R,
    {
        let mut queue = self.channel.queue.lock().expect(THREAD_PANIC_MSG);

        if !queue.is_empty() {
            return Ok(f(queue));
        }

        // If there are no messages we need to make sure the other end hasn't
        // hung up.
        super::ensure_connection_not_dropped(&self.channel)?;

        loop {
            queue = self.channel.notifier.wait(queue).expect(THREAD_PANIC_MSG);

            if !queue.is_empty() {
                return Ok(f(queue));
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
        F: FnOnce(MutexGuard<'_, VecDeque<T>>) -> R,
    {
        let mut queue = self.channel.queue.lock().expect(THREAD_PANIC_MSG);

        if !queue.is_empty() {
            return Ok(f(queue));
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
                return Ok(f(queue));
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
        F: FnOnce(MutexGuard<'_, VecDeque<T>>) -> R,
    {
        let queue = self.channel.queue.lock().expect(THREAD_PANIC_MSG);

        if !queue.is_empty() {
            return Ok(Some(f(queue)));
        }

        // If there are no messages we need to make sure the other end hasn't
        // hung up.
        super::ensure_connection_not_dropped(&self.channel)?;

        Ok(None)
    }

    /// See if the queue's mutx is unlocked and the queue has at least 1 item in
    /// it. If it does, return the queue.
    fn check_for_queue_non_blocking<F, R>(&self, f: F) -> ChannelResult<Option<R>>
    where
        F: FnOnce(MutexGuard<'_, VecDeque<T>>) -> R,
    {
        match self.channel.queue.try_lock() {
            Ok(queue) => {
                if !queue.is_empty() {
                    Ok(Some(f(queue)))
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

    fn queue_pop(mut queue: MutexGuard<'_, VecDeque<T>>) -> T {
        queue.pop_front().expect("The queue should be non-empty.")
    }

    fn queue_pop_all(mut queue: MutexGuard<'_, VecDeque<T>>) -> VecDeque<T> {
        queue.split_off(0)
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
        queue: Mutex::new(VecDeque::with_capacity(capacity)),
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
struct OneWayChannel<T> {
    queue: Mutex<VecDeque<T>>,
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
}

use anyhow::Result;
use crossbeam_channel as cb;
pub mod messages;

// This is going to house the logic for our mailbox/thread-per-component design

#[derive(Debug)]
pub struct Mailbox<T> {
    tx: cb::Sender<T>,
    rx: cb::Receiver<T>,
}

// had to do this manually might be removed later if I understand better
impl<T> Clone for Mailbox<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            rx: self.rx.clone(),
        }
    }
}

impl<T> Mailbox<T> {
    /// Create a directed pair: (A -> B) and (B -> A) via two channels.
    pub fn new_pair(cap: usize) -> (Self, Self) {
        let (a_to_b_tx, a_to_b_rx) = cb::bounded::<T>(cap);
        let (b_to_a_tx, b_to_a_rx) = cb::bounded::<T>(cap);

        // Endpoint A: sends to B, receives from B
        let a = Mailbox {
            tx: a_to_b_tx,
            rx: b_to_a_rx,
        };
        // Endpoint B: sends to A, receives from A
        let b = Mailbox {
            tx: b_to_a_tx,
            rx: a_to_b_rx,
        };

        (a, b)
    }

    pub fn send(&self, msg: T) -> Result<(), cb::SendError<T>> {
        self.tx.send(msg)
    }
    pub fn try_send(&self, msg: T) -> Result<(), cb::TrySendError<T>> {
        self.tx.try_send(msg)
    }

    pub fn recv(&self) -> Result<T, cb::RecvError> {
        self.rx.recv()
    }
    pub fn try_recv(&self) -> Result<T, cb::TryRecvError> {
        self.rx.try_recv()
    }

    pub fn sender(&self) -> cb::Sender<T> {
        self.tx.clone()
    }
    pub fn receiver(&self) -> cb::Receiver<T> {
        self.rx.clone()
    }
}

use super::message::EngineOutpostEvent;
use std::sync::Mutex;
use util::channels::message_channel::{self, Inbox, Outbox};

/// Declares which event categories a subscriber wants to receive.
/// Use `EventFilter::All` during development, then narrow to `Only` in production.
#[derive(Debug, Clone)]
pub enum EventFilter {
    All,
    Only(Vec<EventKind>),
}

/// Coarse-grained event categories for filtering.
/// Maps 1-to-1 with the variants of [`EngineOutpostEvent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    FrameReady,
    StreamState, // StreamsPaused, StreamsPlaying, StreamLoading
    FpsChanged,  // GlobalStreamTargetFpsChanged
    InfoResponse,
    ExecutionError,
}

impl EventFilter {
    pub fn matches(&self, event: &EngineOutpostEvent) -> bool {
        match self {
            EventFilter::All => true,
            EventFilter::Only(kinds) => {
                let kind = EventKind::from(event);
                kinds.contains(&kind)
            }
        }
    }
}

impl From<&EngineOutpostEvent> for EventKind {
    fn from(event: &EngineOutpostEvent) -> Self {
        match event {
            EngineOutpostEvent::FrameReady(_) => EventKind::FrameReady,
            EngineOutpostEvent::StreamsPaused
            | EngineOutpostEvent::StreamsPlaying
            | EngineOutpostEvent::StreamLoading(_) => EventKind::StreamState,
            EngineOutpostEvent::GlobalStreamTargetFpsChanged(_) => EventKind::FpsChanged,
            EngineOutpostEvent::InfoResponse(_) => EventKind::InfoResponse,
            EngineOutpostEvent::ExecutionError(_) => EventKind::ExecutionError,
        }
    }
}

/// Independent per-subscriber event receiver.
/// Not Clone - each subscriber owns its own inbox exclusively.
pub struct EngineEventReceiver {
    rx: Inbox<EngineOutpostEvent>,
}

impl EngineEventReceiver {
    pub fn drain(&self) -> Vec<EngineOutpostEvent> {
        let mut events = Vec::new();
        while let Ok(Some(event)) = self.rx.check_non_blocking() {
            events.push(event);
        }
        events
    }

    pub fn try_recv(&self) -> Option<EngineOutpostEvent> {
        self.rx.check_non_blocking().ok().flatten()
    }
}

struct Subscriber {
    filter: EventFilter,
    tx: Outbox<EngineOutpostEvent>,
}

/// Sits between the engine thread and all subscribers.
/// The engine thread holds an `Arc<EventBroadcaster>` and calls `broadcast`
/// for every event. Each subscriber gets its own independent queue.
pub struct EventBroadcaster {
    subscribers: Mutex<Vec<Subscriber>>,
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBroadcaster {
    pub fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }

    /// Register a new subscriber. Returns an independent receiver.
    /// Dead subscribers (disconnected inbox) are pruned on the next broadcast.
    pub fn subscribe(&self, filter: EventFilter) -> EngineEventReceiver {
        let (rx, tx) = message_channel::new();
        let mut subs = self.subscribers.lock().unwrap();
        subs.push(Subscriber { filter, tx });
        EngineEventReceiver { rx }
    }

    /// Broadcast an event to all matching subscribers.
    /// Subscribers whose inbox has been dropped are silently removed.
    pub fn broadcast(&self, event: EngineOutpostEvent) {
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|sub| {
            if !sub.filter.matches(&event) {
                return true; // keep, just not interested in this event
            }
            // send returns Err if the receiver was dropped — prune those
            sub.tx.send(event.clone()).is_ok()
        });
    }
}

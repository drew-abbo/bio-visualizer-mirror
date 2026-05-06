use super::message::EngineCommand;
use std::sync::Arc;
use util::channels::{ChannelResult, message_channel::Outbox};

/// A cheaply cloneable handle for sending commands to the engine thread.
/// Hand this to any component that needs to drive the engine.
/// Components do not need to hold the full [`EngineOutpostHandle`] just to send commands.
#[derive(Clone)]
pub struct EngineCommandSender {
    command_tx: Arc<Outbox<EngineCommand>>,
}

impl EngineCommandSender {
    pub(super) fn new(command_tx: Arc<Outbox<EngineCommand>>) -> Self {
        Self { command_tx }
    }

    pub fn send(&self, command: EngineCommand) -> ChannelResult<usize, EngineCommand> {
        self.command_tx.send(command)
    }
}

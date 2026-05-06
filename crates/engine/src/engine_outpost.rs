//! Self-driving engine outpost.
//!
//! Owns the [`GraphExecutor`] and runs it on a dedicated thread at the
//! configured FPS. The caller communicates exclusively through
//! [`EngineOutpostHandle`] - sending [`EngineCommand`]s in and draining
//! [`EngineOutpostEvent`]s out.
//!
//! Graph changes refresh execution state, but frame cadence stays driven by the
//! engine timer so parameter edits do not speed up playback.

pub mod broadcast;
pub mod command_sender;
pub mod message;

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use media::fps::Fps;
use media::fps::SwitchTimer;
use media::fps::consts::FPS_60;
use util::channels::ChannelResult;
use util::channels::message_channel::{self, Inbox, Outbox};

use super::graph_executor::{ExecutionError, GraphExecutor, NodeValue};
use crate::node::NodeLibrary;
use crate::node_graph::NodeGraph;

pub use broadcast::{EngineEventReceiver, EventBroadcaster, EventFilter, EventKind};
pub use command_sender::EngineCommandSender;
pub use message::{EngineCommand, EngineOutpostEvent};

/// How long the engine thread blocks waiting for commands while paused.
/// Long enough to not burn CPU, short enough to stay responsive to play/unpause.
const PAUSED_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// A cheaply cloneable handle to the engine thread.
///
/// Cloning produces another handle that shares the same underlying channels,
/// so multiple parts of the UI can send commands or drain events independently.
#[derive(Clone)]
pub struct EngineOutpostHandle {
    command_tx: Arc<Outbox<EngineCommand>>,
    broadcaster: Arc<EventBroadcaster>,
}

impl EngineOutpostHandle {
    pub fn command_sender(&self) -> EngineCommandSender {
        EngineCommandSender::new(self.command_tx.clone())
    }

    pub fn subscribe(&self, filter: EventFilter) -> EngineEventReceiver {
        self.broadcaster.subscribe(filter)
    }

    // send_command can now just delegate, or you can remove it
    // and require callers to go through command_sender() explicitly
    pub fn send_command(&self, command: EngineCommand) -> ChannelResult<usize, EngineCommand> {
        self.command_tx.send(command)
    }
}

/// Spawn the engine thread and return a handle to it.
///
/// The engine thread shuts down automatically when all handles are dropped
/// (the command channel disconnects).
pub fn spawn(
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    library: Arc<NodeLibrary>,
    format: wgpu::TextureFormat,
) -> EngineOutpostHandle {
    let (command_rx, command_tx) = message_channel::new();
    let broadcaster = Arc::new(EventBroadcaster::new());

    let broadcaster_inner = broadcaster.clone();
    thread::Builder::new()
        .name("engine-outpost".into())
        .spawn(move || {
            EngineOutpostInner::new(device, queue, library, broadcaster_inner, format)
                .run(command_rx);
        })
        .expect("failed to spawn engine-outpost thread");

    EngineOutpostHandle {
        command_tx: Arc::new(command_tx),
        broadcaster,
    }
}

struct EngineOutpostInner {
    graph_executor: GraphExecutor,
    graph: NodeGraph,
    library: Arc<NodeLibrary>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    broadcaster: Arc<EventBroadcaster>,
    timer: SwitchTimer,
    paused: bool,
    output_node_id: Option<crate::node_graph::EngineNodeId>,
    /// When true, `try_apply_output_node_fps` is skipped and the timer runs at
    /// the manually-set rate from `SetGlobalStreamTargetFps`.
    manual_fps_locked: bool,
}

impl EngineOutpostInner {
    fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        library: Arc<NodeLibrary>,
        broadcaster: Arc<EventBroadcaster>,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            graph_executor: GraphExecutor::new(format),
            graph: NodeGraph::default(),
            library,
            device,
            queue,
            broadcaster,
            timer: SwitchTimer::new(FPS_60),
            paused: false,
            output_node_id: None,
            manual_fps_locked: false,
        }
    }

    fn run(mut self, command_rx: Inbox<EngineCommand>) {
        loop {
            let timeout = if self.paused {
                PAUSED_POLL_INTERVAL
            } else {
                self.timer.time_until_next_switch()
            };

            match command_rx.wait_timeout(timeout) {
                Ok(command) => {
                    self.handle_command(command);
                    while let Ok(Some(command)) = command_rx.check_non_blocking() {
                        self.handle_command(command);
                    }
                }
                Err(err) if err.is_wait_timeout_error() => {}
                Err(_) => return,
            }

            if !self.paused && self.timer.is_switch_time() {
                self.tick();
            }
        }
    }

    fn handle_command(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::PauseStreams => {
                self.graph_executor.pause_streams();
                self.paused = true;
                let _ = self
                    .broadcaster
                    .broadcast(EngineOutpostEvent::StreamsPaused);
            }
            EngineCommand::PlayStreams => {
                self.graph_executor.play_streams();
                self.paused = false;
                self.timer.reset();
                let _ = self
                    .broadcaster
                    .broadcast(EngineOutpostEvent::StreamsPlaying);
            }
            EngineCommand::SetGlobalStreamTargetFps(fps) => {
                self.manual_fps_locked = true;
                self.graph_executor.set_global_stream_target_fps(fps);
                self.timer.set_target_fps(fps);
                let _ = self
                    .broadcaster
                    .broadcast(EngineOutpostEvent::GlobalStreamTargetFpsChanged(fps));
            }
            EngineCommand::ClearManualFps => {
                self.manual_fps_locked = false;
                if let Some(node_id) = self.output_node_id {
                    self.try_apply_output_node_fps(node_id);
                }
            }
            EngineCommand::SetOutputNode(node_id) => {
                self.output_node_id = node_id;
                // Always tell the app what FPS the engine is running at when an
                // output is set, even if the rate hasn't changed from the default.
                if node_id.is_some() {
                    let _ = self.broadcaster.broadcast(
                        EngineOutpostEvent::GlobalStreamTargetFpsChanged(self.timer.target_fps()),
                    );
                }
            }
            EngineCommand::RequestInfo(req) => match req {
                message::InfoRequest::RecommendedFpsForNode(node_id) => {
                    if let Some(fps) = self.try_apply_output_node_fps(node_id) {
                        let _ = self.broadcaster.broadcast(EngineOutpostEvent::InfoResponse(
                            message::InfoResponse::RecommendedFpsForNode(node_id, fps),
                        ));
                    }
                }
            },
            EngineCommand::UpdateGraph(new_graph) => {
                self.graph_executor.invalidate_execution_order();
                self.graph = new_graph;
            }
        }
    }

    fn try_apply_output_node_fps(
        &mut self,
        node_id: crate::node_graph::EngineNodeId,
    ) -> Option<Fps> {
        let fps =
            self.graph_executor
                .get_target_fps_for_node(&self.graph, &self.library, node_id)?;

        if self.timer.target_fps() != fps {
            self.timer.set_target_fps(fps);
            let _ = self
                .broadcaster
                .broadcast(EngineOutpostEvent::GlobalStreamTargetFpsChanged(fps));
        }

        Some(fps)
    }

    fn tick(&mut self) {
        let result = self.graph_executor.execute(
            &self.graph,
            &self.library,
            &self.device,
            &self.queue,
            self.output_node_id,
            |event| self.broadcaster.broadcast(event),
        );

        let frame = match result {
            Ok(execution_result) => {
                execution_result
                    .outputs
                    .values()
                    .find_map(|value| match value {
                        NodeValue::Frame(frame) => Some(frame.clone()),
                        _ => None,
                    })
            }
            Err(ExecutionError::NoOutputNode) | Err(ExecutionError::NoOutputProduced) => None,
            Err(err) => {
                self.broadcaster
                    .broadcast(EngineOutpostEvent::ExecutionError(err.to_string()));
                None
            }
        };

        if !self.manual_fps_locked {
            if let Some(node_id) = self.output_node_id {
                let _ = self.try_apply_output_node_fps(node_id);
            }
        }

        if let Some(frame) = frame {
            self.broadcaster
                .broadcast(EngineOutpostEvent::FrameReady(frame));
        }
    }
}

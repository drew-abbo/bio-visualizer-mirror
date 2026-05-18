//! Shared engine outpost message types.

use crate::gpu_frame::GpuFrame;
use crate::node_graph::{EngineNodeId, NodeGraph};
use media::fps::Fps;

/// Commands that can be sent into the engine outpost.
#[derive(Debug, Clone)]
pub enum EngineCommand {
    PauseStreams,
    PlayStreams,
    /// Override the engine tick rate and all stream FPS with a fixed value.
    /// The engine will stop auto-adjusting FPS from the output node until
    /// `ClearManualFps` is sent.
    SetGlobalStreamTargetFps(Fps),
    /// Release the manual FPS override and resume auto-adjusting from the
    /// output node's recommended rate.
    ClearManualFps,
    /// Tell the engine which node should be treated as the active output.
    SetOutputNode(Option<EngineNodeId>),
    /// Push an updated graph to the engine. The engine will execute
    /// immediately on the next loop iteration rather than waiting for
    /// the next scheduled tick.
    UpdateGraph(NodeGraph),
    /// Request information from the engine outpost. The engine should
    /// respond by emitting an `EngineOutpostEvent::InfoResponse`.
    RequestInfo(InfoRequest),
}

/// Events emitted by the engine outpost and observed by the app.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineOutpostEvent {
    StreamsPaused,
    StreamsPlaying,
    GlobalStreamTargetFpsChanged(Fps),
    /// A stream is being created; the UI should show a loading indicator.
    StreamLoading(EngineNodeId),
    /// A GPU-backed frame is ready for display.
    FrameReady(GpuFrame),
    /// The engine encountered an error during graph execution.
    ExecutionError(String),
    /// Response to an information request made via `EngineCommand::RequestInfo`.
    InfoResponse(InfoResponse),
}

/// Dynamic information request types the app can ask the engine for.
#[derive(Debug, Clone)]
pub enum InfoRequest {
    /// Ask for a recommended FPS for the given node id (typically a video source).
    RecommendedFpsForNode(EngineNodeId),
}

/// Responses the engine can emit for InfoRequest messages.
#[derive(Debug, Clone, PartialEq)]
pub enum InfoResponse {
    /// Recommended FPS for a node (node id, fps)
    RecommendedFpsForNode(EngineNodeId, Fps),
    /// Generic error
    Error(String),
}

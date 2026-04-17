use std::path::PathBuf;

use thiserror::Error;

use crate::node::engine_node::{AlgorithmStageBackend, NodeOutputKind};
use crate::node_graph::EngineNodeId;

/// Errors that can occur during graph execution
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Graph error: {0}")]
    GraphError(#[from] crate::node_graph::GraphError),

    #[error("Node {0} not found")]
    NodeNotFound(EngineNodeId),

    #[error("Target node {0} not found in graph")]
    TargetNodeNotFound(EngineNodeId),

    #[error("Target node {0} is not in execution order")]
    TargetNodeNotInExecutionOrder(EngineNodeId),

    #[error("Node definition '{0}' not found")]
    DefinitionNotFound(String),

    #[error("Node {0} has not been executed yet")]
    NodeNotExecuted(EngineNodeId),

    #[error("Output '{1}' not found on node {0}")]
    OutputNotFound(EngineNodeId, String),

    #[error("No output node in graph")]
    NoOutputNode,

    #[error("No output produced")]
    NoOutputProduced,

    #[error("Frame input '{1}' on node {0} is not connected")]
    UnconnectedFrameInput(EngineNodeId, String),

    #[error("Node '{0}' has no frame input")]
    NoFrameInput(String),

    #[error("Failed to load shader from {0:?}: {1}")]
    ShaderLoadError(PathBuf, String),

    #[error("Noise execution error: {0}")]
    NoiseExecutionError(String),

    #[error("MIDI stream error: {0}")]
    MidiStreamError(String),

    #[error("Signal envelope error: {0}")]
    SignalEnvelopeError(String),

    #[error("Render error: {0:?}")]
    RenderError(crate::engine_errors::EngineError),

    #[error("Invalid input type")]
    InvalidInputType,

    #[error("Unsupported output type: {0:?}")]
    UnsupportedOutputType(NodeOutputKind),

    #[error("Shader and algorithm nodes cannot currently mix Frame outputs with scalar outputs")]
    UnsupportedNodeOutputCombination,

    #[error("GPU readback error: {0}")]
    GpuReadbackError(String),

    #[error("GPU readback is not ready yet")]
    GpuReadbackNotReady,

    #[error("Unsupported algorithm backend {backend:?} in stage '{stage}'")]
    UnsupportedAlgorithmBackend {
        stage: String,
        backend: AlgorithmStageBackend,
    },

    #[error("Failed to create pipeline: {0}")]
    PipelineCreationError(String),

    #[error("Failed to create producer for {0:?}: {1}")]
    ProducerCreateError(PathBuf, String),

    #[error("Failed to fetch video frame from {0:?}: {1}")]
    VideoFetchError(PathBuf, String),

    #[error("Video stream is not ready for {0:?}")]
    VideoStreamNotReady(PathBuf),

    #[error("Video stream error for {0:?}: {1}")]
    VideoStreamError(PathBuf, String),

    #[error("Texture upload error: {0}")]
    TextureUploadError(String),
}

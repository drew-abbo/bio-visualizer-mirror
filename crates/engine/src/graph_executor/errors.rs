use crate::node::node::NodeOutputKind;
use crate::node_graph::NodeId;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during graph execution
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Graph error: {0}")]
    GraphError(#[from] crate::node_graph::GraphError),

    #[error("Node {0} not found")]
    NodeNotFound(NodeId),

    #[error("Node definition '{0}' not found")]
    DefinitionNotFound(String),

    #[error("Node {0} has not been executed yet")]
    NodeNotExecuted(NodeId),

    #[error("Output '{1}' not found on node {0}")]
    OutputNotFound(NodeId, String),

    #[error("No output node in graph")]
    NoOutputNode,

    #[error("No output produced")]
    NoOutputProduced,

    #[error("Frame input '{1}' on node {0} is not connected")]
    UnconnectedFrameInput(NodeId, String),

    #[error("Node '{0}' has no frame input")]
    NoFrameInput(String),

    #[error("Failed to load shader from {0:?}: {1}")]
    ShaderLoadError(PathBuf, String),

    #[error("Render error: {0:?}")]
    RenderError(crate::engine_errors::EngineError),

    #[error("Invalid input type")]
    InvalidInputType,

    #[error("Unsupported output type: {0:?}")]
    UnsupportedOutputType(NodeOutputKind),

    #[error("Failed to create pipeline: {0}")]
    PipelineCreationError(String),

    #[error("Failed to create producer for {0:?}: {1}")]
    ProducerCreateError(PathBuf, String),

    #[error("Failed to fetch video frame from {0:?}: {1}")]
    VideoFetchError(PathBuf, String),

    #[error("Video stream error for {0:?}: {1}")]
    VideoStreamError(PathBuf, String),

    #[error("Texture upload error: {0}")]
    TextureUploadError(String),
}

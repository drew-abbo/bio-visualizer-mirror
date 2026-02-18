use std::path::PathBuf;

use thiserror::Error;

<<<<<<< HEAD
<<<<<<< HEAD
use crate::node::engine_node::NodeOutputKind;
=======
use crate::node::node::NodeOutputKind;
>>>>>>> e361ed9 (re doing some things and make the values in the engine be used for input and output)
=======
use crate::node::engine_node::NodeOutputKind;
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
use crate::node_graph::EngineNodeId;

/// Errors that can occur during graph execution
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Graph error: {0}")]
    GraphError(#[from] crate::node_graph::GraphError),

    #[error("Node {0} not found")]
    NodeNotFound(EngineNodeId),
<<<<<<< HEAD

    #[error("Target node {0} not found in graph")]
    TargetNodeNotFound(EngineNodeId),

    #[error("Target node {0} is not in execution order")]
    TargetNodeNotInExecutionOrder(EngineNodeId),
=======
>>>>>>> e361ed9 (re doing some things and make the values in the engine be used for input and output)

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

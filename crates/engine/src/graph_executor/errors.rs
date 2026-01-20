use crate::node::node::NodeOutputKind;
use crate::node_graph::NodeId;
use std::path::PathBuf;

impl std::error::Error for ExecutionError {}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionError::GraphError(e) => write!(f, "Graph error: {}", e),
            ExecutionError::NodeNotFound(id) => write!(f, "Node {} not found", id),
            ExecutionError::DefinitionNotFound(name) => {
                write!(f, "Node definition '{}' not found", name)
            }
            ExecutionError::NodeNotExecuted(id) => {
                write!(f, "Node {} has not been executed yet", id)
            }
            ExecutionError::OutputNotFound(id, name) => {
                write!(f, "Output '{}' not found on node {}", name, id)
            }
            ExecutionError::NoOutputNode => write!(f, "No output node in graph"),
            ExecutionError::NoOutputProduced => write!(f, "No output produced"),
            ExecutionError::UnconnectedFrameInput(id, name) => {
                write!(f, "Frame input '{}' on node {} is not connected", name, id)
            }
            ExecutionError::NoFrameInput(name) => {
                write!(f, "Node '{}' has no frame input", name)
            }
            ExecutionError::ShaderLoadError(path, err) => {
                write!(f, "Failed to load shader from {:?}: {}", path, err)
            }
            ExecutionError::RenderError(e) => write!(f, "Render error: {:?}", e),
            ExecutionError::InvalidInputType => write!(f, "Invalid input type"),
            ExecutionError::UnsupportedOutputType(kind) => {
                write!(f, "Unsupported output type: {:?}", kind)
            }
            ExecutionError::PipelineCreationError(e) => {
                write!(f, "Failed to create pipeline: {}", e)
            }
            ExecutionError::ProducerCreateError(path, err) => {
                write!(f, "Failed to create producer for {:?}: {}", path, err)
            }
            ExecutionError::VideoFetchError(path, err) => {
                write!(f, "Failed to fetch video frame from {:?}: {}", path, err)
            }
            ExecutionError::VideoStreamError(path, err) => {
                write!(f, "Video stream error for {:?}: {}", path, err)
            }
            ExecutionError::TextureUploadError(err) => {
                write!(f, "Texture upload error: {}", err)
            }
        }
    }
}

/// Errors that can occur during graph execution
#[derive(Debug)]
pub enum ExecutionError {
    GraphError(crate::node_graph::GraphError),
    NodeNotFound(NodeId),
    DefinitionNotFound(String),
    NodeNotExecuted(NodeId),
    OutputNotFound(NodeId, String),
    NoOutputNode,
    NoOutputProduced,
    UnconnectedFrameInput(NodeId, String),
    NoFrameInput(String),
    ShaderLoadError(std::path::PathBuf, String),
    RenderError(crate::engine_errors::EngineError),
    InvalidInputType,
    UnsupportedOutputType(NodeOutputKind),
    PipelineCreationError(String),
    ProducerCreateError(PathBuf, String),
    VideoFetchError(PathBuf, String),
    VideoStreamError(PathBuf, String),
    TextureUploadError(String),
}

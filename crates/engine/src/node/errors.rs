use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LibraryError {
    #[error("Nodes folder not found: {0:?}")]
    NodesFolderNotFound(PathBuf),

    #[error("IO error reading {0:?}: {1}")]
    IoError(PathBuf, std::io::Error),

    #[error("Failed to parse {0:?}: {1}")]
    ParseError(PathBuf, String),

    #[error("Shader file not found: {0:?}")]
    ShaderNotFound(PathBuf),

    #[error("Node '{0}' is not a shader node")]
    NotAShaderNode(String),
}

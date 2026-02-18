<<<<<<< HEAD
<<<<<<< HEAD
use super::engine_node::NumberInputUiMode;
=======
use super::node::NumberInputUiMode;
>>>>>>> cc1a573 (I think this is very close to being ready)
=======
use super::engine_node::NumberInputUiMode;
>>>>>>> 95b0833 (renamed the node to engine node and added a new function to the node_library)
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

    #[error("Node '{0}' has an invalid 'input_ui': {1:?}")]
    InvalidNumberInputUiMode(String, NumberInputUiMode),
}

use std::path::PathBuf;

#[derive(Debug)]
pub enum LibraryError {
    NodesFolderNotFound(PathBuf),
    IoError(PathBuf, std::io::Error),
    ParseError(PathBuf, String),
    ShaderNotFound(PathBuf),
    NotAShaderNode(String),
}

impl std::fmt::Display for LibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibraryError::NodesFolderNotFound(path) => {
                write!(f, "Nodes folder not found: {:?}", path)
            }
            LibraryError::IoError(path, err) => {
                write!(f, "IO error reading {:?}: {}", path, err)
            }
            LibraryError::ParseError(path, err) => {
                write!(f, "Failed to parse {:?}: {}", path, err)
            }
            LibraryError::ShaderNotFound(path) => {
                write!(f, "Shader file not found: {:?}", path)
            }
            LibraryError::NotAShaderNode(name) => {
                write!(f, "Node '{}' is not a shader node", name)
            }
        }
    }
}

impl std::error::Error for LibraryError {}

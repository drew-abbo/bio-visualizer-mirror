use crate::node::Node;
use crate::node::errors::LibraryError;
use std::path::PathBuf;

/// A loaded node definition with resolved paths
#[derive(Debug, Clone)]
pub struct NodeDefinition {
    /// The node metadata from node.json
    pub node: Node,

    /// Absolute path to the shader file (if this is a shader node)
    pub shader_path: Option<PathBuf>,

    /// Absolute path to the node's folder
    pub folder_path: PathBuf,
}

impl NodeDefinition {
    /// Load the shader code for this node
    pub fn load_shader_code(&self) -> Result<String, LibraryError> {
        let shader_path = self
            .shader_path
            .as_ref()
            .ok_or_else(|| LibraryError::NotAShaderNode(self.node.name.clone()))?;

        std::fs::read_to_string(shader_path)
            .map_err(|e| LibraryError::IoError(shader_path.clone(), e))
    }
}

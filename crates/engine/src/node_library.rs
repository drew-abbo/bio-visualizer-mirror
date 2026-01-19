use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod errors;
pub mod node;
pub mod node_definition;

use errors::LibraryError;
use node::{Node, NodeExecutionPlan};
use node_definition::NodeDefinition;

/// The node library - holds all available node definitions loaded from disk
#[derive(Debug)]
pub struct NodeLibrary {
    /// All loaded node definitions, keyed by node name
    definitions: HashMap<String, NodeDefinition>,

    /// Root path where nodes are stored
    nodes_folder: PathBuf,
}

impl NodeLibrary {
    pub fn get_definition(&self, name: &str) -> Option<&NodeDefinition> {
        self.definitions.get(name)
    }

    /// Load all node definitions from the Nodes/ folder
    pub fn load_from_disk(nodes_folder: impl AsRef<Path>) -> Result<Self, LibraryError> {
        let nodes_folder = nodes_folder.as_ref().to_path_buf();

        if !nodes_folder.exists() {
            return Err(LibraryError::NodesFolderNotFound(nodes_folder));
        }

        let mut definitions = HashMap::new();

        // Recursively scan for node.json files
        Self::scan_directory(&nodes_folder, &nodes_folder, &mut definitions)?;

        println!(
            "Loaded {} node definitions from {:?}",
            definitions.len(),
            nodes_folder
        );

        Ok(Self {
            definitions,
            nodes_folder,
        })
    }

    /// Recursively scan a directory for node folders
    fn scan_directory(
        base_path: &Path,
        current_path: &Path,
        definitions: &mut HashMap<String, NodeDefinition>,
    ) -> Result<(), LibraryError> {
        let entries = std::fs::read_dir(current_path)
            .map_err(|e| LibraryError::IoError(current_path.to_path_buf(), e))?;

        for entry in entries {
            let entry = entry.map_err(|e| LibraryError::IoError(current_path.to_path_buf(), e))?;
            let path = entry.path();

            if path.is_dir() {
                // Check if this directory contains a node.json
                let node_json = path.join("node.json");

                if node_json.exists() {
                    // This is a node folder!
                    match Self::load_node_definition(&path, base_path) {
                        Ok(def) => {
                            println!("Found node: {}", def.node.name);
                            if definitions.contains_key(&def.node.name) {
                                eprintln!(
                                    "Warning: Duplicate node name '{}', skipping",
                                    def.node.name
                                );
                            } else {
                                definitions.insert(def.node.name.clone(), def);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error loading node from {:?}: {}", path, e);
                        }
                    }
                } else {
                    // Not a node folder, recurse into it
                    Self::scan_directory(base_path, &path, definitions)?;
                }
            }
        }

        Ok(())
    }

    /// Load a single node definition from a node folder
    fn load_node_definition(
        node_folder: &Path,
        base_path: &Path,
    ) -> Result<NodeDefinition, LibraryError> {
        let node_json = node_folder.join("node.json");

        // Read and parse node.json
        let json_content = std::fs::read_to_string(&node_json)
            .map_err(|e| LibraryError::IoError(node_json.clone(), e))?;

        let mut node: Node = serde_json::from_str(&json_content)
            .map_err(|e| LibraryError::ParseError(node_json.clone(), e.to_string()))?;

        // Resolve shader file path if this is a shader node
        let shader_path = if let NodeExecutionPlan::Shader { source } = &node.executor {
            let absolute_path = node_folder.join(source);

            // Verify shader file exists
            if !absolute_path.exists() {
                return Err(LibraryError::ShaderNotFound(absolute_path));
            }

            Some(absolute_path)
        } else {
            None
        };

        Ok(NodeDefinition {
            node,
            shader_path,
            folder_path: node_folder.to_path_buf(),
        })
    }

    /// Get a node definition by name
    pub fn get(&self, name: &str) -> Option<&NodeDefinition> {
        self.definitions.get(name)
    }

    /// Get all node definitions
    pub fn definitions(&self) -> &HashMap<String, NodeDefinition> {
        &self.definitions
    }

    /// Get all node names
    pub fn node_names(&self) -> Vec<String> {
        self.definitions.keys().cloned().collect()
    }

    /// Filter nodes by subfolder (for UI organization)
    pub fn nodes_in_subfolder(&self, subfolder: &str) -> Vec<&NodeDefinition> {
        self.definitions
            .values()
            .filter(|def| def.node.sub_folders.contains(&subfolder.to_string()))
            .collect()
    }

    /// Search nodes by keyword
    pub fn search(&self, query: &str) -> Vec<&NodeDefinition> {
        let query_lower = query.to_lowercase();

        self.definitions
            .values()
            .filter(|def| {
                // Search in name
                def.node.name.to_lowercase().contains(&query_lower)
                // Search in keywords
                || def.node.search_keywords.iter().any(|kw| kw.to_lowercase().contains(&query_lower))
                // Search in description
                || def.node.short_description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }
}
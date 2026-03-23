use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::{Path, PathBuf};

use serde_json;

use super::engine_node::{EngineNode, NodeExecutionPlan};
use super::errors::LibraryError;
use super::node_definition::NodeDefinition;

/// The node library - holds all available node definitions loaded from disk
#[derive(Debug)]
pub struct NodeLibrary {
    /// All loaded node definitions, keyed by node name
    definitions: HashMap<String, NodeDefinition>,

    /// Root path where nodes are stored
    _nodes_folder: PathBuf,
}

impl Default for NodeLibrary {
    fn default() -> Self {
        Self {
            definitions: HashMap::new(),
            _nodes_folder: PathBuf::new(),
        }
    }
}

/// Represents a subcategory within a category
#[derive(Debug, Clone)]
pub struct SubcategoryInfo {
    pub name: String,
    pub nodes: Vec<String>,
}

/// Represents organized information about a category
/// This provides a hierarchical view for UI folder displays
#[derive(Debug, Clone)]
pub struct CategoryInfo {
    pub name: String,

    /// Optional subcategories within this category
    /// Each subcategory contains its own list of nodes
    /// Empty if no nodes in this category use subcategories
    pub subcategories: Vec<SubcategoryInfo>,

    /// Nodes that belong directly to this category
    /// not assigned to any subcategory
    pub direct_nodes: Vec<String>,
}

impl NodeLibrary {
    /// Get a node definition by name
    pub fn get_definition(&self, name: &str) -> Option<&NodeDefinition> {
        self.definitions.get(name)
    }
    /// Get comprehensive category information for the entire library
    /// This is useful for UI components that need to build category menus/folders
    ///
    /// Returns a hierarchical structure:
    /// - Categories contain subcategories (if any nodes use them)
    /// - Categories contain direct_nodes (nodes not in any subcategory)
    ///
    /// Example for UI:
    /// ```
    /// Color/
    ///   ├─ Brightness
    ///   └─ Invert
    /// Distortion/
    ///   ├─ Glitch/          (subcategory, if used)
    ///   │  └─ RGB Delay
    ///   └─ Chromatic Aberration  (direct node)
    /// ```
    pub fn get_all_category_info(&self) -> Vec<CategoryInfo> {
        let categories = self.get_all_categories();

        categories
            .into_iter()
            .map(|category| {
                // Get all subcategories used in this category
                let subcategory_names = self.get_category_subcategories(&category);

                // Build SubcategoryInfo for each subcategory
                let subcategories: Vec<SubcategoryInfo> = subcategory_names
                    .into_iter()
                    .map(|subcat_name| {
                        let nodes: Vec<String> = self
                            .get_subcategory_nodes(&category, &subcat_name)
                            .into_iter()
                            .map(|def| def.node.name.clone())
                            .collect();

                        SubcategoryInfo {
                            name: subcat_name,
                            nodes,
                        }
                    })
                    .collect();

                // Get nodes that don't belong to any subcategory
                let direct_nodes: Vec<String> = self
                    .definitions
                    .values()
                    .filter(|def| def.node.category == category)
                    .filter(|def| def.node.subcategories.is_empty())
                    .map(|def| def.node.name.clone())
                    .collect();

                CategoryInfo {
                    name: category,
                    subcategories,
                    direct_nodes,
                }
            })
            .collect()
    }

    /// Load all nodes from the prebuilt nodes folder and the users nodes folder.
    /// Check to make sure that there are no nodes being loaded from the users folder with the same name as prebuilt nodes.
    pub fn load_all() -> Result<Self, LibraryError> {
        let mut library = Self::load_from_disk()?;

        // Load user nodes and check for duplicates
        let user_library = Self::load_from_users_folder()?;

        for (name, def) in user_library.definitions {
            if let Entry::Vacant(e) = library.definitions.entry(name.clone()) {
                e.insert(def);
            } else {
                util::debug_log_warning!(
                    "Warning: User node '{}' has the same name as a prebuilt node. Skipping user node.",
                    name
                );
            }
        }

        Ok(library)
    }

    fn get_all_categories(&self) -> Vec<String> {
        let mut categories: Vec<String> = self
            .definitions
            .values()
            .map(|def| def.node.category.clone())
            .filter(|cat| !cat.is_empty())
            .collect();

        categories.sort();
        categories.dedup();
        categories
    }

    fn get_subcategory_nodes(&self, category: &str, subcategory: &str) -> Vec<&NodeDefinition> {
        self.definitions
            .values()
            .filter(|def| def.node.category == category)
            .filter(|def| def.node.subcategories.contains(&subcategory.to_string()))
            .collect()
    }

    fn get_category_subcategories(&self, category: &str) -> Vec<String> {
        let mut subcategories: Vec<String> = self
            .definitions
            .values()
            .filter(|def| def.node.category == category)
            .flat_map(|def| def.node.subcategories.iter().cloned())
            .collect();

        subcategories.sort();
        subcategories.dedup();
        subcategories
    }

    /// Load all node definitions from the nodes/ folder
    fn load_from_disk() -> Result<Self, LibraryError> {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
        let nodes_path = workspace_root.join("nodes");
        let nodes_folder = nodes_path.to_path_buf();

        if !nodes_folder.exists() {
            return Err(LibraryError::NodesFolderNotFound(nodes_folder));
        }

        let mut definitions = HashMap::new();

        // Recursively scan for node.json files
        Self::scan_directory(&nodes_folder, &nodes_folder, &mut definitions)?;

        if cfg!(debug_assertions) {
            util::debug_log_info!(
                "Loaded {} node definitions from {:?}",
                definitions.len(),
                nodes_folder
            );
        }

        Ok(Self {
            definitions,
            _nodes_folder: nodes_folder,
        })
    }

    fn load_from_users_folder() -> Result<Self, LibraryError> {
        use util::local_data;

        let nodes_folder = PathBuf::from(local_data::nodes_path());

        let mut definitions = HashMap::new();
        Self::scan_directory(&nodes_folder, &nodes_folder, &mut definitions)?;

        if cfg!(debug_assertions) {
            util::debug_log_info!(
                "Loaded {} node definitions from user data: {:?}",
                definitions.len(),
                nodes_folder
            );
        }

        Ok(Self {
            definitions,
            _nodes_folder: nodes_folder,
        })
    }

    /// Recursively scan a directory for node folders
    fn scan_directory(
        _base_path: &Path,
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
                    match Self::load_node_definition(&path) {
                        Ok(def) => {
                            util::debug_println!("Found node: {}", def.node.name);

                            if definitions.contains_key(&def.node.name) {
                                util::debug_log_warning!(
                                    "Warning: Duplicate node name '{}', skipping",
                                    def.node.name
                                );
                            } else {
                                definitions.insert(def.node.name.clone(), def);
                            }
                        }
                        Err(e) => {
                            util::debug_log_error!("Error loading node from {:?}: {}", path, e);
                        }
                    }
                } else {
                    // Not a node folder, recurse into it
                    Self::scan_directory(_base_path, &path, definitions)?;
                }
            }
        }

        Ok(())
    }

    /// Load a single node definition from a node folder
    fn load_node_definition(node_folder: &Path) -> Result<NodeDefinition, LibraryError> {
        let node_json = node_folder.join("node.json");

        // Read and parse node.json
        let json_content = std::fs::read_to_string(&node_json)
            .map_err(|e| LibraryError::IoError(node_json.clone(), e))?;

        let node: EngineNode = serde_json::from_str(&json_content)
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

    /// Get all node definitions
    pub fn definitions(&self) -> &HashMap<String, NodeDefinition> {
        &self.definitions
    }

    /// Get all node names
    pub fn node_names(&self) -> Vec<String> {
        self.definitions.keys().cloned().collect()
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

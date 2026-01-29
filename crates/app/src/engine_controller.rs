use engine::graph_executor::GraphExecutor;
use engine::node::NodeLibrary;
use engine::node_graph::NodeGraph;
use std::path::PathBuf;

pub struct EngineController {
    graph_executor: GraphExecutor,
    node_library: NodeLibrary,
    node_graph: NodeGraph,
}

impl EngineController {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let node_library = if cfg!(debug_assertions) {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
            let nodes_path = workspace_root.join("Nodes");
            NodeLibrary::load_from_disk(nodes_path)?
        } else {
            NodeLibrary::load_from_users_folder()?
        };

        Ok(Self {
            graph_executor: GraphExecutor::default(),
            node_library,
            node_graph: NodeGraph::default(),
        })
    }

    pub fn graph_executor_mut(&mut self) -> &mut GraphExecutor {
        &mut self.graph_executor
    }

    pub fn node_library(&mut self) -> &NodeLibrary {
        &self.node_library
    }

    pub fn node_graph_mut(&mut self) -> &mut NodeGraph {
        &mut self.node_graph
    }
}

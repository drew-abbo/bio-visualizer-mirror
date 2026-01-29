use engine::graph_executor::GraphExecutor;
use engine::node::NodeLibrary;
use engine::node_graph::{NodeGraph, NodeId};
use std::path::PathBuf;
use eframe::wgpu;

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

    #[allow(dead_code)]
    pub fn graph_executor_mut(&mut self) -> &mut GraphExecutor {
        &mut self.graph_executor
    }

    pub fn node_library(&self) -> &NodeLibrary {
        &self.node_library
    }

    #[allow(dead_code)]
    pub fn node_graph(&self) -> &NodeGraph {
        &self.node_graph
    }

    pub fn node_graph_mut(&mut self) -> &mut NodeGraph {
        &mut self.node_graph
    }

    /// Get outputs for a specific node from the cache
    /// Returns None if the node hasn't been executed yet
    #[allow(dead_code)]
    pub fn get_node_outputs(&self, node_id: NodeId) -> Option<&std::collections::HashMap<String, engine::graph_executor::OutputValue>> {
        self.graph_executor.get_node_outputs(node_id)
    }

    /// Get the output node ID (the final output node of the graph)
    pub fn get_output_node_id(&self) -> NodeId {
        self.graph_executor.get_output_node_id()
    }

    /// Get the video FPS from the output node if available
    pub fn get_video_fps(&self) -> Option<f32> {
        let output_node_id = self.get_output_node_id();
        self.get_node_outputs(output_node_id)
            .and_then(|outputs| outputs.get("fps"))
            .and_then(|v| {
                if let engine::graph_executor::OutputValue::Float(val) = v {
                    Some(*val)
                } else {
                    None
                }
            })
    }

    /// Execute the graph with the given wgpu device and queue
    pub fn execute_graph(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.graph_executor.execute(&self.node_graph, &self.node_library, device, queue)?;
        Ok(())
    }
}

use engine::graph_executor::{ExecutionContext, GraphExecutor, NodeValue};
use engine::node::NodeLibrary;
use engine::node_graph::{EngineNodeId, NodeGraph};

pub struct GraphExecutorManager {
    engine_graph: NodeGraph,
    graph_executor: GraphExecutor,
    last_selected_engine_node: Option<EngineNodeId>,
}

impl GraphExecutorManager {
    pub fn new() -> Self {
        Self {
            engine_graph: NodeGraph::default(),
            graph_executor: GraphExecutor::default(),
            last_selected_engine_node: None,
        }
    }

    pub fn engine_graph(&self) -> &NodeGraph {
        &self.engine_graph
    }

    pub fn engine_graph_mut(&mut self) -> &mut NodeGraph {
        &mut self.engine_graph
    }

    pub fn set_last_selected_engine_node(&mut self, node: Option<EngineNodeId>) {
        self.last_selected_engine_node = node;
    }

    pub fn selection_changed(&self, new_selection: Option<EngineNodeId>) -> bool {
        new_selection != self.last_selected_engine_node
    }

    pub fn execute(
        &mut self,
        node_library: &NodeLibrary,
        render_state: &egui_wgpu::RenderState,
        selected_engine_node: Option<EngineNodeId>,
        context: ExecutionContext,
    ) -> Option<std::collections::HashMap<String, NodeValue>> {
        match self.graph_executor.execute(
            &self.engine_graph,
            node_library,
            &render_state.device,
            &render_state.queue,
            selected_engine_node,
            context,
        ) {
            Ok(result) => Some(result.outputs.clone()),
            Err(err) => {
                util::debug_log_error!("Graph execution error: {}", err);
                None
            }
        }
    }

    pub fn get_output_node_id(&self) -> EngineNodeId {
        self.graph_executor.get_output_node_id()
    }

    /// Find the best node to display output from when none is selected.
    /// Looks for nodes with no outgoing connections (sink nodes).
    /// Returns the last one found, or the default output node if none exist.
    pub fn find_display_node(&self) -> EngineNodeId {
        self.engine_graph
            .find_output_nodes()
            .last()
            .copied()
            .unwrap_or_else(|| self.get_output_node_id())
    }
}

impl Default for GraphExecutorManager {
    fn default() -> Self {
        Self::new()
    }
}

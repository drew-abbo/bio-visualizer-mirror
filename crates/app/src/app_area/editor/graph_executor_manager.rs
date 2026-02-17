<<<<<<< HEAD
//! Manager for the node graph and its execution, separate from the UI state in EditorArea
//! This module defines the GraphExecutorManager, which holds the engine graph and the GraphExecutor instance.
//! It provides methods to check for changes, execute the graph, and determine which node's output to display based on selection or graph structure.
=======
>>>>>>> a665ac9 (commit now so I don't screw something up)
use engine::graph_executor::{ExecutionContext, GraphExecutor, NodeValue};
use engine::node::NodeLibrary;
use engine::node_graph::{EngineNodeId, NodeGraph};

<<<<<<< HEAD
<<<<<<< HEAD
/// Manager for the node graph and its execution, separate from the UI state in EditorArea
=======
>>>>>>> a665ac9 (commit now so I don't screw something up)
=======
/// Manager for the node graph and its execution, separate from the UI state in EditorArea
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
pub struct GraphExecutorManager {
    engine_graph: NodeGraph,
    graph_executor: GraphExecutor,
    last_selected_engine_node: Option<EngineNodeId>,
<<<<<<< HEAD
    graph_changed: bool,
=======
>>>>>>> a665ac9 (commit now so I don't screw something up)
}

impl GraphExecutorManager {
    pub fn new() -> Self {
        Self {
            engine_graph: NodeGraph::default(),
            graph_executor: GraphExecutor::default(),
            last_selected_engine_node: None,
<<<<<<< HEAD
            graph_changed: false,
=======
>>>>>>> a665ac9 (commit now so I don't screw something up)
        }
    }

    pub fn engine_graph(&self) -> &NodeGraph {
        &self.engine_graph
    }

<<<<<<< HEAD
    /// Get mutable access to the engine graph without automatically marking as changed
    /// Use this when you want to check or sync the graph but track changes manually
    pub fn engine_graph_mut_no_flag(&mut self) -> &mut NodeGraph {
        &mut self.engine_graph
    }

    /// Manually mark the graph as changed (typically after sync_to_engine returns true)
    pub fn mark_graph_changed(&mut self) {
        self.graph_changed = true;
    }

    /// Check if the graph has changed since last execution and clear the flag
    pub fn consume_graph_changed(&mut self) -> bool {
        let changed = self.graph_changed;
        self.graph_changed = false;
        changed
    }

=======
    pub fn engine_graph_mut(&mut self) -> &mut NodeGraph {
        &mut self.engine_graph
    }

>>>>>>> a665ac9 (commit now so I don't screw something up)
    pub fn set_last_selected_engine_node(&mut self, node: Option<EngineNodeId>) {
        self.last_selected_engine_node = node;
    }

<<<<<<< HEAD
<<<<<<< HEAD
    /// Check if the selection has changed since the last execution
    /// Used to determine if we need to re-execute the graph when the user selects a different node
=======
>>>>>>> a665ac9 (commit now so I don't screw something up)
=======
    /// Check if the selection has changed since the last execution
    /// Used to determine if we need to re-execute the graph when the user selects a different node
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
    pub fn selection_changed(&self, new_selection: Option<EngineNodeId>) -> bool {
        new_selection != self.last_selected_engine_node
    }

<<<<<<< HEAD
<<<<<<< HEAD
    /// Execute the graph and return the outputs of the selected node (or output node if none selected)
=======
>>>>>>> a665ac9 (commit now so I don't screw something up)
=======
    /// Execute the graph and return the outputs of the selected node (or output node if none selected)
>>>>>>> bc26540 (spreading things out from the node_graph and added another node to rotate things.)
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

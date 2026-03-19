//! Manager for the node graph and its execution, separate from the UI state in EditorArea
//! This module defines the GraphExecutorManager, which holds the engine graph and the GraphExecutor instance.
//! It provides methods to check for changes, execute the graph, and determine which node's output to display based on selection or graph structure.
use engine::graph_executor::{ExecutionContext, GraphExecutor, NodeValue};
use engine::node::NodeLibrary;
use engine::node_graph::{EngineNodeId, InputValue, NodeGraph};
use std::collections::HashSet;

/// Manager for the node graph and its execution, separate from the UI state in EditorArea
pub struct GraphExecutorManager {
    engine_graph: NodeGraph,
    graph_executor: GraphExecutor,
    last_selected_engine_node: Option<EngineNodeId>,
    graph_changed: bool,
}

impl GraphExecutorManager {
    pub fn new() -> Self {
        Self {
            engine_graph: NodeGraph::default(),
            graph_executor: GraphExecutor::default(),
            last_selected_engine_node: None,
            graph_changed: false,
        }
    }

    pub fn engine_graph(&self) -> &NodeGraph {
        &self.engine_graph
    }

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

    pub fn set_last_selected_engine_node(&mut self, node: Option<EngineNodeId>) {
        self.last_selected_engine_node = node;
    }

    /// Check if the selection has changed since the last execution
    /// Used to determine if we need to re-execute the graph when the user selects a different node
    pub fn selection_changed(&self, new_selection: Option<EngineNodeId>) -> bool {
        new_selection != self.last_selected_engine_node
    }

    /// Execute the graph and return the outputs of the selected node (or output node if none selected)
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

    /// Query target FPS for a specific node id directly from the executor.
    pub fn get_target_fps_for_node(
        &mut self,
        node_library: &NodeLibrary,
        node_id: EngineNodeId,
    ) -> Option<f64> {
        self.graph_executor
            .get_target_fps_for_node(&self.engine_graph, node_library, node_id)
    }

    /// Resolve playback FPS for a display node by checking the node itself first,
    /// then traversing upstream connections for video sources.
    pub fn get_target_fps_for_display_node(
        &mut self,
        node_library: &NodeLibrary,
        node_id: EngineNodeId,
    ) -> Option<f64> {
        if let Some(fps) = self.get_target_fps_for_node(node_library, node_id) {
            return Some(fps);
        }

        let mut visited = HashSet::new();
        let mut stack = vec![node_id];
        let mut best_fps: Option<f64> = None;

        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }

            if current != node_id
                && let Some(fps) = self.get_target_fps_for_node(node_library, current)
            {
                best_fps = Some(match best_fps {
                    Some(existing) => existing.max(fps),
                    None => fps,
                });
            }

            if let Some(instance) = self.engine_graph.get_instance(current) {
                for input in instance.input_values.values() {
                    if let InputValue::Connection { from_node, .. } = input {
                        stack.push(*from_node);
                    }
                }
            }
        }

        best_fps
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

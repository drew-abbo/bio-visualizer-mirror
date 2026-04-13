//! Manager for the node graph and its execution, separate from the UI state in EditorArea
//! This module defines the GraphExecutorManager, which holds the engine graph and the GraphExecutor instance.
//! It provides methods to check for changes, execute the graph, and determine which node's output to display based on selection or graph structure.
use engine::graph_executor::{ExecutionError, GraphExecutor, NodeValue};
use engine::node::NodeLibrary;
use engine::node::engine_node::{BuiltInHandler, NodeExecutionPlan};
use engine::node_graph::{EngineNodeId, InputValue, NodeGraph};
use media::fps::Fps;
use media::fps::consts::FPS_30;
use std::collections::HashSet;

/// Manager for the node graph and its execution, separate from the UI state in EditorArea
pub struct GraphExecutorManager {
    engine_graph: NodeGraph,
    graph_executor: GraphExecutor,
    last_selected_engine_node: Option<EngineNodeId>,
    output_source_engine_node: Option<EngineNodeId>,
    graph_changed: bool,
}

impl GraphExecutorManager {
    pub fn new() -> Self {
        Self {
            engine_graph: NodeGraph::default(),
            graph_executor: GraphExecutor::default(),
            last_selected_engine_node: None,
            output_source_engine_node: None,
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

    pub fn set_output_source_engine_node(&mut self, node: Option<EngineNodeId>) {
        self.output_source_engine_node = node;
    }

    pub fn node_in_output_subgraph(
        &self,
        selected_node: EngineNodeId,
        output_node: EngineNodeId,
    ) -> bool {
        if selected_node == output_node {
            return true;
        }

        let mut visited = HashSet::new();
        let mut stack = vec![output_node];

        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }

            if current == selected_node {
                return true;
            }

            if let Some(instance) = self.engine_graph.get_instance(current) {
                for input in instance.input_values.values() {
                    if let InputValue::Connection { from_node, .. } = input {
                        stack.push(*from_node);
                    }
                }
            }
        }

        false
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
    ) -> Result<Option<NodeValue>, ExecutionError> {
        // Always execute up to the output node - that covers the full graph
        let Some(output_node_id) = self.output_source_engine_node else {
            return Ok(None);
        };

        let target_node_id = selected_engine_node.unwrap_or(output_node_id);

        // Single execute call - runs up to whichever node is further downstream.
        // If selected node IS the output node or is upstream of it, one call covers both.
        // If selected node is downstream/unrelated, fall back to output node.
        let exec_target = if selected_engine_node.is_some()
            && self.node_in_output_subgraph(target_node_id, output_node_id)
        {
            output_node_id // running to output covers selected node too (it's upstream)
        } else {
            target_node_id
        };

        self.graph_executor.execute(
            &self.engine_graph,
            node_library,
            &render_state.device,
            &render_state.queue,
            Some(exec_target),
        )?;

        // Try selected node's output first, then fall back to output node - both
        // are already in the output_cache from the single execute call above.
        let frame_from_selected = selected_engine_node
            .and_then(|id| self.graph_executor.get_node_outputs(id))
            .and_then(|outputs| {
                outputs.values().find_map(|v| {
                    if matches!(v, NodeValue::Frame(_)) {
                        Some(v.clone())
                    } else {
                        None
                    }
                })
            });

        if frame_from_selected.is_some() {
            return Ok(frame_from_selected);
        }

        Ok(self
            .graph_executor
            .get_node_outputs(output_node_id)
            .and_then(|outputs| {
                outputs.values().find_map(|v| {
                    if matches!(v, NodeValue::Frame(_)) {
                        Some(v.clone())
                    } else {
                        None
                    }
                })
            }))
    }

    /// Query target FPS for a specific node id directly from the executor.
    pub fn get_target_fps_for_node(
        &mut self,
        node_library: &NodeLibrary,
        node_id: EngineNodeId,
    ) -> Option<Fps> {
        self.graph_executor
            .get_target_fps_for_node(&self.engine_graph, node_library, node_id)
    }

    /// Resolve a recommended global playback FPS for the display node subgraph.
    pub fn get_target_fps_for_display_node(
        &mut self,
        node_library: &NodeLibrary,
        node_id: EngineNodeId,
    ) -> Option<Fps> {
        let mut visited = HashSet::new();
        let mut stack = vec![node_id];
        let mut best_video_fps: Option<Fps> = None;

        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }

            if let Some(instance) = self.engine_graph.get_instance(current)
                && let Some(definition) = node_library.get_definition(&instance.definition_name)
                && let NodeExecutionPlan::BuiltIn(BuiltInHandler::VideoSource) =
                    definition.node.executor
                && let Some(fps) = self.get_target_fps_for_node(node_library, current)
            {
                best_video_fps = Some(match best_video_fps {
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

        if let Some(fps) = best_video_fps {
            Some(fps)
        } else {
            // default to 30
            Some(FPS_30)
        }
    }

    pub fn pause_streams(&mut self) {
        self.graph_executor.pause_streams();
    }

    pub fn play_streams(&mut self) {
        self.graph_executor.play_streams();
    }

    pub fn set_global_stream_target_fps(&mut self, target_fps: Fps) {
        self.graph_executor.set_global_stream_target_fps(target_fps);
    }
}

impl Default for GraphExecutorManager {
    fn default() -> Self {
        Self::new()
    }
}

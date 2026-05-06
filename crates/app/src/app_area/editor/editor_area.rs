use super::editor_state_context::EditorStateContext;
use super::node_graph::{
    GraphSyncResult, InputWidgetState, NodeGraphState, NodeGraphViewer, sync_graph,
};
use super::snarl_style;
// EditorArea should not depend on MainOutputArea; AppArea will pass primitives.

use eframe;
use egui;
use egui_wgpu::wgpu;
use engine::engine_outpost::{EngineCommand, EngineCommandSender};
use engine::node::NodeLibrary;
use engine::node_graph::{EngineNodeId, InputValue, NodeGraph};
use std::collections::VecDeque;
use std::sync::Arc;
use util::ui::ErrorPopup;

/// Manages all editor-related state: node graph, output display, and playback
pub struct EditorArea {
    /// Local node graph used when no project is open
    local_node_graph: NodeGraphState,
    error_popup_queue: VecDeque<String>,
    engine_tx: Option<EngineCommandSender>,
    engine_graph: NodeGraph,
    last_selected_engine_node: Option<EngineNodeId>,
    output_source_engine_node: Option<EngineNodeId>,
    node_library: Arc<NodeLibrary>,
    editor_state_context: EditorStateContext,
    input_widget_state: InputWidgetState,
    playback_enabled: bool,
    last_warned_disconnected_selected_node: Option<engine::node_graph::EngineNodeId>,
    snarl_view_generation: u64,
    apply_saved_graph_zoom_once: bool,
    last_synced_topology_hash: Option<u64>,
    last_graph_errors: Vec<String>,
}

impl EditorArea {
    pub fn new() -> Self {
        let node_library = match NodeLibrary::load_all() {
            Ok(lib) => Arc::new(lib),
            Err(err) => {
                util::debug_log_error!("Failed to load node library: {:?}", err);
                Arc::new(NodeLibrary::default())
            }
        };

        Self {
            local_node_graph: NodeGraphState::new(),
            error_popup_queue: VecDeque::default(),
            engine_tx: None,
            engine_graph: NodeGraph::default(),
            last_selected_engine_node: None,
            output_source_engine_node: None,
            node_library,
            editor_state_context: EditorStateContext::new(),
            input_widget_state: InputWidgetState::new(),
            playback_enabled: true,
            last_warned_disconnected_selected_node: None,
            snarl_view_generation: 0,
            apply_saved_graph_zoom_once: true,
            last_synced_topology_hash: None,
            last_graph_errors: Vec::new(),
        }
    }

    /// Get the active node graph (from project if open, otherwise local)
    fn active_node_graph_mut(&mut self) -> &mut NodeGraphState {
        self.editor_state_context
            .node_graph_mut()
            .unwrap_or(&mut self.local_node_graph)
    }

    /// Access to the editor state context for project operations
    pub fn editor_state_context_mut(&mut self) -> &mut EditorStateContext {
        &mut self.editor_state_context
    }

    pub fn engine_command_sender(&self) -> Option<EngineCommandSender> {
        self.engine_tx.clone()
    }

    /// Spawn the engine outpost using editor's node library and attach local command sender.
    /// Returns the `EngineOutpostHandle` so the caller (AppArea) can distribute it.
    pub fn spawn_engine(
        &mut self,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        format: wgpu::TextureFormat,
    ) -> engine::engine_outpost::EngineOutpostHandle {
        let handle = engine::spawn(device, queue, self.node_library.clone(), format);
        self.engine_tx = Some(handle.command_sender());
        handle
    }

    /// Load a project, normalizing node inputs to match current schema definitions.
    /// This ensures missing inputs from schema changes are populated with defaults.
    pub fn load_project(
        &mut self,
        mut project: util::local_data::project::OpenProject<NodeGraphState>,
    ) {
        // Normalize inputs to handle schema changes since project was saved
        super::node_graph::normalize_node_inputs(project.data_mut(), &self.node_library);

        let warnings =
            super::node_graph::validate_midi_ports(project.data_mut(), &self.node_library);
        for warning in warnings {
            util::debug_log_warning!("Graph startup validation: {}", warning);
            self.error_popup_queue.push_back(warning);
        }

        self.editor_state_context.set_project(project);
    }

    fn set_playback_enabled(&mut self, enabled: bool) {
        if self.playback_enabled != enabled
            && let Some(tx) = self.engine_tx.clone()
        {
            let command = if enabled {
                EngineCommand::PlayStreams
            } else {
                EngineCommand::PauseStreams
            };

            if let Err(err) = tx.send(command) {
                util::debug_log_warning!("Failed to queue playback command: {err}");
            }
        }
        self.playback_enabled = enabled;
    }

    // `playback_due` removed — engine outpost drives timing; local timers are deprecated.
}

impl EditorArea {
    // `show_with_main_output` removed; AppArea calls `show` directly with primitive values.

    /// Render the entire editor area
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        preview_selected_node_enabled: bool,
        output_has_frame: bool,
        playback_enabled: bool,
    ) {
        // Apply playback controls handed down from AppArea
        self.set_playback_enabled(playback_enabled);
        // Engine spawning is performed centrally by AppArea; EditorArea only uses `engine_tx`.

        // Render graph UI, then update preview/output from current selection.
        let selected_nodes = self.show_node_graph(ctx);
        let selected_snarl_node = self.update_output_selection(&selected_nodes);
        self.update_output_from_graph(
            frame,
            selected_snarl_node,
            preview_selected_node_enabled,
            output_has_frame,
        );

        self.show_any_error_popups(ctx);
    }

    // Output window owns its event loop; no syncing required from EditorArea.

    pub fn save_state(&mut self) {
        match self.editor_state_context.save() {
            Ok(true) => {
                util::debug_log_info!("Project saved successfully");
            }
            Ok(false) => {
                util::debug_log_info!("No changes to save");
            }
            Err(e) => {
                util::debug_log_error!("Failed to save project: {}", e);
                self.error_popup_queue
                    .push_back(format!("Failed to save project: {}", e));
            }
        }
    }

    fn show_node_graph(&mut self, ctx: &egui::Context) -> Vec<egui_snarl::NodeId> {
        let mut selected_nodes = Vec::new();
        let mut pending_errors = Vec::new();
        let mut input_widget_state = std::mem::take(&mut self.input_widget_state);

        // First, render the UI
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(16, 20, 22)))
            .show(ctx, |ui| {
                let mut viewer =
                    NodeGraphViewer::new(self.node_library.clone(), &mut input_widget_state);

                let snarl_widget = egui_snarl::ui::SnarlWidget::new()
                    .id(egui::Id::new(("node_graph", self.snarl_view_generation)))
                    .style(snarl_style::snarl_style());

                let apply_saved_graph_zoom_once = self.apply_saved_graph_zoom_once;
                let mut reset_view_requested = false;
                {
                    let node_graph = self.active_node_graph_mut();
                    node_graph.ensure_output_sink();
                    viewer.set_initial_graph_view(
                        node_graph.graph_view,
                        node_graph.legacy_graph_view_zoom,
                        apply_saved_graph_zoom_once,
                    );
                    snarl_widget.show(&mut node_graph.snarl, &mut viewer, ui);
                    node_graph.graph_view = viewer.latest_graph_view();
                    node_graph.legacy_graph_view_zoom = None;

                    if viewer.take_reset_view_requested() {
                        node_graph.graph_view = None;
                        node_graph.legacy_graph_view_zoom = None;
                        reset_view_requested = true;
                    }
                }

                self.apply_saved_graph_zoom_once = false;
                if reset_view_requested {
                    self.snarl_view_generation = self.snarl_view_generation.wrapping_add(1);
                    self.apply_saved_graph_zoom_once = true;
                    self.editor_state_context.mark_edited();
                }

                selected_nodes = snarl_widget.get_selected_nodes(ui);
                pending_errors = viewer.take_pending_errors();
            });

        self.input_widget_state = input_widget_state;

        for error in pending_errors {
            self.error_popup_queue.push_back(error);
        }

        // Sync to engine only when graph TOPOLOGY has changed (not when moving nodes).
        let has_project = self.editor_state_context.has_open_project();

        let current_topology_hash = self.active_node_graph_mut().compute_topology_hash();
        if self.last_synced_topology_hash != current_topology_hash {
            self.last_synced_topology_hash = current_topology_hash;
            let node_library = self.node_library.clone();
            let warnings =
                super::node_graph::validate_midi_ports(self.active_node_graph_mut(), &node_library);
            for warning in warnings {
                self.error_popup_queue.push_back(warning);
            }
            self.push_graph_to_engine();
        }

        // Keep only the dirty-state tracking below, remove everything that calls sync_to_engine:
        let is_interacting_with_graph = ctx.input(|i| {
            i.pointer.any_down()
                || i.pointer.any_released()
                || i.raw_scroll_delta != egui::Vec2::ZERO
                || (i.zoom_delta() - 1.0).abs() > f32::EPSILON
        });

        if has_project
            && is_interacting_with_graph
            && let Some(current_state) = self.editor_state_context.node_graph_mut()
            && let Some(current_hash) = EditorStateContext::compute_state_hash(current_state)
        {
            self.editor_state_context.check_hash_changed(current_hash);
        }

        selected_nodes
    }

    fn push_graph_to_engine(&mut self) {
        let Some(tx) = self.engine_tx.clone() else {
            return;
        };

        let node_library = self.node_library.clone();
        let sync_result = sync_graph(self.active_node_graph_mut(), &node_library);

        match sync_result {
            GraphSyncResult::Valid {
                graph,
                output_node,
                snarl_to_engine,
            } => {
                self.last_graph_errors.clear();
                // Write stable engine IDs back so preview-selection lookups stay valid.
                let node_graph = self.active_node_graph_mut();
                for (&snarl_id, &engine_id) in &snarl_to_engine {
                    node_graph.snarl[snarl_id].engine_node_id = Some(engine_id);
                }
                self.engine_graph = graph.clone();
                let _ = tx.send(EngineCommand::UpdateGraph(graph));
                let _ = tx.send(EngineCommand::SetOutputNode(Some(output_node)));
            }
            GraphSyncResult::NoOutput => {
                self.last_graph_errors.clear();
                self.engine_graph = NodeGraph::default();
                let _ = tx.send(EngineCommand::UpdateGraph(NodeGraph::default()));
                let _ = tx.send(EngineCommand::SetOutputNode(None));
            }
            GraphSyncResult::Invalid(errors) => {
                self.engine_graph = NodeGraph::default();
                let _ = tx.send(EngineCommand::UpdateGraph(NodeGraph::default()));
                let _ = tx.send(EngineCommand::SetOutputNode(None));
                for error in &errors {
                    if !self.last_graph_errors.contains(error) {
                        self.error_popup_queue.push_back(error.clone());
                    }
                }
                self.last_graph_errors = errors;
            }
        }
    }

    fn update_output_selection(
        &mut self,
        selected_nodes: &[egui_snarl::NodeId],
    ) -> Option<egui_snarl::NodeId> {
        if selected_nodes.is_empty() {
            None
        } else {
            selected_nodes.last().copied()
        }
    }

    fn update_output_from_graph(
        &mut self,
        frame: &eframe::Frame,
        selected_snarl_node: Option<egui_snarl::NodeId>,
        preview_selected_node_enabled: bool,
        _output_has_frame: bool,
    ) {
        let Some(_render_state) = frame.wgpu_render_state() else {
            return;
        };

        let (selected_engine_node, output_source_engine_node) = {
            let node_graph = self.active_node_graph_mut();
            let selected_engine_node =
                selected_snarl_node.and_then(|id| node_graph.snarl[id].engine_node_id);
            let output_source_engine_node = node_graph
                .output_source_snarl_node()
                .and_then(|id| node_graph.snarl[id].engine_node_id);
            (selected_engine_node, output_source_engine_node)
        };

        self.output_source_engine_node = output_source_engine_node;

        // Warn about selected node being outside the output subgraph
        if !preview_selected_node_enabled
            && let Some(selected) = selected_engine_node
            && let Some(output) = output_source_engine_node
            && !self.node_in_output_subgraph(selected, output)
            && self.last_warned_disconnected_selected_node != Some(selected)
        {
            self.error_popup_queue.push_back(
            "Selected node is outside the output-connected graph. Enable 'Preview Selected Node' to view it."
                .to_string(),
        );
            self.last_warned_disconnected_selected_node = Some(selected);
        } else {
            self.last_warned_disconnected_selected_node = None;
        }

        // Update preview target via engine SetOutputNode if selection changed
        let preview_target = if preview_selected_node_enabled {
            selected_engine_node
        } else {
            None
        };

        if self.selection_changed(preview_target) {
            self.last_selected_engine_node = preview_target;
            if let Some(tx) = self.engine_tx.clone() {
                let _ = tx.send(EngineCommand::SetOutputNode(
                    preview_target.or(output_source_engine_node),
                ));
            }
        }
    }

    fn selection_changed(&self, new_selection: Option<EngineNodeId>) -> bool {
        new_selection != self.last_selected_engine_node
    }

    fn node_in_output_subgraph(
        &self,
        selected_node: EngineNodeId,
        output_node: EngineNodeId,
    ) -> bool {
        if selected_node == output_node {
            return true;
        }

        let mut visited = std::collections::HashSet::new();
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
}

impl ErrorPopup<String> for EditorArea {
    fn error_queue_mut(&mut self) -> &mut VecDeque<String> {
        &mut self.error_popup_queue
    }
}

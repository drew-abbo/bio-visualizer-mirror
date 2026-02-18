use super::graph_executor_manager::GraphExecutorManager;
use super::node_graph::{NodeGraphState, NodeGraphViewer};
use super::output_controller::OutputController;
use super::output_panel::OutputPanel;
use super::playback_controls::PlaybackControls;
use super::playback_state::PlaybackState;
use super::snarl_style;
use engine::node::NodeLibrary;
use std::sync::Arc;
use util::eframe;
use util::egui;

/// Manages all editor-related state: node graph, output display, and playback
pub struct EditorArea {
    node_graph: NodeGraphState,
    output_panel: OutputPanel,
    playback_controls: PlaybackControls,
    playback_state: PlaybackState,
    executor_manager: GraphExecutorManager,
    node_library: Arc<NodeLibrary>,
    last_graph_state: (usize, usize, u64), // (node_count, wire_count, input_hash)
}

impl EditorArea {
    pub fn new() -> Self {
        let node_library = if cfg!(debug_assertions) {
            let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
            let nodes_path = workspace_root.join("Nodes");
            match NodeLibrary::load_from_disk(nodes_path.clone()) {
                Ok(lib) => Arc::new(lib),
                Err(err) => {
                    util::debug_log_error!(
                        "Failed to load node library from disk at {:?}: {}",
                        nodes_path,
                        err
                    );
                    Arc::new(NodeLibrary::default())
                }
            }
        } else {
            match NodeLibrary::load_from_users_folder() {
                Ok(lib) => Arc::new(lib),
                Err(err) => {
                    util::debug_log_error!(
                        "Failed to load node library from users folder: {}",
                        err
                    );
                    Arc::new(NodeLibrary::default())
                }
            }
        };

        Self {
            node_graph: NodeGraphState::new(),
            output_panel: OutputPanel::new(),
            playback_controls: PlaybackControls::new(),
            playback_state: PlaybackState::new(),
            executor_manager: GraphExecutorManager::new(),
            node_library,
            last_graph_state: (0, 0, 0),
        }
    }
}

impl EditorArea {
    /// Render the entire editor area
    pub fn show(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        // show the node graph and get selected nodes
        // feed selected node into output panel to update its content
        let selected_nodes = self.show_node_graph(ctx);
        let selected_snarl_node = self.update_output_selection(&selected_nodes);
        self.update_output_from_graph(ctx, frame, selected_snarl_node);
        self.show_output_window(ctx);
    }

    fn show_node_graph(&mut self, ctx: &egui::Context) -> Vec<egui_snarl::NodeId> {
        let mut selected_nodes = Vec::new();

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let mut viewer = NodeGraphViewer::new(self.node_library.clone());

                let snarl_widget = egui_snarl::ui::SnarlWidget::new()
                    .id(egui::Id::new("node_graph"))
                    .style(snarl_style::snarl_style());

                snarl_widget.show(&mut self.node_graph.snarl, &mut viewer, ui);
                selected_nodes = snarl_widget.get_selected_nodes(ui);

                // Sync when graph structure or input values change
                let current_state = Self::compute_graph_state(&self.node_graph.snarl);
                if current_state != self.last_graph_state {
                    self.node_graph.sync_to_engine(
                        self.executor_manager.engine_graph_mut(),
                        &self.node_library,
                    );

                    self.last_graph_state = current_state;
                }
            });

        selected_nodes
    }

    fn update_output_selection(
        &mut self,
        selected_nodes: &[egui_snarl::NodeId],
    ) -> Option<egui_snarl::NodeId> {
        let selected_snarl_node = if selected_nodes.is_empty() {
            None
        } else {
            selected_nodes.last().copied()
        };
        self.output_panel.set_selected_node(selected_snarl_node);
        selected_snarl_node
    }

    fn update_output_from_graph(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        selected_snarl_node: Option<egui_snarl::NodeId>,
    ) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };

        // Get the snarl node's associated engine node id
        let selected_engine_node =
            selected_snarl_node.and_then(|snarl_id| self.node_graph.snarl[snarl_id].engine_node_id);

        OutputController::update(
            ctx,
            &mut self.output_panel,
            &self.playback_controls,
            &mut self.playback_state,
            &mut self.executor_manager,
            &self.node_library,
            render_state,
            selected_engine_node,
        );
    }

    fn show_output_window(&mut self, ctx: &egui::Context) {
        egui::Window::new("Output")
            .default_size(egui::vec2(640.0, 480.0))
            .resizable(true)
            .movable(true)
            .show(ctx, |ui| {
                // Playback controls at the top
                self.playback_controls.ui(ui);
                ui.separator();

                // Output panel content
                self.output_panel.render_content(ui);
            });
    }

    /// Compute a state fingerprint for detecting graph changes
    /// Returns (node_count, wire_count, input_values_hash)
    fn compute_graph_state(snarl: &egui_snarl::Snarl<super::node_graph::NodeData>) -> (usize, usize, u64) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let node_count = snarl.node_ids().count();
        let wire_count = snarl.wires().count();
        
        // Hash all input values to detect property changes
        let mut hasher = DefaultHasher::new();
        for (_, node) in snarl.node_ids() {
            // Hash the number of input values and their presence/count
            node.input_values.len().hash(&mut hasher);
            
            // Simple hash based on keys and value discriminants
            // We don't need perfect hashing, just change detection
            let mut keys: Vec<_> = node.input_values.keys().collect();
            keys.sort();
            for key in keys {
                key.hash(&mut hasher);
                // Hash the value variant and key data
                if let Some(value) = node.input_values.get(key) {
                    std::mem::discriminant(value).hash(&mut hasher);
                    // For files, hash the path
                    if let engine::node_graph::InputValue::File(path) = value {
                        path.hash(&mut hasher);
                    }
                }
            }
        }
        
        (node_count, wire_count, hasher.finish())
    }
}

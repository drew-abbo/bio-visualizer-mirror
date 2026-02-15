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
}

impl EditorArea {
<<<<<<< HEAD:crates/app/src/area/editor/editor_area.rs
<<<<<<< HEAD
    pub fn new() -> Self {
        let node_library = match NodeLibrary::load_all() {
            Ok(lib) => Arc::new(lib),
            Err(err) => {
                util::debug_log_error!("Failed to load node library: {:?}", err);
                Arc::new(NodeLibrary::default())
            }
        };

=======
    pub fn new(node_library: Arc<NodeLibrary>) -> Self {
>>>>>>> a665ac9 (commit now so I don't screw something up)
=======
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

>>>>>>> ee4c645 (restructure and some comments):crates/app/src/app_area/editor/editor_area.rs
        Self {
            node_graph: NodeGraphState::new(),
            output_panel: OutputPanel::new(),
            playback_controls: PlaybackControls::new(),
            playback_state: PlaybackState::new(),
            executor_manager: GraphExecutorManager::new(),
            node_library,
        }
    }
<<<<<<< HEAD:crates/app/src/area/editor/editor_area.rs
<<<<<<< HEAD
}

impl EditorArea {
    /// Render the entire editor area
    pub fn show(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        // show the node graph and get selected nodes
        // feed selected node into output panel to update its content
=======
=======
}
>>>>>>> ee4c645 (restructure and some comments):crates/app/src/app_area/editor/editor_area.rs

impl EditorArea {
    /// Render the entire editor area
    pub fn show(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
>>>>>>> a665ac9 (commit now so I don't screw something up)
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
<<<<<<< HEAD:crates/app/src/area/editor/editor_area.rs
<<<<<<< HEAD
                let mut viewer = NodeGraphViewer::new(self.node_library.clone());
=======
                let mut viewer = NodeGraphViewer::new(Arc::clone(&self.node_library));
>>>>>>> a665ac9 (commit now so I don't screw something up)
=======
                let mut viewer = NodeGraphViewer::new(self.node_library.clone());
>>>>>>> ee4c645 (restructure and some comments):crates/app/src/app_area/editor/editor_area.rs

                let snarl_widget = egui_snarl::ui::SnarlWidget::new()
                    .id(egui::Id::new("node_graph"))
                    .style(snarl_style::snarl_style());

<<<<<<< HEAD
                snarl_widget.show(&mut self.node_graph.snarl, &mut viewer, ui);
                selected_nodes = snarl_widget.get_selected_nodes(ui);

                // Sync every frame - the sync logic detects what actually changed
                let graph_changed = self.node_graph.sync_to_engine(
                    self.executor_manager.engine_graph_mut_no_flag(),
                    &self.node_library,
                );

                // Only mark as changed if sync actually made changes
                if graph_changed {
                    self.executor_manager.mark_graph_changed();
                }
=======
                // IMPORTANT: Capture the response to ensure proper interaction handling
                let _response = snarl_widget.show(&mut self.node_graph.snarl, &mut viewer, ui);
                selected_nodes = snarl_widget.get_selected_nodes(ui);

                // Sync every frame for now (simple + reliable)
                self.node_graph
                    .sync_to_engine(self.executor_manager.engine_graph_mut(), &self.node_library);
>>>>>>> a665ac9 (commit now so I don't screw something up)
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

<<<<<<< HEAD
        // Get the snarl node's associated engine node id
=======
>>>>>>> a665ac9 (commit now so I don't screw something up)
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
}
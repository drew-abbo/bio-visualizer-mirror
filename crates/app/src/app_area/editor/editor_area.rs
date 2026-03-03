use super::editor_state_context::EditorStateContext;
use super::graph_executor_manager::GraphExecutorManager;
use super::node_graph::{NodeGraphState, NodeGraphViewer};
use super::output_controller::OutputController;
use super::output_panel::OutputPanel;
use super::playback_controls::PlaybackControls;
use super::playback_state::PlaybackState;
use super::snarl_style;
use crate::launcher_comm;
use engine::node::NodeLibrary;
use std::sync::Arc;
use util::eframe;
use util::egui;

/// Manages all editor-related state: node graph, output display, and playback
pub struct EditorArea {
    /// Local node graph used when no project is open
    local_node_graph: NodeGraphState,
    output_panel: OutputPanel,
    playback_controls: PlaybackControls,
    playback_state: PlaybackState,
    executor_manager: GraphExecutorManager,
    node_library: Arc<NodeLibrary>,
    editor_state_context: EditorStateContext,
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
            output_panel: OutputPanel::new(),
            playback_controls: PlaybackControls::new(),
            playback_state: PlaybackState::new(),
            executor_manager: GraphExecutorManager::new(),
            node_library,
            editor_state_context: EditorStateContext::new(),
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

    pub fn save_state(&mut self, skip_notification: bool) {
        match self.editor_state_context.save() {
            Ok(true) => {
                util::debug_log_info!("Project saved successfully");
                // Notify launcher that the project was updated (unless we're exiting)
                if !skip_notification {
                    launcher_comm::notify_project_updated();
                }
            }
            Ok(false) => {
                util::debug_log_info!("No changes to save");
            }
            Err(e) => {
                util::debug_log_error!("Failed to save project: {}", e);
            }
        }
    }

    fn show_node_graph(&mut self, ctx: &egui::Context) -> Vec<egui_snarl::NodeId> {
        let mut selected_nodes = Vec::new();

        // First, render the UI
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let mut viewer = NodeGraphViewer::new(self.node_library.clone());

                let snarl_widget = egui_snarl::ui::SnarlWidget::new()
                    .id(egui::Id::new("node_graph"))
                    .style(snarl_style::snarl_style());

                let node_graph = self.active_node_graph_mut();
                snarl_widget.show(&mut node_graph.snarl, &mut viewer, ui);
                selected_nodes = snarl_widget.get_selected_nodes(ui);
            });

        // Then sync to engine (after UI to avoid multiple borrows)
        // Check if we have a project graph or use local graph
        let has_project = self.editor_state_context.has_open_project();

        let graph_changed = if has_project {
            // Sync project graph to engine
            let project_graph = self.editor_state_context.node_graph_mut().unwrap();
            project_graph.sync_to_engine(
                self.executor_manager.engine_graph_mut_no_flag(),
                &self.node_library,
            )
        } else {
            // Sync local graph to engine
            self.local_node_graph.sync_to_engine(
                self.executor_manager.engine_graph_mut_no_flag(),
                &self.node_library,
            )
        };

        // Mark executor as changed if graph sync made changes
        if graph_changed {
            self.executor_manager.mark_graph_changed();
        }

        // Check if node count changed and mark as edited if so
        let current_node_count = if has_project {
            self.editor_state_context
                .node_graph_mut()
                .unwrap()
                .snarl
                .node_ids()
                .count()
        } else {
            self.local_node_graph.snarl.node_ids().count()
        };
        self.editor_state_context
            .check_node_count_changed(current_node_count);

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
        let node_graph = self.active_node_graph_mut();
        let selected_engine_node =
            selected_snarl_node.and_then(|snarl_id| node_graph.snarl[snarl_id].engine_node_id);

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

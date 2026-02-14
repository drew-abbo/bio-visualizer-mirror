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
    pub fn new(node_library: Arc<NodeLibrary>) -> Self {
        Self {
            node_graph: NodeGraphState::new(),
            output_panel: OutputPanel::new(),
            playback_controls: PlaybackControls::new(),
            playback_state: PlaybackState::new(),
            executor_manager: GraphExecutorManager::new(),
            node_library,
        }
    }

    /// Render the entire editor area
    pub fn show(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
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
                let mut viewer = NodeGraphViewer::new(Arc::clone(&self.node_library));

                let snarl_widget = egui_snarl::ui::SnarlWidget::new()
                    .id(egui::Id::new("node_graph"))
                    .style(snarl_style::snarl_style());

                // IMPORTANT: Capture the response to ensure proper interaction handling
                let _response = snarl_widget.show(&mut self.node_graph.snarl, &mut viewer, ui);
                selected_nodes = snarl_widget.get_selected_nodes(ui);

                // Sync every frame for now (simple + reliable)
                self.node_graph
                    .sync_to_engine(self.executor_manager.engine_graph_mut(), &self.node_library);
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

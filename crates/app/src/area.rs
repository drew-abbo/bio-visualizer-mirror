mod graph_executor_manager;
mod node_graph;
mod output_controller;
mod output_panel;
mod playback_controls;
mod snarl_style;
mod title_bar;

use crate::view::View;
use engine::node::NodeLibrary;
use graph_executor_manager::GraphExecutorManager;
use node_graph::{NodeGraphState, NodeGraphViewer};
use output_controller::OutputController;
use output_panel::OutputPanel;
use std::sync::Arc;
use util::eframe;
use util::egui;

pub struct App {
    title_bar: title_bar::TitleBar,
    node_graph: NodeGraphState,
    output_panel: OutputPanel,
    node_library: Arc<NodeLibrary>,
    executor_manager: GraphExecutorManager,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        // Load node library
        let node_library = if cfg!(debug_assertions) {
            let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
            let nodes_path = workspace_root.join("Nodes");
            match NodeLibrary::load_from_disk(nodes_path.clone()) {
                Ok(lib) => lib,
                Err(err) => {
                    util::debug_log_error!(
                        "Failed to load node library from disk at {:?}: {}",
                        nodes_path,
                        err
                    );
                    NodeLibrary::default()
                }
            }
        } else {
            match NodeLibrary::load_from_users_folder() {
                Ok(lib) => lib,
                Err(err) => {
                    util::debug_log_error!(
                        "Failed to load node library from users folder: {}",
                        err
                    );
                    NodeLibrary::default()
                }
            }
        };

        let node_library = Arc::new(node_library);

        // Initialize node graph state
        let node_graph = NodeGraphState::new();

        Self {
            title_bar: title_bar::TitleBar::new(),
            node_graph,
            output_panel: OutputPanel::new(),
            node_library,
            executor_manager: GraphExecutorManager::new(),
        }
    }

    fn show_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu")
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(24, 29, 31))
                    .inner_margin(egui::Margin::symmetric(12, 6)),
            )
            .show(ctx, |ui| {
                self.title_bar.ui(ui);
            });
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
                snarl_widget.show(&mut self.node_graph.snarl, &mut viewer, ui);
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
            &mut self.executor_manager,
            &self.node_library,
            render_state,
            selected_engine_node,
        );
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.show_top_bar(ctx);
        let selected_nodes = self.show_node_graph(ctx);
        let selected_snarl_node = self.update_output_selection(&selected_nodes);
        self.update_output_from_graph(ctx, frame, selected_snarl_node);
        self.output_panel.show(ctx);
    }
}

mod title_bar;
pub use title_bar::TitleBar;
// mod node_graph_view;
mod node_graph;
mod output_panel;
mod playback_controls;
use crate::view::View;
// use node_graph_view::NodeGraphView;
use engine::graph_executor::GraphExecutor;
use engine::graph_executor::NodeValue;
use engine::node::NodeLibrary;
use engine::node_graph::NodeGraph;
use node_graph::{NodeGraphState, NodeGraphViewer};
use output_panel::OutputPanel;
use std::sync::Arc;
use util::eframe;
use util::egui;

pub struct App {
    title_bar: TitleBar,
    // node_blueprint: NodeGraphView,
    node_graph: NodeGraphState,
    output_panel: OutputPanel,
    node_library: Arc<NodeLibrary>,
    engine_graph: NodeGraph,
    graph_executor: GraphExecutor,
    last_selected_engine_node: Option<engine::node_graph::EngineNodeId>,
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
            NodeLibrary::load_from_disk(nodes_path).expect("Failed to load node library")
        } else {
            NodeLibrary::load_from_users_folder().expect("Failed to load node library")
        };

        let node_library = Arc::new(node_library);

        // Initialize node graph state
        let node_graph = NodeGraphState::new();

        Self {
            title_bar: TitleBar::new(),
            node_graph,
            output_panel: OutputPanel::new(),
            node_library,
            engine_graph: NodeGraph::default(),
            graph_executor: GraphExecutor::default(),
            last_selected_engine_node: None,
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

                // Configure snarl style with visible background and selection
                let style = egui_snarl::ui::SnarlStyle {
                    bg_pattern: Some(egui_snarl::ui::BackgroundPattern::grid(
                        egui::vec2(40.0, 40.0),
                        0.0,
                    )),
                    bg_pattern_stroke: Some(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgb(50, 50, 55),
                    )),
                    bg_frame: Some(
                        egui::Frame::default()
                            .fill(egui::Color32::from_rgb(30, 30, 35))
                            .inner_margin(0.0),
                    ),
                    select_style: Some(egui_snarl::ui::SelectionStyle {
                        margin: egui::Margin::same(4),
                        rounding: egui::CornerRadius::same(6),
                        fill: egui::Color32::from_rgba_unmultiplied(100, 150, 255, 30),
                        stroke: egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 150, 255)),
                    }),
                    ..egui_snarl::ui::SnarlStyle::new()
                };

                self.node_graph
                    .snarl
                    .show(&mut viewer, &style, egui::Id::new("node_graph"), ui);

                selected_nodes =
                    egui_snarl::ui::get_selected_nodes(egui::Id::new("node_graph"), ui.ctx());

                // Sync every frame for now (simple + reliable)
                self.node_graph
                    .sync_to_engine(&mut self.engine_graph, &self.node_library);
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

        let is_playing = self.output_panel.playback_controls().is_playing();
        let selected_engine_node =
            selected_snarl_node.and_then(|snarl_id| self.node_graph.snarl[snarl_id].engine_node_id);
        let selection_changed = selected_engine_node != self.last_selected_engine_node;
        self.last_selected_engine_node = selected_engine_node;

        let mut output_value: Option<NodeValue> = None;

        if is_playing {
            ctx.request_repaint();
        }

        self.output_panel.update_playback_tick(is_playing);

        if (is_playing && !self.engine_graph.is_empty() && self.output_panel.should_advance_frame())
            || (selection_changed && !self.engine_graph.is_empty())
        {
            ctx.request_repaint();
            if let Ok(result) = self.graph_executor.execute(
                &self.engine_graph,
                &self.node_library,
                &render_state.device,
                &render_state.queue,
                selected_engine_node,
            ) {
                if let Some(NodeValue::Float(fps)) = result.outputs.get("fps") {
                    self.output_panel.set_playback_fps(*fps as f64);
                }
                output_value = result.outputs.get("output").cloned().or_else(|| {
                    result
                        .outputs
                        .values()
                        .find(|value| matches!(value, NodeValue::Frame(_)))
                        .cloned()
                });
            } else {
                self.output_panel.clear_output();
                self.output_panel.clear_frame(Some(render_state));
            }
        } else if let Some(engine_id) = selected_engine_node {
            if let Some(outputs) = self.graph_executor.get_node_outputs(engine_id) {
                output_value = outputs.get("output").cloned().or_else(|| {
                    outputs
                        .values()
                        .find(|value| matches!(value, NodeValue::Frame(_)))
                        .cloned()
                });
            }
        } else {
            let output_node_id = self.graph_executor.get_output_node_id();
            if let Some(outputs) = self.graph_executor.get_node_outputs(output_node_id) {
                output_value = outputs.get("output").cloned().or_else(|| {
                    outputs
                        .values()
                        .find(|value| matches!(value, NodeValue::Frame(_)))
                        .cloned()
                });
            }
        }

        if let Some(output_value) = output_value {
            self.output_panel.set_output_value(output_value.clone());
            self.output_panel
                .set_output_frame(render_state, &output_value);
        } else if selected_engine_node.is_none() {
            self.output_panel.clear_output();
            self.output_panel.clear_frame(Some(render_state));
        }
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

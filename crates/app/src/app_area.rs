mod title_bar;
pub use title_bar::TitleBar;
mod node_graph_view;
mod output_panel;
mod playback_controls;
use crate::engine_controller::EngineController;
use crate::view::View;
use node_graph_view::NodeGraphView;
use output_panel::OutputPanel;

pub struct App {
    title_bar: TitleBar,
    node_blueprint: NodeGraphView,
    output_panel: OutputPanel,
    engine_controller: EngineController,
    last_frame_time: std::time::Instant,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        // TODO: Handle error properly
        let engine_controller = EngineController::new().unwrap();
        
        // Initialize node graph view with loaded node library
        let mut node_blueprint = NodeGraphView::new();
        let node_definitions = engine_controller.node_library().definitions().clone();
        node_blueprint.set_node_library(node_definitions);

        Self {
            title_bar: TitleBar::new(),
            node_blueprint,
            output_panel: OutputPanel::new(),
            engine_controller,
            last_frame_time: std::time::Instant::now(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu")
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(24, 29, 31))
                    .inner_margin(egui::Margin::symmetric(12, 6)),
            )
            .show(ctx, |ui| {
                self.title_bar.ui(ui);
            });

        // Blueprint takes the remaining space
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.node_blueprint.ui(ui);
            });

        // Sync selected node from graph to output panel
        let selected_node = self.node_blueprint.selected_node_id();
        self.output_panel.set_selected_node(selected_node);

        // Sync the editor graph to the engine controller
        let engine_graph = self.node_blueprint.build_engine_graph();
        *self.engine_controller.node_graph_mut() = engine_graph;

        // Execute the graph with wgpu device and queue (only if there are nodes)
        if let Some(render_state) = frame.wgpu_render_state() {
            let graph = self.engine_controller.node_graph();
            let has_nodes = !graph.is_empty();
            let has_missing_file = graph
                .instances()
                .values()
                .any(|instance| {
                    instance.input_values.values().any(|value| {
                        matches!(
                            value,
                            engine::node_graph::InputValue::File(path)
                                if path.as_os_str().is_empty() || !path.exists()
                        )
                    })
                });

            // Sync FPS from engine to playback controls before throttling check
            if let Some(fps) = self.engine_controller.get_video_fps() {
                if fps > 0.0 {
                    self.output_panel.playback_controls_mut().set_fps(fps as f64);
                }
            }

            // Use wall-clock time instead of egui's unreliable unstable_dt
            let now = std::time::Instant::now();
            let dt = now.duration_since(self.last_frame_time).as_secs_f32();
            self.last_frame_time = now;
            
            let is_playing = self.output_panel.playback_controls().playing;
            let should_advance = if is_playing {
                self.output_panel
                    .playback_controls_mut()
                    .update_with_dt(dt)
            } else {
                false
            };
            
            if has_nodes && !has_missing_file && should_advance {
                // Both engine and app now use the same wgpu version (27) via egui-wgpu
                if let Err(e) = self.engine_controller.execute_graph(&render_state.device, &render_state.queue) {
                    eprintln!("Graph execution error: {}", e);
                }

                // Display output from selected node or graph's final output
                let mut chosen_outputs = None;

                if let Some(editor_node_id) = selected_node {
                    if let Some(engine_node_id) = self.node_blueprint.get_engine_node_id(editor_node_id) {
                        if let Some(outputs) = self.engine_controller.get_node_outputs(engine_node_id) {
                            chosen_outputs = Some(outputs);
                        }
                    }
                }

                if chosen_outputs.is_none() {
                    let output_node_id = self.engine_controller.get_output_node_id();
                    if let Some(outputs) = self.engine_controller.get_node_outputs(output_node_id) {
                        chosen_outputs = Some(outputs);
                    }
                }

                if let Some(outputs) = chosen_outputs {
                    let output_value = outputs.get("output").or_else(|| outputs.values().next());
                    
                    if let Some(output_value) = output_value {
                        self.output_panel.set_output_value(output_value.clone());
                        self.output_panel.set_output_frame(render_state, output_value);
                    }
                }
            } else if !has_nodes || has_missing_file {
                self.output_panel.clear_output();
                self.output_panel.clear_frame(Some(render_state));
            }

            // Control UI frame rate based on video FPS when playing
            let is_playing = self.output_panel.playback_controls().playing;
            if is_playing {
                if let Some(fps) = self.engine_controller.get_video_fps() {
                    if fps > 0.0 {
                        let frame_duration = std::time::Duration::from_secs_f32(1.0 / fps);
                        ctx.request_repaint_after(frame_duration);
                    }
                }
            } else if has_nodes && !has_missing_file {
                // When paused but still showing the output, request repaint to keep UI responsive
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }

        // Render the output panel (docked or floating) after execution
        self.output_panel.show(ctx);
    }
}

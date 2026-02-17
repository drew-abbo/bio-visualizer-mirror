use crate::components::FrameDisplay;
use egui_snarl::NodeId;
use engine::graph_executor::NodeValue;
use media::frame::Uid;
use util::egui;

/// Displays output from a node graph
/// This is for sure going to be changed in the near future
pub struct OutputPanel {
    frame_display: FrameDisplay,
    selected_node_id: Option<NodeId>,
    current_output: Option<NodeValue>,
    playback_fps: f64,
}

impl OutputPanel {
    pub fn new() -> Self {
        Self {
            frame_display: FrameDisplay::default_config(),
            selected_node_id: None,
            current_output: None,
            playback_fps: 30.0,
        }
    }

    pub fn reset(&mut self) {
        self.current_output = None;
    }

    /// Set the currently selected output node
    pub fn set_selected_node(&mut self, node_id: Option<NodeId>) {
        if self.selected_node_id != node_id {
            self.selected_node_id = node_id;
        }
    }

    /// Set the current output value to display
    pub fn set_output_value(&mut self, output: NodeValue) {
        self.current_output = Some(output);
    }

    /// Set playback FPS for display info
    pub fn set_playback_fps(&mut self, fps: f64) {
        if fps > 0.0 {
            self.playback_fps = fps;
        }
    }

    /// Update the displayed frame from output value
    pub fn set_output_frame(&mut self, render_state: &egui_wgpu::RenderState, output: &NodeValue) {
        match output {
            NodeValue::Frame(gpu_frame) => {
                let frame_id = Uid::generate_new();
                let size = [
                    gpu_frame.size.width as usize,
                    gpu_frame.size.height as usize,
                ];
                self.frame_display.set_wgpu_texture_if_changed(
                    render_state,
                    gpu_frame.view(),
                    size,
                    frame_id,
                );
            }
            _ => {
                self.frame_display.clear(Some(render_state));
            }
        }
    }

    /// Render panel content (caller must provide playback controls UI separately)
    pub fn render_content(&mut self, ui: &mut egui::Ui) {
        if let Some(node_id) = self.selected_node_id {
            ui.label(format!("Node: {:?}", node_id));
        } else {
            ui.label("Main output");
        }
        ui.separator();

        if let Some(ref output) = self.current_output {
            match output {
                NodeValue::Frame(gpu_frame) => {
                    ui.label(format!(
                        "Frame: {}x{}",
                        gpu_frame.size.width, gpu_frame.size.height
                    ));
                    egui::Frame::NONE.show(ui, |ui| {
                        self.frame_display.render_content(ui);
                    });
                }
                NodeValue::Bool(val) => {
                    ui.label(format!("Bool: {}", val));
                }
                NodeValue::Int(val) => {
                    ui.label(format!("Int: {}", val));
                }
                NodeValue::Float(val) => {
                    ui.label(format!("Float: {}", val));
                }
                NodeValue::Dimensions(w, h) => {
                    ui.label(format!("Dimensions: {}x{}", w, h));
                }
                NodeValue::Pixel(rgba) => {
                    ui.label(format!(
                        "Pixel: RGBA({}, {}, {}, {})",
                        rgba[0], rgba[1], rgba[2], rgba[3]
                    ));
                }
                NodeValue::Text(text) => {
                    ui.label(format!("Text: {}", text));
                }
                NodeValue::Enum(val) => {
                    ui.label(format!("Enum: {}", val));
                }
                NodeValue::File(path) => {
                    ui.label(format!("File: {}", path.display()));
                }
            }
        } else {
            ui.label("No output available");
        }
    }
}

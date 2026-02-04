use super::playback_controls::PlaybackControls;
use crate::components::FrameDisplay;
use crate::view::View;
use egui_snarl::NodeId;
use engine::graph_executor::NodeValue;
use media::frame::Uid;
use util::egui;

/// Manages the output panel display state and behavior
pub struct OutputPanel {
    frame_display: FrameDisplay,
    window_size: egui::Vec2,
    last_tick: std::time::Instant,
    playback_accumulator: std::time::Duration,
    playback_fps: f64,
    last_playing: bool,
    // Output tracking
    selected_node_id: Option<NodeId>,
    playback_controls: PlaybackControls,
    current_output: Option<NodeValue>,
}

impl OutputPanel {
    pub fn new() -> Self {
        Self {
            frame_display: FrameDisplay::default_config(),
            window_size: egui::vec2(640.0, 480.0),
            last_tick: std::time::Instant::now(),
            playback_accumulator: std::time::Duration::ZERO,
            playback_fps: 30.0,
            last_playing: false,
            selected_node_id: None,
            playback_controls: PlaybackControls::new(),
            current_output: None,
        }
    }

    /// Set the currently selected output node
    pub fn set_selected_node(&mut self, node_id: Option<NodeId>) {
        if self.selected_node_id != node_id {
            self.selected_node_id = node_id;
            // Reset playback when switching nodes
            self.playback_controls.reset();
        }
    }

    /// Set the current output value to display
    pub fn set_output_value(&mut self, output: NodeValue) {
        self.current_output = Some(output);
    }

    /// Clear the current output value
    pub fn clear_output(&mut self) {
        self.current_output = None;
    }

    /// Get reference to playback controls
    pub fn playback_controls(&self) -> &PlaybackControls {
        &self.playback_controls
    }

    pub fn update_playback_tick(&mut self, is_playing: bool) {
        if is_playing && !self.last_playing {
            self.last_tick = std::time::Instant::now();
            self.playback_accumulator = std::time::Duration::ZERO;
        }

        if !is_playing {
            self.last_playing = is_playing;
            return;
        }

        let now = std::time::Instant::now();
        let dt = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;
        self.playback_accumulator += dt;
        let frame_duration = if self.playback_fps > 0.0 {
            std::time::Duration::from_secs_f64(1.0 / self.playback_fps)
        } else {
            std::time::Duration::from_secs_f64(1.0 / 30.0)
        };
        if self.playback_accumulator > frame_duration {
            self.playback_accumulator = frame_duration;
        }
        self.last_playing = is_playing;
    }

    pub fn should_advance_frame(&mut self) -> bool {
        let frame_duration = if self.playback_fps > 0.0 {
            std::time::Duration::from_secs_f64(1.0 / self.playback_fps)
        } else {
            std::time::Duration::from_secs_f64(1.0 / 30.0)
        };

        if self.playback_accumulator >= frame_duration {
            self.playback_accumulator -= frame_duration;
            true
        } else {
            false
        }
    }

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

    /// Clear the displayed frame
    pub fn clear_frame(&mut self, render_state: Option<&egui_wgpu::RenderState>) {
        self.frame_display.clear(render_state);
    }

    /// Show the output panel (basic docked view)
    pub fn show(&mut self, ctx: &egui::Context) {
        egui::Window::new("Output")
            .default_size(self.window_size)
            .resizable(true)
            .movable(true)
            .show(ctx, |ui| {
                self.render_panel_content(ui);
            });
    }

    /// Render the main panel content with playback controls
    fn render_panel_content(&mut self, ui: &mut egui::Ui) {
        self.playback_controls.ui(ui);
        ui.separator();

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

impl View for OutputPanel {
    fn ui(&mut self, ui: &mut egui::Ui) {
        self.frame_display.ui(ui);
    }
}

use crate::components::FrameDisplay;
use engine::graph_executor::NodeValue;
use media::frame::Uid;
use util::egui;

/// Main output window for displaying frames with native FPS tracking
pub struct OutputWindow {
    frame_display: FrameDisplay,
    current_output: Option<NodeValue>,
    playback_fps: Option<f64>,
    last_texture_view_ptr: Option<usize>,
    last_renderer_ptr: Option<usize>,

    // Metadata
    frame_width: u32,
    frame_height: u32,
}

impl OutputWindow {
    pub fn new() -> Self {
        Self {
            frame_display: FrameDisplay::default_config(),
            current_output: None,
            playback_fps: None,
            last_texture_view_ptr: None,
            last_renderer_ptr: None,
            frame_width: 0,
            frame_height: 0,
        }
    }

    pub fn reset(&mut self) {
        self.current_output = None;
        self.playback_fps = None;
        self.last_texture_view_ptr = None;
        self.last_renderer_ptr = None;
        self.frame_width = 0;
        self.frame_height = 0;
    }

    /// Set the current output value to display
    pub fn set_output_value(&mut self, output: NodeValue) {
        self.current_output = Some(output);
    }

    /// Set playback FPS for display info
    pub fn set_playback_fps(&mut self, fps: f64) {
        if fps > 0.0 {
            self.playback_fps = Some(fps);
        }
    }

    /// Update the displayed frame from output value
    pub fn set_output_frame(&mut self, render_state: &egui_wgpu::RenderState, output: &NodeValue) {
        match output {
            NodeValue::Frame(gpu_frame) => {
                let texture_view_ptr = std::sync::Arc::as_ptr(&gpu_frame.view) as usize;
                let renderer_ptr = std::sync::Arc::as_ptr(&render_state.renderer) as usize;
                if self.last_texture_view_ptr == Some(texture_view_ptr)
                    && self.last_renderer_ptr == Some(renderer_ptr)
                {
                    self.frame_width = gpu_frame.size.width;
                    self.frame_height = gpu_frame.size.height;
                    return;
                }

                let frame_id = Uid::generate_new();
                self.last_texture_view_ptr = Some(texture_view_ptr);
                self.last_renderer_ptr = Some(renderer_ptr);
                self.frame_width = gpu_frame.size.width;
                self.frame_height = gpu_frame.size.height;
                let size = [self.frame_width as usize, self.frame_height as usize];
                self.frame_display.set_wgpu_texture_if_changed(
                    render_state,
                    gpu_frame.view(),
                    size,
                    frame_id,
                );
            }
            _ => {
                self.frame_display.clear(Some(render_state));
                self.last_texture_view_ptr = None;
                self.last_renderer_ptr = None;
                self.frame_width = 0;
                self.frame_height = 0;
            }
        }
    }

    /// Render the output window to a UI
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(12, 16, 18))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 47, 51)))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // Header with title
                    ui.heading("Output");
                    ui.separator();

                    // Frame display section
                    if matches!(&self.current_output, Some(NodeValue::Frame(_))) {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}x{}", self.frame_width, self.frame_height));
                            ui.separator();
                            match self.playback_fps {
                                Some(fps) => ui.label(format!("{:.1} FPS", fps)),
                                None => ui.label("-- FPS"),
                            };
                        });
                        ui.separator();

                        // Display the frame
                        egui::Frame::canvas(ui.style())
                            .inner_margin(egui::Margin::same(5))
                            .show(ui, |ui| {
                                self.frame_display.render_content(ui);
                            });
                    } else {
                        ui.label(egui::RichText::new("No output available").weak());
                    }
                });
            });
    }
}

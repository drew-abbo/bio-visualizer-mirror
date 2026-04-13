use super::output_controls::OutputControls;
use crate::components::FrameDisplay;
use engine::graph_executor::NodeValue;
use media::fps::Fps;

/// Main output window for displaying frames with native FPS tracking
pub struct OutputWindow {
    current_output: Option<NodeValue>,
    playback_fps: Option<Fps>,
    last_texture_view_ptr: Option<usize>,
    last_renderer_ptr: Option<usize>,
    frame_width: u32,
    frame_height: u32,
    frame_display: FrameDisplay,
}

impl OutputWindow {
    pub fn new() -> Self {
        Self {
            current_output: None,
            playback_fps: None,
            last_texture_view_ptr: None,
            last_renderer_ptr: None,
            frame_width: 0,
            frame_height: 0,
            frame_display: FrameDisplay::new(),
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
    pub fn set_playback_fps(&mut self, fps: Fps) {
        self.playback_fps = Some(fps);
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

                self.last_texture_view_ptr = Some(texture_view_ptr);
                self.last_renderer_ptr = Some(renderer_ptr);
                self.frame_width = gpu_frame.size.width;
                self.frame_height = gpu_frame.size.height;
                let size = [self.frame_width as usize, self.frame_height as usize];
                self.frame_display.set_wgpu_texture_if_changed(
                    render_state,
                    gpu_frame.view(),
                    size,
                    texture_view_ptr,
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
    pub fn show(&mut self, ui: &mut egui::Ui, controls: &mut OutputControls) {
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(12, 16, 18))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 47, 51)))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        controls.show(ui);
                    });
                    ui.separator();

                    if matches!(&self.current_output, Some(NodeValue::Frame(_))) {
                        if controls.show_info() {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}x{}", self.frame_width, self.frame_height));
                                ui.separator();
                                match self.playback_fps {
                                    Some(fps) => ui.label(format!("{:.1} FPS", fps.as_float())),
                                    None => ui.label("-- FPS"),
                                };
                            });
                            ui.separator();
                        }

                        // Allocate all remaining vertical space for the frame
                        let available = ui.available_size();
                        ui.allocate_ui(available, |ui| {
                            self.frame_display.render_content(ui);
                        });
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label(egui::RichText::new("No output available").weak());
                        });
                    }
                });
            });
    }
}

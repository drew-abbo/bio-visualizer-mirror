use super::output_controls::OutputControls;
use crate::components::FrameDisplay;
use engine::graph_executor::NodeValue;
use engine::node::handler::StreamLoadingStatus;
use media::fps::Fps;
use util::channels::message_channel::Inbox;
use std::time::{Duration, Instant};

/// Main output window for displaying frames with native FPS tracking
pub struct OutputWindow {
    current_output: Option<NodeValue>,
    playback_fps: Option<Fps>,
    last_texture_view_ptr: Option<usize>,
    last_renderer_ptr: Option<usize>,
    frame_width: u32,
    frame_height: u32,
    frame_display: FrameDisplay,
    loading_status: Option<String>,
    loading_status_time: Option<Instant>,
    loading_spinner_rotation: f32,
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
            loading_status: None,
            loading_status_time: None,
            loading_spinner_rotation: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.current_output = None;
        self.playback_fps = None;
        self.last_texture_view_ptr = None;
        self.last_renderer_ptr = None;
        self.frame_width = 0;
        self.frame_height = 0;
        self.loading_status = None;
        self.loading_status_time = None;
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

    pub fn render_fullscreen(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(egui::Color32::BLACK)
            .show(ui, |ui| {
                self.frame_display.render_content(ui);
            });
    }

    /// Render the output window to a UI
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        controls: &mut OutputControls,
        stream_status_inbox: Option<&Inbox<StreamLoadingStatus>>,
    ) {
        // Poll for stream loading status messages
        if let Some(inbox) = stream_status_inbox {
            while let Ok(Some(status)) = inbox.check() {
                match status {
                    StreamLoadingStatus::LoadingStarted { path } => {
                        self.loading_status = Some(format!("Loading: {}", path.display()));
                        self.loading_status_time = Some(Instant::now());
                        self.loading_spinner_rotation = 0.0;
                    }
                    StreamLoadingStatus::LoadingCompleted { path: _ } => {
                        // Clear loading status immediately on completion - don't keep showing it
                        self.loading_status = None;
                        self.loading_status_time = None;
                    }
                    StreamLoadingStatus::LoadingFailed { path, error } => {
                        self.loading_status = Some(format!("Failed to load {}: {}", path.display(), error));
                        self.loading_status_time = Some(Instant::now());
                    }
                }
            }
            
            // Auto-clear failed messages after 2 seconds
            if let Some(loading_time) = self.loading_status_time {
                if let Some(status) = &self.loading_status {
                    if status.starts_with("Failed") && loading_time.elapsed() > Duration::from_secs(2) {
                        self.loading_status = None;
                        self.loading_status_time = None;
                    }
                }
            }
            
            // Increment spinner rotation for animation
            self.loading_spinner_rotation = (self.loading_spinner_rotation + 0.05) % (2.0 * std::f32::consts::PI);
        }

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
                        let (_id, rect) = ui.allocate_space(available);
                        
                        // Draw frame display
                        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                            self.frame_display.render_content(ui);
                        });

                        // Draw loading spinner overlay if loading
                        if let Some(ref status) = self.loading_status {
                            self.render_loading_overlay(ui.painter(), rect, status);
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label(egui::RichText::new("No output available").weak());
                        });
                    }
                });
            });
    }

    /// Render a loading spinner overlay on top of the frame display
    fn render_loading_overlay(&self, painter: &egui::Painter, rect: egui::Rect, status: &str) {
        // Semi-transparent dark overlay
        painter.rect_filled(
            rect,
            0.0,
            egui::Color32::from_black_alpha(180),
        );

        // Draw spinning circle indicator
        let center = rect.center();
        let radius = 30.0;
        
        // Draw circle segments for spinner effect
        for i in 0..8 {
            let angle = (self.loading_spinner_rotation + (i as f32 * std::f32::consts::PI / 4.0)) as f32;
            let alpha = 255 - (i * 30) as u8;
            let color = egui::Color32::from_rgba_premultiplied(100, 200, 255, alpha);
            
            let start_angle = angle;
            let end_angle = angle + std::f32::consts::PI / 6.0;
            
            let start_point = egui::Pos2::new(
                center.x + radius * start_angle.cos(),
                center.y + radius * start_angle.sin(),
            );
            let end_point = egui::Pos2::new(
                center.x + radius * end_angle.cos(),
                center.y + radius * end_angle.sin(),
            );
            
            painter.line_segment([start_point, end_point], egui::Stroke::new(3.0, color));
        }

        // Draw status text below spinner
        let text_pos = egui::Pos2::new(
            center.x,
            center.y + radius + 30.0,
        );
        
        painter.text(text_pos, egui::Align2::CENTER_CENTER, status, egui::FontId::default(), egui::Color32::from_rgb(200, 200, 200));
    }
}

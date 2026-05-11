use super::output_controls::OutputControls;
use crate::components::FrameDisplay;
use engine::engine_outpost::EngineOutpostEvent;
use engine::engine_outpost::message::EngineCommand;
use engine::engine_outpost::{EngineCommandSender, EngineEventReceiver};
use engine::graph_executor::NodeValue;
use media::fps::Fps;

/// Main output window for displaying frames with native FPS tracking
pub struct OutputWindow {
    engine_tx: Option<EngineCommandSender>,
    engine_rx: Option<EngineEventReceiver>,
    current_output: Option<NodeValue>,
    playback_fps: Option<Fps>,
    last_texture_view_ptr: Option<usize>,
    last_renderer_ptr: Option<usize>,
    frame_width: u32,
    frame_height: u32,
    frame_display: FrameDisplay,
    /// Tracks whether a stream is explicitly loading (not just "no frame available")
    is_stream_loading: bool,
    /// The last manual FPS value sent to the engine, or None if auto mode is active.
    last_sent_manual_fps: Option<Fps>,
}

impl OutputWindow {
    pub fn new() -> Self {
        Self {
            engine_tx: None,
            engine_rx: None,
            current_output: None,
            playback_fps: None,
            last_texture_view_ptr: None,
            last_renderer_ptr: None,
            frame_width: 0,
            frame_height: 0,
            frame_display: FrameDisplay::new(),
            is_stream_loading: false,
            last_sent_manual_fps: None,
        }
    }

    pub fn init_engine(&mut self, tx: EngineCommandSender, rx: EngineEventReceiver) {
        self.engine_tx = Some(tx);
        self.engine_rx = Some(rx);
    }

    pub fn current_playback_fps(&self) -> Option<Fps> {
        self.playback_fps
    }

    pub fn has_frame(&self) -> bool {
        matches!(&self.current_output, Some(NodeValue::Frame(_)))
    }

    pub fn drain_engine_events(&mut self, render_state: &egui_wgpu::RenderState) {
        let Some(ref rx) = self.engine_rx else {
            return;
        };
        let events = rx.drain();

        for event in events {
            match event {
                EngineOutpostEvent::StreamsPaused => {}
                EngineOutpostEvent::StreamsPlaying => {}
                EngineOutpostEvent::GlobalStreamTargetFpsChanged(fps) => {
                    self.playback_fps = Some(fps);
                }
                EngineOutpostEvent::StreamLoading(_) => {
                    self.is_stream_loading = true;
                    self.current_output = None;
                    self.frame_display.clear(None);
                    self.last_texture_view_ptr = None;
                    self.last_renderer_ptr = None;
                    self.frame_width = 0;
                    self.frame_height = 0;
                }
                EngineOutpostEvent::InfoResponse(resp) => match resp {
                    engine::engine_outpost::message::InfoResponse::RecommendedFpsForNode(
                        _,
                        fps,
                    ) => {
                        self.playback_fps = Some(fps);
                    }
                    engine::engine_outpost::message::InfoResponse::Error(msg) => {
                        util::debug_log_warning!("Engine InfoResponse error: {msg}");
                    }
                },
                EngineOutpostEvent::FrameReady(frame) => {
                    self.is_stream_loading = false;
                    let output = NodeValue::Frame(frame);
                    self.current_output = Some(output.clone());
                    self.set_output_frame(render_state, &output);
                }
                EngineOutpostEvent::ExecutionError(_) => {
                    self.is_stream_loading = false;
                }
            }
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
                if self.is_stream_loading {
                    let available = ui.available_size();
                    ui.allocate_ui(available, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.vertical_centered(|ui| {
                                ui.add(
                                    egui::Spinner::new()
                                        .size(64.0)
                                        .color(egui::Color32::from_rgb(80, 160, 220)),
                                );
                                ui.add_space(16.0);
                                ui.label(
                                    egui::RichText::new("Loading stream...")
                                        .size(16.0)
                                        .color(egui::Color32::from_rgb(130, 155, 170)),
                                );
                            });
                        });
                    });
                } else if self.current_output.is_some() {
                    self.frame_display.render_content(ui);
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new("No output available").weak());
                    });
                }
            });
    }

    fn sync_fps_to_engine(&mut self, controls: &OutputControls) {
        let Some(ref tx) = self.engine_tx else {
            return;
        };

        if controls.manual_fps_enabled() {
            let desired = Fps::from_float(controls.manual_fps_value() as f64).ok();
            if desired != self.last_sent_manual_fps
                && let Some(fps) = desired
            {
                let _ = tx.send(EngineCommand::SetGlobalStreamTargetFps(fps));
                self.last_sent_manual_fps = Some(fps);
            }
        } else if self.last_sent_manual_fps.is_some() {
            let _ = tx.send(EngineCommand::ClearManualFps);
            self.last_sent_manual_fps = None;
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
                    self.sync_fps_to_engine(controls);
                    ui.separator();

                    if self.is_stream_loading {
                        let available = ui.available_size();
                        ui.allocate_ui(available, |ui| {
                            ui.centered_and_justified(|ui| {
                                ui.vertical_centered(|ui| {
                                    ui.add(
                                        egui::Spinner::new()
                                            .size(48.0)
                                            .color(egui::Color32::from_rgb(80, 160, 220)),
                                    );
                                    ui.add_space(12.0);
                                    ui.label(
                                        egui::RichText::new("Loading stream...")
                                            .size(14.0)
                                            .color(egui::Color32::from_rgb(130, 155, 170)),
                                    );
                                });
                            });
                        });
                    } else if matches!(&self.current_output, Some(NodeValue::Frame(_))) {
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

use super::output_controls::OutputControls;
use super::output_window::OutputWindow;
use engine::graph_executor::NodeValue;
use media::fps::Fps;

pub struct MainOutputArea {
    controls: OutputControls,
    output_window: OutputWindow,
}

impl MainOutputArea {
    pub fn new() -> Self {
        Self {
            controls: OutputControls::new(),
            output_window: OutputWindow::new(),
        }
    }

    pub fn playback_enabled(&self) -> bool {
        self.controls.playback_enabled()
    }

    pub fn preview_selected_node_enabled(&self) -> bool {
        self.controls.preview_selected_node()
    }

    pub fn fps_override(&self) -> Option<Fps> {
        self.controls.fps_override()
    }

    /// Update the output window with frame and FPS data
    pub fn update_from_editor(
        &mut self,
        frame_output: Option<&NodeValue>,
        fps_output: Option<Fps>,
        render_state: &egui_wgpu::RenderState,
    ) {
        if let Some(frame_output) = frame_output {
            // Set both the value AND the GPU frame
            self.output_window.set_output_value(frame_output.clone());
            self.output_window
                .set_output_frame(render_state, frame_output);
        } else {
            self.output_window.reset();
        }

        if let Some(fps) = fps_output {
            self.output_window.set_playback_fps(fps);
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if self.controls.fullscreen_enabled() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            *self.controls.fullscreen_enabled_mut() = false;
        }

        if self.controls.fullscreen_enabled() {
            egui::Area::new(egui::Id::new("fullscreen_output"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let screen_rect = ctx.content_rect();
                    ui.painter()
                        .rect_filled(screen_rect, 0.0, egui::Color32::BLACK);
                    ui.set_clip_rect(screen_rect);
                    ui.scope_builder(egui::UiBuilder::new().max_rect(screen_rect), |ui| {
                        self.output_window.render_fullscreen(ui);
                    });
                });

            egui::Area::new(egui::Id::new("fullscreen_hint"))
                .fixed_pos(egui::pos2(8.0, 8.0))
                .order(egui::Order::Tooltip)
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new("ESC to exit fullscreen")
                            .color(egui::Color32::from_rgba_unmultiplied(200, 200, 200, 120))
                            .small(),
                    );
                });

            return;
        }

        egui::Window::new("Output")
            .default_pos(egui::pos2(100.0, 100.0))
            .default_size(egui::vec2(520.0, 620.0))
            .min_size(egui::vec2(320.0, 280.0))
            .resizable(true)
            .collapsible(true)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(18, 22, 24))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 54, 58)))
                    .inner_margin(egui::Margin::same(10)),
            )
            .show(ctx, |ui| {
                self.output_window.show(ui, &mut self.controls);
            });
    }
}

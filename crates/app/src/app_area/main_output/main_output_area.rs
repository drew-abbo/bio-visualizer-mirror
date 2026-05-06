use super::output_controls::OutputControls;
use super::output_window::OutputWindow;
use engine::engine_outpost::{EngineCommandSender, EngineEventReceiver};
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

    pub fn init_engine(&mut self, tx: EngineCommandSender, rx: EngineEventReceiver) {
        self.output_window.init_engine(tx, rx);
    }

    // `set_engine_handle` removed — AppArea now calls `init_engine` directly.

    pub fn playback_enabled(&self) -> bool {
        self.controls.playback_enabled()
    }

    pub fn preview_selected_node_enabled(&self) -> bool {
        self.controls.preview_selected_node()
    }

    pub fn has_frame(&self) -> bool {
        self.output_window.has_frame()
    }

    pub fn current_playback_fps(&self) -> Option<Fps> {
        self.output_window.current_playback_fps()
    }

    // OutputWindow owns its event receiver; poll it during show.

    pub fn show(&mut self, ctx: &egui::Context, render_state: &egui_wgpu::RenderState) {
        // Poll engine events first — OutputWindow owns its receiver
        self.output_window.drain_engine_events(render_state);

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

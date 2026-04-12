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
        let palette = util::ui::app_palette();

        // Keep output window orchestration in the output area so AppArea only delegates.
        egui::Window::new("Output")
            .default_pos(egui::pos2(100.0, 100.0))
            .default_size(egui::vec2(520.0, 620.0))
            .min_size(egui::vec2(320.0, 280.0))
            .resizable(true)
            .collapsible(true)
            .frame(
                egui::Frame::new()
                    .fill(palette.panel)
                    .stroke(egui::Stroke::new(1.0, palette.border))
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(10)),
            )
            .show(ctx, |ui| {
                self.output_window.show(ui, &mut self.controls);
            });
    }
}

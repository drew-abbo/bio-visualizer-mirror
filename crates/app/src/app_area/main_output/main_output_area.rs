use super::output_controls::OutputControls;
use super::output_window::OutputWindow;
use engine::graph_executor::NodeValue;
use util::egui;

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

    /// Update the output window with frame and FPS data
    pub fn update_from_editor(
        &mut self,
        frame_output: Option<&NodeValue>,
        fps_output: Option<f64>,
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

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.output_window.show(ui, &mut self.controls);
    }
}

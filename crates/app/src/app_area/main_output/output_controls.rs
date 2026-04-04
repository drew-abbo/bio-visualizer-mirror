pub struct OutputControls {
    playback_enabled: bool,
    show_info: bool,
    preview_selected_node: bool,
}

impl OutputControls {
    pub fn new() -> Self {
        Self {
            playback_enabled: true,
            show_info: true,
            preview_selected_node: false,
        }
    }

    pub fn playback_enabled(&self) -> bool {
        self.playback_enabled
    }

    pub fn show_info(&self) -> bool {
        self.show_info
    }

    pub fn preview_selected_node(&self) -> bool {
        self.preview_selected_node
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let play_pause_label = if self.playback_enabled {
                "Pause"
            } else {
                "Play"
            };

            if ui.button(play_pause_label).clicked() {
                self.playback_enabled = !self.playback_enabled;
            }

            ui.separator();
            ui.checkbox(&mut self.show_info, "Info");
            ui.separator();
            ui.checkbox(&mut self.preview_selected_node, "Preview Selected Node");
        });
    }
}

impl Default for OutputControls {
    fn default() -> Self {
        Self::new()
    }
}

use media::fps::Fps;

pub struct OutputControls {
    playback_enabled: bool,
    show_info: bool,
    preview_selected_node: bool,
    manual_fps_enabled: bool,
    manual_fps_value: f32,
}

impl OutputControls {
    pub fn new() -> Self {
        Self {
            playback_enabled: true,
            show_info: true,
            preview_selected_node: false,
            manual_fps_enabled: false,
            manual_fps_value: 30.0,
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

    pub fn fps_override(&self) -> Option<Fps> {
        if !self.manual_fps_enabled {
            return None;
        }

        Fps::from_float(self.manual_fps_value as f64).ok()
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let palette = util::ui::app_palette();
        ui.horizontal(|ui| {
            let play_pause_label = if self.playback_enabled {
                "|| Pause"
            } else {
                "> Play"
            };

            if ui
                .add(
                    egui::Button::new(play_pause_label)
                        .fill(egui::Color32::from_rgb(20, 56, 72))
                        .stroke(egui::Stroke::new(1.0, palette.accent_secondary)),
                )
                .clicked()
            {
                self.playback_enabled = !self.playback_enabled;
            }

            ui.separator();
            ui.checkbox(&mut self.show_info, "Info");
            ui.separator();
            ui.checkbox(&mut self.preview_selected_node, "Preview Selected Node");
            ui.separator();
            ui.checkbox(&mut self.manual_fps_enabled, "Manual FPS");

            let fps_widget = egui::DragValue::new(&mut self.manual_fps_value)
                .range(1.0..=360.0)
                .speed(0.25)
                .suffix(" fps");

            ui.add_enabled(self.manual_fps_enabled, fps_widget);
        });
    }
}

impl Default for OutputControls {
    fn default() -> Self {
        Self::new()
    }
}

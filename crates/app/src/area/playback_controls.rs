use util::egui;

#[derive(Clone, Debug)]
pub struct PlaybackControls {
    playing: bool,
}

impl PlaybackControls {
    pub fn new() -> Self {
        Self { playing: true }
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn toggle(&mut self) {
        self.playing = !self.playing;
    }

    pub fn reset(&mut self) {
        self.playing = true;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let label = if self.playing {
                "⏸ Pause"
            } else {
                "▶ Play"
            };
            if ui.button(label).clicked() {
                self.toggle();
            }
        });
    }
}

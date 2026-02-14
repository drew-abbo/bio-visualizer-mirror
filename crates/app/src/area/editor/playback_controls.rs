use util::egui;

#[derive(Clone, Debug)]
pub struct PlaybackControls {
    playing: bool,
    sampling_rate: SamplingRate,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SamplingRate {
    Auto,
    Fixed(f64),
}

impl SamplingRate {
    pub fn resolve(&self, output_fps: f64) -> f64 {
        match self {
            SamplingRate::Auto => output_fps,
            SamplingRate::Fixed(fps) => *fps,
        }
    }
}

impl PlaybackControls {
    pub fn new() -> Self {
        Self {
            playing: true,
            sampling_rate: SamplingRate::Auto,
        }
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn toggle(&mut self) {
        self.playing = !self.playing;
    }

    pub fn sampling_rate(&self) -> &SamplingRate {
        &self.sampling_rate
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

            ui.separator();

            // Timeline sampling rate selector
<<<<<<< HEAD
            // This will probably get removed soon
=======
>>>>>>> a665ac9 (commit now so I don't screw something up)
            ui.label("Sampling Rate:");
            egui::ComboBox::from_id_salt("sampling_rate")
                .selected_text(match self.sampling_rate {
                    SamplingRate::Auto => "Auto".to_string(),
                    SamplingRate::Fixed(fps) => format!("{:.0}", fps),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.sampling_rate, SamplingRate::Auto, "Auto");
                    ui.selectable_value(&mut self.sampling_rate, SamplingRate::Fixed(24.0), "24");
                    ui.selectable_value(&mut self.sampling_rate, SamplingRate::Fixed(30.0), "30");
                    ui.selectable_value(&mut self.sampling_rate, SamplingRate::Fixed(60.0), "60");
                    ui.selectable_value(&mut self.sampling_rate, SamplingRate::Fixed(120.0), "120");
                    ui.selectable_value(&mut self.sampling_rate, SamplingRate::Fixed(240.0), "240");
                    ui.selectable_value(&mut self.sampling_rate, SamplingRate::Fixed(480.0), "480");
                    ui.selectable_value(
                        &mut self.sampling_rate,
                        SamplingRate::Fixed(1000.0),
                        "1000",
                    );
                });
        });
    }
}

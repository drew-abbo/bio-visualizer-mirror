use media::VideoPlayer;

pub struct PlaybackControls;

impl PlaybackControls {
    pub fn show(ui: &mut egui::Ui, player: &mut VideoPlayer) {
        ui.separator();

        if ui
            .button(if player.is_playing() {
                "⏸ Pause"
            } else {
                "▶ Play"
            })
            .clicked()
        {
            player.toggle_play_pause();
        }

        // Step forward button
        if ui.button("⏭ Step").clicked() {
            player.step_forward();
        }
    }
}

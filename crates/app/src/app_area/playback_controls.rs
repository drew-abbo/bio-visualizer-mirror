use std::time::Duration;

/// Controls for playback of node execution
#[derive(Clone, Debug)]
pub struct PlaybackControls {
    /// Whether playback is currently active
    pub playing: bool,

    /// Current frame number in the execution
    pub current_frame: u64,

    /// Target FPS for playback
    pub target_fps: f64,

    /// Duration per frame
    pub frame_duration: Duration,

    /// Accumulated time for frame timing
    time_accumulator: Duration,
}

impl PlaybackControls {
    pub fn new() -> Self {
        Self {
            playing: false,
            current_frame: 0,
            target_fps: 30.0,
            frame_duration: Duration::from_secs_f64(1.0 / 30.0),
            time_accumulator: Duration::ZERO,
        }
    }

    #[allow(dead_code)]
    pub fn with_fps(fps: f64) -> Self {
        Self {
            target_fps: fps,
            frame_duration: Duration::from_secs_f64(1.0 / fps),
            ..Self::new()
        }
    }

    /// Play the execution
    pub fn play(&mut self) {
        self.playing = true;
        self.time_accumulator = Duration::ZERO;
    }

    /// Pause the execution
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Toggle play/pause
    pub fn toggle(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.play();
        }
    }

    /// Step forward one frame
    pub fn step_forward(&mut self) {
        self.pause();
        self.current_frame = self.current_frame.saturating_add(1);
        self.time_accumulator = Duration::ZERO;
    }

    /// Step backward one frame
    pub fn step_backward(&mut self) {
        self.pause();
        self.current_frame = self.current_frame.saturating_sub(1);
        self.time_accumulator = Duration::ZERO;
    }

    /// Set the target FPS and update frame duration
    pub fn set_fps(&mut self, fps: f64) {
        self.target_fps = fps.max(1.0);
        self.frame_duration = Duration::from_secs_f64(1.0 / self.target_fps);
    }

    /// Reset to frame 0
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.time_accumulator = Duration::ZERO;
    }

    /// Update with delta time and return true if frame should advance
    #[allow(dead_code)]
    pub fn update_with_dt(&mut self, dt: f32) -> bool {
        if !self.playing {
            return false;
        }

        self.time_accumulator += Duration::from_secs_f32(dt);

        if self.time_accumulator >= self.frame_duration {
            self.time_accumulator -= self.frame_duration;
            self.current_frame += 1;
            true
        } else {
            false
        }
    }

    /// Render playback UI controls
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Play/Pause button
            let play_button_text = if self.playing { "⏸ Pause" } else { "▶ Play" };
            if ui.button(play_button_text).clicked() {
                self.toggle();
            }

            // Step forward
            if ui.button("⏭ Step").clicked() {
                self.step_forward();
            }

            // Step backward
            if ui.button("⏮ Back").clicked() {
                self.step_backward();
            }

            // Reset
            if ui.button("⏹ Reset").clicked() {
                self.reset();
            }

            ui.separator();

            // Frame counter
            ui.label(format!("Frame: {}", self.current_frame));
        });
    }
}

impl Default for PlaybackControls {
    fn default() -> Self {
        Self::new()
    }
}

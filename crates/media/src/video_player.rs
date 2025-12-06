use super::frame::streams::StreamStats;
use super::frame::{Frame, Producer};
use std::time::Duration;

pub struct VideoPlayer {
    producer: Producer,
    current_frame: Option<Frame>,

    // Playback state
    playing: bool,
    current_time: Duration,

    // Timing
    fps: f64,
    frame_duration: Duration,
    time_accumulator: Duration,
}

impl VideoPlayer {
    pub fn new(producer: Producer) -> Self {
        let fps = producer.stats().fps;
        let frame_duration = Duration::from_secs_f64(1.0 / fps);

        Self {
            producer,
            current_frame: None,
            playing: false,
            current_time: Duration::ZERO,
            fps,
            frame_duration,
            time_accumulator: Duration::ZERO,
        }
    }

    pub fn current_frame(&self) -> Option<&Frame> {
        self.current_frame.as_ref()
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn current_time(&self) -> Duration {
        self.current_time
    }

    pub fn fps(&self) -> f64 {
        self.fps
    }

    pub fn stats(&self) -> StreamStats {
        self.producer.stats()
    }

    pub fn play(&mut self) {
        if !self.playing {
            self.playing = true;
            self.time_accumulator = Duration::ZERO;
        }
    }

    pub fn pause(&mut self) {
        self.playing = false;
        self.time_accumulator = Duration::ZERO;
    }

    pub fn toggle_play_pause(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn step_forward(&mut self) {
        self.pause();
        self.fetch_next_frame();
    }

    pub fn step_backward(&mut self) {
        self.pause();
        // TODO: Implement if producer supports seeking backwards
    }

    pub fn seek(&mut self, time: Duration) {
        self.current_time = time;
        self.time_accumulator = Duration::ZERO;
        self.fetch_next_frame();
    }

    /// Update the player with delta time from egui
    /// Pass in ctx.input(|i| i.unstable_dt) for frame-accurate timing
    pub fn update_with_dt(&mut self, dt: f32) -> bool {
        if !self.playing {
            return false;
        }

        // Clamp incoming dt to prevent huge spikes from window events
        // (backgrounding, resizing, etc.)
        let delta = Duration::from_secs_f32(dt.min(0.1)); // Max 100ms per frame
        self.time_accumulator += delta;

        // Check if we have accumulated enough time for the next frame
        if self.time_accumulator >= self.frame_duration {
            self.time_accumulator -= self.frame_duration;
            self.current_time += self.frame_duration;
            self.fetch_next_frame();

            // If accumulator gets too large (> 3 frames), we're falling behind
            // Reset to prevent spiral of death
            // Essentially saying instead of trying to catch up, just skip ahead so we don't freeze to death
            if self.time_accumulator > self.frame_duration * 3 {
                self.time_accumulator = Duration::ZERO;
            }
            
            true
        } else {
            false
        }
    }

    fn fetch_next_frame(&mut self) {
        // Recycle old frame
        if let Some(old_frame) = self.current_frame.take() {
            self.producer.recycle_frame(old_frame);
        }

        // Fetch next frame
        match self.producer.fetch_frame() {
            Ok(frame) => {
                self.current_frame = Some(frame);
            }
            Err(e) => {
                // TODO handle this error
                eprintln!("Failed to fetch frame: {}", e);
                self.playing = false;
            }
        }
    }

    pub fn frame_duration(&self) -> Duration {
        self.frame_duration
    }
}
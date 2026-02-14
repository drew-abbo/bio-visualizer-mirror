use std::time::{Duration, Instant};

/// Manages playback timing state shared across all output displays
pub struct PlaybackState {
    last_tick: Instant,
    playback_accumulator: Duration,
    timeline_frame_index: u64,
    last_sampling_rate_hz: f64,
    last_playing: bool,
}

impl PlaybackState {
    pub fn new() -> Self {
        Self {
            last_tick: Instant::now(),
            playback_accumulator: Duration::ZERO,
            timeline_frame_index: 0,
            last_sampling_rate_hz: 30.0,
            last_playing: false,
        }
    }

    pub fn reset(&mut self) {
        self.playback_accumulator = Duration::ZERO;
        self.timeline_frame_index = 0;
        self.last_tick = Instant::now();
    }

    /// Update timing accumulator based on elapsed time
    pub fn update_tick(&mut self, is_playing: bool, sampling_rate_hz: f64) {
        // Reset accumulator when starting playback
        if is_playing && !self.last_playing {
            self.last_tick = Instant::now();
            self.playback_accumulator = Duration::ZERO;
        }

        if !is_playing {
            self.last_playing = is_playing;
            return;
        }

        // If sampling rate changed, reset timing to avoid jumps
        if (sampling_rate_hz - self.last_sampling_rate_hz).abs() > f64::EPSILON {
            self.last_tick = Instant::now();
            self.playback_accumulator = Duration::ZERO;
            self.last_sampling_rate_hz = sampling_rate_hz;
        }

        // Accumulate time
        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;
        self.playback_accumulator += dt;

        // Clamp accumulator to one frame duration to prevent huge jumps
        let frame_duration = if sampling_rate_hz > 0.0 {
            Duration::from_secs_f64(1.0 / sampling_rate_hz)
        } else {
            Duration::from_secs_f64(1.0 / 30.0)
        };

        if self.playback_accumulator > frame_duration {
            self.playback_accumulator = frame_duration;
        }

        self.last_playing = is_playing;
    }

    /// Check if we should advance to the next frame, consuming time from accumulator
    pub fn should_advance_frame(&mut self, sampling_rate_hz: f64) -> bool {
        let frame_duration = if sampling_rate_hz > 0.0 {
            Duration::from_secs_f64(1.0 / sampling_rate_hz)
        } else {
            Duration::from_secs_f64(1.0 / 30.0)
        };

        if self.playback_accumulator >= frame_duration {
            self.playback_accumulator -= frame_duration;
            self.timeline_frame_index = self.timeline_frame_index.saturating_add(1);
            true
        } else {
            false
        }
    }

    /// Calculate current timeline time in seconds
    pub fn timeline_time_secs(&self, sampling_rate_hz: f64) -> f64 {
        if sampling_rate_hz > 0.0 {
            self.timeline_frame_index as f64 / sampling_rate_hz
        } else {
            0.0
        }
    }
}

use media::frame;
use std::time::{Duration, Instant};

pub struct VideoPlayer {
    producer: frame::Producer,
    current_frame: Option<frame::Frame>,
    
    // Playback state
    playing: bool,
    current_time: Duration,
    
    // Timing
    fps: f64,
    frame_duration: Duration,
    last_update: Option<Instant>,
}

impl VideoPlayer {
    /// Create a new video player from a producer
    pub fn new(producer: frame::Producer) -> Self {
        let fps = producer.stats().fps;
        let frame_duration = Duration::from_secs_f64(1.0 / fps);
        
        Self {
            producer,
            current_frame: None,
            playing: false,
            current_time: Duration::ZERO,
            fps,
            frame_duration,
            last_update: None,
        }
    }

    /// Get the current frame (if available)
    pub fn current_frame(&self) -> Option<&frame::Frame> {
        self.current_frame.as_ref()
    }

    /// Check if the player is currently playing
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Get the current playback time
    pub fn current_time(&self) -> Duration {
        self.current_time
    }

    /// Get the video FPS
    pub fn fps(&self) -> f64 {
        self.fps
    }

    /// Get video stats
    pub fn stats(&self) -> media::frame::streams::StreamStats {
        self.producer.stats()
    }

    /// Start playing
    pub fn play(&mut self) {
        if !self.playing {
            self.playing = true;
            self.last_update = Some(Instant::now());
        }
    }

    /// Pause playback
    pub fn pause(&mut self) {
        self.playing = false;
        self.last_update = None;
    }

    /// Toggle play/pause
    pub fn toggle_play_pause(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.play();
        }
    }

    /// Step forward one frame
    pub fn step_forward(&mut self) {
        self.pause();
        self.fetch_next_frame();
    }

    /// Step backward one frame (if supported by producer)
    pub fn step_backward(&mut self) {
        self.pause();
        // TODO: Implement if producer supports seeking backwards
    }

    /// Seek to a specific time
    pub fn seek(&mut self, time: Duration) {
        self.current_time = time;
        // TODO: Tell producer to seek
        self.fetch_next_frame();
    }

    /// Update the player - call this every frame from your event loop
    /// Returns true if a new frame was fetched
    pub fn update(&mut self) -> bool {
        if !self.playing {
            return false;
        }

        let now = Instant::now();
        let elapsed = self.last_update
            .map(|last| now - last)
            .unwrap_or(Duration::ZERO);

        // Check if enough time has passed for next frame
        if elapsed >= self.frame_duration {
            self.last_update = Some(now);
            self.current_time += self.frame_duration;
            self.fetch_next_frame();
            true
        } else {
            false
        }
    }

    /// Force fetch the next frame (for manual stepping)
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
                eprintln!("Failed to fetch frame: {}", e);
                self.playing = false;
            }
        }
    }

    /// Get frame duration (for external timing if needed)
    pub fn frame_duration(&self) -> Duration {
        self.frame_duration
    }
}
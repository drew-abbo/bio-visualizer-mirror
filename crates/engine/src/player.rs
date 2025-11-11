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

//old state
// use crate::{
//     renderer::{FrameRenderer, Renderer},
// };
// use media::frame;
// use std::sync::Arc;
// use std::time::{Duration, Instant};
// use winit::window::Window;

// pub struct State {
//     producer: frame::Producer,
//     current_frame: Option<frame::Frame>,
//     renderer: Renderer,
//     window: Arc<Window>,
//     playing: bool,
    
//     // Frame rate control
//     target_frame_duration: Duration,
//     last_frame_time: Option<Instant>,
// }

// impl State {
//     pub fn new(
//         producer: frame::Producer,
//         window: std::sync::Arc<winit::window::Window>,
//     ) -> anyhow::Result<Self> {
//         // Get the video's FPS from stream stats
//         let fps = producer.stats().fps;
//         let target_frame_duration = Duration::from_secs_f64(1.0 / fps);
        
//         Ok(Self {
//             producer,
//             current_frame: None,
//             renderer: Renderer::new(window.clone())?,
//             window,
//             playing: true,
//             target_frame_duration,
//             last_frame_time: None,
//         })
//     }

//     pub fn on_resize(&mut self, w: u32, h: u32) {
//         self.renderer.resize(w, h);
//     }

//     pub fn frame(&mut self) {
//         if !self.playing {
//             // When paused, just re-render the current frame
//             if let Some(frame) = &self.current_frame {
//                 self.renderer.render_frame(frame);
//             }
//             return;
//         }

//         // Check if enough time has passed for the next frame
//         // this is just a simple approach, more advanced timing could be done soon (still testing)
//         let now = Instant::now();
//         if let Some(last_time) = self.last_frame_time {
//             let elapsed = now - last_time;
//             if elapsed < self.target_frame_duration {
//                 // Not time yet - request another redraw and wait
//                 self.window.request_redraw();
//                 return;
//             }
//         }

//         // Time to fetch and render the next frame (just testing fps control)
//         self.fetch_and_render_frame();
//         self.last_frame_time = Some(now);
//     }

//     fn fetch_and_render_frame(&mut self) {
//         // Recycle old frame
//         if let Some(old_frame) = self.current_frame.take() {
//             self.producer.recycle_frame(old_frame);
//         }

//         // Fetch next frame
//         match self.producer.fetch_frame() {
//             Ok(frame) => {
//                 self.renderer.render_frame(&frame);
//                 self.current_frame = Some(frame);
//                 self.window.request_redraw();
//             }
//             Err(e) => {
//                 eprintln!("Failed to fetch frame: {}", e);
//                 self.playing = false;
//             }
//         }
//     }

//     pub fn force_frame(&mut self) {
//         // For manual stepping, ignore frame timing
//         self.fetch_and_render_frame();
//         self.last_frame_time = Some(Instant::now());
//     }

//     pub fn handle_key(
//         &mut self,
//         event_loop: &winit::event_loop::ActiveEventLoop,
//         key: winit::keyboard::KeyCode,
//         pressed: bool,
//     ) {
//         if !pressed {
//             return;
//         }

//         match key {
//             winit::keyboard::KeyCode::Escape => {
//                 event_loop.exit();
//             }
//             winit::keyboard::KeyCode::Space => {
//                 self.playing = !self.playing;
//                 if self.playing {
//                     // Reset timing when resuming playback
//                     self.last_frame_time = Some(Instant::now());
//                     self.window.request_redraw();
//                 }
//             }
//             winit::keyboard::KeyCode::ArrowRight => {
//                 self.playing = false;
//                 self.force_frame();
//             }
//             winit::keyboard::KeyCode::ArrowLeft => {
//                 self.playing = false;
//                 self.force_frame();
//             }
//             _ => {}
//         }
//     }
// }
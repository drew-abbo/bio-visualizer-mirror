// state.rs
use crate::{
    renderer::{FrameRenderer, Renderer},
};
use media::frame;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::window::Window;

pub struct State {
    producer: frame::Producer,
    current_frame: Option<frame::Frame>,
    renderer: Renderer,
    window: Arc<Window>,
    playing: bool,
    
    // Frame rate control
    target_frame_duration: Duration,
    last_frame_time: Option<Instant>,
}

impl State {
    pub fn new(
        producer: frame::Producer,
        window: std::sync::Arc<winit::window::Window>,
    ) -> anyhow::Result<Self> {
        // Get the video's FPS from stream stats
        let fps = producer.stats().fps;
        let target_frame_duration = Duration::from_secs_f64(1.0 / fps);
        
        println!("Video FPS: {}, Frame duration: {:?}", fps, target_frame_duration);
        
        Ok(Self {
            producer,
            current_frame: None,
            renderer: Renderer::new(window.clone())?,
            window,
            playing: true,
            target_frame_duration,
            last_frame_time: None,
        })
    }

    pub fn on_resize(&mut self, w: u32, h: u32) {
        self.renderer.resize(w, h);
    }

    pub fn frame(&mut self) {
        if !self.playing {
            // When paused, just re-render the current frame
            if let Some(frame) = &self.current_frame {
                self.renderer.render_frame(frame);
            }
            return;
        }

        // Check if enough time has passed for the next frame
        let now = Instant::now();
        if let Some(last_time) = self.last_frame_time {
            let elapsed = now - last_time;
            if elapsed < self.target_frame_duration {
                // Not time yet - request another redraw and wait
                self.window.request_redraw();
                return;
            }
        }

        // Time to fetch and render the next frame
        self.fetch_and_render_frame();
        self.last_frame_time = Some(now);
    }

    fn fetch_and_render_frame(&mut self) {
        // Recycle old frame
        if let Some(old_frame) = self.current_frame.take() {
            self.producer.recycle_frame(old_frame);
        }

        // Fetch next frame
        match self.producer.fetch_frame() {
            Ok(frame) => {
                self.renderer.render_frame(&frame);
                self.current_frame = Some(frame);
                self.window.request_redraw();
            }
            Err(e) => {
                eprintln!("Failed to fetch frame: {}", e);
                self.playing = false;
            }
        }
    }

    pub fn force_frame(&mut self) {
        // For manual stepping, ignore frame timing
        self.fetch_and_render_frame();
        self.last_frame_time = Some(Instant::now());
    }

    pub fn handle_key(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        key: winit::keyboard::KeyCode,
        pressed: bool,
    ) {
        if !pressed {
            return;
        }

        match key {
            winit::keyboard::KeyCode::Escape => {
                event_loop.exit();
            }
            winit::keyboard::KeyCode::Space => {
                self.playing = !self.playing;
                if self.playing {
                    // Reset timing when resuming playback
                    self.last_frame_time = Some(Instant::now());
                    self.window.request_redraw();
                }
            }
            winit::keyboard::KeyCode::ArrowRight => {
                self.playing = false;
                self.force_frame();
            }
            winit::keyboard::KeyCode::ArrowLeft => {
                self.playing = false;
                self.force_frame();
            }
            _ => {}
        }
    }
}
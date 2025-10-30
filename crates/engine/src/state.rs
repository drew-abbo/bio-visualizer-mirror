use crate::{
    renderer::{FrameRenderer, Renderer},
};
use std::sync::Arc;
use media::frame;
use winit::window::Window;

pub struct State {
    producer: frame::Producer,
    current_frame: Option<frame::Frame>, // Store the frame so we can recycle it
    renderer: Renderer,
    window: Arc<Window>,
}

impl State {
    pub fn new(
        producer: frame::Producer,
        window: std::sync::Arc<winit::window::Window>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            producer,
            current_frame: None,
            renderer: Renderer::new(window.clone())?,
            window,
        })
    }

    pub fn on_resize(&mut self, w: u32, h: u32) {
        self.renderer.resize(w, h);
    }

    pub fn frame(&mut self) {
        // CRITICAL: Recycle the old frame before fetching a new one
        if let Some(old_frame) = self.current_frame.take() {
            self.producer.recycle_frame(old_frame);
        }

        // Fetch the next frame
        match self.producer.fetch_frame() {
            Ok(frame) => {
                // Render it (pass by reference!)
                self.renderer.render_frame(&frame);
                
                // Store it so we can recycle it next time
                self.current_frame = Some(frame);
                
                // Keep the loop going
                self.window.request_redraw();
            }
            Err(e) => {
                log::error!("Failed to fetch frame: {}", e);
                // Could add pause logic here
            }
        }
    }

    pub fn handle_key(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, 
                      key: winit::keyboard::KeyCode, pressed: bool) {
        if !pressed { return; }
        
        match key {
            winit::keyboard::KeyCode::Escape => {
                event_loop.exit();
            }
            _ => {}
        }
    }
}
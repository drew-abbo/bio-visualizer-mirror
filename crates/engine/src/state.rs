use crate::{
    renderer::{FrameRenderer, Renderer},
    frame_handler::FrameHandler,
};
use std::sync::Arc;
use media::frame;
use winit::window::Window;

pub struct State {
    frame_handler: FrameHandler,
    renderer: Renderer,
    window: Arc<Window>,
}

impl State {
    pub fn new(
        frame_handler: FrameHandler,
        window: std::sync::Arc<winit::window::Window>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            frame_handler,
            renderer: Renderer::new(window.clone())?,
            window,
        })
    }

    pub fn on_resize(&mut self, w: u32, h: u32) {
        self.renderer.resize(w, h);
    }

    // Called every winit redraw.
    pub fn frame(&mut self) {

        let some_frame = self.frame_handler.something();

        

        // self.render_inbox.drain(&mut self.store, &mut self.playback);
        // self.playback.tick(&self.store);

        // if let Some(idx) = self.playback.current_index() {
        //     if let Some(f) = self.store.get(idx) {
        //         log::info!("Rendering frame {} - First pixel: [{}, {}, {}, {}]", 
        //                   idx, f.pixels[0], f.pixels[1], f.pixels[2], f.pixels[3]);
        //         self.renderer.render_rgba(&f);
        //     } else {
        //         log::warn!("Playback index {} not found in store", idx);
        //     }
        // } else {
        //     log::warn!("No current playback index");
        // }
        
        // // Request another frame if playing
        // if self.playback.is_playing() {
        //     self.window.request_redraw();
        // }
    }

    // UI controls
    // pub fn play(&mut self) {
    //     self.playback.set_playing(true, &self.store);
    //     self.window.request_redraw(); // Trigger rendering!
    // }
    // pub fn pause(&mut self) {
    //     self.playback.set_playing(false, &self.store);
    // }
    // pub fn step_fwd(&mut self) {
    //     self.playback.step_fwd(&self.store);
    //     self.window.request_redraw(); // Show the new frame!
    // }
    // pub fn step_back(&mut self) {
    //     self.playback.step_back();
    //     self.window.request_redraw(); // Show the new frame!
    // }
    pub fn handle_key(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, key: winit::keyboard::KeyCode, pressed: bool) {
        if !pressed {
            return;
        }
        // match key {
        //     winit::keyboard::KeyCode::Space => {
        //         if self.playback.is_playing() {
        //             self.pause();
        //         } else {
        //             self.play();
        //         }
        //     }
        //     winit::keyboard::KeyCode::ArrowRight => {
        //         self.step_fwd();
        //     }
        //     winit::keyboard::KeyCode::ArrowLeft => {
        //         self.step_back();
        //     }
        //     winit::keyboard::KeyCode::Escape => {
        //         _event_loop.exit();
        //     }
        //     _ => {}
        // }
    }
}
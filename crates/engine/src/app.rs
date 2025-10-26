use std::sync::Arc;

use super::state::State;
use super::video::RgbaFrame;
use util::channels::message_channel::Inbox;
use winit::{
    application::ApplicationHandler, event::*, event_loop::ActiveEventLoop, keyboard::PhysicalKey,
    window::WindowAttributes,
};

pub struct App {
    state: Option<State>,
    receiver: Inbox<RgbaFrame>,
}

impl App {
    pub fn new(receiver: util::channels::message_channel::Inbox<RgbaFrame>) -> Self {
        Self {
            state: None,
            receiver,
        }
    }
    fn pump_frames(&mut self) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        // Drain everything available right now; keep only the newest.
        let mut last: Option<RgbaFrame> = None;
        loop {
            match self.receiver.check() {
                Ok(Some(f)) => last = Some(f),
                Ok(None) => break,
                Err(_) => break, // producer gone—fine
                Err(e) => {
                    log::warn!("frame inbox error: {e:?}");
                    break;
                }
            }
        }
        if let Some(f) = last.as_ref() {
            state.submit_rgba_frame(f);
        }
    }
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .expect("failed to create window"),
        );
        self.state = Some(pollster::block_on(State::new(window)).expect("State::new failed"));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                let Some(state) = self.state.as_mut() else {
                    return;
                };
                state.resize(size.width, size.height);
            }
            WindowEvent::RedrawRequested => {
                // accept any frames produced since last tick
                self.pump_frames();

                let Some(state) = self.state.as_mut() else {
                    return;
                };

                // Optional per-frame logic
                state.update();

                match state.render() {
                    Ok(()) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        let size = state.window.inner_size();
                        state.resize(size.width, size.height);
                    }
                    Err(e) => log::error!("render error: {e}"),
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => {
                let Some(state) = self.state.as_mut() else {
                    return;
                };
                state.handle_key(event_loop, code, key_state.is_pressed())
            }
            _ => {}
        }
    }
}

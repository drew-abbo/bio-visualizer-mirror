use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::ActiveEventLoop,
    keyboard::PhysicalKey,
    window::WindowAttributes,
};

use crate::{frame_handler, state::State};

pub struct App {
    state: Option<State>,
    producer: media::frame::Producer,
}

impl App {
    pub fn new(producer: media::frame::Producer) -> Self {
        Self {
            state: None,
            producer,
        }
    }
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Only initialize once; guard against spurious resume events
        if self.state.is_some() { return; }

        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .expect("failed to create window"),
        );

        // let inbox = self
        //     .receiver
        //     .take()
        //     .expect("App::resumed called twice before State init (inbox already moved)");

        // create a frame handler, this will handle things between the state and the producer.

        let frame_handler = frame_handler::FrameHandler::new(self.producer);

        self.state = Some(
            State::new(frame_handler, window)
                .expect("failed to initialize State"),
        );
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
                if let Some(state) = self.state.as_mut() {
                    state.on_resize(size.width, size.height);
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(state) = self.state.as_mut() {
                    state.frame(); // handles inbox drain + render
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

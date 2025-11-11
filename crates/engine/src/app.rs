// use std::sync::Arc;
// use winit::{
//     application::ApplicationHandler, event::*, event_loop::ActiveEventLoop, keyboard::PhysicalKey,
//     window::WindowAttributes,
// };

// use crate::state::State;

// pub struct App {
//     state: Option<State>,
//     producer: Option<media::frame::Producer>,
// }

// impl App {
//     pub fn new(producer: media::frame::Producer) -> Self {
//         Self {
//             state: None,
//             producer: Some(producer),
//         }
//     }
// }

// impl ApplicationHandler<()> for App {
//     fn resumed(&mut self, event_loop: &ActiveEventLoop) {
//         // Only initialize once; guard against spurious resume events
//         if self.state.is_some() {
//             return;
//         }

//         let window = Arc::new(
//             event_loop
//                 .create_window(WindowAttributes::default())
//                 .expect("failed to create window"),
//         );

//         let producer = self
//             .producer
//             .take()
//             .expect("App::resumed called twice before State init");

//         self.state = Some(State::new(producer, window).expect("failed to initialize State"));
//     }

//     fn window_event(
//         &mut self,
//         event_loop: &ActiveEventLoop,
//         _id: winit::window::WindowId,
//         event: WindowEvent,
//     ) {
//         match event {
//             WindowEvent::CloseRequested => event_loop.exit(),

//             WindowEvent::Resized(size) => {
//                 if let Some(state) = self.state.as_mut() {
//                     state.on_resize(size.width, size.height);
//                 }
//             }

//             WindowEvent::RedrawRequested => {
//                 if let Some(state) = self.state.as_mut() {
//                     state.frame(); // handles inbox drain + render
//                 }
//             }

//             WindowEvent::KeyboardInput {
//                 event:
//                     KeyEvent {
//                         physical_key: PhysicalKey::Code(code),
//                         state: key_state,
//                         ..
//                     },
//                 ..
//             } => {
//                 let Some(state) = self.state.as_mut() else {
//                     return;
//                 };
//                 state.handle_key(event_loop, code, key_state.is_pressed())
//             }
//             _ => {}
//         }
//     }
// }

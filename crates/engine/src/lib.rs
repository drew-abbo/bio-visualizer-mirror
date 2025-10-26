mod app;
mod state;
pub mod video;

pub use app::App;
pub use state::State;

pub fn run(receiver: util::channels::message_channel::Inbox<crate::video::RgbaFrame>) -> anyhow::Result<()> {
    use winit::{event_loop::EventLoop, application::ApplicationHandler};
    let event_loop = EventLoop::new()?;
    let mut app = crate::app::App::new(receiver);
    event_loop.run_app(&mut app)?;
    Ok(())
}
use winit::event_loop::EventLoop;

mod app;
mod state;
pub mod video;
pub use app::App;
pub use state::State;

pub fn run(receiver: util::channels::message_channel::Inbox<crate::video::RgbaFrame>) -> anyhow::Result<()> {
    // Not sure if this will affect the front end usage at all. Might need to be integrated there.
    let event_loop = EventLoop::new()?;
    let mut app = crate::app::App::new(receiver);
    event_loop.run_app(&mut app)?;
    Ok(())
}
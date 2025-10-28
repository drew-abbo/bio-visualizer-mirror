use winit::event_loop::EventLoop;

mod app;
mod state;
mod render_inbox;
mod frame_store;
mod playback;
mod renderer;
pub mod types;
pub use state::State;
pub use app::App;

pub fn run(receiver: util::channels::message_channel::Inbox<types::RgbaFrame>) -> anyhow::Result<()> {
    // Not sure if this will affect the front end usage at all. Might need to be integrated there.
    let event_loop = EventLoop::new()?;
    let mut app = crate::app::App::new(receiver);
    event_loop.run_app(&mut app)?;
    Ok(())
}
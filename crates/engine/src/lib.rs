use winit::event_loop::EventLoop;

mod app;
mod state;
mod renderer;
pub mod types;
pub use state::State;
pub use app::App;

pub fn run(producer: media::frame::Producer) -> anyhow::Result<()> {
    // Not sure if this will affect the front end usage at all. Might need to be integrated there.
    let event_loop = EventLoop::new()?;
    let mut app = crate::app::App::new(producer);
    event_loop.run_app(&mut app)?;
    Ok(())
}
mod app;
mod state;
mod renderer;
mod types;
pub use state::State;
pub use app::App;

pub fn run(producer: media::frame::Producer) -> anyhow::Result<()> {
    // Build an event loop that’s ready for ApplicationHandler
    let event_loop = winit::event_loop::EventLoop::with_user_event().build()?;

    let mut app = crate::app::App::new(producer);
    event_loop.run_app(&mut app)?;
    Ok(())
}
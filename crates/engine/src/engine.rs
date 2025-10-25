pub mod app;
pub mod state;

use self::app::App;

pub fn run() -> anyhow::Result<()> {

    let event_loop = winit::event_loop::EventLoop::new()?; 
    let mut app = App::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

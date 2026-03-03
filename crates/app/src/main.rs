mod app_area;
mod args;
mod components;
mod launcher_comm;
mod windows_resize;

use app_area::AppArea;
use clap::Parser;
use util::version;
const APP_NAME: &str = version::APP_NAME;

fn main() -> Result<(), util::eframe::Error> {
    env_logger::init();

    let args = args::Args::parse();

    // Configure the native window with custom title bar
    let viewport = util::egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_decorations(false)
        .with_resizable(true)
        .with_inner_size([1280.0, 720.0])
        .with_min_inner_size([800.0, 600.0])
        .with_maximized(false);

    let native_options = util::eframe::NativeOptions {
        viewport,
        persist_window: true,
        centered: true,
        ..Default::default()
    };

    util::debug_log_info!("Starting app with resizable window");

    util::eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(|cc| {
            // Setup Windows-specific borderless resize after window creation
            windows_resize::setup_borderless_resize(cc);
            Ok(Box::new(AppArea::new(cc, args.clone())))
        }),
    )
}

#![cfg_attr(all(windows, feature = "no-console"), windows_subsystem = "windows")]

mod app_area;
mod args;
mod components;
mod launcher_comm;
mod windows_resize;

use app_area::AppArea;
use clap::Parser;
const APP_NAME: &str = util::version::APP_NAME;

fn main() -> Result<(), eframe::Error> {
    util::crash_reporting::init();

    let args = args::Args::parse();

    #[cfg(debug_assertions)]
    {
        use util::debug_log;
        if args.no_debug_logging {
            debug_log::disable();
        } else if !args.debug_error_log_panics {
            debug_log::panic_on_errors::disable();
        }
    }

    // Configure the native window with custom title bar.
    // with_fullscreen is only used on Windows — on Linux/macOS it causes actual
    // exclusive fullscreen (hides taskbar). On all platforms, app_area sends
    // ViewportCommand::Maximized(true) on the first frame to start maximized.
    let viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_decorations(false)
        .with_resizable(true)
        .with_inner_size([1280.0, 720.0])
        .with_min_inner_size([800.0, 600.0]);

    #[cfg(target_os = "windows")]
    let viewport = viewport.with_fullscreen(true);

    let native_options = eframe::NativeOptions {
        viewport,
        // Native window persistence can restore stale minimized/tiny sizes on
        // some platforms; keep this off so startup min-size constraints win.
        persist_window: false,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(|cc| {
            // Setup Windows-specific borderless resize after window creation
            windows_resize::setup_borderless_resize(cc);
            Ok(Box::new(AppArea::new(cc, args.clone())))
        }),
    )
}

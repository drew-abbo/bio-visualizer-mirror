//! Exports [editor] which runs the editor portion of the app.

mod app_area;
mod args;
mod components;
mod launcher_comm;
mod windows_resize;

use std::process::ExitCode;

use util::version;

use app_area::AppArea;
use args::Args;

/// Runs the editor portion of the app.
pub fn editor() -> ExitCode {
    util::crash_reporting::init();

    let args = Args::default();

    #[cfg(debug_assertions)]
    {
        use util::debug_log;
        if args.no_debug_logging {
            debug_log::disable();
        } else if !args.debug_error_log_panics {
            debug_log::panic_on_errors::disable();
        }
    }

    // TODO: Enable stop signal polling and handle stop signals gracefully.

    if let Some(version_outfile) = args.version {
        return match version::print(version_outfile) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                util::debug_log_error!("Failed to print version: {e}");
                ExitCode::FAILURE
            }
        };
    }

    // Configure the native window with custom title bar
    let viewport = egui::ViewportBuilder::default()
        .with_icon(util::ui::load_app_icon())
        .with_title(version::APP_NAME)
        .with_decorations(false)
        .with_resizable(true)
        .with_inner_size([1280.0, 720.0])
        .with_min_inner_size([800.0, 600.0])
        .with_fullscreen(true);

    let native_options = eframe::NativeOptions {
        viewport,
        // Native window persistence can restore stale minimized/tiny sizes on
        // some platforms; keep this off so startup min-size constraints win.
        persist_window: false,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        version::APP_NAME,
        native_options,
        Box::new(|cc| {
            // Setup Windows-specific borderless resize after window creation
            windows_resize::setup_borderless_resize(cc);
            Ok(Box::new(AppArea::new(cc, args.clone())))
        }),
    )
    .map_or_else(
        |e| {
            util::debug_log_error!("UI (run native) failed: {e}");
            ExitCode::FAILURE
        },
        |_| ExitCode::SUCCESS,
    )
}

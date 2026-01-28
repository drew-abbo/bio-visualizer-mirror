mod app;
mod areas;
mod components;
mod video;
mod view;
use app::App;

fn main() -> Result<(), eframe::Error> {
    // Initialize logger
    env_logger::init();

    // Configure the native window
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([60.0, 40.0])
            .with_title("Bio Visualizer"),
        ..Default::default()
    };

    // Run the app
    eframe::run_native(
        "Bio Visualizer",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

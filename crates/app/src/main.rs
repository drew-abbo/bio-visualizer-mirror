pub mod area;
mod components;
mod view;
use area::App;

fn main() -> Result<(), util::eframe::Error> {
    // Initialize logger
    env_logger::init();

    // Configure the native window
    let native_options = util::eframe::NativeOptions {
        viewport: util::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([60.0, 40.0])
            .with_title("Bio Visualizer")
            .with_decorations(false),
        ..Default::default()
    };

    // Run the app
    util::eframe::run_native(
        "Bio Visualizer",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

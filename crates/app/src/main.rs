mod app_area;
mod components;

use app_area::AppArea;

fn main() -> Result<(), util::eframe::Error> {
    env_logger::init();

    // Configure the native window
    // TODO
    // I think here we are going to want to import what is in the users settings
    // This will make sure that when they restart the app, it will open with the same window size and position as before
    let native_options = util::eframe::NativeOptions {
        viewport: util::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([60.0, 40.0])
            .with_title("Bio Visualizer")
            .with_decorations(false),
        ..Default::default()
    };

    util::eframe::run_native(
        "Bio Visualizer",
        native_options,
        Box::new(|cc| Ok(Box::new(AppArea::new(cc)))),
    )
}
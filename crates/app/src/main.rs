mod app_area;
mod components;
<<<<<<< HEAD
<<<<<<< HEAD
<<<<<<< HEAD
=======
mod engine_controller;
>>>>>>> 59a6b68 (started adding some basic node and video stuff)
=======
>>>>>>> dc5fe4f (I have a working UI finally)
mod view;
<<<<<<< HEAD
<<<<<<< HEAD
mod areas;
use app::App;
=======
use app_area::App;
>>>>>>> 2069524 (trying to get the UI looking right)
=======
=======
>>>>>>> a665ac9 (commit now so I don't screw something up)
use area::App;
>>>>>>> e361ed9 (re doing some things and make the values in the engine be used for input and output)
=======
=======

>>>>>>> cc1a573 (I think this is very close to being ready)
use app_area::AppArea;
use util::version::APP_NAME;

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
            .with_title(APP_NAME)
            .with_decorations(false),
        ..Default::default()
    };

    util::eframe::run_native(
        "Bio Visualizer",
        native_options,
        Box::new(|cc| Ok(Box::new(AppArea::new(cc)))),
    )
}
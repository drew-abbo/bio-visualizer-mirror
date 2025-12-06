use crate::main_window::BioVisualizerMainWindow;

pub fn start_ui() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_maximized(true)
            .with_title("Bio Visualizer UI"),
        ..Default::default()
    };

    eframe::run_native(
        "Bio Visualizer",
        native_options,
        Box::new(|cc| Ok(Box::new(BioVisualizerMainWindow::new(cc)))),
    )
}

mod editor;
mod title_bar;
mod app_save_context;

use app_save_context::AppSaveContext;
use editor::EditorArea;
use util::eframe;
use util::egui;

/// This is the main area of the app.
/// Anything you add to this please make sure it is contained within an _area file
/// The app struct should handle as little logic as possible, and should just be responsible for rendering the different areas of the app and passing data between them
pub struct AppArea {
    title_bar: title_bar::TitleBarArea,
    editor_area: EditorArea,
    app_save_context: AppSaveContext,
}

impl AppArea {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        Self {
            title_bar: title_bar::TitleBarArea::new(),
            editor_area: EditorArea::new(),
            app_save_context: AppSaveContext::new(),
        }
    }

    fn show_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu")
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(24, 29, 31))
                    .inner_margin(egui::Margin::symmetric(12, 6)),
            )
            .show(ctx, |ui| {
                self.title_bar.ui(ui);
            });
    }
}

impl eframe::App for AppArea {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.show_top_bar(ctx);
        self.editor_area.show(ctx, frame);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        println!("Saving app state...");
    }
}

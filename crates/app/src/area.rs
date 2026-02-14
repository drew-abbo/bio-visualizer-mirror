mod editor;
mod title_bar;
use editor::EditorArea;

use engine::node::NodeLibrary;
use std::sync::Arc;
use util::eframe;
use util::egui;

pub struct App {
    title_bar: title_bar::TitleBar,
    editor_area: EditorArea,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        // Load node library
        let node_library = if cfg!(debug_assertions) {
            let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
            let nodes_path = workspace_root.join("Nodes");
            match NodeLibrary::load_from_disk(nodes_path.clone()) {
                Ok(lib) => lib,
                Err(err) => {
                    util::debug_log_error!(
                        "Failed to load node library from disk at {:?}: {}",
                        nodes_path,
                        err
                    );
                    NodeLibrary::default()
                }
            }
        } else {
            match NodeLibrary::load_from_users_folder() {
                Ok(lib) => lib,
                Err(err) => {
                    util::debug_log_error!(
                        "Failed to load node library from users folder: {}",
                        err
                    );
                    NodeLibrary::default()
                }
            }
        };

        let node_library = Arc::new(node_library);

        Self {
            title_bar: title_bar::TitleBar::new(),
            editor_area: EditorArea::new(node_library),
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

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Render minimal top-level UI
        self.show_top_bar(ctx);

        // Editor area handles all its own logic
        self.editor_area.show(ctx, frame);
    }
}

use crate::app_area::menu_bar::{MenuAction, MenuBarButton};

pub struct LoadVideoFile;

impl MenuBarButton for LoadVideoFile {
    fn label(&self) -> &str {
        "Import"
    }

    fn on_click(&mut self, _ctx: &egui::Context) -> Option<MenuAction> {
        let file = rfd::FileDialog::new()
            .add_filter("Video", &["mp4", "mov", "mkv"])
            .pick_file()?;

        Some(MenuAction::ImportVideo(file))
    }
}

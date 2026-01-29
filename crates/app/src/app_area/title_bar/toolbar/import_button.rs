use crate::app_area::title_bar::toolbar::Command;
use crate::app_area::title_bar::toolbar::tool_bar_button::ToolBarButton;

pub struct LoadVideoFile;

impl ToolBarButton for LoadVideoFile {
    fn label(&self) -> &str {
        "Import"
    }

    fn on_click(&mut self, _ctx: &egui::Context) -> Option<Command> {
        let file = rfd::FileDialog::new()
            .add_filter("Video", &["mp4", "mov", "mkv"])
            .pick_file()?;

        Some(Command::ImportVideo(file))
    }
}

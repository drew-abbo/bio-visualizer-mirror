use crate::area::title_bar::toolbar::Command;
use crate::area::title_bar::toolbar::tool_bar_button::ToolBarButton;
use util::egui::Context;

pub struct LoadVideoFile;

impl ToolBarButton for LoadVideoFile {
    fn label(&self) -> &str {
        "Import"
    }

    fn on_click(&mut self, _ctx: &Context) -> Option<Command> {
        let file = rfd::FileDialog::new()
            .add_filter("Video", &["mp4", "mov", "mkv"])
            .pick_file()?;

        Some(Command::ImportVideo(file))
    }
}

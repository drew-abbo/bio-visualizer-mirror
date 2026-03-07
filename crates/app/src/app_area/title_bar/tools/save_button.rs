use crate::app_area::title_bar::tools::tool_bar_button::ToolBarButton;
use util::egui::Context;
use super::command::Command;

pub struct SaveButton;

impl ToolBarButton for SaveButton {
    fn label(&self) -> &str {
        "Save"
    }

    fn on_click(&mut self, _ctx: &Context) -> Option<Command> {
        Command::SaveProject.into()
    }
}
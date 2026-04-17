use super::command::Command;
use crate::app_area::title_bar::tools::toolbar_button::ToolBarButton;
use egui::Context;

pub struct SaveButton;

impl ToolBarButton for SaveButton {
    fn label(&self) -> &str {
        "Save"
    }

    fn on_click(&mut self, _ctx: &Context) -> Option<Command> {
        Command::SaveProject.into()
    }
}

use super::command::Command;
use egui::Context;

pub trait ToolBarButton {
    fn label(&self) -> &str;
    fn on_click(&mut self, ctx: &Context) -> Option<Command>;
}

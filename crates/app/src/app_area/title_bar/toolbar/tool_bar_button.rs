use crate::app_area::title_bar::toolbar::Command;

pub trait ToolBarButton {
    fn label(&self) -> &str;
    fn on_click(&mut self, ctx: &egui::Context) -> Option<Command>;
}

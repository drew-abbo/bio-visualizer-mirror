pub trait MenuBarButton {
    fn label(&self) -> &str;
    fn on_click(&mut self, ctx: &egui::Context) -> Option<MenuAction>;
}

pub enum MenuAction {
    ImportVideo(std::path::PathBuf),
}
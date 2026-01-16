use crate::view::View;
mod import_button;

pub trait MenuBarButton {
    fn label(&self) -> &str;
    fn on_click(&mut self, ctx: &egui::Context) -> Option<MenuAction>;
}

pub enum MenuAction {
    ImportVideo(std::path::PathBuf),
}

pub struct MenuBar {
    file_buttons: Vec<Box<dyn MenuBarButton>>,
    pending: Vec<MenuAction>,
}

impl MenuBar {
    pub fn new() -> Self {
        Self {
            file_buttons: vec![Box::new(import_button::LoadVideoFile)],
            pending: Vec::new(),
        }
    }

    pub fn drain_actions(&mut self) -> Vec<MenuAction> {
        self.pending.drain(..).collect()
    }
}

impl View for MenuBar {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let old = ui.visuals().override_text_color;

        egui::MenuBar::new().ui(ui, |ui| {
            ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);

            ui.menu_button("File", |ui| {
                for button in &mut self.file_buttons {
                    if ui.button(button.label()).clicked() {
                        if let Some(action) = button.on_click(ui.ctx()) {
                            self.pending.push(action);
                        }
                    }
                }
            });
        });

        ui.visuals_mut().override_text_color = old;
    }
}

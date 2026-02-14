use crate::area::title_bar::toolbar::tool_bar_button::ToolBarButton;
pub mod command;
mod import_button;
mod tool_bar_button;
pub use crate::area::title_bar::toolbar::command::Command;
use util::egui;

pub struct ToolBar {
    file_buttons: Vec<Box<dyn ToolBarButton>>,
    pending: Vec<Command>,
}

impl ToolBar {
    pub fn new() -> Self {
        Self {
            file_buttons: vec![Box::new(import_button::LoadVideoFile)],
            pending: Vec::new(),
        }
    }
}

impl ToolBar {
    fn add_pending(&mut self, command: Command) {
        self.pending.push(command);
    }
}

impl ToolBar {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Add vertical centering to match the window controls
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                // File menu with dropdown - larger text
                ui.menu_button(egui::RichText::new("File").size(16.0), |ui| {
                    for button in &mut self.file_buttons {
                        if ui.button(button.label()).clicked() {
                            if let Some(action) = button.on_click(ui.ctx()) {
                                self.pending.push(action);
                            }
                        }
                    }
                });
            });
        });
    }
}

use util::egui;

use super::command::Command;
use super::import_button::LoadVideoFile;
use super::tool_bar_button::ToolBarButton;

pub struct ToolBar {
    file_buttons: Vec<Box<dyn ToolBarButton>>,
    pending: Vec<Command>,
}

impl ToolBar {
    pub fn new() -> Self {
        Self {
            file_buttons: vec![Box::new(LoadVideoFile)],
            pending: Vec::new(),
        }
    }
}

impl ToolBar {
    #[allow(dead_code)]
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
                ui.menu_button(
                    egui::RichText::new("File")
                        .size(16.0)
                        .color(egui::Color32::from_rgb(0xEB, 0x0C, 0xB7)),
                    |ui| {
                        for button in &mut self.file_buttons {
                            if ui.button(button.label()).clicked()
                                && let Some(action) = button.on_click(ui.ctx())
                            {
                                self.pending.push(action);
                            }
                        }
                    },
                );
            });
        });
    }
}

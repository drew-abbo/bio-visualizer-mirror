use crate::ui::View;

pub struct MenuBar;

impl View for MenuBar {
    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            //use the menu_buttons mod here
            // File menu
            ui.menu_button("File", |ui| {
                if ui.button("Import").clicked() {
                    // Handle open action
                }
                if ui.button("Export").clicked() {
                    // Handle save action
                }
                if ui.button("Settings").clicked() {
                    // Handle exit action
                }
            });
        });
    }
}

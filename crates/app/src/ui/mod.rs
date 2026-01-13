pub mod node_blueprint;
pub mod content_bar_left;
pub mod menu_buttons;

pub trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}

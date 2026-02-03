pub trait View {
    fn ui(&mut self, ui: &mut util::egui::Ui);
}

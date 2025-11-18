pub mod video_frame;

pub trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}
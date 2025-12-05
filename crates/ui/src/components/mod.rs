pub mod video_frame;
pub use video_frame::VideoFrame;
pub mod video_controller;
pub use video_controller::VideoController;
pub mod playback_controls;
pub use playback_controls::PlaybackControls;
pub mod effects;

pub trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}
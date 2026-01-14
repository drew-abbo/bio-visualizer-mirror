mod video_controller;
mod video_frame;

use crate::view::View;
use eframe::wgpu;
use thiserror::Error;
use video_controller::VideoController;
use video_frame::VideoFrame;

pub struct VideoContext {
    controller: VideoController,
    frame: VideoFrame,
}

impl VideoContext {
    pub fn new(target_format: wgpu::TextureFormat) -> Result<Self, VideoError> {
        let ctr = VideoController::new(target_format)?;

        Ok(Self {
            controller: ctr,
            frame: VideoFrame::default(),
        })
    }

    pub fn video_loaded(&mut self) -> bool {
        self.controller.has_video()
    }

    pub fn load_video(
        &mut self,
        path: std::path::PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.controller.load_video(path)?;
        Ok(())
    }

    pub fn update_and_render(&mut self, render_state: &egui_wgpu::RenderState, dt: f32) {
        if let Ok(Some(render_result)) =
            self.controller
                .update_and_render(&render_state.device, &render_state.queue, dt)
        {
            self.frame.set_wgpu_texture_if_changed(
                render_state,
                &render_result.texture_view,
                render_result.size,
                render_result.frame_id,
            );
        }
    }

    pub fn is_playing(&self) -> bool {
        self.controller.is_playing()
    }

    pub fn toggle_playback(&mut self) {
        self.controller.player_mut().unwrap().toggle_play_pause();
    }
}

impl View for VideoContext {
    fn ui(&mut self, ui: &mut egui::Ui) {
        self.frame.ui(ui);
    }
}

#[derive(Error, Debug)]
pub enum VideoError {
    #[error("Failed to initialize video engine")]
    EngineInit(#[from] engine::errors::EngineError),
}

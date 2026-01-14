use media::frame::Uid;
use crate::view::View;

pub struct VideoFrame {
    frame: egui::Frame,
    texture_id: Option<egui::TextureId>,
    texture_size: [usize; 2],
    last_frame_id: Option<Uid>,
    position: egui::Pos2,  // Store position for dragging
}

impl Default for VideoFrame {
    fn default() -> Self {
        Self {
            frame: egui::Frame::new()
                .inner_margin(12)
                .outer_margin(0)  // Changed from 24 to 0 (area will handle positioning)
                .corner_radius(14)
                .shadow(egui::Shadow {
                    offset: [8, 12],
                    blur: 16,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(180),
                })
                .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 200))
                .stroke(egui::Stroke::new(1.0, egui::Color32::GRAY)),
            texture_id: None,
            texture_size: [0, 0],
            last_frame_id: None,
            position: egui::Pos2::new(100.0, 100.0),  // Default starting position
        }
    }
}

impl VideoFrame {
    pub fn set_wgpu_texture_if_changed(
        &mut self,
        render_state: &egui_wgpu::RenderState,
        texture_view: &eframe::wgpu::TextureView,
        size: [usize; 2],
        frame_id: Uid,
    ) {
        if self.last_frame_id == Some(frame_id) {
            return;
        }
        
        if let Some(old_id) = self.texture_id.take() {
            render_state.renderer.write().free_texture(&old_id);
        }

        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            texture_view,
            eframe::wgpu::FilterMode::Linear,
        );

        self.texture_id = Some(texture_id);
        self.texture_size = size;
        self.last_frame_id = Some(frame_id);
    }
}

impl View for VideoFrame {
    fn ui(&mut self, ui: &mut egui::Ui) {
        // Use Area for draggable floating window
        let area_response = egui::Area::new(egui::Id::new("video_frame_area"))
            .movable(true)
            .default_pos(self.position)
            .show(ui.ctx(), |ui| {
                self.frame.show(ui, |ui| {
                    if let Some(texture_id) = self.texture_id {
                        let original_size =
                            egui::vec2(self.texture_size[0] as f32, self.texture_size[1] as f32);

                        // Fixed size or you can make it resizable
                        let max_size = egui::vec2(640.0, 480.0);
                        let scale = (max_size.x / original_size.x)
                            .min(max_size.y / original_size.y)
                            .min(1.0);

                        let display_size = original_size * scale;

                        ui.image(egui::load::SizedTexture::new(texture_id, display_size));
                    }
                });
            });
        
        // Update stored position when dragged
        self.position = area_response.response.rect.left_top();
    }
}
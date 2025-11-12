pub struct VideoFrame {
    frame: egui::Frame,
    texture_id: Option<egui::TextureId>,
    texture_size: [usize; 2],
}

pub trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}

impl Default for VideoFrame {
    fn default() -> Self {
        Self {
            frame: egui::Frame::new()
                .inner_margin(12)
                .outer_margin(24)
                .corner_radius(14)
                .shadow(egui::Shadow {
                    offset: [8, 12],
                    blur: 16,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(180),
                })
                .fill(egui::Color32::from_rgba_unmultiplied(97, 0, 255, 128))
                .stroke(egui::Stroke::new(1.0, egui::Color32::GRAY)),
            texture_id: None,
            texture_size: [0, 0],
        }
    }
}

impl VideoFrame {
    pub fn set_wgpu_texture(
        &mut self,
        render_state: &egui_wgpu::RenderState,
        texture_view: &eframe::wgpu::TextureView,
        size: [usize; 2],
    ) {
        // Remove old texture if exists
        if let Some(old_id) = self.texture_id.take() {
            render_state.renderer.write().free_texture(&old_id);
        }

        // Register the new texture with egui
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            texture_view,
            eframe::wgpu::FilterMode::Linear,
        );

        self.texture_id = Some(texture_id);
        self.texture_size = size;
    }
}

impl View for VideoFrame {
    fn ui(&mut self, ui: &mut egui::Ui) {
        self.frame.show(ui, |ui| {
            if let Some(texture_id) = self.texture_id {
                let size = egui::vec2(self.texture_size[0] as f32, self.texture_size[1] as f32);
                ui.image(egui::load::SizedTexture::new(texture_id, size));
            } else {
                ui.label("No video loaded");
            }
        });
    }
}

use crate::components::View;
use eframe::wgpu;
use egui::{self, Color32};

pub struct VideoFrame {
    frame: egui::Frame,
    texture: Option<wgpu::Texture>,      // Persistent UI-owned texture
    texture_id: Option<egui::TextureId>, // Registered with egui
    texture_size: [usize; 2],
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
                    color: Color32::from_black_alpha(180),
                })
                .fill(Color32::from_rgba_unmultiplied(30, 30, 30, 200))
                .stroke(egui::Stroke::new(1.0, Color32::GRAY)),
            texture: None,
            texture_id: None,
            texture_size: [0, 0],
        }
    }
}

impl VideoFrame {
    /// Get a reference to the persistent texture
    pub fn texture(&self) -> Option<&wgpu::Texture> {
        self.texture.as_ref()
    }

    /// Set or replace the persistent UI texture
    pub fn set_texture(&mut self, texture: wgpu::Texture) {
        self.texture = Some(texture);
    }

    /// Update the egui texture ID to point to the GPU texture
    pub fn set_wgpu_texture(
        &mut self,
        render_state: &egui_wgpu::RenderState,
        texture_view: &wgpu::TextureView,
        size: [usize; 2],
    ) {
        // Free old egui texture if it exists
        if let Some(old_id) = self.texture_id.take() {
            render_state.renderer.write().free_texture(&old_id);
        }

        // Register new GPU texture with egui
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            texture_view,
            eframe::wgpu::FilterMode::Linear,
        );

        self.texture_id = Some(texture_id);
        self.texture_size = size;
    }

    pub fn texture_size(&self) -> [usize; 2] {
        self.texture_size
    }
}

impl View for VideoFrame {
    fn ui(&mut self, ui: &mut egui::Ui) {
        self.frame.show(ui, |ui| {
            if let Some(texture_id) = self.texture_id {
                let original_size =
                    egui::vec2(self.texture_size[0] as f32, self.texture_size[1] as f32);
                let available_size = ui.available_size();

                let scale = (available_size.x / original_size.x)
                    .min(available_size.y / original_size.y)
                    .min(1.0);

                let display_size = original_size * scale;

                ui.image(egui::load::SizedTexture::new(texture_id, display_size));
            } else {
                ui.label("No video loaded");
            }
        });
    }
}

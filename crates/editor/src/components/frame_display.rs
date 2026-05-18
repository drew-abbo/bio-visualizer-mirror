use std::collections::VecDeque;

use eframe::wgpu;
use egui::load::SizedTexture;

const MAX_TEXTURE_CACHE_SIZE: usize = 3;

/// A reusable component for displaying wgpu TextureViews
pub struct FrameDisplay {
    texture_id: Option<egui::TextureId>,
    texture_size: [usize; 2],
    last_frame_key: Option<usize>,
    last_renderer_ptr: Option<usize>,
    texture_cache: VecDeque<(usize, egui::TextureId)>,
}

impl FrameDisplay {
    pub fn new() -> Self {
        Self {
            texture_id: None,
            texture_size: [0, 0],
            last_frame_key: None,
            last_renderer_ptr: None,
            texture_cache: VecDeque::new(),
        }
    }

    /// Update the texture if the frame has changed
    pub fn set_wgpu_texture_if_changed(
        &mut self,
        render_state: &egui_wgpu::RenderState,
        texture_view: &wgpu::TextureView,
        size: [usize; 2],
        frame_key: usize,
    ) {
        let renderer_ptr = std::sync::Arc::as_ptr(&render_state.renderer) as usize;
        let renderer_changed = self.last_renderer_ptr != Some(renderer_ptr);

        if self.last_frame_key == Some(frame_key) && !renderer_changed {
            return;
        }

        if renderer_changed {
            let mut renderer = render_state.renderer.write();
            for (_, texture_id) in self.texture_cache.drain(..) {
                renderer.free_texture(&texture_id);
            }
            self.texture_id = None;
        }

        let texture_id = if let Some((_, cached_id)) = self
            .texture_cache
            .iter()
            .find(|(cached_key, _)| *cached_key == frame_key)
        {
            *cached_id
        } else {
            let new_id = render_state.renderer.write().register_native_texture(
                &render_state.device,
                texture_view,
                wgpu::FilterMode::Linear,
            );
            self.texture_cache.push_back((frame_key, new_id));

            while self.texture_cache.len() > MAX_TEXTURE_CACHE_SIZE {
                if let Some((evicted_key, evicted_id)) = self.texture_cache.pop_front() {
                    if Some(evicted_key) == self.last_frame_key
                        || Some(evicted_id) == self.texture_id
                    {
                        self.texture_cache.push_back((evicted_key, evicted_id));
                        break;
                    }

                    render_state.renderer.write().free_texture(&evicted_id);
                }
            }

            new_id
        };

        if let Some(pos) = self
            .texture_cache
            .iter()
            .position(|(cached_key, _)| *cached_key == frame_key)
            && let Some(entry) = self.texture_cache.remove(pos)
        {
            self.texture_cache.push_back(entry);
        }

        self.texture_id = Some(texture_id);
        self.texture_size = size;
        self.last_frame_key = Some(frame_key);
        self.last_renderer_ptr = Some(renderer_ptr);
    }

    /// Clear the current texture
    pub fn clear(&mut self, render_state: Option<&egui_wgpu::RenderState>) {
        if let Some(rs) = render_state {
            let mut renderer = rs.renderer.write();
            for (_, texture_id) in self.texture_cache.drain(..) {
                renderer.free_texture(&texture_id);
            }
        }

        self.texture_id = None;
        self.texture_size = [0, 0];
        self.last_frame_key = None;
        self.last_renderer_ptr = None;
    }

    /// Render just the texture content
    pub fn render_content(&self, ui: &mut egui::Ui) {
        if let Some(texture_id) = self.texture_id {
            let original_size =
                egui::vec2(self.texture_size[0] as f32, self.texture_size[1] as f32);

            // Use the space egui has actually allocated, not a fixed config value
            let available = ui.available_size();
            let scale = (available.x / original_size.x).min(available.y / original_size.y);

            let display_size = original_size * scale;

            ui.centered_and_justified(|ui| {
                ui.image(SizedTexture::new(texture_id, display_size));
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("No frame data").weak());
            });
        }
    }
}

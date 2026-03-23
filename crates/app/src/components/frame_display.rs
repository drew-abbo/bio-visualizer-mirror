use egui::load::SizedTexture;
use util::eframe::wgpu;
use util::egui;

/// Display configuration for output frames rendered via egui.
#[derive(Clone, Debug)]
pub struct FrameDisplayConfig {
    pub max_size: egui::Vec2,
}

impl Default for FrameDisplayConfig {
    fn default() -> Self {
        Self {
            max_size: egui::vec2(640.0, 480.0),
        }
    }
}

/// A reusable component for displaying wgpu TextureViews
pub struct FrameDisplay {
    config: FrameDisplayConfig,
    texture_id: Option<egui::TextureId>,
    texture_size: [usize; 2],
    last_frame_key: Option<usize>,
    last_renderer_ptr: Option<usize>,
    pending_free_texture_id: Option<egui::TextureId>,
}

impl FrameDisplay {
    pub fn new(config: FrameDisplayConfig) -> Self {
        Self {
            config,
            texture_id: None,
            texture_size: [0, 0],
            last_frame_key: None,
            last_renderer_ptr: None,
            pending_free_texture_id: None,
        }
    }

    pub fn default_config() -> Self {
        Self::new(FrameDisplayConfig::default())
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

        // Free one-update-old texture ids. Delaying by one update helps avoid
        // transient flashing when the renderer still references the previous id.
        if let Some(old_pending) = self.pending_free_texture_id.take()
            && !renderer_changed
        {
            render_state.renderer.write().free_texture(&old_pending);
        }

        // Register new texture first, then free old texture. This avoids transient
        // blanking/flicker when frames are updated rapidly.
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            texture_view,
            wgpu::FilterMode::Linear,
        );

        if let Some(old_id) = self.texture_id.take() {
            if renderer_changed {
                render_state.renderer.write().free_texture(&old_id);
            } else {
                self.pending_free_texture_id = Some(old_id);
            }
        }

        self.texture_id = Some(texture_id);
        self.texture_size = size;
        self.last_frame_key = Some(frame_key);
        self.last_renderer_ptr = Some(renderer_ptr);
    }

    /// Clear the current texture
    pub fn clear(&mut self, render_state: Option<&egui_wgpu::RenderState>) {
        if let Some(old_id) = self.texture_id.take()
            && let Some(rs) = render_state
        {
            rs.renderer.write().free_texture(&old_id);
        }

        if let Some(old_pending) = self.pending_free_texture_id.take()
            && let Some(rs) = render_state
        {
            rs.renderer.write().free_texture(&old_pending);
        }

        self.texture_size = [0, 0];
        self.last_frame_key = None;
        self.last_renderer_ptr = None;
    }

    /// Render just the texture content
    pub fn render_content(&self, ui: &mut egui::Ui) {
        if let Some(texture_id) = self.texture_id {
            let original_size =
                egui::vec2(self.texture_size[0] as f32, self.texture_size[1] as f32);

            let scale = (self.config.max_size.x / original_size.x)
                .min(self.config.max_size.y / original_size.y)
                .min(1.0);

            let display_size = original_size * scale;

            ui.image(SizedTexture::new(texture_id, display_size));
        } else {
            ui.label("No frame data");
        }
    }
}

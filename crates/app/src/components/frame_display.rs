use egui::load::SizedTexture;
use media::frame::Uid;
use util::eframe::wgpu;
use util::egui;

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
    last_frame_id: Option<Uid>,
}

impl FrameDisplay {
    pub fn new(config: FrameDisplayConfig) -> Self {
        Self {
            config,
            texture_id: None,
            texture_size: [0, 0],
            last_frame_id: None,
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
        frame_id: Uid,
    ) {
        if self.last_frame_id == Some(frame_id) {
            return;
        }

        // Free old texture
        if let Some(old_id) = self.texture_id.take() {
            render_state.renderer.write().free_texture(&old_id);
        }

        // Register new texture
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            texture_view,
            wgpu::FilterMode::Linear,
        );

        self.texture_id = Some(texture_id);
        self.texture_size = size;
        self.last_frame_id = Some(frame_id);
    }

    /// Clear the current texture
    pub fn clear(&mut self, render_state: Option<&egui_wgpu::RenderState>) {
        if let Some(old_id) = self.texture_id.take()
            && let Some(rs) = render_state
        {
                rs.renderer.write().free_texture(&old_id);
        }
        self.texture_size = [0, 0];
        self.last_frame_id = None;
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

use crate::view::View;
use egui::load::SizedTexture;
use media::frame::Uid;

/// Configuration for how a frame should be displayed
#[derive(Clone, Debug)]
pub struct FrameDisplayConfig {
    /// Maximum size to display the frame at
    pub max_size: egui::Vec2,
    /// Corner radius for the frame styling
    pub corner_radius: f32,
    /// Whether to show a shadow
    pub show_shadow: bool,
    /// Background color
    pub bg_color: egui::Color32,
    /// Border stroke
    pub border_stroke: egui::Stroke,
}

impl Default for FrameDisplayConfig {
    fn default() -> Self {
        Self {
            max_size: egui::vec2(640.0, 480.0),
            corner_radius: 14.0,
            show_shadow: true,
            bg_color: egui::Color32::from_rgba_unmultiplied(30, 30, 30, 200),
            border_stroke: egui::Stroke::new(1.0, egui::Color32::GRAY),
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

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(FrameDisplayConfig::default())
    }

    /// Update the texture if the frame has changed
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

        // Free old texture
        if let Some(old_id) = self.texture_id.take() {
            render_state.renderer.write().free_texture(&old_id);
        }

        // Register new texture
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            texture_view,
            eframe::wgpu::FilterMode::Linear,
        );

        self.texture_id = Some(texture_id);
        self.texture_size = size;
        self.last_frame_id = Some(frame_id);
    }

    /// Clear the current texture
    pub fn clear(&mut self, render_state: Option<&egui_wgpu::RenderState>) {
        if let Some(old_id) = self.texture_id.take() {
            if let Some(rs) = render_state {
                rs.renderer.write().free_texture(&old_id);
            }
        }
        self.texture_size = [0, 0];
        self.last_frame_id = None;
    }

    /// Render just the texture content (for embedding in other UIs)
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

    /// Get the display size that would be used for rendering
    pub fn display_size(&self) -> egui::Vec2 {
        if self.texture_id.is_none() {
            return egui::Vec2::ZERO;
        }

        let original_size =
            egui::vec2(self.texture_size[0] as f32, self.texture_size[1] as f32);

        let scale = (self.config.max_size.x / original_size.x)
            .min(self.config.max_size.y / original_size.y)
            .min(1.0);

        original_size * scale
    }

    /// Check if a texture is currently loaded
    pub fn has_texture(&self) -> bool {
        self.texture_id.is_some()
    }
}

impl View for FrameDisplay {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let frame = egui::Frame::new()
            .inner_margin(12)
            .outer_margin(0)
            .corner_radius(self.config.corner_radius)
            .shadow(if self.config.show_shadow {
                egui::Shadow {
                    offset: [8, 12],
                    blur: 16,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(180),
                }
            } else {
                egui::Shadow::NONE
            })
            .fill(self.config.bg_color)
            .stroke(self.config.border_stroke);

        frame.show(ui, |ui| {
            self.render_content(ui);
        });
    }
}

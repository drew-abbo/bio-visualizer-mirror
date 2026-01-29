use super::node::Node;
use crate::view::View;
use crate::components::FrameDisplay;
use eframe::wgpu::TextureView;

/// A base node implementation that can be used as a starting point
pub struct BaseNode {
    id: usize,
    label: String,
    frame_display: FrameDisplay,
    show_frame: bool,
}

impl BaseNode {
    pub fn new(id: usize, label: String) -> Self {
        Self {
            id,
            label,
            frame_display: FrameDisplay::default_config(),
            show_frame: true,
        }
    }

    /// Get mutable reference to the frame display for texture updates
    pub fn frame_display_mut(&mut self) -> &mut FrameDisplay {
        &mut self.frame_display
    }

    /// Get reference to the frame display
    pub fn frame_display(&self) -> &FrameDisplay {
        &self.frame_display
    }
}

impl View for BaseNode {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label(egui::RichText::new(&self.label).strong());

            if self.show_frame {
                // Display frame using FrameDisplay
                self.frame_display.render_content(ui);
            } else {
                ui.label("Output hidden");
            }
        });
    }
}

impl Node for BaseNode {
    fn id(&self) -> usize {
        self.id
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn set_frame(&mut self, view: TextureView) {
        // Note: This would need access to render_state and frame_id for proper texture registration
        // For now, this is a placeholder. Use frame_display_mut().set_wgpu_texture_if_changed() directly
    }

    fn set_show_frame(&mut self, show: bool) {
        self.show_frame = show;
    }

    fn show_frame(&self) -> bool {
        self.show_frame
    }

    fn clear_frame(&mut self) {
        self.frame_display.clear(None);
    }
}

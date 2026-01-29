use crate::components::{BaseNode, Node};
use crate::view::View;
use egui::{Pos2, Rect, Vec2};

/// A positioned node instance on the blueprint
pub struct NodeItem {
    pub node: BaseNode,
    pub position: Pos2,
    /// The last known size of the rendered node (for collision detection)
    pub size: Vec2,
}

impl NodeItem {
    pub fn new(node: BaseNode, position: Pos2) -> Self {
        Self {
            node,
            position,
            size: Vec2::new(200.0, 150.0), // Default estimated size
        }
    }

    /// Get the bounding rect of this node in canvas space
    pub fn bounds(&self) -> Rect {
        Rect::from_min_size(self.position, self.size)
    }

    /// Render the node at its position and handle dragging
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        is_dragging: &mut Option<usize>,
        canvas_to_screen: impl Fn(Pos2) -> Pos2,
        _screen_to_canvas: impl Fn(Pos2) -> Pos2,
        zoom: f32,
    ) {
        // Convert canvas position to screen position for rendering
        let screen_pos = canvas_to_screen(self.position);
        
        // Scale node size based on zoom level
        let scaled_size = self.size * zoom;
        
        // Create an interactive rect for dragging
        let node_rect = Rect::from_min_size(screen_pos, scaled_size);
        let response = ui.interact(node_rect, ui.id().with(self.node.id()), egui::Sense::drag());

        // Create a child UI for rendering at the node position with scaled size
        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(node_rect));
        self.node.ui(&mut child_ui);

        // Update size based on what was rendered (before zoom)
        self.size = child_ui.min_rect().size() / zoom.max(0.1);

        // Handle dragging
        if response.drag_started() {
            *is_dragging = Some(self.node.id());
        }

        if let Some(dragging_id) = *is_dragging {
            if dragging_id == self.node.id() {
                // Get drag delta in screen space and convert to canvas space
                let drag_delta_screen = response.drag_delta();
                let drag_delta_canvas = drag_delta_screen / zoom;
                self.position += drag_delta_canvas;
            }
        }
    }
}

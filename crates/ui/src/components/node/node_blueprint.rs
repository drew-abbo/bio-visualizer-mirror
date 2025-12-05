use egui::{Rect, Vec2};

use crate::components::{
    View,
    node::node_type::{NodeType, PlacedNode},
};

pub struct NodeBlueprint {
    placed_nodes: Vec<PlacedNode>,
    dragging_node: Option<usize>, // Index of node being dragged
    drag_offset: Vec2,
}

impl NodeBlueprint {
    pub fn new() -> Self {
        Self {
            placed_nodes: Vec::new(),
            dragging_node: None,
            drag_offset: Vec2::ZERO,
        }
    }

    fn draw_grid(&self, ui: &mut egui::Ui, rect: Rect) {
        let painter = ui.painter();
        let grid_size = 20.0;
        let color = ui.visuals().weak_text_color().linear_multiply(0.1);

        // Just to look cool
        // Vertical lines
        let mut x = rect.min.x;
        while x < rect.max.x {
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                egui::Stroke::new(1.0, color),
            );
            x += grid_size;
        }

        // Horizontal lines
        let mut y = rect.min.y;
        while y < rect.max.y {
            painter.line_segment(
                [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                egui::Stroke::new(1.0, color),
            );
            y += grid_size;
        }
    }

    fn draw_node(&self, ui: &mut egui::Ui, node: &PlacedNode, is_dragging: bool) {
        let rect = node.rect();
        let painter = ui.painter();

        let base_color = node.node_type.color();
        let fill_color = if is_dragging {
            base_color.linear_multiply(0.3)
        } else {
            base_color.linear_multiply(0.2)
        };

        // Draw node box
        painter.rect(
            rect,
            8.0,
            fill_color,
            egui::Stroke::new(2.0, base_color),
            egui::StrokeKind::Inside,
        );

        // Draw icon
        // let icon_pos = rect.min + Vec2::new(12.0, 16.0);
        // painter.text(
        //     icon_pos,
        //     egui::Align2::LEFT_TOP,
        //     // node.node_type.icon(),
        //     egui::FontId::
        //     egui::FontId::proportional(24.0),
        //     egui::Color32::WHITE,
        // );

        // Draw name
        let name_pos = rect.min + Vec2::new(12.0, 48.0);
        painter.text(
            name_pos,
            egui::Align2::LEFT_TOP,
            node.node_type.name(),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );

        // Draw input/output ports
        let port_radius = 6.0;
        let port_color = egui::Color32::from_rgb(200, 200, 200);

        // Input port (left)
        let input_pos = egui::pos2(rect.min.x, rect.center().y);
        painter.circle(
            input_pos,
            port_radius,
            port_color,
            egui::Stroke::new(2.0, base_color),
        );

        // Output port (right)
        let output_pos = egui::pos2(rect.max.x, rect.center().y);
        painter.circle(
            output_pos,
            port_radius,
            port_color,
            egui::Stroke::new(2.0, base_color),
        );
    }

    fn handle_drop(&mut self, ui: &mut egui::Ui, response: &egui::Response, rect: Rect) {
        if let Some(node_type) = response.dnd_release_payload::<NodeType>() {
            // Use the context's pointer position at the time of release
            if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
                if rect.contains(pointer_pos) {
                    // Convert to canvas-local coordinates
                    let local_pos = pointer_pos.to_vec2() - rect.min.to_vec2();
                    let node = PlacedNode::new((*node_type).clone(), rect.min + local_pos);
                    self.placed_nodes.push(node);
                }
            }
        }
    }

    fn handle_node_dragging(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let pointer_pos = ui.ctx().pointer_interact_pos();

        if ui.input(|i| i.pointer.primary_pressed()) {
            if let Some(pos) = pointer_pos {
                // Check if clicked on a node
                for (i, node) in self.placed_nodes.iter().enumerate() {
                    if node.contains(pos) {
                        self.dragging_node = Some(i);
                        self.drag_offset = pos - node.position;
                        break;
                    }
                }
            }
        }

        if ui.input(|i| i.pointer.primary_released()) {
            self.dragging_node = None;
        }

        // Update dragging node position
        if let Some(idx) = self.dragging_node {
            if let Some(pos) = pointer_pos {
                if let Some(node) = self.placed_nodes.get_mut(idx) {
                    node.position = pos - self.drag_offset;
                    // Keep node within bounds
                    node.position.x = node
                        .position
                        .x
                        .max(rect.min.x)
                        .min(rect.max.x - node.size.x);
                    node.position.y = node
                        .position
                        .y
                        .max(rect.min.y)
                        .min(rect.max.y - node.size.y);
                }
            }
        }
    }

    pub fn placed_nodes(&self) -> &[PlacedNode] {
        &self.placed_nodes
    }

    pub fn clear_nodes(&mut self) {
        self.placed_nodes.clear();
    }
}

impl View for NodeBlueprint {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let available_size = ui.available_size();
        let (rect, response) =
            ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());

        // Draw background
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);

        // Draw grid
        self.draw_grid(ui, rect);

        // Handle dropping new nodes from the sidebar
        self.handle_drop(ui, &response, rect);

        // Handle dragging existing nodes
        self.handle_node_dragging(ui, rect);

        // Draw all nodes
        for (i, node) in self.placed_nodes.iter().enumerate() {
            let is_dragging = self.dragging_node == Some(i);
            self.draw_node(ui, node, is_dragging);
        }

        if self.placed_nodes.is_empty() {
            let center = rect.center();
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                "Drag nodes from the left panel",
                egui::FontId::proportional(18.0),
                ui.visuals().weak_text_color(),
            );
        }
    }
}

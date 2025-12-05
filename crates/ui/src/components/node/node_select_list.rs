use egui::Id;

use crate::components::View;
use crate::components::node::node_type::NodeType;
pub struct NodeSelectList {
    available_nodes: Vec<NodeType>,
}

impl NodeSelectList {
    pub fn new() -> Self {
        Self {
            available_nodes: NodeType::all(),
        }
    }

    fn draw_node_template(&self, ui: &mut egui::Ui, node_type: &NodeType) {
        // let icon = node_type.icon();
        let name = node_type.name();

        ui.horizontal(|ui| {
            // Icon
            // ui.label(egui::RichText::new(icon).size(24.0));
            
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(name).strong());
                ui.label(
                    egui::RichText::new("Drag to add")
                        .small()
                        .color(ui.visuals().weak_text_color())
                );
            });
        });
    }
}

impl View for NodeSelectList {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Effect Nodes");
        ui.separator();
        ui.add_space(8.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            for node_type in &self.available_nodes {
                let node_id = Id::new(format!("template_{:?}", node_type));
                
                // Create draggable node template
                let response = ui.dnd_drag_source(node_id, node_type.clone(), |ui| {
                    egui::Frame::new()
                        .fill(node_type.color().linear_multiply(0.1))
                        .stroke(egui::Stroke::new(2.0, node_type.color()))
                        .corner_radius(8.0)
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            self.draw_node_template(ui, node_type);
                        });
                });

                // Hover effect
                if response.response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }

                ui.add_space(8.0);
            }
        });

        ui.add_space(16.0);
        ui.separator();
        ui.label(
            egui::RichText::new("💡 Drag nodes onto the blueprint")
                .small()
                .italics()
                .color(ui.visuals().weak_text_color())
        );
    }
}
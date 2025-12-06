// components/node/node_select_list.rs

use crate::components::View;
use super::node_item::NodeItem;

pub struct NodeSelectList {
    available_nodes: Vec<NodeItem>,
}

impl NodeSelectList {
    pub fn new() -> Self {
        Self {
            available_nodes: NodeItem::all_nodes(),
        }
    }
    
    fn draw_node_template(&self, ui: &mut egui::Ui, node_item: &NodeItem) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(&node_item.display_name).strong());
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
            for node_item in &self.available_nodes {
                let node_id = egui::Id::new(format!("template_{:?}", node_item.id));
                
                let response = ui.dnd_drag_source(node_id, node_item.clone(), |ui| {
                    egui::Frame::new()
                        .fill(node_item.color.linear_multiply(0.1))
                        .stroke(egui::Stroke::new(2.0, node_item.color))
                        .corner_radius(8.0)
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            self.draw_node_template(ui, node_item);
                        });
                });

                if response.response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }

                ui.add_space(8.0);
            }
        });
    }
}
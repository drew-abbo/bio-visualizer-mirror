use egui;
use engine::node::engine_node::NodeOutputKind;
use engine::node::{NodeInputKind, input_kind_to_output_kind};

/// Get the color for a node input pin based on its type
pub fn input_kind_color(kind: &NodeInputKind) -> egui::Color32 {
    output_kind_color(&input_kind_to_output_kind(kind))
}

/// Get the color for a node output pin based on its type
pub fn output_kind_color(kind: &NodeOutputKind) -> egui::Color32 {
    match kind {
        NodeOutputKind::Bool => egui::Color32::from_rgb(200, 100, 100),
        NodeOutputKind::Int => egui::Color32::from_rgb(100, 200, 100),
        NodeOutputKind::Float => egui::Color32::from_rgb(100, 100, 200),
        NodeOutputKind::Frame => egui::Color32::from_rgb(200, 200, 100),
        NodeOutputKind::MidiPacket => egui::Color32::from_rgb(100, 200, 200),
        NodeOutputKind::Dimensions => egui::Color32::from_rgb(200, 100, 200),
        NodeOutputKind::Pixel => egui::Color32::from_rgb(150, 150, 150),
        NodeOutputKind::Text => egui::Color32::from_rgb(255, 165, 0),
    }
}

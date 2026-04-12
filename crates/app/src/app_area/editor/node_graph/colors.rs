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
        NodeOutputKind::Bool => egui::Color32::from_rgb(214, 134, 63),
        NodeOutputKind::Int => egui::Color32::from_rgb(98, 196, 94),
        NodeOutputKind::Float => egui::Color32::from_rgb(220, 79, 88),
        NodeOutputKind::Frame => egui::Color32::from_rgb(194, 182, 7),
        NodeOutputKind::MidiPacket => egui::Color32::from_rgb(84, 186, 208),
        NodeOutputKind::Dimensions => egui::Color32::from_rgb(109, 142, 221),
        NodeOutputKind::Pixel => egui::Color32::from_rgb(171, 177, 184),
        NodeOutputKind::Text => egui::Color32::from_rgb(235, 12, 183),
    }
}

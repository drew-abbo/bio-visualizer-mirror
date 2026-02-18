use engine::node::engine_node::NodeOutputKind;
use engine::node::{NodeInputKind, input_kind_to_output_kind};
use util::egui;

/// Get the color for a node input pin based on its type
pub fn input_kind_color(kind: &NodeInputKind) -> egui::Color32 {
    match kind {
        NodeInputKind::Bool { .. } => egui::Color32::from_rgb(0xB0, 0x00, 0xD1),        // #B000D1
        NodeInputKind::Int { .. } => egui::Color32::from_rgb(0x3B, 0x00, 0xE6),         // #3B00E6
        NodeInputKind::Float { .. }=> egui::Color32::from_rgb(0x01, 0xB2, 0xFF),        // #01B2FF
        NodeInputKind::Frame => egui::Color32::from_rgb(0x00, 0xFF, 0x07),              // #00FF07
        NodeInputKind::Midi => egui::Color32::from_rgb(0xFF, 0x70, 0x00),               // #FF7000
        NodeInputKind::Dimensions { .. }=> egui::Color32::from_rgb(0x00, 0xFF, 0x8C),   // #00FF8C
        NodeInputKind::Pixel { .. }=> egui::Color32::from_rgb(0xBE, 0xD7, 0x01),        // #BED701
        NodeInputKind::Text { .. } => egui::Color32::from_rgb(0xD7, 0x21, 0x01),        // #D72101
        NodeInputKind::Enum { .. }=> egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),         // #FFFFFF
        NodeInputKind::File { .. }=> egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),         // #FFFFFF
    }
    // output_kind_color(&input_kind_to_output_kind(kind))
}

/// Get the color for a node output pin based on its type
pub fn output_kind_color(kind: &NodeOutputKind) -> egui::Color32 {
    match kind {
        NodeOutputKind::Bool => egui::Color32::from_rgb(0xB0, 0x00, 0xD1), // #B000D1
        NodeOutputKind::Int => egui::Color32::from_rgb(0x3B, 0x00, 0xE6),  // #3B00E6
        NodeOutputKind::Float => egui::Color32::from_rgb(0x01, 0xB2, 0xFF), // #01B2FF
        NodeOutputKind::Frame => egui::Color32::from_rgb(0x00, 0xFF, 0x07), // #00FF07
        NodeOutputKind::Midi => egui::Color32::from_rgb(0xFF, 0x70, 0x00), // #FF7000
        NodeOutputKind::Dimensions => egui::Color32::from_rgb(0x00, 0xFF, 0x8C), // #00FF8C
        NodeOutputKind::Pixel => egui::Color32::from_rgb(0xBE, 0xD7, 0x01), // #BED701
        NodeOutputKind::Text => egui::Color32::from_rgb(0xD7, 0x21, 0x01), // #D72101
    }
}

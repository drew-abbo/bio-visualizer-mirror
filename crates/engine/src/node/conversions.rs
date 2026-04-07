//! Conversion utilities between node input/output kinds and resolved values

use crate::graph_executor::NodeValue;
use crate::node::engine_node::{NodeInputKind, NodeOutputKind};
use media::midi::MidiPacket;

/// Convert a NodeInputKind to its corresponding NodeOutputKind for connection typing
pub fn input_kind_to_output_kind(input_kind: &NodeInputKind) -> NodeOutputKind {
    match input_kind {
        NodeInputKind::Frame => NodeOutputKind::Frame,
        NodeInputKind::Midi => NodeOutputKind::Midi,
        NodeInputKind::Bool { .. } => NodeOutputKind::Bool,
        NodeInputKind::Int { .. } => NodeOutputKind::Int,
        NodeInputKind::Float { .. } => NodeOutputKind::Float,
        NodeInputKind::Dimensions { .. } => NodeOutputKind::Dimensions,
        NodeInputKind::Pixel { .. } => NodeOutputKind::Pixel,
        NodeInputKind::Text { .. } => NodeOutputKind::Text,
        NodeInputKind::Enum { .. } => NodeOutputKind::Int,
        NodeInputKind::File { .. } => NodeOutputKind::Text,
    }
}

/// Create a default Value from a NodeInputKind
pub fn default_value_for_input_kind(input_kind: &NodeInputKind) -> NodeValue {
    match input_kind {
        NodeInputKind::Frame => panic!("Frame inputs cannot have default values"),
        NodeInputKind::Midi => NodeValue::Midi(MidiPacket::default()),
        NodeInputKind::Bool { default, .. } => NodeValue::Bool(*default),
        NodeInputKind::Int { default, .. } => NodeValue::Int(*default),
        NodeInputKind::Float { default, .. } => NodeValue::Float(*default),
        NodeInputKind::Dimensions { default, .. } => NodeValue::Dimensions(default.0, default.1),
        NodeInputKind::Pixel { default, .. } => NodeValue::Pixel(*default),
        NodeInputKind::Text { default, .. } => NodeValue::Text(default.clone()),
        NodeInputKind::Enum { default_idx, .. } => NodeValue::Enum(default_idx.unwrap_or(0)),
        NodeInputKind::File { default, .. } => NodeValue::File(default.clone().unwrap_or_default()),
    }
}

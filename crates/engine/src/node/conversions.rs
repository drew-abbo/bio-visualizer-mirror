//! Conversion utilities between node input/output kinds and resolved values

use crate::graph_executor::Value;
use crate::node::node::{NodeInputKind, NodeOutputKind};

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
        NodeInputKind::Enum { .. } => NodeOutputKind::Int, // Enum uses int for selection
        NodeInputKind::File { .. } => NodeOutputKind::Text, // File paths as text
    }
}

/// Create a default Value from a NodeInputKind
pub fn default_value_for_input_kind(input_kind: &NodeInputKind) -> Value {
    match input_kind {
        NodeInputKind::Frame => panic!("Frame inputs cannot have default values"),
        NodeInputKind::Midi => Value::Float(0.0), // Placeholder
        NodeInputKind::Bool { default, .. } => Value::Bool(*default),
        NodeInputKind::Int { default, .. } => Value::Int(*default),
        NodeInputKind::Float { default, .. } => Value::Float(*default),
        NodeInputKind::Dimensions { default, .. } => {
            Value::Dimensions(default.0, default.1)
        }
        NodeInputKind::Pixel { default, .. } => Value::Pixel(*default),
        NodeInputKind::Text { default, .. } => Value::Text(default.clone()),
        NodeInputKind::Enum { default_idx, .. } => {
            Value::Enum(default_idx.unwrap_or(0))
        }
        NodeInputKind::File { default, .. } => {
            Value::File(default.clone().unwrap_or_default())
        }
    }
}

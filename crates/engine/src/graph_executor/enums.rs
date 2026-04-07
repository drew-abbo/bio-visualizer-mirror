use std::path::PathBuf;

use crate::gpu_frame::GpuFrame;
use media::midi::MidiPacket;

/// A value in the node graph execution system.
/// Represents both input values consumed by nodes and output values produced by nodes.
/// This type is used throughout the execution pipeline, from initial input resolution
/// to final output collection.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeValue {
    /// A GPU-backed frame/texture
    Frame(GpuFrame),
    Midi(MidiPacket),
    Bool(bool),
    Int(i32),
    Float(f32),
    Dimensions(u32, u32),
    Pixel([f32; 4]),
    Text(String),
    /// Enum selection index (inputs only)
    Enum(usize),
    /// File path (inputs only)
    File(PathBuf),
    Device, // THIS WILL TAKE THE DEVICE OBJECT FROM THE NEW MIDI CODE
}

impl Default for NodeValue {
    fn default() -> Self {
        NodeValue::Float(0.0)
    }
}

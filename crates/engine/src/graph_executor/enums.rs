use std::path::PathBuf;

use crate::gpu_frame::GpuFrame;

/// A value in the node graph execution system.
/// Represents both input values consumed by nodes and output values produced by nodes.
/// This type is used throughout the execution pipeline, from initial input resolution
/// to final output collection.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A GPU-backed frame/texture
    Frame(GpuFrame),
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
}

impl Default for Value {
    fn default() -> Self {
        Value::Float(0.0)
    }
}

// Type aliases for backward compatibility and semantic clarity
pub type ResolvedInput = Value;
pub type OutputValue = Value;


use std::path::PathBuf;

use crate::gpu_frame::GpuFrame;

/// Resolved input value after [crate::graph_executor::GraphExecutor] resolves [crate::node_graph::InputValue] references.
/// This type represents the concrete value passed to node handlers and
/// shader pipelines. It contains owned or cloned data where appropriate so
/// downstream code does not need to reference the original graph.
#[derive(Debug, Clone)]
pub enum ResolvedInput {
    /// A GPU-backed frame/texture returned from another node
    Frame(GpuFrame),
    Bool(bool),
    Int(i32),
    Float(f32),
    Dimensions(u32, u32),
    Pixel([f32; 4]),
    Text(String),
    /// Enum selection index
    Enum(usize),
    /// File path (used by source nodes)
    File(PathBuf),
}

/// Output value produced by a node execution.
/// Note: shader-based nodes currently only produce [OutputValue::Frame]
/// outputs. Other variants are used by built-in handlers (image/video
/// producers, etc.).
#[derive(Debug, Clone)]
pub enum OutputValue {
    Frame(GpuFrame),
    Bool(bool),
    Int(i32),
    Float(f32),
    Dimensions(u32, u32),
    Pixel([f32; 4]),
    Text(String),
}

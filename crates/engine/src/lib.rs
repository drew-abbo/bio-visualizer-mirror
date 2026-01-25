//! Engine crate: runtime for executing node graphs and GPU-based image/video processing.
//!
//! This crate provides the core [crate::graph_executor::GraphExecutor] that runs a [crate::node_graph::NodeGraph], a minimal
//! runtime pipeline builder [node_render_pipeline] that creates GPU render pipelines from WGSL shaders, and
//! helpers for uploading CPU image data to GPU textures [UploadStager].
pub mod gpu_frame;
pub mod graph_executor;
pub mod node;
pub mod node_graph;
pub mod node_render_pipeline;

mod engine_errors;
mod upload_stager;

pub use engine_errors::EngineError;
pub use upload_stager::UploadStager;

pub use wgpu;

//! Brief
//! -----
//! The `engine` crate is the core execution engine for node graphs. It resolves node inputs, runs
//! shader-based nodes and built-in handlers (image/video sources), uploads CPU frames to the GPU,
//! and caches intermediate outputs and compiled render pipelines.
//!
//! Key modules
//! -----------
//! - `graph_executor.rs` - [crate::graph_executor::GraphExecutor] executes a [crate::node_graph::NodeGraph]
//!   using definitions from a node library and maintains caches such as `output_cache` and
//!   `pipeline_cache`.
//! - `node/handler` - built-in handlers implementing the `NodeHandler` trait (for example,
//!   `ImageSourceHandler` and `VideoSourceHandler`) for loading media and producing `GpuFrame`s.
//! - `upload_stager` - utilities to upload CPU image/frame data to GPU textures ([UploadStager]).
//! - `node_render_pipeline` - dynamic creation of GPU render pipelines from WGSL shaders.
//!
//! Usage
//! -------------
//! - Create an executor:
//!
//! ```ignore
//! let mut executor = GraphExecutor::new(wgpu::TextureFormat::Rgba8Unorm);
//! ```
//!
//! - Run the graph (returns first output node's results):
//!
//! ```ignore
//! let result = executor.execute(&graph, &library, &device, &queue)?;
//! ```
//!
//! - Manage caches between runs:
//!
//! ```ignore
//! executor.clear_producer_cache();
//! executor.clear_image_cache();
//! executor.invalidate_execution_order();
//! ```
//!
//! View the `render_testing` crate for examples.
//!
//! Errors
//! ------
//! Use `ExecutionError` to inspect and handle runtime failures (for example, `TextureUploadError`,
//! `VideoFetchError`, or `ShaderLoadError`).
//!
//! Shaders and bind groups
//! -----------------------
//! The engine loads WGSL shaders from node definitions and follows a small convention:
//!
//! - Entry points: the shader must provide `vs_main` (vertex) and `fs_main` (fragment).
//! - Bind group layout used by the runtime:
//!   - binding 0: a `sampler` (linear filtering)
//!   - bindings 1..N: `texture_2d` views corresponding to each `Frame` input (primary input is binding 1)
//!   - binding (N+1): a uniform buffer containing non-texture parameters (bool/int/float/pixel/dimensions/enum)
//!
//! Parameters are passed as a `HashMap<String, ResolvedInput>` by name and packed into a uniform buffer
//! using a simple std140-like alignment. Text/file inputs are not passed to the shader; `Frame` inputs
//! are provided as texture views in the order declared by the node definition.
//!
//! Examples
//! --------
//! See the `nodes/` folder at the repository root for example `shader.wgsl` files demonstrating
//! bindings and entry points.
pub mod engine_errors;
pub mod graph_executor;
pub mod node;
pub mod node_graph;
pub mod node_render_pipeline;

mod gpu_frame;
mod upload_stager;

pub use engine_errors::EngineError;
pub use gpu_frame::GpuFrame;
pub use upload_stager::UploadStager;

pub use wgpu;

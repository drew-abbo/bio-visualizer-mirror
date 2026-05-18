//! Brief
//! -----
//! The `engine` crate is a self-driving GPU execution engine for node graphs. It runs on a
//! dedicated thread, executing the active graph at a configured frame rate and broadcasting
//! rendered frames and state changes back to the application via a channel-based event system.
//!
//! Key modules
//! -----------
//! - [`engine_outpost`] — thread management and the public API surface. [`spawn`] starts the
//!   engine thread and returns an [`EngineOutpostHandle`] for sending commands and subscribing
//!   to events.
//! - [`graph_executor`][`crate::graph_executor`] — resolves node inputs, runs shader-based nodes
//!   and built-in handlers (image/video sources, noise, MIDI), and caches intermediate GPU
//!   outputs and compiled render pipelines. Internal to the outpost; not called directly by
//!   application code.
//! - `node/handler` — built-in node handlers: video/image frame streams, procedural noise,
//!   MIDI input, and signal envelope processing.
//! - [`node_graph`][`crate::node_graph`] — the [`node_graph::NodeGraph`] data model shared
//!   between the app and engine, containing node instances and their wired input connections.
//! - `node_pipelines` — dynamic creation of GPU render and compute pipelines from WGSL shaders.
//! - `upload_stager` — utilities for staging CPU image data into GPU textures ([`UploadStager`]).
//!
//! Usage
//! -----
//! All interaction with the engine goes through [`EngineOutpostHandle`]. Start the engine once
//! wgpu render resources are available:
//!
//! ```ignore
//! let handle = engine::spawn(device, queue, node_library, texture_format);
//! let command_tx = handle.command_sender();
//! let event_rx = handle.subscribe(EventFilter::Only(vec![EventKind::FrameReady]));
//! ```
//!
//! Push a graph and tell the engine which node to render to:
//!
//! ```ignore
//! command_tx.send(EngineCommand::UpdateGraph(node_graph))?;
//! command_tx.send(EngineCommand::SetOutputNode(Some(output_node_id)))?;
//! ```
//!
//! Drain events each frame to receive rendered output:
//!
//! ```ignore
//! for event in event_rx.drain() {
//!     if let EngineOutpostEvent::FrameReady(frame) = event {
//!         // display the GPU-backed frame
//!     }
//! }
//! ```
//!
//! Control playback and frame rate:
//!
//! ```ignore
//! // Pause/resume all streams
//! command_tx.send(EngineCommand::PauseStreams)?;
//! command_tx.send(EngineCommand::PlayStreams)?;
//!
//! // Lock the engine to a fixed FPS (overrides auto-detection from the video source)
//! command_tx.send(EngineCommand::SetGlobalStreamTargetFps(Fps::from_int(60).unwrap()))?;
//!
//! // Release the lock; engine reverts to the output node's recommended rate
//! command_tx.send(EngineCommand::ClearManualFps)?;
//! ```
//!
//! The engine shuts down automatically when all [`EngineOutpostHandle`] clones are dropped.
//!
//! Errors
//! ------
//! Execution failures are emitted as [`engine_outpost::message::EngineOutpostEvent::ExecutionError`]
//! events rather than returned directly, since the engine runs asynchronously on its own thread.
//! Subscribe to [`engine_outpost::EventKind::ExecutionError`] to surface these to the user.
//!
//! Shaders and bind groups
//! -----------------------
//! The engine loads WGSL shaders from node definitions and follows a small convention:
//!
//! - Entry points: the shader must provide `vs_main` (vertex) and `fs_main` (fragment).
//! - Bind group layout used by the runtime:
//!   - binding 0: a `sampler` (linear filtering)
//!   - bindings 1..N: `texture_2d` views for each `Frame` input (primary input is binding 1)
//!   - binding (N+1): a uniform buffer containing non-texture parameters (bool/int/float/pixel/dimensions/enum)
//!
//! Parameters are packed into the uniform buffer by name using simple std140-like alignment.
//! Text and file inputs are not passed to shaders; `Frame` inputs are provided as texture views
//! in the order they are declared in the node definition.
//!
//! Examples
//! --------
//! See the `nodes/` folder at the repository root for example `shader.wgsl` files demonstrating
//! bindings and entry points.
pub mod engine_errors;
pub mod engine_outpost;
pub mod graph_executor;
pub mod node;
pub mod node_graph;
pub mod node_pipelines;

mod gpu_frame;
mod graph_executor_effects;
mod upload_stager;

pub use engine_errors::EngineError;
pub use engine_outpost::{EngineOutpostHandle, spawn};
pub use gpu_frame::GpuFrame;
pub use upload_stager::UploadStager;

pub use wgpu;

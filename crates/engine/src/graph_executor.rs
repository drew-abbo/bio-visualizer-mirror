//! Executes a [NodeGraph] and returns node outputs. Public types re-exported
//! at [crate::graph_executor]: [NodeValue], [NodeValue], [ExecutionError].
mod enums;
mod errors;
use std::any::Any;
use std::collections::{HashMap, HashSet};

use crate::gpu_frame::GpuFrame;
use crate::graph_executor_effects::EffectStage;
use crate::node::NodeDefinition;
use crate::node::NodeLibrary;
use crate::node::engine_node::{
    AlgorithmStageBackend, BuiltInHandler, NodeExecutionPlan, NodeOutputKind,
};
use crate::node::handler::{
    FrameStreamHandler, MidiStreamHandler, NodeFrameStreamRequest, NodeMidiStreamRequest,
    NodeNoiseStreamRequest, NoiseStreamHandler, StreamKind,
};
use crate::node_graph::EngineNodeId;
use crate::node_graph::{InputValue, NodeGraph, NodeInstance};
use crate::node_render_pipeline::{ComputePipeline, PipelineBase};
use crate::upload_stager::UploadStager;
use media::fps::Fps;

pub use enums::*;
pub use errors::*;

/// The executor that runs a node graph and produces results.
///
/// [GraphExecutor] holds transient caches used during execution (compiled
/// pipelines, output values, and temporary GPU upload staging resources).
/// Construct it with [GraphExecutor::new] or prefer [GraphExecutor::default]
pub struct GraphExecutor {
    /// For uploading CPU textures to GPU
    upload_stager: UploadStager,

    /// Cache of node outputs from the current execution
    /// Maps: EngineNodeId -> { "output_name" -> NodeValue }
    output_cache: HashMap<EngineNodeId, HashMap<String, NodeValue>>,

    /// Cache of compiled pipelines
    /// Maps: definition_name -> compiled pipeline
    pub(crate) pipeline_cache: HashMap<String, Box<dyn PipelineBase>>,

    /// Cache of compiled compute pipelines for algorithm stages
    pub(crate) compute_pipeline_cache: HashMap<String, ComputePipeline>,

    /// Cache of shader output targets reused per node instance.
    render_target_cache: HashMap<EngineNodeId, CachedRenderTarget>,

    /// Cache of compute stage output targets reused per node stage instance.
    compute_stage_target_cache: HashMap<(EngineNodeId, usize, String), CachedRenderTarget>,

    /// Target texture format for rendering
    pub(crate) target_format: wgpu::TextureFormat,

    /// Handles any nodes that need frames including images and videos
    frame_stream_handler: FrameStreamHandler,

    /// Handles built-in noise nodes
    noise_stream_handler: NoiseStreamHandler,

    /// Handles built-in live MIDI source nodes
    midi_stream_handler: MidiStreamHandler,

    /// Last globally requested target FPS for stream handlers.
    global_stream_target_fps: Option<Fps>,

    /// Cached execution order to avoid recomputing topology every frame
    cached_execution_order: Option<Vec<EngineNodeId>>,

    /// The ID of the current output node (last execution)
    output_node_id: EngineNodeId,
}

/// The result of executing a node graph.
///
/// Contains the id of the chosen output node and a reference to the map of
/// outputs produced by that node. The `outputs` reference borrows from the
/// executor's internal cache and therefore has the same lifetime as the
/// executor borrow used during [crate::graph_executor::GraphExecutor::execute].
#[derive(Debug)]
pub struct ExecutionResult<'a> {
    /// The node id chosen as the graph's output
    pub output_node_id: EngineNodeId,

    /// Map of output name -> [NodeValue] produced by the output node.
    ///
    /// Note: the [&'a] lifetime ties this reference to the executor borrow
    /// used for the execution call; the consumer must not expect the
    /// outputs to outlive the executor or subsequent executions.
    pub outputs: &'a HashMap<String, NodeValue>,
}

#[derive(Debug)]
struct CachedRenderTarget {
    view: std::sync::Arc<wgpu::TextureView>,
    size: wgpu::Extent3d,
}

/// NOTE: This will change depending on the Media producer API changes in the future.
/// Execution context supplied by the app for time-based playback control.
///
/// This context provides the timeline state and frame advancement control
/// for video and time-based nodes. It allows the executor to know the current
/// playback position and whether to advance to the next frame.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionContext {
    pub advance_frame: bool,
    pub playback_running: bool,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            advance_frame: true,
            playback_running: true,
        }
    }
}

impl GraphExecutor {
    fn collect_required_nodes_for_target(
        graph: &NodeGraph,
        target: EngineNodeId,
    ) -> HashSet<EngineNodeId> {
        let mut required = HashSet::new();
        let mut stack = vec![target];

        while let Some(node_id) = stack.pop() {
            if !required.insert(node_id) {
                continue;
            }

            let Some(instance) = graph.get_instance(node_id) else {
                continue;
            };

            for input in instance.input_values.values() {
                if let InputValue::Connection { from_node, .. } = input {
                    stack.push(*from_node);
                }
            }
        }

        required
    }

    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            upload_stager: UploadStager::new(),
            output_cache: HashMap::new(),
            pipeline_cache: HashMap::new(),
            compute_pipeline_cache: HashMap::new(),
            render_target_cache: HashMap::new(),
            compute_stage_target_cache: HashMap::new(),
            frame_stream_handler: FrameStreamHandler::new(),
            noise_stream_handler: NoiseStreamHandler::new(),
            midi_stream_handler: MidiStreamHandler::new(),
            global_stream_target_fps: None,
            target_format: format,
            cached_execution_order: None,
            output_node_id: EngineNodeId::default(),
        }
    }

    /// Create a default GraphExecutor with RGBA8Unorm target format.
    /// For UI use it will be a different format more than likely.
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(wgpu::TextureFormat::Rgba8Unorm)
    }

    /// Clear producer cache to release video and MIDI streams.
    pub fn clear_producer_cache(&mut self) {
        self.frame_stream_handler.clear_cache();
        self.midi_stream_handler.clear_cache();
    }

    /// Clear image cache to release textures.
    pub fn clear_image_cache(&mut self) {
        self.frame_stream_handler.clear_cache();
    }

    /// Invalidate cached execution order (call when graph structure changes)
    pub fn invalidate_execution_order(&mut self) {
        self.cached_execution_order = None;
    }

    /// Get the cached outputs for a specific node, if available.
    /// Returns None if the node hasn't been executed yet.
    pub fn get_node_outputs(&self, node_id: EngineNodeId) -> Option<&HashMap<String, NodeValue>> {
        self.output_cache.get(&node_id)
    }

    /// Get the ID of the current output node (from the last execution)
    pub fn get_output_node_id(&self) -> EngineNodeId {
        self.output_node_id
    }

    /// Return the measured target FPS for a specific node when it is a video source.
    ///
    /// This intentionally avoids relying on runtime output-name matching.
    /// Instead, it inspects the node definition and queries the video handler
    /// directly from the node's configured file input.
    pub fn get_target_fps_for_node(
        &mut self,
        graph: &NodeGraph,
        library: &NodeLibrary,
        node_id: EngineNodeId,
    ) -> Option<media::fps::Fps> {
        let instance = graph.get_instance(node_id)?;
        let definition = library.get_definition(&instance.definition_name)?;

        if !matches!(
            definition.node.executor,
            NodeExecutionPlan::BuiltIn(BuiltInHandler::VideoSource)
        ) {
            return None;
        }

        let path = instance.input_values.values().find_map(|input| {
            if let InputValue::File(path) = input {
                Some(path)
            } else {
                None
            }
        })?;

        let request = NodeFrameStreamRequest {
            node_id,
            file_path: path.clone(),
            stream_kind: StreamKind::Video,
        };

        self.frame_stream_handler.get_recommended_fps(&request).ok()
    }

    /// Execute the node graph with an execution context.
    /// Supply an optional target node id to execute only up to that node (for partial execution).
    /// You should always be calling this no matter what is happening
    /// if you want to pause use [GraphExecutor::pause_streams]
    pub fn execute<'a>(
        &'a mut self,
        graph: &NodeGraph,
        library: &NodeLibrary,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_node_id: Option<EngineNodeId>, // runs a graph up to this node
    ) -> Result<ExecutionResult<'a>, ExecutionError> {
        // Clear cache from previous execution
        self.output_cache.clear();

        if let Some(target) = target_node_id
            && graph.get_instance(target).is_none()
        {
            return Err(ExecutionError::TargetNodeNotFound(target));
        }

        // Get execution order (topologically sorted)
        // Always recompute to handle graph structure changes (nodes added/removed)
        let order = graph
            .execution_order()
            .map_err(ExecutionError::GraphError)?;

        if let Some(target) = target_node_id
            && !order.contains(&target)
        {
            return Err(ExecutionError::TargetNodeNotInExecutionOrder(target));
        }

        let required_nodes =
            target_node_id.map(|target| Self::collect_required_nodes_for_target(graph, target));

        let execution_node_ids: Vec<EngineNodeId> = order
            .iter()
            .copied()
            .filter(|node_id| {
                required_nodes
                    .as_ref()
                    .is_none_or(|required| required.contains(node_id))
            })
            .collect();

        let active_nodes: HashSet<EngineNodeId> = execution_node_ids.iter().copied().collect();
        self.frame_stream_handler
            .set_playback_for_nodes(&active_nodes);
        self.noise_stream_handler
            .set_playback_for_nodes(&active_nodes);
        self.midi_stream_handler
            .set_playback_for_nodes(&active_nodes);

        // Keep newly created active streams aligned with the last global FPS.
        if let Some(target_fps) = self.global_stream_target_fps {
            self.frame_stream_handler
                .set_target_fps_for_nodes_non_video(target_fps, &active_nodes);
            self.noise_stream_handler
                .set_target_fps_for_nodes(target_fps, &active_nodes);
            self.midi_stream_handler
                .set_target_fps_for_nodes(target_fps, &active_nodes);
        }

        // Execute each node in order
        let live_node_ids: HashSet<EngineNodeId> = order.iter().copied().collect();
        self.render_target_cache
            .retain(|node_id, _| live_node_ids.contains(node_id));
        self.compute_stage_target_cache
            .retain(|(node_id, _, _), _| live_node_ids.contains(node_id));

        let mut graph_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("graph_execution"),
        });
        let mut has_shader_commands = false;

        for &node_id in &execution_node_ids {
            let instance = graph
                .get_instance(node_id)
                .ok_or(ExecutionError::NodeNotFound(node_id))?;

            // Get the node definition
            let definition = library
                .get_definition(&instance.definition_name)
                .ok_or_else(|| {
                    ExecutionError::DefinitionNotFound(instance.definition_name.clone())
                })?;

            // Resolve all inputs for this node
            let resolved_inputs = self.resolve_inputs(instance)?;

            // Execute the node based on its type
            let outputs = match &definition.node.executor {
                NodeExecutionPlan::Shader { .. } => {
                    has_shader_commands = true;
                    self.execute_shader_node(
                        node_id,
                        device,
                        queue,
                        &mut graph_encoder,
                        definition,
                        &resolved_inputs,
                    )?
                }
                NodeExecutionPlan::Algorithm { .. } => {
                    has_shader_commands = true;
                    self.execute_algorithm_node(
                        node_id,
                        device,
                        queue,
                        &mut graph_encoder,
                        definition,
                        &resolved_inputs,
                    )?
                }
                NodeExecutionPlan::BuiltIn(handler) => self.execute_builtin_node(
                    node_id,
                    handler,
                    &resolved_inputs,
                    device,
                    queue,
                    definition,
                )?,
            };

            // Cache the outputs
            self.output_cache.insert(node_id, outputs);

            if Some(node_id) == target_node_id {
                break;
            }
        }

        if has_shader_commands {
            queue.submit(Some(graph_encoder.finish()));
        }

        // Determine output node id
        let output_node_id = if let Some(target) = target_node_id {
            target
        } else {
            let output_nodes = graph.find_output_nodes();

            if output_nodes.is_empty() {
                return Err(ExecutionError::NoOutputNode);
            }

            // For now, return the first output node's result
            output_nodes[0]
        };
        self.output_node_id = output_node_id;
        let outputs = self
            .output_cache
            .get(&output_node_id)
            .ok_or(ExecutionError::NoOutputProduced)?;

        Ok(ExecutionResult {
            output_node_id,
            outputs,
        })
    }

    /// Tell the executor to pause all video streams
    /// Will be called if the user want to stop on a frame.
    /// This is different from stopping graph execution.
    /// Graph execution should still happen to keep the UI responsive, but video frames should not advance.
    pub fn pause_streams(&mut self) {
        self.frame_stream_handler.pause_all_streams();
        self.noise_stream_handler.pause_all_streams();
        self.midi_stream_handler.pause_all_streams();
    }

    pub fn play_streams(&mut self) {
        self.frame_stream_handler.play_all_streams();
        self.noise_stream_handler.play_all_streams();
        self.midi_stream_handler.play_all_streams();
    }

    pub fn set_global_stream_target_fps(&mut self, target_fps: Fps) {
        if self.global_stream_target_fps == Some(target_fps) {
            return;
        }

        self.global_stream_target_fps = Some(target_fps);
        self.frame_stream_handler
            .set_target_fps_all_non_video(target_fps);
        self.noise_stream_handler.set_target_fps_all(target_fps);
        self.midi_stream_handler.set_target_fps_all(target_fps);
    }

    /// Resolve all inputs for a node instance
    /// Converts InputValue::Connection references into actual NodeValues
    fn resolve_inputs(
        &self,
        instance: &NodeInstance,
    ) -> Result<HashMap<String, NodeValue>, ExecutionError> {
        let mut resolved = HashMap::new();

        for (input_name, input_value) in &instance.input_values {
            let resolved_value = match input_value {
                InputValue::Connection {
                    from_node,
                    output_name,
                } => {
                    // Look up the output from the cache
                    let source_outputs = self
                        .output_cache
                        .get(from_node)
                        .ok_or(ExecutionError::NodeNotExecuted(*from_node))?;

                    let output = source_outputs.get(output_name).ok_or_else(|| {
                        ExecutionError::OutputNotFound(*from_node, output_name.clone())
                    })?;

                    output.clone()
                }
                InputValue::Bool(b) => NodeValue::Bool(*b),
                InputValue::Int(i) => NodeValue::Int(*i),
                InputValue::Float(f) => NodeValue::Float(*f),
                InputValue::Dimensions { width, height } => NodeValue::Dimensions(*width, *height),
                InputValue::Pixel { r, g, b, a } => NodeValue::Pixel([*r, *g, *b, *a]),
                InputValue::Text(t) => NodeValue::Text(t.clone()),
                InputValue::Enum(idx) => NodeValue::Enum(*idx),
                InputValue::File(path) => NodeValue::File(path.clone()),
                InputValue::Frame => {
                    // Default empty frame
                    return Err(ExecutionError::UnconnectedFrameInput(
                        instance.id,
                        input_name.clone(),
                    ));
                }
            };

            resolved.insert(input_name.clone(), resolved_value);
        }

        Ok(resolved)
    }

    /// Execute a shader-based node
    fn execute_shader_node(
        &mut self,
        node_id: EngineNodeId,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        definition: &NodeDefinition,
        inputs: &HashMap<String, NodeValue>,
    ) -> Result<HashMap<String, NodeValue>, ExecutionError> {
        let NodeExecutionPlan::Shader { source, passes } = &definition.node.executor else {
            return Err(ExecutionError::PipelineCreationError(format!(
                "{} is not a shader node",
                definition.node.name
            )));
        };

        if !passes.is_empty() {
            let mut stages = Vec::with_capacity(passes.len() + 1);
            for (index, pass) in passes.iter().enumerate() {
                stages.push(EffectStage {
                    backend: AlgorithmStageBackend::Render,
                    source: pass.source.as_path(),
                    extra_frame_inputs: index,
                    dispatch: None,
                });
            }
            stages.push(EffectStage {
                backend: AlgorithmStageBackend::Render,
                source: source.as_path(),
                extra_frame_inputs: passes.len(),
                dispatch: None,
            });

            return self.execute_effect_stages(
                node_id, device, queue, encoder, definition, inputs, &stages,
            );
        }

        // Get or create the pipeline for this shader
        let cache_key = definition.node.name.clone();

        if !self.pipeline_cache.contains_key(&cache_key) {
            // Load shader code
            let shader_code = definition.load_shader_code().map_err(|e| {
                ExecutionError::ShaderLoadError(
                    definition.shader_path.clone().unwrap(),
                    e.to_string(),
                )
            })?;

            // Create pipeline from shader
            let pipeline = self.create_shader_pipeline(device, &shader_code, definition)?;

            self.pipeline_cache.insert(cache_key.clone(), pipeline);
        }

        // Collect frame inputs in the order defined by the node definition
        let frame_inputs: Vec<&GpuFrame> = definition
            .node
            .inputs
            .iter()
            .filter_map(|input_def| {
                if matches!(
                    input_def.kind,
                    crate::node::engine_node::NodeInputKind::Frame
                ) {
                    inputs.get(&input_def.name).and_then(|input| match input {
                        NodeValue::Frame(frame) => Some(frame),
                        _ => None,
                    })
                } else {
                    None
                }
            })
            .collect();

        let primary_frame = frame_inputs
            .first()
            .ok_or(ExecutionError::NoFrameInput(definition.node.name.clone()))?;

        let additional_frames: Vec<&wgpu::TextureView> = frame_inputs
            .iter()
            .skip(1)
            .map(|frame| frame.view())
            .collect();

        let output_size = primary_frame.size();

        let output_view_arc = self.get_or_create_render_target(device, node_id, output_size);

        let pipeline = self.pipeline_cache.get(&cache_key).unwrap();

        let output_frame = GpuFrame {
            view: output_view_arc.clone(),
            size: output_size,
        };

        // Convert inputs to shader parameters
        let params = self.inputs_to_shader_params(inputs)?;

        // Execute the pipeline
        pipeline
            .apply(
                device,
                queue,
                encoder,
                primary_frame.view(),
                &additional_frames,
                &output_view_arc,
                params.as_ref(),
            )
            .map_err(ExecutionError::RenderError)?;

        // Return outputs based on node definition
        let mut outputs = HashMap::new();
        for output_def in &definition.node.outputs {
            match output_def.kind {
                NodeOutputKind::Frame => {
                    outputs.insert(
                        output_def.name.clone(),
                        NodeValue::Frame(output_frame.clone()),
                    );
                }
                _ => {
                    // Non-frame outputs from shaders not yet supported
                    return Err(ExecutionError::UnsupportedOutputType(output_def.kind));
                }
            }
        }

        Ok(outputs)
    }

    /// Execute a built-in node
    ///
    fn execute_builtin_node(
        &mut self,
        node_id: EngineNodeId,
        handler_type: &BuiltInHandler,
        inputs: &HashMap<String, NodeValue>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        definition: &NodeDefinition,
    ) -> Result<HashMap<String, NodeValue>, ExecutionError> {
        let output_values = match *handler_type {
            BuiltInHandler::ImageSource => {
                let path = inputs
                    .values()
                    .find_map(|v| match v {
                        NodeValue::File(p) => Some(p),
                        _ => None,
                    })
                    .ok_or(ExecutionError::InvalidInputType)?;

                let request = NodeFrameStreamRequest {
                    node_id,
                    file_path: path.clone(),
                    stream_kind: StreamKind::Image,
                };

                self.frame_stream_handler
                    .execute_handler(&request, device, queue, &mut self.upload_stager)
                    .map_err(|e| {
                        ExecutionError::TextureUploadError(format!(
                            "Image source stream execution failed: {:?}",
                            e
                        ))
                    })?
            }
            BuiltInHandler::VideoSource => {
                let path = inputs
                    .values()
                    .find_map(|v| match v {
                        NodeValue::File(p) => Some(p),
                        _ => None,
                    })
                    .ok_or(ExecutionError::InvalidInputType)?;

                let request = NodeFrameStreamRequest {
                    node_id,
                    file_path: path.clone(),
                    stream_kind: StreamKind::Video,
                };

                self.frame_stream_handler
                    .execute_handler(&request, device, queue, &mut self.upload_stager)
                    .map_err(|e| {
                        ExecutionError::VideoStreamError(
                            path.clone(),
                            format!("Video source stream execution failed: {:?}", e),
                        )
                    })?
            }
            BuiltInHandler::Noise(noise_kind) => {
                let request = NodeNoiseStreamRequest {
                    node_id,
                    noise_kind,
                    inputs,
                };

                self.noise_stream_handler
                    .execute_handler(&request)
                    .map_err(|error| ExecutionError::NoiseExecutionError(error.to_string()))?
            }
            BuiltInHandler::MidiSource => {
                let request = NodeMidiStreamRequest { node_id, inputs };

                self.midi_stream_handler
                    .execute_handler(&request)
                    .map_err(|error| ExecutionError::MidiStreamError(error.to_string()))?
            }
            BuiltInHandler::MidiProperties => {
                self.midi_stream_handler
                    .extract_properties(inputs)
                    .map_err(|error| ExecutionError::MidiStreamError(error.to_string()))?
            }
        };

        let mut outputs = HashMap::new();
        for (i, value) in output_values.into_iter().enumerate() {
            if let Some(output_def) = definition.node.outputs.get(i) {
                outputs.insert(output_def.name.clone(), value);
            }
        }
        Ok(outputs)
    }

    pub(crate) fn get_or_create_render_target(
        &mut self,
        device: &wgpu::Device,
        node_id: EngineNodeId,
        output_size: wgpu::Extent3d,
    ) -> std::sync::Arc<wgpu::TextureView> {
        let cached = self.render_target_cache.entry(node_id).or_insert_with(|| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("shader_output"),
                size: output_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.target_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            CachedRenderTarget {
                view: std::sync::Arc::new(view),
                size: output_size,
            }
        });

        if cached.size != output_size {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("shader_output"),
                size: output_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.target_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            cached.view = std::sync::Arc::new(view);
            cached.size = output_size;
        }

        cached.view.clone()
    }

    pub(crate) fn get_or_create_compute_stage_target(
        &mut self,
        device: &wgpu::Device,
        node_id: EngineNodeId,
        stage_index: usize,
        output_size: wgpu::Extent3d,
        format: wgpu::TextureFormat,
    ) -> std::sync::Arc<wgpu::TextureView> {
        let cache_key = (node_id, stage_index, format_to_cache_key(format));
        let cached = self
            .compute_stage_target_cache
            .entry(cache_key)
            .or_insert_with(|| {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("compute_stage_output"),
                    size: output_size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::STORAGE_BINDING
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                CachedRenderTarget {
                    view: std::sync::Arc::new(view),
                    size: output_size,
                }
            });

        if cached.size != output_size {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("compute_stage_output"),
                size: output_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            cached.view = std::sync::Arc::new(view);
            cached.size = output_size;
        }

        cached.view.clone()
    }

    /// Convert resolved inputs into shader parameters
    pub(crate) fn inputs_to_shader_params(
        &self,
        inputs: &HashMap<String, NodeValue>,
    ) -> Result<Box<dyn Any>, ExecutionError> {
        // Filter out Frame inputs (they're bound as textures, not uniform params)
        let shader_params: HashMap<String, NodeValue> = inputs
            .iter()
            .filter(|(.., value)| !matches!(value, NodeValue::Frame(_)))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();

        Ok(Box::new(shader_params))
    }
}

fn format_to_cache_key(format: wgpu::TextureFormat) -> String {
    format!("{format:?}")
}

impl Default for GraphExecutor {
    fn default() -> Self {
        Self::new(wgpu::TextureFormat::Rgba8Unorm)
    }
}

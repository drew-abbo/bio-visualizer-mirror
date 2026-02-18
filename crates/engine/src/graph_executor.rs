//! Executes a [NodeGraph] and returns node outputs. Public types re-exported
//! at [crate::graph_executor]: [NodeValue], [NodeValue], [ExecutionError].
mod enums;
mod errors;
use std::any::Any;
use std::collections::HashMap;

use crate::gpu_frame::GpuFrame;
use crate::node::NodeDefinition;
use crate::node::NodeLibrary;
use crate::node::handler::ImageSourceHandler;
use crate::node::handler::NodeHandler;
use crate::node::handler::VideoSourceHandler;
use crate::node::node::{BuiltInHandler, NodeExecutionPlan, NodeOutputKind};
use crate::node_graph::EngineNodeId;
use crate::node_graph::{InputValue, NodeGraph, NodeInstance};
use crate::node_render_pipeline::NodeRenderPipeline;
use crate::node_render_pipeline::PipelineBase;
use crate::upload_stager::UploadStager;

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
    pipeline_cache: HashMap<String, Box<dyn PipelineBase>>,

    /// Target texture format for rendering
    target_format: wgpu::TextureFormat,

    /// Handler for video sources (maintains producer cache)
    video_handler: VideoSourceHandler,

    /// Handler for image sources (maintains frame cache)
    image_handler: ImageSourceHandler,

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

/// NOTE: This will change depending on the Media producer API changes in the future.
/// Execution context supplied by the app for time-based playback control.
///
/// This context provides the timeline state and frame advancement control
/// for video and time-based nodes. It allows the executor to know the current
/// playback position and whether to advance to the next frame.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionContext {
    /// Current timeline position in seconds
    pub timeline_time_secs: f64,
    /// Sampling rate in Hz (frames per second)
    pub sampling_rate_hz: f64,
    /// Whether to advance to the next frame in video sources
    pub advance_frame: bool,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            timeline_time_secs: 0.0,
            sampling_rate_hz: 30.0,
            advance_frame: true,
        }
    }
}

impl GraphExecutor {
    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            upload_stager: UploadStager::new(),
            output_cache: HashMap::new(),
            pipeline_cache: HashMap::new(),
            video_handler: VideoSourceHandler::new(),
            image_handler: ImageSourceHandler::new(),
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

    /// Clear producer cache to release video files
    pub fn clear_producer_cache(&mut self) {
        self.video_handler.clear_cache();
    }

    /// Clear image cache to release textures
    pub fn clear_image_cache(&mut self) {
        self.image_handler.clear_cache();
    }

    /// Invalidate cached execution order (call when graph structure changes)
    pub fn invalidate_execution_order(&mut self) {
        self.cached_execution_order = None;
    }

    /// Get the cached outputs for a specific node, if available
    /// Returns None if the node hasn't been executed yet
    pub fn get_node_outputs(&self, node_id: EngineNodeId) -> Option<&HashMap<String, NodeValue>> {
        self.output_cache.get(&node_id)
    }

    /// Get the ID of the current output node (from the last execution)
    pub fn get_output_node_id(&self) -> EngineNodeId {
        self.output_node_id
    }

    /// Execute the node graph with an execution context.
    /// Supply an optional target node id to execute only up to that node (for partial execution).
    pub fn execute<'a>(
        &'a mut self,
        graph: &NodeGraph,
        library: &NodeLibrary,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_node_id: Option<EngineNodeId>,
        context: ExecutionContext,
    ) -> Result<ExecutionResult<'a>, ExecutionError> {
        // Clear cache from previous execution
        self.output_cache.clear();

        if let Some(target) = target_node_id {
            if graph.get_instance(target).is_none() {
                return Err(ExecutionError::TargetNodeNotFound(target));
            }
        }

        // Get execution order (topologically sorted)
        // Always recompute to handle graph structure changes (nodes added/removed)
        let order = graph
            .execution_order()
            .map_err(ExecutionError::GraphError)?;

        if let Some(target) = target_node_id {
            if !order.contains(&target) {
                return Err(ExecutionError::TargetNodeNotInExecutionOrder(target));
            }
        }

        // Execute each node in order
        for &node_id in &order {
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
                    self.execute_shader_node(device, queue, definition, &resolved_inputs)?
                }
                NodeExecutionPlan::BuiltIn(handler) => self.execute_builtin_node(
                    handler,
                    &resolved_inputs,
                    device,
                    queue,
                    definition,
                    &context,
                )?,
            };

            // Cache the outputs
            self.output_cache.insert(node_id, outputs);

            if Some(node_id) == target_node_id {
                break;
            }
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
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        definition: &NodeDefinition,
        inputs: &HashMap<String, NodeValue>,
    ) -> Result<HashMap<String, NodeValue>, ExecutionError> {
        // Get or create the pipeline for this shader
        if !self.pipeline_cache.contains_key(&definition.node.name) {
            // Load shader code
            let shader_code = definition.load_shader_code().map_err(|e| {
                ExecutionError::ShaderLoadError(
                    definition.shader_path.clone().unwrap(),
                    e.to_string(),
                )
            })?;

            // Create pipeline from shader
            let pipeline = self.create_shader_pipeline(device, &shader_code, definition)?;

            self.pipeline_cache
                .insert(definition.node.name.clone(), pipeline);
        }

        let pipeline = self.pipeline_cache.get(&definition.node.name).unwrap();

        // Collect frame inputs in the order defined by the node definition
        let frame_inputs: Vec<&GpuFrame> = definition
            .node
            .inputs
            .iter()
            .filter_map(|input_def| {
                if matches!(input_def.kind, crate::node::node::NodeInputKind::Frame) {
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

        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shader_output"),
            size: output_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let output_view_arc = std::sync::Arc::new(output_view);
        let output_frame = GpuFrame {
            view: output_view_arc.clone(),
            size: output_size,
        };

        // Convert inputs to shader parameters
        let params = self.inputs_to_shader_params(inputs)?;

        // Execute the pipeline
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("node_execution"),
        });

        pipeline
            .apply(
                device,
                queue,
                &mut encoder,
                primary_frame.view(),
                &additional_frames,
                &output_view_arc,
                params.as_ref(),
            )
            .map_err(ExecutionError::RenderError)?;

        queue.submit(Some(encoder.finish()));

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
    fn execute_builtin_node(
        &mut self,
        handler_type: &BuiltInHandler,
        inputs: &HashMap<String, NodeValue>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        definition: &NodeDefinition,
        context: &ExecutionContext,
    ) -> Result<HashMap<String, NodeValue>, ExecutionError> {
        let output_values = match *handler_type {
            BuiltInHandler::ImageSource => self.image_handler.execute(
                inputs,
                device,
                queue,
                &mut self.upload_stager,
                context,
            )?,
            BuiltInHandler::VideoSource => self.video_handler.execute(
                inputs,
                device,
                queue,
                &mut self.upload_stager,
                context,
            )?,
            BuiltInHandler::MidiSource => return Err(ExecutionError::InvalidInputType),
        };

        // Map the Vec<NodeValue> to HashMap<String, NodeValue> using output names from definition
        // I wanted this to be more generic because so in the future maybe we have more outputs
        // Would require some changes to NodeHandlers to supply the outputs on the other hand
        // However those kinds of nodes would likely not be user defined so I think this is acceptable
        let mut outputs = HashMap::new();
        for (i, value) in output_values.into_iter().enumerate() {
            if let Some(output_def) = definition.node.outputs.get(i) {
                outputs.insert(output_def.name.clone(), value);
            }
        }
        Ok(outputs)
    }

    /// Create a shader pipeline dynamically from shader code
    fn create_shader_pipeline(
        &self,
        device: &wgpu::Device,
        shader_code: &str,
        definition: &NodeDefinition,
    ) -> Result<Box<dyn PipelineBase>, ExecutionError> {
        NodeRenderPipeline::from_shader(device, shader_code, definition, self.target_format)
            .map(|pipeline| Box::new(pipeline) as Box<dyn PipelineBase>)
            .map_err(ExecutionError::PipelineCreationError)
    }

    /// Convert resolved inputs into shader parameters
    fn inputs_to_shader_params(
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

impl Default for GraphExecutor {
    fn default() -> Self {
        Self::new(wgpu::TextureFormat::Rgba8Unorm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_basic_flow() {
        // This test would require a full wgpu setup
        // For now, just verify the structure compiles
        let executor = GraphExecutor::new(wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(executor.output_cache.len(), 0);
    }
}

pub mod enums;
pub mod errors;
use crate::node::NodeDefinition;
use crate::node::NodeLibrary;
use crate::node::handler::image_handler::ImageSourceHandler;
use crate::node::handler::node_handler::NodeHandler;
use crate::node::handler::video_handler::VideoSourceHandler;
use crate::node::node::{BuiltInHandler, NodeExecutionPlan, NodeOutputKind};
use crate::node_graph::NodeId;
use crate::node_graph::{InputValue, NodeGraph, NodeInstance};
use crate::node_render_pipeline::NodeRenderPipeline;
use crate::node_render_pipeline::PipelineBase;
use crate::upload_stager::UploadStager;
use enums::*;
use errors::*;
use std::any::Any;
use std::collections::HashMap;

/// The executor that runs a node graph and produces results
pub struct GraphExecutor {
    /// For uploading CPU textures to GPU
    upload_stager: UploadStager,

    /// Cache of node outputs from the current execution
    /// Maps: NodeId -> { "output_name" -> OutputValue }
    output_cache: HashMap<NodeId, HashMap<String, OutputValue>>,

    /// Cache of compiled pipelines
    /// Maps: definition_name -> compiled pipeline
    pipeline_cache: HashMap<String, Box<dyn PipelineBase>>,

    /// Target texture format for rendering
    target_format: wgpu::TextureFormat,

    /// Handler for video sources (maintains producer cache)
    video_handler: VideoSourceHandler,
}

/// The result of executing a node graph
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub output_node_id: NodeId,
    pub outputs: HashMap<String, OutputValue>,
}

impl GraphExecutor {
    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            upload_stager: UploadStager::new(),
            output_cache: HashMap::new(),
            pipeline_cache: HashMap::new(),
            video_handler: VideoSourceHandler::new(),
            target_format: format,
        }
    }

    /// Clear producer cache to release video files
    pub fn clear_producer_cache(&mut self) {
        self.video_handler.clear_cache();
    }

    /// Execute the entire node graph
    pub fn execute(
        &mut self,
        graph: &NodeGraph,
        library: &NodeLibrary,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<ExecutionResult, ExecutionError> {
        // Clear cache from previous execution
        self.output_cache.clear();

        // Get execution order (topologically sorted)
        let order = graph
            .execution_order()
            .map_err(ExecutionError::GraphError)?;

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
                NodeExecutionPlan::BuiltIn(handler) => {
                    self.execute_builtin_node(handler, &resolved_inputs, device, queue)?
                }
            };

            // Cache the outputs
            self.output_cache.insert(node_id, outputs);
        }

        // Find output nodes and return their results
        let output_nodes = graph.find_output_nodes();

        if output_nodes.is_empty() {
            return Err(ExecutionError::NoOutputNode);
        }

        // For now, return the first output node's result
        let output_node_id = output_nodes[0];
        let outputs = self
            .output_cache
            .get(&output_node_id)
            .ok_or(ExecutionError::NoOutputProduced)?;

        Ok(ExecutionResult {
            output_node_id,
            outputs: outputs.clone(),
        })
    }

    /// Resolve all inputs for a node instance
    /// Converts InputValue::Connection references into actual OutputValues
    fn resolve_inputs(
        &self,
        instance: &NodeInstance,
    ) -> Result<HashMap<String, ResolvedInput>, ExecutionError> {
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
                        .ok_or_else(|| ExecutionError::NodeNotExecuted(*from_node))?;

                    let output = source_outputs.get(output_name).ok_or_else(|| {
                        ExecutionError::OutputNotFound(*from_node, output_name.clone())
                    })?;

                    // Convert OutputValue to ResolvedInput
                    match output {
                        OutputValue::Frame(frame) => ResolvedInput::Frame(frame.clone()),
                        OutputValue::Bool(b) => ResolvedInput::Bool(*b),
                        OutputValue::Int(i) => ResolvedInput::Int(*i),
                        OutputValue::Float(f) => ResolvedInput::Float(*f),
                        OutputValue::Dimensions(w, h) => ResolvedInput::Dimensions(*w, *h),
                        OutputValue::Pixel(p) => ResolvedInput::Pixel(*p),
                        OutputValue::Text(t) => ResolvedInput::Text(t.clone()),
                    }
                }
                InputValue::Bool(b) => ResolvedInput::Bool(*b),
                InputValue::Int(i) => ResolvedInput::Int(*i),
                InputValue::Float(f) => ResolvedInput::Float(*f),
                InputValue::Dimensions { width, height } => {
                    ResolvedInput::Dimensions(*width, *height)
                }
                InputValue::Pixel { r, g, b, a } => ResolvedInput::Pixel([*r, *g, *b, *a]),
                InputValue::Text(t) => ResolvedInput::Text(t.clone()),
                InputValue::Enum(idx) => ResolvedInput::Enum(*idx),
                InputValue::File(path) => ResolvedInput::File(path.clone()),
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
        inputs: &HashMap<String, ResolvedInput>,
    ) -> Result<HashMap<String, OutputValue>, ExecutionError> {
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
                        ResolvedInput::Frame(frame) => Some(frame),
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
        let output_frame = enums::GpuFrame::new(output_view.clone(), output_size);

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
                &output_view,
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
                        OutputValue::Frame(output_frame.clone()),
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

    /// Execute a built-in node (CPU-based operations)
    fn execute_builtin_node(
        &mut self,
        handler_type: &BuiltInHandler,
        inputs: &HashMap<String, ResolvedInput>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<HashMap<String, OutputValue>, ExecutionError> {
        match handler_type {
            BuiltInHandler::ImageSource => {
                let handler = ImageSourceHandler;
                handler.execute(inputs, device, queue, &mut self.upload_stager)
            }
            BuiltInHandler::VideoSource => {
                // VideoSource needs special handling for mutable state
                let path = inputs
                    .get("path")
                    .and_then(|v| match v {
                        ResolvedInput::File(p) => Some(p),
                        _ => None,
                    })
                    .ok_or(ExecutionError::InvalidInputType)?;

                let frame = self.video_handler.fetch_frame(path)?;
                let (fps, duration_secs) = self.video_handler.get_stats(path)?;

                let width = frame.dimensions().width();
                let height = frame.dimensions().height();

                // Upload frame to GPU
                let texture_view = self
                    .upload_stager
                    .cpu_to_gpu_rgba(device, queue, width, height, frame.raw_data())
                    .map_err(|e| {
                        ExecutionError::TextureUploadError(format!(
                            "Failed to upload texture: {:?}",
                            e
                        ))
                    })?;

                let gpu_frame = enums::GpuFrame::new(
                    texture_view,
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );

                // Prepare outputs
                let mut outputs = HashMap::new();
                outputs.insert("output".to_string(), OutputValue::Frame(gpu_frame));
                outputs.insert("fps".to_string(), OutputValue::Float(fps as f32));
                outputs.insert(
                    "duration".to_string(),
                    OutputValue::Float(duration_secs as f32),
                );

                Ok(outputs)
            }
            BuiltInHandler::MidiSource => {
                // TODO: implement MidiSource
                Err(ExecutionError::InvalidInputType)
            }
        }
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
            .map_err(|e| ExecutionError::PipelineCreationError(e))
    }

    /// Convert resolved inputs into shader parameters
    fn inputs_to_shader_params(
        &self,
        inputs: &HashMap<String, ResolvedInput>,
    ) -> Result<Box<dyn Any>, ExecutionError> {
        // Filter out Frame inputs (they're bound as textures, not uniform params)
        let shader_params: HashMap<String, ResolvedInput> = inputs
            .iter()
            .filter(|(.., value)| !matches!(value, ResolvedInput::Frame(_)))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();

        Ok(Box::new(shader_params))
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

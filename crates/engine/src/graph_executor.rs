// graph_executor.rs
use crate::node_graph::{InputValue, NodeGraph, NodeId, NodeInstance};
use crate::pipelines::common::Pipeline;
use crate::upload_stager::UploadStager;
use std::any::Any;
use std::collections::HashMap;

mod enums;
mod errors;
mod node_library;
use enums::*;
use errors::*;
use node_library::*;

/// The executor that runs a node graph and produces results
pub struct GraphExecutor {
    /// For uploading CPU textures to GPU
    upload_stager: UploadStager,

    /// Cache of node outputs from the current execution
    /// Maps: NodeId -> { "output_name" -> OutputValue }
    output_cache: HashMap<NodeId, HashMap<String, OutputValue>>,

    /// Cache of compiled pipelines
    /// Maps: definition_name -> compiled pipeline
    pipeline_cache: HashMap<String, Box<dyn Pipeline>>,

    /// Target texture format for rendering
    target_format: wgpu::TextureFormat,
}

impl GraphExecutor {
    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            upload_stager: UploadStager::new(),
            output_cache: HashMap::new(),
            pipeline_cache: HashMap::new(),
            target_format: format,
        }
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
            let resolved_inputs = self.resolve_inputs(graph, instance)?;

            // Execute the node based on its type
            let outputs = match &definition.executor {
                NodeExecutionPlan::Shader { file_path } => self.execute_shader_node(
                    device,
                    queue,
                    definition,
                    &resolved_inputs,
                    file_path,
                )?,
                NodeExecutionPlan::BuiltIn(handler) => {
                    self.execute_builtin_node(handler, &resolved_inputs, definition)?
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
        graph: &NodeGraph,
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
                        OutputValue::Frame(view) => ResolvedInput::Frame(view.clone()),
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
        definition: &Node,
        inputs: &HashMap<String, ResolvedInput>,
        shader_path: &std::path::Path,
    ) -> Result<HashMap<String, OutputValue>, ExecutionError> {
        // Get or create the pipeline for this shader
        let pipeline = if !self.pipeline_cache.contains_key(&definition.name) {
            // Load shader code
            let shader_code = std::fs::read_to_string(shader_path).map_err(|e| {
                ExecutionError::ShaderLoadError(shader_path.to_path_buf(), e.to_string())
            })?;

            // Create pipeline from shader
            let pipeline = self.create_shader_pipeline(device, &shader_code, definition)?;

            self.pipeline_cache
                .insert(definition.name.clone(), pipeline);
        };

        let pipeline = self.pipeline_cache.get(&definition.name).unwrap();

        // Find the primary frame input
        let primary_frame = inputs
            .values()
            .find_map(|input| match input {
                ResolvedInput::Frame(view) => Some(view),
                _ => None,
            })
            .ok_or(ExecutionError::NoFrameInput(definition.name.clone()))?;

        // Collect additional frame inputs (if any)
        let additional_frames: Vec<&wgpu::TextureView> = inputs
            .values()
            .skip(1) // Skip the first frame (primary)
            .filter_map(|input| match input {
                ResolvedInput::Frame(view) => Some(view),
                _ => None,
            })
            .collect();

        // Create output texture (same size as input for now)
        // TODO: Get actual dimensions from input texture
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shader_output"),
            size: wgpu::Extent3d {
                width: 1920, // TODO: Get from input
                height: 1080,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Convert inputs to shader parameters
        let params = self.inputs_to_shader_params(inputs, definition)?;

        // Execute the pipeline
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("node_execution"),
        });

        pipeline
            .apply(
                device,
                queue,
                &mut encoder,
                primary_frame,
                &additional_frames,
                &output_view,
                params.as_ref(),
            )
            .map_err(ExecutionError::RenderError)?;

        queue.submit(Some(encoder.finish()));

        // Return outputs based on node definition
        let mut outputs = HashMap::new();
        for output_def in &definition.outputs {
            match output_def.kind {
                NodeOutputKind::Frame => {
                    outputs.insert(
                        output_def.name.clone(),
                        OutputValue::Frame(output_view.clone()),
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
        &self,
        handler: &BuiltInHandler,
        inputs: &HashMap<String, ResolvedInput>,
        definition: &Node,
    ) -> Result<HashMap<String, OutputValue>, ExecutionError> {
        match handler {
            BuiltInHandler::SumInputs => {
                // Example: Add two numbers
                let a = inputs
                    .get("A")
                    .and_then(|v| match v {
                        ResolvedInput::Int(i) => Some(*i),
                        _ => None,
                    })
                    .ok_or(ExecutionError::InvalidInputType)?;

                let b = inputs
                    .get("B")
                    .and_then(|v| match v {
                        ResolvedInput::Int(i) => Some(*i),
                        _ => None,
                    })
                    .ok_or(ExecutionError::InvalidInputType)?;

                let mut outputs = HashMap::new();
                outputs.insert("Sum".to_string(), OutputValue::Int(a + b));
                Ok(outputs)
            } // Add more built-in handlers here
        }
    }

    /// Create a shader pipeline dynamically from shader code
    fn create_shader_pipeline(
        &self,
        device: &wgpu::Device,
        shader_code: &str,
        definition: &Node,
    ) -> Result<Box<dyn Pipeline>, ExecutionError> {
        // TODO: Implement dynamic pipeline creation
        // This would create a pipeline similar to ColorGradingPipeline
        // but from shader code loaded at runtime

        // For now, return an error
        Err(ExecutionError::DynamicPipelineNotImplemented)
    }

    /// Convert resolved inputs into shader parameters
    fn inputs_to_shader_params(
        &self,
        inputs: &HashMap<String, ResolvedInput>,
        definition: &Node,
    ) -> Result<Box<dyn Any>, ExecutionError> {
        // TODO: Build a parameter struct from inputs
        // This needs to match the layout expected by the shader

        // For now, return a placeholder
        Err(ExecutionError::ParamConversionNotImplemented)
    }
}

/// The result of executing a node graph
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub output_node_id: NodeId,
    pub outputs: HashMap<String, OutputValue>,
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

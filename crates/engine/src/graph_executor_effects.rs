use std::collections::HashMap;

use crate::gpu_frame::GpuFrame;
use crate::graph_executor::{ExecutionError, GraphExecutor, NodeValue};
use crate::node::NodeDefinition;
use crate::node::engine_node::{
    AlgorithmStageBackend, NodeExecutionPlan, NodeInput, NodeInputKind, NodeOutputKind,
};
use crate::node_graph::EngineNodeId;
use crate::node_render_pipeline::PipelineBase;
use crate::node_render_pipeline::{ComputePipeline, NodeRenderPipeline};

#[derive(Clone, Copy)]
pub(crate) struct EffectStage<'a> {
    pub backend: AlgorithmStageBackend,
    pub source: &'a std::path::Path,
    pub extra_frame_inputs: usize,
}

impl GraphExecutor {
    pub(crate) fn execute_algorithm_node(
        &mut self,
        node_id: EngineNodeId,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        definition: &NodeDefinition,
        inputs: &HashMap<String, NodeValue>,
    ) -> Result<HashMap<String, NodeValue>, ExecutionError> {
        let NodeExecutionPlan::Algorithm {
            kind: _kind,
            stages,
        } = &definition.node.executor
        else {
            return Err(ExecutionError::PipelineCreationError(format!(
                "{} is not an algorithm node",
                definition.node.name
            )));
        };

        let stage_plan: Vec<EffectStage<'_>> = stages
            .iter()
            .map(|stage| EffectStage {
                backend: stage.backend,
                source: stage.source.as_path(),
                extra_frame_inputs: stage.extra_frame_inputs,
            })
            .collect();

        self.execute_effect_stages(
            node_id,
            device,
            queue,
            encoder,
            definition,
            inputs,
            &stage_plan,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn execute_effect_stages(
        &mut self,
        node_id: EngineNodeId,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        definition: &NodeDefinition,
        inputs: &HashMap<String, NodeValue>,
        stages: &[EffectStage<'_>],
    ) -> Result<HashMap<String, NodeValue>, ExecutionError> {
        let frame_inputs = self.collect_frame_inputs(definition, inputs)?;
        let primary_frame = frame_inputs
            .first()
            .ok_or(ExecutionError::NoFrameInput(definition.node.name.clone()))?;
        let additional_inputs: Vec<&wgpu::TextureView> = frame_inputs
            .iter()
            .skip(1)
            .map(|frame| frame.view())
            .collect();

        let output_size = primary_frame.size();
        let final_output_view = self.get_or_create_render_target(device, node_id, output_size);
        let mut actual_output_view = final_output_view.clone();

        let params = self.inputs_to_shader_params(inputs)?;
        let mut intermediate_views: Vec<std::sync::Arc<wgpu::TextureView>> = Vec::new();
        let target_format = self.target_format;

        for (stage_index, stage) in stages.iter().enumerate() {
            let stage_name = format!("{}::stage{stage_index}", definition.node.name);

            let mut stage_additional_inputs = additional_inputs.clone();
            stage_additional_inputs.extend(intermediate_views.iter().map(|view| view.as_ref()));

            let stage_definition = self.build_shader_stage_definition(
                definition,
                &stage_name,
                stage.extra_frame_inputs,
            );

            match stage.backend {
                AlgorithmStageBackend::Render => {
                    let shader_code = self.load_shader_source(
                        definition,
                        stage.source,
                        &format!("{stage_name} shader"),
                    )?;
                    let cache_key = format!("{}::{}", stage_name, stage.source.display());
                    let pipeline = self.get_or_create_cached_shader_pipeline(
                        cache_key,
                        device,
                        &shader_code,
                        &stage_definition,
                    )?;

                    let is_final_stage = stage_index + 1 == stages.len();
                    let stage_output_view = if is_final_stage {
                        actual_output_view.clone()
                    } else {
                        let texture = device.create_texture(&wgpu::TextureDescriptor {
                            label: Some("multi_pass_intermediate"),
                            size: output_size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: target_format,
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                                | wgpu::TextureUsages::TEXTURE_BINDING,
                            view_formats: &[],
                        });
                        std::sync::Arc::new(
                            texture.create_view(&wgpu::TextureViewDescriptor::default()),
                        )
                    };

                    pipeline
                        .apply(
                            device,
                            queue,
                            encoder,
                            primary_frame.view(),
                            &stage_additional_inputs,
                            &stage_output_view,
                            params.as_ref(),
                        )
                        .map_err(ExecutionError::RenderError)?;

                    if !is_final_stage {
                        intermediate_views.push(stage_output_view);
                    }
                }
                AlgorithmStageBackend::Compute => {
                    let shader_code = self.load_shader_source(
                        definition,
                        stage.source,
                        &format!("{stage_name} shader"),
                    )?;
                    let cache_key = format!("{}::{}", stage_name, stage.source.display());

                    // Get or create compute pipeline
                    if !self.compute_pipeline_cache.contains_key(&cache_key) {
                        let compute_pipeline =
                            ComputePipeline::from_shader(device, &shader_code, &stage_definition)
                                .map_err(|e| ExecutionError::PipelineCreationError(e.to_string()))?;
                        self.compute_pipeline_cache
                            .insert(cache_key.clone(), compute_pipeline);
                    }

                    let is_final_stage = stage_index + 1 == stages.len();
                    let stage_output_view = self.get_or_create_compute_stage_target(
                        device,
                        node_id,
                        stage_index,
                        output_size,
                    );

                    let compute_pipeline = &self.compute_pipeline_cache[&cache_key];

                    // Execute compute shader
                    compute_pipeline
                        .apply(
                            device,
                            queue,
                            encoder,
                            primary_frame.view(),
                            &stage_additional_inputs,
                            stage_output_view.as_ref(),
                            params.as_ref(),
                            output_size,
                        )
                        .map_err(|e| ExecutionError::PipelineCreationError(e.to_string()))?;

                    if is_final_stage {
                        actual_output_view = stage_output_view;
                    } else {
                        intermediate_views.push(stage_output_view);
                    }
                }
            }
        }

        let output_frame = GpuFrame {
            view: actual_output_view.clone(),
            size: output_size,
        };

        let mut outputs = HashMap::new();
        for output_def in &definition.node.outputs {
            match output_def.kind {
                NodeOutputKind::Frame => {
                    outputs.insert(
                        output_def.name.clone(),
                        NodeValue::Frame(output_frame.clone()),
                    );
                }
                _ => return Err(ExecutionError::UnsupportedOutputType(output_def.kind)),
            }
        }

        Ok(outputs)
    }

    fn collect_frame_inputs<'a>(
        &self,
        definition: &'a NodeDefinition,
        inputs: &'a HashMap<String, NodeValue>,
    ) -> Result<Vec<&'a GpuFrame>, ExecutionError> {
        let mut frame_inputs = Vec::new();

        for input_def in &definition.node.inputs {
            if matches!(input_def.kind, NodeInputKind::Frame)
                && let Some(NodeValue::Frame(frame)) = inputs.get(&input_def.name)
            {
                frame_inputs.push(frame);
            }
        }

        if frame_inputs.is_empty() {
            return Err(ExecutionError::NoFrameInput(definition.node.name.clone()));
        }

        Ok(frame_inputs)
    }

    fn build_shader_stage_definition(
        &self,
        definition: &NodeDefinition,
        stage_name: &str,
        extra_frame_inputs: usize,
    ) -> NodeDefinition {
        let mut stage_definition = definition.clone();
        stage_definition.node.name = stage_name.to_string();

        for index in 0..extra_frame_inputs {
            stage_definition.node.inputs.push(NodeInput {
                name: format!("Pass Input {}", index + 1),
                kind: NodeInputKind::Frame,
                show_pin: false,
            });
        }

        stage_definition
    }

    fn load_shader_source(
        &self,
        definition: &NodeDefinition,
        source: &std::path::Path,
        context: &str,
    ) -> Result<String, ExecutionError> {
        let shader_path = definition.folder_path.join(source);
        std::fs::read_to_string(&shader_path)
            .map_err(|e| ExecutionError::ShaderLoadError(shader_path, format!("{context}: {e}")))
    }

    fn get_or_create_cached_shader_pipeline<'a>(
        &'a mut self,
        cache_key: String,
        device: &wgpu::Device,
        shader_code: &str,
        definition: &NodeDefinition,
    ) -> Result<&'a dyn PipelineBase, ExecutionError> {
        if !self.pipeline_cache.contains_key(&cache_key) {
            let pipeline = self.create_shader_pipeline(device, shader_code, definition)?;
            self.pipeline_cache.insert(cache_key.clone(), pipeline);
        }

        Ok(self
            .pipeline_cache
            .get(&cache_key)
            .expect("pipeline inserted above")
            .as_ref())
    }

    pub(crate) fn create_shader_pipeline(
        &self,
        device: &wgpu::Device,
        shader_code: &str,
        definition: &NodeDefinition,
    ) -> Result<Box<dyn PipelineBase>, ExecutionError> {
        NodeRenderPipeline::from_shader(device, shader_code, definition, self.target_format)
            .map(|pipeline| Box::new(pipeline) as Box<dyn PipelineBase>)
            .map_err(ExecutionError::PipelineCreationError)
    }
}

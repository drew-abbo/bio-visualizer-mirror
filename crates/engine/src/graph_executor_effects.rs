use std::collections::HashMap;
use std::sync::mpsc;

use crate::gpu_frame::GpuFrame;
use crate::graph_executor::{ExecutionError, GraphExecutor, NodeValue};
use crate::node::NodeDefinition;
use crate::node::engine_node::{
    AlgorithmStageBackend, AlgorithmStageDispatch, AlgorithmStageDispatchMode, NodeExecutionPlan,
    NodeInput, NodeInputKind, NodeOutputKind,
};
use crate::node_graph::EngineNodeId;
use crate::node_render_pipeline::PipelineBase;
use crate::node_render_pipeline::{ComputePipeline, NodeRenderPipeline};

#[derive(Clone, Copy)]
pub(crate) struct EffectStage<'a> {
    pub backend: AlgorithmStageBackend,
    pub source: &'a std::path::Path,
    pub extra_frame_inputs: usize,
    pub dispatch: Option<&'a AlgorithmStageDispatch>,
}

impl GraphExecutor {
    pub(crate) fn execute_algorithm_node(
        &mut self,
        node_id: EngineNodeId,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
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
                dispatch: stage.dispatch.as_ref(),
            })
            .collect();

        self.execute_effect_stages(
            node_id,
            device,
            queue,
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
        definition: &NodeDefinition,
        inputs: &HashMap<String, NodeValue>,
        stages: &[EffectStage<'_>],
    ) -> Result<HashMap<String, NodeValue>, ExecutionError> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("effect_stages"),
        });

        let frame_inputs = self.collect_frame_inputs(definition, inputs);
        let has_scalar_output = definition
            .node
            .outputs
            .iter()
            .any(|output| !matches!(output.kind, NodeOutputKind::Frame));
        let has_frame_output = definition
            .node
            .outputs
            .iter()
            .any(|output| matches!(output.kind, NodeOutputKind::Frame));

        if has_frame_output && frame_inputs.is_empty() {
            return Err(ExecutionError::NoFrameInput(definition.node.name.clone()));
        }
        if has_frame_output && has_scalar_output {
            return Err(ExecutionError::UnsupportedNodeOutputCombination);
        }

        let output_size = frame_inputs
            .first()
            .map(|frame| frame.size())
            .unwrap_or(wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            });

        let fallback_primary_texture = frame_inputs.is_empty().then(|| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("effect_stages_primary_fallback"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.target_format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        });
        let fallback_primary_view = fallback_primary_texture
            .as_ref()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
        let primary_input_view = if let Some(primary_frame) = frame_inputs.first() {
            primary_frame.view()
        } else {
            fallback_primary_view
                .as_ref()
                .expect("view exists when no frame input")
        };

        let additional_inputs: Vec<&wgpu::TextureView> = frame_inputs
            .iter()
            .skip(1)
            .map(|frame| frame.view())
            .collect();

        let scalar_output_texture = has_scalar_output.then(|| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("effect_stages_scalar_output"),
                size: output_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.target_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        });
        let scalar_output_view = scalar_output_texture
            .as_ref()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));

        let final_output_view = if let Some(view) = &scalar_output_view {
            std::sync::Arc::new(view.clone())
        } else {
            self.get_or_create_render_target(device, node_id, output_size)
        };
        let mut actual_output_view = final_output_view.clone();

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
                            &mut encoder,
                            primary_input_view,
                            &stage_additional_inputs,
                            &stage_output_view,
                            inputs,
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
                    let is_final_stage = stage_index + 1 == stages.len();
                    let storage_format = if is_final_stage {
                        target_format
                    } else {
                        wgpu::TextureFormat::Rgba16Float
                    };
                    let cache_key = format!(
                        "{}::{}::{:?}",
                        stage_name,
                        stage.source.display(),
                        storage_format
                    );

                    // Get or create compute pipeline
                    if !self.compute_pipeline_cache.contains_key(&cache_key) {
                        let compute_pipeline = ComputePipeline::from_shader(
                            device,
                            &shader_code,
                            &stage_definition,
                            storage_format,
                        )
                        .map_err(|e| ExecutionError::PipelineCreationError(e.to_string()))?;
                        self.compute_pipeline_cache
                            .insert(cache_key.clone(), compute_pipeline);
                    }

                    let stage_output_view = if is_final_stage && has_scalar_output {
                        // Scalar-output nodes read back from this texture, so final output must target it.
                        actual_output_view.clone()
                    } else {
                        // Frame-output compute stages need STORAGE_BINDING on their output texture.
                        self.get_or_create_compute_stage_target(
                            device,
                            node_id,
                            stage_index,
                            output_size,
                            storage_format,
                        )
                    };

                    let compute_pipeline = &self.compute_pipeline_cache[&cache_key];

                    let dispatch_override = if !has_frame_output && has_scalar_output {
                        // Scalar-only compute nodes write one pixel and should not fan out
                        // dispatch across full frame dimensions.
                        Some((1, 1, 1))
                    } else {
                        match stage.dispatch {
                            Some(dispatch) => match dispatch.mode {
                                AlgorithmStageDispatchMode::Auto => None,
                                AlgorithmStageDispatchMode::Rows => {
                                    Some((1, output_size.height, 1))
                                }
                                AlgorithmStageDispatchMode::Columns => {
                                    Some((output_size.width, 1, 1))
                                }
                                AlgorithmStageDispatchMode::AxisFromEnumInput => {
                                    let use_columns = dispatch
                                        .enum_input
                                        .as_ref()
                                        .and_then(|name| inputs.get(name))
                                        .and_then(|value| {
                                            if let NodeValue::Enum(enum_value) = value {
                                                Some(*enum_value == dispatch.columns_enum_value)
                                            } else {
                                                None
                                            }
                                        })
                                        .unwrap_or(false);

                                    if use_columns {
                                        Some((output_size.width, 1, 1))
                                    } else {
                                        Some((1, output_size.height, 1))
                                    }
                                }
                            },
                            None => None,
                        }
                    };

                    // Execute compute shader
                    compute_pipeline
                        .apply(
                            device,
                            queue,
                            &mut encoder,
                            primary_input_view,
                            &stage_additional_inputs,
                            stage_output_view.as_ref(),
                            inputs,
                            output_size,
                            dispatch_override,
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

        let mut scalar_data: Option<[f32; 4]> = None;
        if has_scalar_output {
            let scalar_texture = scalar_output_texture
                .as_ref()
                .ok_or(ExecutionError::GpuReadbackError(
                    "scalar output texture missing".to_string(),
                ))?;
            let bytes_per_pixel = 4_u32;
            let bytes_per_row = 256_u32;
            let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("effect_stages_scalar_readback"),
                size: bytes_per_row as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: scalar_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback_buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );

            queue.submit(Some(encoder.finish()));

            let slice = readback_buffer.slice(..);
            let (tx, rx) = mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
            let _ = device.poll(wgpu::PollType::Poll);

            let Ok(map_result) = rx.try_recv() else {
                return Err(ExecutionError::GpuReadbackNotReady);
            };
            map_result.map_err(|e| {
                ExecutionError::GpuReadbackError(format!("GPU readback map failed: {e:?}"))
            })?;

            {
                let data = slice.get_mapped_range();
                let pixel = &data[..bytes_per_pixel as usize];
                scalar_data = Some(decode_rgba8_like(self.target_format, pixel));
            }
            readback_buffer.unmap();
        } else {
            queue.submit(Some(encoder.finish()));
        }

        let mut outputs = HashMap::new();
        let mut scalar_channel_idx = 0usize;
        for output_def in &definition.node.outputs {
            match output_def.kind {
                NodeOutputKind::Frame => {
                    outputs.insert(
                        output_def.name.clone(),
                        NodeValue::Frame(output_frame.clone()),
                    );
                }
                NodeOutputKind::Float => {
                    let rgba = scalar_data.ok_or_else(|| {
                        ExecutionError::GpuReadbackError(
                            "missing scalar data for float output".to_string(),
                        )
                    })?;
                    let channel = rgba[scalar_channel_idx.min(3)];
                    scalar_channel_idx = scalar_channel_idx.saturating_add(1);
                    outputs.insert(output_def.name.clone(), NodeValue::Float(channel));
                }
                NodeOutputKind::Int => {
                    let rgba = scalar_data.ok_or_else(|| {
                        ExecutionError::GpuReadbackError(
                            "missing scalar data for int output".to_string(),
                        )
                    })?;
                    let channel = rgba[scalar_channel_idx.min(3)];
                    scalar_channel_idx = scalar_channel_idx.saturating_add(1);
                    outputs.insert(output_def.name.clone(), NodeValue::Int(channel.round() as i32));
                }
                NodeOutputKind::Bool => {
                    let rgba = scalar_data.ok_or_else(|| {
                        ExecutionError::GpuReadbackError(
                            "missing scalar data for bool output".to_string(),
                        )
                    })?;
                    let channel = rgba[scalar_channel_idx.min(3)];
                    scalar_channel_idx = scalar_channel_idx.saturating_add(1);
                    outputs.insert(output_def.name.clone(), NodeValue::Bool(channel > 0.5));
                }
                NodeOutputKind::Pixel => {
                    let rgba = scalar_data.ok_or_else(|| {
                        ExecutionError::GpuReadbackError(
                            "missing scalar data for pixel output".to_string(),
                        )
                    })?;
                    outputs.insert(output_def.name.clone(), NodeValue::Pixel(rgba));
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
    ) -> Vec<&'a GpuFrame> {
        let mut frame_inputs = Vec::new();

        for input_def in &definition.node.inputs {
            if matches!(input_def.kind, NodeInputKind::Frame)
                && let Some(NodeValue::Frame(frame)) = inputs.get(&input_def.name)
            {
                frame_inputs.push(frame);
            }
        }

        frame_inputs
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

fn decode_rgba8_like(format: wgpu::TextureFormat, pixel: &[u8]) -> [f32; 4] {
    if pixel.len() < 4 {
        return [0.0, 0.0, 0.0, 1.0];
    }

    match format {
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => [
            pixel[2] as f32 / 255.0,
            pixel[1] as f32 / 255.0,
            pixel[0] as f32 / 255.0,
            pixel[3] as f32 / 255.0,
        ],
        _ => [
            pixel[0] as f32 / 255.0,
            pixel[1] as f32 / 255.0,
            pixel[2] as f32 / 255.0,
            pixel[3] as f32 / 255.0,
        ],
    }
}

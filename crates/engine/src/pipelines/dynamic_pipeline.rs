use std::any::Any;
use std::collections::HashMap;

use crate::graph_executor::enums::ResolvedInput;
use crate::node_library::node::NodeInputKind;
use crate::node_library::node_definition::NodeDefinition;
use crate::pipelines::{
    Pipeline, common::create_linear_sampler, common::create_standard_bind_group_layout,
};

/// A dynamically created pipeline from a shader file
pub struct DynamicPipeline {
    sampler: wgpu::Sampler,
    bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    params_buf: wgpu::Buffer,
    name: String,

    /// Information about what parameters this shader expects
    param_layout: Vec<ShaderParam>,
}

#[derive(Debug, Clone)]
struct ShaderParam {
    name: String,
    kind: NodeInputKind,
    offset: usize,
}

impl DynamicPipeline {
    /// Create a new dynamic pipeline from shader code and node definition
    pub fn from_shader(
        device: &wgpu::Device,
        shader_code: &str,
        definition: &NodeDefinition,
        target_format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        let sampler = create_linear_sampler(device);
        let bgl =
            create_standard_bind_group_layout(device, &format!("bgl/{}", definition.node.name));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("layout/{}", definition.node.name)),
            bind_group_layouts: &[&bgl],
            ..Default::default()
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("shader/{}", definition.node.name)),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(shader_code)),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("pipeline/{}", definition.node.name)),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            cache: None,
            multiview_mask: None,
        });

        // Build parameter layout from node definition
        let param_layout = Self::build_param_layout(&definition.node.inputs);
        let params_size = Self::calculate_params_size(&param_layout);

        // Create uniform buffer
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("ubo/{}_params", definition.node.name)),
            size: params_size.max(16) as u64, // Minimum 16 bytes for uniform buffers
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            sampler,
            bgl,
            pipeline,
            params_buf,
            name: definition.node.name.clone(),
            param_layout,
        })
    }

    /// Build the parameter layout from node inputs
    fn build_param_layout(inputs: &[crate::node_library::node::NodeInput]) -> Vec<ShaderParam> {
        let mut params = Vec::new();
        let mut offset = 0usize;

        for input in inputs {
            // Skip Frame inputs (they're textures, not uniform params)
            if matches!(input.kind, NodeInputKind::Frame | NodeInputKind::Midi) {
                continue;
            }

            params.push(ShaderParam {
                name: input.name.clone(),
                kind: input.kind.clone(),
                offset,
            });

            // Calculate size and advance offset
            let size = match &input.kind {
                NodeInputKind::Bool { .. } => 4, // bool is 4 bytes in WGSL
                NodeInputKind::Int { .. } => 4,
                NodeInputKind::Float { .. } => 4,
                NodeInputKind::Pixel { .. } => 16,     // vec4<f32>
                NodeInputKind::Dimensions { .. } => 8, // 2x u32
                NodeInputKind::Text { .. } => 0,       // Text isn't passed to shaders
                NodeInputKind::Enum { .. } => 4,       // Passed as u32
                NodeInputKind::File { .. } => 0,       // Files aren't passed to shaders
                _ => 0,
            };

            // Align to 4 bytes
            offset += (size + 3) & !3;
        }

        params
    }

    /// Calculate total size needed for parameters buffer
    fn calculate_params_size(layout: &[ShaderParam]) -> usize {
        if layout.is_empty() {
            return 16; // Minimum size
        }

        let last = &layout[layout.len() - 1];
        let size = match &last.kind {
            NodeInputKind::Bool { .. } => 4,
            NodeInputKind::Int { .. } => 4,
            NodeInputKind::Float { .. } => 4,
            NodeInputKind::Pixel { .. } => 16,
            NodeInputKind::Dimensions { .. } => 8,
            NodeInputKind::Enum { .. } => 4,
            _ => 0,
        };

        // Align to 16 bytes (std140 layout requirement)
        ((last.offset + size) + 15) & !15
    }
}

impl Pipeline for DynamicPipeline {
    fn new(
        _device: &wgpu::Device,
        _target_format: wgpu::TextureFormat,
    ) -> Result<Self, crate::engine_errors::EngineError>
    where
        Self: Sized,
    {
        // Dynamic pipelines are created via from_shader(), not new()
        panic!("Use DynamicPipeline::from_shader() instead of new()")
    }

    fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bgl
    }

    fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    fn params_buffer(&self) -> &wgpu::Buffer {
        &self.params_buf
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn expected_param_type(&self) -> &str {
        "DynamicParams"
    }

    fn update_params(
        &self,
        queue: &wgpu::Queue,
        params: &dyn Any,
    ) -> Result<(), crate::engine_errors::EngineError> {
        // Params come as DynamicParams (HashMap<String, ResolvedInput>)
        if let Some(param_map) = params.downcast_ref::<HashMap<String, ResolvedInput>>() {
            // Build byte buffer from params
            let params_size = Self::calculate_params_size(&self.param_layout);
            let mut buffer = vec![0u8; params_size];

            for param in &self.param_layout {
                if let Some(value) = param_map.get(&param.name) {
                    Self::write_param_to_buffer(&mut buffer, param, value);
                }
            }

            queue.write_buffer(&self.params_buf, 0, &buffer);
            Ok(())
        } else {
            Err(crate::engine_errors::EngineError::InvalidParamType {
                pipeline: self.name.clone(),
                expected: "HashMap<String, ResolvedInput>".to_string(),
                actual: std::any::type_name_of_val(params).to_string(),
            })
        }
    }
}

impl DynamicPipeline {
    /// Write a single parameter value to the buffer at the correct offset
    fn write_param_to_buffer(buffer: &mut [u8], param: &ShaderParam, value: &ResolvedInput) {
        let offset = param.offset;

        match (&param.kind, value) {
            (NodeInputKind::Bool { .. }, ResolvedInput::Bool(b)) => {
                let val = if *b { 1u32 } else { 0u32 };
                buffer[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
            }
            (NodeInputKind::Int { .. }, ResolvedInput::Int(i)) => {
                buffer[offset..offset + 4].copy_from_slice(&i.to_le_bytes());
            }
            (NodeInputKind::Float { .. }, ResolvedInput::Float(f)) => {
                buffer[offset..offset + 4].copy_from_slice(&f.to_le_bytes());
            }
            (NodeInputKind::Pixel { .. }, ResolvedInput::Pixel(p)) => {
                for (i, &component) in p.iter().enumerate() {
                    let bytes = component.to_le_bytes();
                    buffer[offset + i * 4..offset + i * 4 + 4].copy_from_slice(&bytes);
                }
            }
            (NodeInputKind::Dimensions { .. }, ResolvedInput::Dimensions(w, h)) => {
                buffer[offset..offset + 4].copy_from_slice(&w.to_le_bytes());
                buffer[offset + 4..offset + 8].copy_from_slice(&h.to_le_bytes());
            }
            (NodeInputKind::Enum { .. }, ResolvedInput::Enum(idx)) => {
                let idx_u32 = *idx as u32;
                buffer[offset..offset + 4].copy_from_slice(&idx_u32.to_le_bytes());
            }
            _ => {
                // Type mismatch or unsupported type
                eprintln!("Warning: Type mismatch for parameter {}", param.name);
            }
        }
    }
}

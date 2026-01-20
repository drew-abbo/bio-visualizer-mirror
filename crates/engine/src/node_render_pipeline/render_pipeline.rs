use std::any::Any;
use std::collections::HashMap;

use crate::engine_errors::EngineError;
use crate::graph_executor::enums::ResolvedInput;
use crate::node::NodeDefinition;
use crate::node::node::NodeInputKind;
use crate::node_render_pipeline::helpers::create_linear_sampler;
use crate::node_render_pipeline::pipeline_base::PipelineBase;

/// Pipeline created dynamically from node definition and WGSL shader.
/// Automatically configures bind groups based on Frame input count and parameter types.
pub struct NodeRenderPipeline {
    // GPU resources
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,

    // Pipeline metadata
    name: String,
    texture_input_count: usize,
    param_layout: Vec<ShaderParam>,
}

#[derive(Debug, Clone)]
struct ShaderParam {
    name: String,
    kind: NodeInputKind,
    offset: usize,
}

impl PipelineBase for NodeRenderPipeline {
    fn apply(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        primary_input: &wgpu::TextureView,
        additional_inputs: &[&wgpu::TextureView],
        output: &wgpu::TextureView,
        params: &dyn Any,
    ) -> Result<(), EngineError> {
        // Validate input count
        let expected = self.texture_input_count.saturating_sub(1);
        if additional_inputs.len() != expected {
            return Err(EngineError::InvalidInputCount {
                expected,
                actual: additional_inputs.len(),
            });
        }

        // Update uniform buffer
        if let Some(param_map) = params.downcast_ref::<HashMap<String, ResolvedInput>>() {
            let params_size = Self::calculate_params_size(&self.param_layout);
            let mut buffer = vec![0u8; params_size];
            for param in &self.param_layout {
                if let Some(value) = param_map.get(&param.name) {
                    Self::write_param_to_buffer(&mut buffer, param, value);
                }
            }
            queue.write_buffer(&self.params_buf, 0, &buffer);
        } else {
            return Err(EngineError::InvalidParamType {
                pipeline: self.name.clone(),
                expected: "HashMap<String, ResolvedInput>".to_string(),
                actual: std::any::type_name_of_val(params).to_string(),
            });
        }

        // Build bind group
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&self.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(primary_input),
            },
        ];
        for (i, texture) in additional_inputs.iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: (i + 2) as u32,
                resource: wgpu::BindingResource::TextureView(texture),
            });
        }
        entries.push(wgpu::BindGroupEntry {
            binding: (self.texture_input_count + 1) as u32,
            resource: self.params_buf.as_entire_binding(),
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("bg/{}", self.name)),
            layout: &self.bgl,
            entries: &entries,
        });

        // Render pass
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("effect_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            ..Default::default()
        });
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..3, 0..1);

        Ok(())
    }
}

impl NodeRenderPipeline {
    /// Create pipeline from WGSL shader code and node definition
    pub fn from_shader(
        device: &wgpu::Device,
        shader_code: &str,
        definition: &NodeDefinition,
        target_format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        let sampler = create_linear_sampler(device);

        // Count how many Frame inputs this node has
        let texture_input_count = definition
            .node
            .inputs
            .iter()
            .filter(|input| matches!(input.kind, NodeInputKind::Frame))
            .count();

        let bgl =
            Self::create_bind_group_layout(device, &definition.node.name, texture_input_count);

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

        // Create uniform buffer (minimum 32 bytes to satisfy std140 alignment)
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("ubo/{}_params", definition.node.name)),
            size: params_size.max(32) as u64,
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
            texture_input_count,
        })
    }

    /// Write parameter to buffer at correct offset with proper alignment
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

    /// Create bind group layout dynamically based on texture count
    fn create_bind_group_layout(
        device: &wgpu::Device,
        name: &str,
        texture_count: usize,
    ) -> wgpu::BindGroupLayout {
        let mut entries = Vec::new();

        // Binding 0: Sampler
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });

        // Bindings 1..1+texture_count: Textures
        for i in 0..texture_count {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: (i + 1) as u32,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
        }

        // Last binding: Uniform buffer for parameters
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: (texture_count + 1) as u32,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });

        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("bgl/{}", name)),
            entries: &entries,
        })
    }

    /// Extract non-texture parameters and calculate their buffer offsets
    fn build_param_layout(inputs: &[crate::node::node::NodeInput]) -> Vec<ShaderParam> {
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

    /// Calculate uniform buffer size with std140 alignment
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

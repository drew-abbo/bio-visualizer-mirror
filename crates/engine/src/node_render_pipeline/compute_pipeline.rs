//! Compute pipeline abstraction for algorithm stages that require buffer operations.
//!
//! A [ComputePipeline] is to compute shaders what [crate::node_render_pipeline::NodeRenderPipeline]
//! is to render shaders. It:
//! - Compiles a WGSL compute shader
//! - Manages bind group layout and bind group for textures, buffers, and parameters
//! - Executes compute dispatches and handles buffer read-back
//!
//! Compute stages are used by algorithmic effects like pixel sorting, optical flow,
//! and datamoshing that need to process pixel data as structured buffers.

use std::any::Any;
use std::collections::HashMap;

use crate::engine_errors::EngineError;
use crate::graph_executor::NodeValue;
use crate::node::NodeDefinition;
use crate::node::engine_node::NodeInputKind;
use crate::node_render_pipeline::helpers::{align_to, uniform_param_size};

/// Runtime compute pipeline constructed from WGSL and a [NodeDefinition].
/// Responsible for creating the [wgpu::ComputePipeline], bind group layout,
/// uniform buffer for parameters, and managing intermediate compute buffers.
pub struct ComputePipeline {
    // GPU resources
    pipeline: wgpu::ComputePipeline,
    bgl: wgpu::BindGroupLayout,
    params_buf: wgpu::Buffer,

    // Pipeline metadata
    name: String,
    texture_input_count: usize,
    param_layout: Vec<ComputeShaderParam>,

    // Compute dispatch dimensions (workgroup size)
    // Default: 8x8 workgroups; will be customized by node definition if needed
    workgroup_size: (u32, u32, u32),
}

/// Internal representation of a compute shader parameter.
#[derive(Debug, Clone)]
struct ComputeShaderParam {
    name: String,
    kind: NodeInputKind,
    offset: usize,
}

impl ComputePipeline {
    /// Create a compute pipeline from shader source and node definition.
    pub fn from_shader(
        device: &wgpu::Device,
        shader_code: &str,
        definition: &NodeDefinition,
        storage_format: wgpu::TextureFormat,
    ) -> Result<Self, EngineError> {
        // Compile shader module
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&definition.node.name),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(shader_code)),
        });

        // Count texture inputs (Frame inputs)
        let texture_input_count = definition
            .node
            .inputs
            .iter()
            .filter(|input| matches!(input.kind, NodeInputKind::Frame))
            .count();

        // Build parameter layout (non-texture inputs)
        let param_layout = Self::build_param_layout(&definition.node.inputs);
        let params_buf_size = param_layout.last().map(|p| p.offset + 16).unwrap_or(0) as u64;

        // Create bind group layout
        // Binding 0: storage for output (unused for now; compute reads/writes via function)
        // Binding 1+: input textures
        // Last binding: uniform parameter buffer
        let bgl_entries = Self::build_bgl_entries(texture_input_count, storage_format);
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{} compute bgl", definition.node.name)),
            entries: &bgl_entries,
        });

        // Create uniform buffer for parameters
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{} compute params", definition.node.name)),
            size: params_buf_size.max(256), // Ensure minimum size for alignment
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(&definition.node.name),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(&format!("{} compute layout", definition.node.name)),
                    bind_group_layouts: &[&bgl],
                    push_constant_ranges: &[],
                }),
            ),
            module: &shader_module,
            entry_point: Some("cs_main"),
            cache: None,
            compilation_options: Default::default(),
        });

        Ok(Self {
            pipeline,
            bgl,
            params_buf,
            name: definition.node.name.clone(),
            texture_input_count,
            param_layout,
            workgroup_size: (8, 8, 1),
        })
    }

    /// Execute the compute pipeline with input textures and parameters.
    ///
    /// # Arguments
    /// * `output_texture` - Output storage texture for compute shaders to write to.
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        primary_input: &wgpu::TextureView,
        additional_inputs: &[&wgpu::TextureView],
        output_texture: &wgpu::TextureView,
        params: &dyn Any,
        output_size: wgpu::Extent3d,
        dispatch_override: Option<(u32, u32, u32)>,
    ) -> Result<(), EngineError> {
        // Validate texture input count
        let total_inputs = 1 + additional_inputs.len();
        if total_inputs < self.texture_input_count {
            return Err(EngineError::UnsupportedOperation(format!(
                "Expected {} texture inputs, got {}",
                self.texture_input_count, total_inputs
            )));
        }

        // Write parameters
        if let Some(params_map) = params.downcast_ref::<HashMap<String, NodeValue>>() {
            Self::write_params_to_buffer(queue, &self.params_buf, &self.param_layout, params_map)?;
        }

        // Create bind group
        let mut bg_entries = vec![];
        let mut binding = 0u32;

        // Bind input textures
        let mut all_inputs = vec![primary_input];
        all_inputs.extend(additional_inputs);

        for (i, texture_view) in all_inputs.iter().enumerate() {
            if i < self.texture_input_count {
                bg_entries.push(wgpu::BindGroupEntry {
                    binding,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                });
                binding += 1;
            }
        }

        // Bind output storage texture
        bg_entries.push(wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(output_texture),
        });
        binding += 1;

        // Bind parameter buffer
        bg_entries.push(wgpu::BindGroupEntry {
            binding,
            resource: self.params_buf.as_entire_binding(),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} compute bg", self.name)),
            layout: &self.bgl,
            entries: &bg_entries,
        });

        // Execute compute dispatch
        let (workgroup_x, workgroup_y, workgroup_z) = dispatch_override.unwrap_or((
            output_size.width.div_ceil(self.workgroup_size.0),
            output_size.height.div_ceil(self.workgroup_size.1),
            1,
        ));

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(&self.name),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(workgroup_x, workgroup_y, workgroup_z);
        }

        Ok(())
    }

    fn build_param_layout(
        inputs: &[crate::node::engine_node::NodeInput],
    ) -> Vec<ComputeShaderParam> {
        let mut layout = Vec::new();
        let mut offset = 0;

        for input in inputs {
            if matches!(input.kind, NodeInputKind::Frame) {
                continue; // Skip frame inputs
            }

            let size = uniform_param_size(&input.kind);
            layout.push(ComputeShaderParam {
                name: input.name.clone(),
                kind: input.kind.clone(),
                offset,
            });
            offset = align_to(offset + size, 4);
        }

        layout
    }

    fn build_bgl_entries(
        texture_count: usize,
        target_format: wgpu::TextureFormat,
    ) -> Vec<wgpu::BindGroupLayoutEntry> {
        let mut entries = Vec::new();

        // Input texture bindings
        for i in 0..texture_count {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: i as u32,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
        }

        // Output storage texture binding (for scatter/output stages)
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: texture_count as u32,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::WriteOnly,
                format: target_format,
                view_dimension: wgpu::TextureViewDimension::D2,
            },
            count: None,
        });

        // Parameter buffer binding
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: (texture_count + 1) as u32,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });

        entries
    }

    fn write_params_to_buffer(
        queue: &wgpu::Queue,
        buffer: &wgpu::Buffer,
        layout: &[ComputeShaderParam],
        params: &HashMap<String, NodeValue>,
    ) -> Result<(), EngineError> {
        // Marshal parameters into a buffer using simple std140-like alignment
        let total_size = layout
            .last()
            .map(|p| align_to(p.offset + uniform_param_size(&p.kind), 16))
            .unwrap_or(0);
        let mut data = vec![0u8; total_size];

        for param in layout {
            if let Some(value) = params.get(&param.name) {
                Self::write_value_to_buffer(&mut data, param.offset, value, &param.kind)?;
            }
        }

        queue.write_buffer(buffer, 0, &data);
        Ok(())
    }

    fn write_value_to_buffer(
        data: &mut [u8],
        offset: usize,
        value: &NodeValue,
        _kind: &NodeInputKind,
    ) -> Result<(), EngineError> {
        match value {
            NodeValue::Bool(b) => {
                let bytes = (*b as u32).to_le_bytes();
                data[offset..offset + 4].copy_from_slice(&bytes);
            }
            NodeValue::Int(i) => {
                let bytes = i.to_le_bytes();
                data[offset..offset + 4].copy_from_slice(&bytes);
            }
            NodeValue::Float(f) => {
                let bytes = f.to_le_bytes();
                data[offset..offset + 4].copy_from_slice(&bytes);
            }
            NodeValue::Dimensions(w, h) => {
                let bytes_w = w.to_le_bytes();
                let bytes_h = h.to_le_bytes();
                data[offset..offset + 4].copy_from_slice(&bytes_w);
                data[offset + 4..offset + 8].copy_from_slice(&bytes_h);
            }
            NodeValue::Pixel([r, g, b, a]) => {
                let bytes_r = r.to_le_bytes();
                let bytes_g = g.to_le_bytes();
                let bytes_b = b.to_le_bytes();
                let bytes_a = a.to_le_bytes();
                data[offset..offset + 4].copy_from_slice(&bytes_r);
                data[offset + 4..offset + 8].copy_from_slice(&bytes_g);
                data[offset + 8..offset + 12].copy_from_slice(&bytes_b);
                data[offset + 12..offset + 16].copy_from_slice(&bytes_a);
            }
            NodeValue::Enum(idx) => {
                let bytes = (*idx as u32).to_le_bytes();
                data[offset..offset + 4].copy_from_slice(&bytes);
            }
            _ => {} // Text, File, Frame, Device not supported in compute params
        }
        Ok(())
    }
}

use super::common::{self, Pipeline};
use crate::errors::EngineError;
use crate::types::ColorGradingParams;
use std::any::Any;

#[derive(Debug)]
pub struct ColorGradingPipeline {
    sampler: wgpu::Sampler,
    bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    params_buf: wgpu::Buffer,
}

impl Pipeline for ColorGradingPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Result<Self, EngineError>
    where
        Self: Sized,
    {
        let sampler = common::create_nearest_sampler(device);
        let bgl = common::create_standard_bind_group_layout(device, "bgl/color_grading");

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layout/color_grading"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader/color_grading"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "../shaders/color_grading.wgsl"
            ))),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline/color_grading"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("main_vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("main_fs"),
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
            multiview: None,
            cache: None,
        });

        // Could remove this and supplement an error if there were no params uploaded to the pipeline
        let params_buf = create_default_params_buffer(device, "ubo/color_grading_params");

        Ok(Self {
            sampler,
            bgl,
            pipeline,
            params_buf,
        })
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
        "ColorGrading"
    }

    fn expected_param_type(&self) -> &str {
        "ColorGradingParams"
    }

    fn update_params(&self, queue: &wgpu::Queue, params: &dyn Any) -> Result<(), EngineError> {
        if let Some(cg_params) = params.downcast_ref::<ColorGradingParams>() {
            // Use bytemuck for safe POD-to-bytes conversion
            let bytes = bytemuck::bytes_of(cg_params);

            let expected_size = std::mem::size_of::<ColorGradingParams>();
            if bytes.len() != expected_size {
                return Err(EngineError::BufferSizeMismatch {
                    expected: expected_size,
                    actual: bytes.len(),
                });
            }

            queue.write_buffer(&self.params_buf, 0, bytes);
            Ok(())
        } else {
            Err(EngineError::InvalidParamType {
                pipeline: self.name().to_string(),
                expected: self.expected_param_type().to_string(),
                actual: std::any::type_name_of_val(params).to_string(),
            })
        }
    }
}

pub fn create_default_params_buffer(device: &wgpu::Device, label: &str) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;

    let params = ColorGradingParams::default();

    let bytes = bytemuck::bytes_of(&params);

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

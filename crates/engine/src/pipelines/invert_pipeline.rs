use crate::engine_errors::EngineError;
use crate::pipelines::common::{
    Pipeline, create_empty_params_buffer, create_linear_sampler, create_standard_bind_group_layout,
};
use std::any::Any;

use crate::pipelines::shaders::invert_shader::INVERT_SHADER;

pub struct InvertPipeline {
    sampler: wgpu::Sampler,
    bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    params_buf: wgpu::Buffer,
}

impl Pipeline for InvertPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Result<Self, EngineError> {
        let sampler = create_linear_sampler(device);
        let bgl = create_standard_bind_group_layout(device, "bgl/invert");

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layout/invert"),
            bind_group_layouts: &[&bgl],
            ..Default::default()
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader/invert"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(INVERT_SHADER)),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline/invert"),
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

        let params_buf = create_empty_params_buffer(device, "ubo/invert");

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
        "Invert"
    }
    fn expected_param_type(&self) -> &str {
        "EmptyParams"
    }

    fn update_params(&self, _queue: &wgpu::Queue, _params: &dyn Any) -> Result<(), EngineError> {
        Ok(()) // No params needed
    }
}

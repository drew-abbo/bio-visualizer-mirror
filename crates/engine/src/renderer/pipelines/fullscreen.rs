use crate::renderer::ParamsUbo;
use super::common::{self, Pipeline};

pub struct FullScreenPipeline {
    sampler: wgpu::Sampler,
    bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    params_buf: wgpu::Buffer,
}

impl Pipeline for FullScreenPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let sampler = common::create_nearest_sampler(device);
        let bgl = common::create_standard_bind_group_layout(device, "bgl/fullscreen");

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layout/fullscreen"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader/fullscreen"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "../shaders/fullscreen.wgsl"
            ))),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline/fullscreen"),
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

        let params_buf = common::create_default_params_buffer(device, "ubo/fullscreen_params");

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
}

impl FullScreenPipeline {
    pub fn update_params(&self, queue: &wgpu::Queue, params: &ParamsUbo) {
        <Self as Pipeline>::update_params(self, queue, params)
    }

    /// just uses the default bind group creation from the trait for now
    pub fn bind_group_for(
        &self,
        device: &wgpu::Device,
        tex_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        <Self as Pipeline>::bind_group_for(self, device, tex_view)
    }
}
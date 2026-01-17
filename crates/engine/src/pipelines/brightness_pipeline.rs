use crate::engine_errors::EngineError;
use crate::pipelines::common::{
    Pipeline, create_linear_sampler, create_standard_bind_group_layout,
};
use std::any::Any;

use crate::pipelines::shaders::brightness_shader::BRIGHTNESS_SHADER;

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct BrightnessParams {
    pub brightness: f32,
    pub _padding: [f32; 3],
}

unsafe impl bytemuck::Pod for BrightnessParams {}
unsafe impl bytemuck::Zeroable for BrightnessParams {}

pub struct BrightnessPipeline {
    sampler: wgpu::Sampler,
    bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    params_buf: wgpu::Buffer,
}

impl Pipeline for BrightnessPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Result<Self, EngineError> {
        let sampler = create_linear_sampler(device);
        let bgl = create_standard_bind_group_layout(device, "bgl/brightness");

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layout/brightness"),
            bind_group_layouts: &[&bgl],
            ..Default::default()
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader/brightness"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(BRIGHTNESS_SHADER)),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline/brightness"),
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

        let params_buf = {
            use wgpu::util::DeviceExt;
            let params = BrightnessParams {
                brightness: 1.0,
                _padding: [0.0; 3],
            };
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ubo/brightness"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        };

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
        "Brightness"
    }
    fn expected_param_type(&self) -> &str {
        "BrightnessParams"
    }

    fn update_params(&self, queue: &wgpu::Queue, params: &dyn Any) -> Result<(), EngineError> {
        if let Some(p) = params.downcast_ref::<BrightnessParams>() {
            queue.write_buffer(&self.params_buf, 0, bytemuck::bytes_of(p));
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

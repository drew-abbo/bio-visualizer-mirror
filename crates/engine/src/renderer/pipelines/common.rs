use crate::errors::PipelineError;
use std::any::Any;

pub trait Pipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Result<Self, PipelineError>
    where
        Self: Sized;

    fn pipeline(&self) -> &wgpu::RenderPipeline;
    fn bind_group_layout(&self) -> &wgpu::BindGroupLayout;
    fn sampler(&self) -> &wgpu::Sampler;
    fn params_buffer(&self) -> &wgpu::Buffer;
    
    /// Get the name of this pipeline for error messages
    fn name(&self) -> &str;
    
    /// Get the expected parameter type name for error messages
    fn expected_param_type(&self) -> &str;

    /// Update the params uniform buffer with new values
    /// Pipelines should downcast the params to their expected type
    fn update_params(&self, queue: &wgpu::Queue, params: &dyn Any) -> Result<(), PipelineError>;

    fn bind_group_for(&self, device: &wgpu::Device, tex_view: &wgpu::TextureView) -> wgpu::BindGroup {
        self.create_standard_bind_group(device, tex_view, None)
    }

    fn create_standard_bind_group(
        &self,
        device: &wgpu::Device,
        tex_view: &wgpu::TextureView,
        label: Option<&str>,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: self.bind_group_layout(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(self.sampler()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(tex_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buffer().as_entire_binding(),
                },
            ],
        })
    }

    fn apply(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        params: &dyn Any,
    ) -> Result<(), PipelineError> {
        // Update parameters
        self.update_params(queue, params)?;
        
        // Create bind group
        let bind_group = self.bind_group_for(device, input);

        // Create render pass
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
        });

        rpass.set_pipeline(self.pipeline());
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..3, 0..1);
        
        Ok(())
    }
}

/// Creates a basic nearest-neighbor sampler
/// used for video playback, pixel perfect rendering, when texture coordinates align with pixels
pub fn create_nearest_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("sampler/nearest"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    })
}

/// Creates a linear (smooth) sampler
/// used for blur effects, scaling images, creating smooth visuals, and anti-aliasing
pub fn create_linear_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("sampler/linear"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    })
}

/// Helper to create the standard bind group layout used by most pipelines
/// (sampler + texture + uniform buffer)
pub fn create_standard_bind_group_layout(
    device: &wgpu::Device,
    label: &str,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[
            // binding 0: sampler
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // binding 1: texture_2d<f32>
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            // binding 2: uniform buffer (params)
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}
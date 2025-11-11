use crate::renderer::ParamsUbo;

/// Common utilities for all pipelines

/// Trait that all effect pipelines should implement
pub trait Pipeline<> {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> anyhow::Result<Self>
    where
        Self: Sized;

    fn pipeline(&self) -> &wgpu::RenderPipeline;

    fn bind_group_layout(&self) -> &wgpu::BindGroupLayout;

    fn sampler(&self) -> &wgpu::Sampler;

    fn params_buffer(&self) -> &wgpu::Buffer;

    /// Update the params uniform buffer with new values
    fn update_params(&self, queue: &wgpu::Queue, params: &ParamsUbo) {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (params as *const ParamsUbo) as *const u8,
                std::mem::size_of::<ParamsUbo>(),
            )
        };
        queue.write_buffer(self.params_buffer(), 0, bytes);
    }

    /// Create a bind group for the given source texture view
    /// This assumes the standard layout: binding 0 = sampler, 1 = texture, 2 = params
    /// If you look in the shaders you will see things like: 
    /// 
    /// Binding 0: Sampler
    /// @group(0) @binding(0) var samp: sampler;

    /// Binding 1: Texture
    /// @group(0) @binding(1) var vid_tex: texture_2d<f32>;

    /// Binding 2: Uniform buffer (params)
    /// @group(0) @binding(2) var<uniform> params: Params;
    fn bind_group_for(
        &self,
        device: &wgpu::Device,
        tex_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        // Default implementation uses the standard bind group
        self.create_standard_bind_group(device, tex_view, None)
    }

    /// Create a standard bind group with optional custom label
    /// This is considered default for now it just depends on how we want to setup our shaders
    /// If another shader needs a different layout it can implement its own version of bind_group_for above
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

    /// Apply the effect provided by the pipline and the given parameters
    fn apply(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        params: &ParamsUbo,
    ) {
        // Update parameters
        self.update_params(queue, params);
        
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

/// Helper to initialize a default ParamsUbo buffer
pub fn create_default_params_buffer(device: &wgpu::Device, label: &str) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;

    let params = ParamsUbo {
        exposure: 1.0,
        contrast: 1.0,
        saturation: 1.0,
        vignette: 0.5,
        time: 0.0,
        surface_w: 0.0,
        surface_h: 0.0,
        _pad0: 0.0,
    };

    let bytes = unsafe {
        std::slice::from_raw_parts(
            (&params as *const ParamsUbo) as *const u8,
            std::mem::size_of::<ParamsUbo>(),
        )
    };

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

use crate::renderer;
use crate::renderer::ParamsUbo;
use wgpu::util::DeviceExt;

pub struct Pipelines {
    pub sampler: wgpu::Sampler,
    pub bgl: wgpu::BindGroupLayout,
    pub pipeline: wgpu::RenderPipeline,
    pub params_buf: wgpu::Buffer,
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        // Simple non-filtering sampler (nearest). Swap to Linear if you want smoothing.
        // Decides how to read pixels from the texture
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sampler/nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        // Declares "I need a texture and a sampler" (like declaring function parameters)
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bgl/video+params"),
            entries: &[
                // binding 0: sampler  (Filtering if you use textureSample)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 1: texture_2d<f32>  (filterable: true if you use textureSample)
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
                // binding 2: uniform buffer
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
        });

        // Organizes all the bind groups a shader needs
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layout/render"),
            bind_group_layouts: &[&bgl], // only one for now
            push_constant_ranges: &[],
        });

        // Load shader from external file at compile time
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader/frame-blit"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shaders/fullscreen.wgsl"
            ))),
        });

        // Combines everything above into a complete rendering setup
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline/frame-blit"),
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
                    format: surface_format,
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

        let params = renderer::ParamsUbo {
            exposure: 0.25,
            contrast: 1.1,
            saturation: 0.9,
            vignette: 0.2,
            time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f32(),
            _pad0: 0.0,
            surface_h: 5.0,
            surface_w: 20.0,
        };

        let bytes = unsafe {
            std::slice::from_raw_parts(
                (&params as *const ParamsUbo) as *const u8,
                std::mem::size_of::<ParamsUbo>(),
            )
        };

        let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ubo/params"),
            contents: bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Ok(Self {
            sampler,
            bgl,
            pipeline,
            params_buf,
        })
    }

    /// Create a bind group for the given source texture view.
    pub fn bind_group_for(
        &self,
        device: &wgpu::Device,
        tex_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg/frame"),
            layout: &self.bgl,
            entries: &[
                // binding 0: sampler
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                // binding 1: texture
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(tex_view),
                },
                // binding 2: params UBO
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
            ],
        })
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &ParamsUbo) {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (params as *const ParamsUbo) as *const u8,
                std::mem::size_of::<ParamsUbo>(),
            )
        };
        queue.write_buffer(&self.params_buf, 0, bytes);
    }
}

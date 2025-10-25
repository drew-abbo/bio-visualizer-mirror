use std::sync::Arc;
use winit::{event_loop::ActiveEventLoop, keyboard::KeyCode, window::Window};

// Labels used for GPU objects
mod labels {
    pub const SHADER: &str = "shader/frame";
    pub const FRAME_BGL: &str = "bgl/frame";
    pub const FRAME_SAMPLER: &str = "sampler/frame";
    pub const PIPELINE_LAYOUT: &str = "layout/render";
    pub const PIPELINE: &str = "pipeline/frame-blit";
    pub const ENCODER: &str = "encoder/render";
    pub const PASS_PRESENT: &str = "pass/present";
    pub const FRAME_BIND_GROUP: &str = "bg/frame";
}

pub struct State {
    // Swapchain and device
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,

    // Pipeline stuff
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,

    // Dynamic frame resources, created on first upload or when size changes
    frame_tex: Option<wgpu::Texture>,
    frame_view: Option<wgpu::TextureView>,
    frame_size: (u32, u32),
    bind_group: Option<wgpu::BindGroup>,

    pub(crate) window: Arc<Window>,
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let (instance, surface) = Self::create_instance_and_surface(&window)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let config = Self::make_surface_config(&surface, &adapter, window.inner_size());
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(labels::SHADER),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(labels::FRAME_BGL),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(labels::FRAME_SAMPLER),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(labels::PIPELINE_LAYOUT),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(labels::PIPELINE),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            render_pipeline,
            bind_group_layout,
            sampler,
            frame_tex: None,
            frame_view: None,
            frame_size: (0, 0),
            bind_group: None,
            window,
        })
    }

    #[inline]
    fn create_instance_and_surface(
        window: &Arc<Window>,
    ) -> anyhow::Result<(wgpu::Instance, wgpu::Surface<'static>)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        // Safety: window outlives surface via Arc<Window>
        let surface = instance.create_surface(window.clone())?;
        Ok((instance, surface))
    }

    #[inline]
    fn make_surface_config(
        surface: &wgpu::Surface<'static>,
        adapter: &wgpu::Adapter,
        size: winit::dpi::PhysicalSize<u32>,
    ) -> wgpu::SurfaceConfiguration {
        let caps = surface.get_capabilities(adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: caps.present_modes[0],
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.is_surface_configured = true;
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.window.request_redraw();
        if !self.is_surface_configured {
            return Ok(());
        }

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(labels::ENCODER),
            });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(labels::PASS_PRESENT),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.05,
                        g: 0.05,
                        b: 0.05,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        if let Some(bg) = &self.bind_group {
            render_pass.set_bind_group(0, bg, &[]);
        }

        render_pass.draw(0..3, 0..1);

        drop(render_pass);

        self.queue.submit(Some(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn handle_key(&self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        if (code, is_pressed) == (KeyCode::Escape, true) {
            event_loop.exit();
        }
    }

    /// Upload an RGBA8 frame (width x height x 4 bytes) to a GPU texture.
    /// Rebuilds the bind group if the resolution changes.
    pub fn upload_rgba_frame(&mut self, width: u32, height: u32, rgba: &[u8]) {
        let needs_new_tex = self
            .frame_tex
            .as_ref()
            .map(|t| {
                let s = t.size();
                s.width != width || s.height != height
            })
            .unwrap_or(true);

        if needs_new_tex {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("tex/frame"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm, // linear for now
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());

            self.frame_tex = Some(tex);
            self.frame_view = Some(view);
            self.frame_size = (width, height);

            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(labels::FRAME_BIND_GROUP),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            self.frame_view.as_ref().unwrap(),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.bind_group = Some(bg);
        }

        let (w, _h) = (width, height);
        self.queue.write_texture(
            // Keep the same API you’re already using
            wgpu::TexelCopyTextureInfo {
                texture: self.frame_tex.as_ref().unwrap(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn update(&mut self) {
        // Seed a synthetic frame once so we can see pixels immediately.
        if self.frame_tex.is_none() {
            let (w, h) = (640, 360);
            let mut data = vec![0u8; (w * h * 4) as usize];
            for y in 0..h {
                for x in 0..w {
                    let i = ((y * w + x) * 4) as usize;
                    data[i + 0] = (x % 256) as u8;
                    data[i + 1] = (y % 256) as u8;
                    data[i + 2] = ((x + y) % 256) as u8;
                    data[i + 3] = 255;
                }
            }
            self.upload_rgba_frame(w, h, &data);
        }
    }
}

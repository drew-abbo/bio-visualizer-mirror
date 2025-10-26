use image::GenericImageView;
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
                required_limits: wgpu::Limits::default(),
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

    pub fn submit_rgba_frame(&mut self, frame: &crate::video::RgbaFrame) {
        // (Re)create texture if size changed
        let needs_new_tex = self.frame_tex.as_ref().map_or(true, |t| {
            let s = t.size();
            s.width != frame.width || s.height != frame.height
        });
        if needs_new_tex {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("tex/stream-frame"),
                size: wgpu::Extent3d {
                    width: frame.width,
                    height: frame.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            self.frame_tex = Some(tex);
            self.frame_view = Some(view);
            self.bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bg/stream-frame"),
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
            }));
            self.frame_size = (frame.width, frame.height);
        }

        // Upload
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: self.frame_tex.as_ref().unwrap(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(frame.bpr),
                rows_per_image: Some(frame.height),
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );
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
            render_pass.draw(0..3, 0..1);
        } else {
            // No frame bound yet — just clear this frame (no draw).
            // (Nothing else to do; we already cleared the target color above.)
        }

        // render_pass.draw(0..3, 0..1);

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

    pub fn load_image_from_bytes(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        let img = image::load_from_memory(bytes)?;
        let (w, h) = img.dimensions();
        let rgba = img.to_rgba8();
        let (padded, bpr) = Self::pad_rows_rgba(&rgba, w, h);

        // (re)create texture/bind group if needed
        let needs_new_tex = self.frame_tex.as_ref().map_or(true, |t| {
            let s = t.size();
            s.width != w || s.height != h
        });
        if needs_new_tex {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("tex/image"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            self.frame_tex = Some(tex);
            self.frame_view = Some(view);
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
            self.frame_size = (w, h);
        }

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: self.frame_tex.as_ref().unwrap(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &padded,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bpr),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }

    pub fn update(&mut self) {
        // purely for testing: load a test image once
        if self.frame_tex.is_none() {
            let bytes = include_bytes!("./assets/test.png");
            if let Err(e) = self.load_image_from_bytes(bytes) {
                eprintln!("load_image_from_bytes failed: {e}");
            }
        }
    }

    #[inline]
    fn create_instance_and_surface(
        window: &Arc<Window>,
    ) -> anyhow::Result<(wgpu::Instance, wgpu::Surface<'static>)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
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

    /// Pads RGBA image data so that each row is aligned to wgpu::COPY_BYTES_PER_ROW_ALIGNMENT.
    /// Returns the padded data and the new bytes_per_row value.
    ///
    fn pad_rows_rgba(src: &[u8], width: u32, height: u32) -> (Vec<u8>, u32) {
        let bytes_per_pixel = 4u32;
        let unpadded_bpr = width * bytes_per_pixel;

        // WebGPU requires bytes_per_row % 256 == 0
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bpr = ((unpadded_bpr + align - 1) / align) * align;

        if padded_bpr == unpadded_bpr {
            // Already aligned; no copy needed
            return (src.to_vec(), unpadded_bpr);
        }

        let mut out = vec![0u8; padded_bpr as usize * height as usize];
        for y in 0..height {
            let src_off = (y * unpadded_bpr) as usize;
            let dst_off = (y * padded_bpr) as usize;
            out[dst_off..dst_off + unpadded_bpr as usize]
                .copy_from_slice(&src[src_off..src_off + unpadded_bpr as usize]);
        }
        (out, padded_bpr)
    }
}

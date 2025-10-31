pub mod pipelines;
pub mod surface;
pub mod upload;

#[repr(C)]
pub struct ParamsUbo {
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub vignette: f32,
    pub time: f32,
    pub surface_w: f32,
    pub surface_h: f32,
    pub _pad0: f32, // padding to 32 bytes
}

pub trait FrameRenderer {
    fn resize(&mut self, width: u32, height: u32);
    fn render_frame(&mut self, frame: &media::frame::Frame); // uploads + draws + presents
}

pub struct Renderer {
    surface: surface::SurfaceMgr, // Represents the window and the GPU connection
    upload: upload::UploadStager, // (CPU → GPU Transfer) Manages staging buffers for texture uploads. Handles the annoying alignment requirements wgpu has
    pipes: pipelines::Pipelines, // Contains the shader that draws on the surface texture. Tells GPU how to read pixels
}

impl Renderer {
    pub fn new(window: std::sync::Arc<winit::window::Window>) -> anyhow::Result<Self> {
        let surface = surface::SurfaceMgr::new(window)?;
        let pipes = pipelines::Pipelines::new(&surface.device(), surface.format())?;
        Ok(Self {
            surface,
            upload: upload::UploadStager::new(),
            pipes,
        })
    }
}

impl FrameRenderer for Renderer {
    fn resize(&mut self, w: u32, h: u32) {
        self.surface.configure(w, h);
    }

    fn render_frame(&mut self, frame: &media::frame::Frame) {
        let dimensions = frame.dimensions();
        let buffer = frame.raw_data();

        // Pass individual components, not the whole frame
        let texture_view = self.upload.blit_rgba(
            self.surface.device(),
            self.surface.queue(),
            dimensions.width(),
            dimensions.height(),
            buffer, // This is &[u8]
        );

        let (surface_texture, surface_view) = match self.surface.acquire() {
            Ok(frame) => frame,
            Err(e) => {
                eprintln!("Failed to acquire surface texture: {}", e); // Can fail during window resize/minimize
                return;
            }
        };

        let bind_group = self
            .pipes
            .bind_group_for(self.surface.device(), texture_view);

        let mut encoder =
            self.surface
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render_encoder"),
                });

        // Tells GPU: "Start rendering to this surface"
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), // Clear the screen to black before drawing
                    store: wgpu::StoreOp::Store,                   // Save the results when done
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Activates the shader and all rendering state
        rpass.set_pipeline(&self.pipes.pipeline);

        // Binds the texture to the shader
        rpass.set_bind_group(0, &bind_group, &[]);

        rpass.draw(0..3, 0..1); // fullscreen triangle .... I guess this is for full screen things

        drop(rpass);

        // Submit commands to GPU
        self.surface.queue().submit(Some(encoder.finish()));

        // Present the frame to the screen
        self.surface.present(surface_texture);
    }
}

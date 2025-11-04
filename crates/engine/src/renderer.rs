use crate::{renderer::pipelines::common_pipeline::Pipeline, types::ParamsUbo};

pub mod pipelines;
pub mod surface;
pub mod upload;

pub trait FrameRenderer {
    fn resize(&mut self, width: u32, height: u32);
    fn render_frame(&mut self, frame: &media::frame::Frame);
}

pub struct Renderer {
    surface: surface::SurfaceMgr,
    upload: upload::UploadStager,
    pipes: pipelines::Pipelines,            // all our pipelines/effects
    pipeline_chain: Vec<Box<dyn Pipeline>>, // ordered list of pipelines to apply
}

impl Renderer {
    pub fn new(window: std::sync::Arc<winit::window::Window>) -> anyhow::Result<Self> {
        let surface = surface::SurfaceMgr::new(window)?;
        let pipes = pipelines::Pipelines::new(&surface.device(), surface.format())?;
        Ok(Self {
            surface,
            upload: upload::UploadStager::new(),
            pipes,
            pipeline_chain: Vec::new(),
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

        let texture_view = self.upload.blit_rgba(
            self.surface.device(),
            self.surface.queue(),
            dimensions.width(),
            dimensions.height(),
            buffer,
        );

        let (surface_texture, surface_view) = match self.surface.acquire() {
            Ok(frame) => frame,
            Err(e) => {
                eprintln!("Failed to acquire surface texture: {}", e);
                return;
            }
        };
        
        // create the bind group for whatever pipeline we are using
        let bind_group = self
            .pipes
            .color_grading
            .bind_group_for(self.surface.device(), texture_view);

        let mut encoder =
            self.surface
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render_encoder"),
                });

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_view,
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

        // we can set the pipeline now
        rpass.set_pipeline(&self.pipes.color_grading.pipeline());
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..3, 0..1);

        drop(rpass);

        self.surface.queue().submit(Some(encoder.finish()));
        self.surface.present(surface_texture);
    }
}

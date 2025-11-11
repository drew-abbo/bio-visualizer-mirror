use crate::{renderer::pipelines::common::Pipeline, types::ParamsUbo};

pub mod pipelines;
pub mod surface;
pub mod upload;

pub trait FrameRenderer {
    fn render_frame(&mut self, frame: &media::frame::Frame, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView;
}

pub struct Renderer {
    upload: upload::UploadStager,
    pipes: pipelines::Pipelines,
    pipeline_chain: Vec<Box<dyn Pipeline>>,
    
    // Store the output texture
    output_texture: Option<wgpu::Texture>,
    output_size: (u32, u32),
    target_format: wgpu::TextureFormat, // Add this field to store the format
}

impl Renderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let pipes = pipelines::Pipelines::new(device, format)?;
        Ok(Self {
            upload: upload::UploadStager::new(),
            pipes,
            pipeline_chain: Vec::new(),
            output_texture: None,
            output_size: (0, 0),
            target_format: format, // Store the format
        })
    }
    
    fn ensure_output_texture(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.output_size != (width, height) || self.output_texture.is_none() {
            self.output_texture = Some(device.create_texture(&wgpu::TextureDescriptor {
                label: Some("renderer_output"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.target_format, // Use the stored format instead of hardcoded Rgba8UnormSrgb
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }));
            self.output_size = (width, height);
        }
    }
}

// Rest of the code stays the same
impl FrameRenderer for Renderer {
    fn render_frame(&mut self, frame: &media::frame::Frame, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
        let dimensions = frame.dimensions();
        let width = dimensions.width();
        let height = dimensions.height();
        let buffer = frame.raw_data();

        // Ensure we have an output texture of the right size
        self.ensure_output_texture(device, width, height);
        
        let output_texture = self.output_texture.as_ref().unwrap();
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let texture_view = self.upload.blit_rgba(
            device,
            queue,
            width,
            height,
            buffer,
        );

        let bind_group = self.pipes.color_grading.bind_group_for(device, texture_view);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
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

            rpass.set_pipeline(&self.pipes.color_grading.pipeline());
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }

        queue.submit(Some(encoder.finish()));
        
        output_view
    }
}
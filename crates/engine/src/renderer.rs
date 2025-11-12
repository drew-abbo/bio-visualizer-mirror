use crate::types::ColorGradingParams;
use pipelines::common::Pipeline;
use std::any::Any;
pub mod pipelines;
pub mod surface;
pub mod upload;

pub trait FrameRenderer {
    fn render_frame(&mut self, frame: &media::frame::Frame, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView;
}

pub struct Renderer {
    upload: upload::UploadStager,
    pipeline_chain: Vec<Box<dyn Pipeline>>, //TODO: this should store a pipeline along with its params
    texture_domain: Vec<wgpu::Texture>, // Stores 2 textures to ping-pong between
    texture_size: (u32, u32),
    target_format: wgpu::TextureFormat,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        Ok(Self {
            upload: upload::UploadStager::new(),
            pipeline_chain: Vec::new(),
            texture_domain: Vec::new(),
            texture_size: (0, 0),
            target_format: format,
        })
    }
    
    fn create_texture(&self, device: &wgpu::Device, width: u32, height: u32, label: &str) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn ensure_intermediate_textures(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.texture_size != (width, height) || self.texture_domain.is_empty() {
            // Clear old textures and create new ones
            self.texture_domain.clear();
            
            let ping = self.create_texture(device, width, height, "ping_texture");
            let pong = self.create_texture(device, width, height, "pong_texture");
            
            self.texture_domain.push(ping);
            self.texture_domain.push(pong);
            self.texture_size = (width, height);
        }
    }

    pub fn add_pipeline<P: Pipeline + 'static>(&mut self, pipeline: P) {
        self.pipeline_chain.push(Box::new(pipeline));
    }
}

impl FrameRenderer for Renderer {
    fn render_frame(&mut self, frame: &media::frame::Frame, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
        let dimensions = frame.dimensions();
        let width = dimensions.width();
        let height = dimensions.height();
        let buffer = frame.raw_data();

        // Upload video frame to GPU
        let input_view = self.upload.blit_rgba(device, queue, width, height, buffer);

        // If no pipelines in chain, just return the input (just whatever frame is uploaded)
        if self.pipeline_chain.is_empty() {
            return input_view;
        }

        // Ensure we have intermediate textures for ping-ponging
        self.ensure_intermediate_textures(device, width, height);

        // Create texture views from the ping-pong textures
        let texture_views: [wgpu::TextureView; 2] = [
            self.texture_domain[0].create_view(&wgpu::TextureViewDescriptor::default()),
            self.texture_domain[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });

        let params = ColorGradingParams {
            exposure: 1.0,
            contrast: 1.2,
            saturation: 1.1,
            vignette: 0.3,
            time: 0.0,
            surface_w: width as f32,
            surface_h: height as f32,
            _pad0: 0.0,
        };

        for (i, pipeline) in self.pipeline_chain.iter().enumerate() {
            let current_input = if i == 0 { &input_view } else { &texture_views[(i - 1) % 2] };
            let current_output = &texture_views[i % 2];

            pipeline.apply(device, queue, &mut encoder, current_input, current_output, &params);
        }

        queue.submit(Some(encoder.finish()));
        
        // The last pipeline wrote to texture_views[(n-1) % 2]
        texture_views[(self.pipeline_chain.len() - 1) % 2].clone()
    }
}
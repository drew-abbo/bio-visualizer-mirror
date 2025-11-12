use pipelines::common::Pipeline;
pub mod pipelines;
pub mod surface;
pub mod upload;
use crate::errors::RendererError;

pub trait FrameRenderer {
    fn render_frame(
        &mut self,
        frame: &media::frame::Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> wgpu::TextureView;
}

pub struct Effect {
    pipeline: Box<dyn Pipeline>,
    params: Box<dyn std::any::Any + Send + Sync>,
}

impl Effect {
    pub fn new<P: Pipeline + 'static, T: 'static + Send + Sync>(
        pipeline: P,
        params: T,
    ) -> Self {
        Self {
            pipeline: Box::new(pipeline),
            params: Box::new(params),
        }
    }
    
    /// Update parameters for this effect
    pub fn set_params<T: 'static + Send + Sync>(&mut self, params: T) {
        self.params = Box::new(params);
    }
    
    /// Try to get params as a specific type
    pub fn get_params<T: 'static>(&self) -> Option<&T> {
        self.params.downcast_ref::<T>()
    }
    
    /// Try to get mutable params as a specific type
    pub fn get_params_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.params.downcast_mut::<T>()
    }
}

pub struct Renderer {
    upload: upload::UploadStager,
    effect_chain: Vec<Effect>,
    texture_domain: Vec<wgpu::Texture>,
    texture_size: (u32, u32),
    target_format: wgpu::TextureFormat,
}

impl Renderer {
    pub fn new(format: wgpu::TextureFormat) -> Result<Self, RendererError> {
        Ok(Self {
            upload: upload::UploadStager::new(),
            effect_chain: Vec::new(),
            texture_domain: Vec::new(),
            texture_size: (0, 0),
            target_format: format,
        })
    }

    fn create_texture(
        &self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        label: &str,
    ) -> wgpu::Texture {
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

    pub fn add_effect<P: Pipeline + 'static>(&mut self, pipeline: P, params: Box<dyn std::any::Any + Send + Sync>) {
        self.effect_chain.push(Effect { pipeline: Box::new(pipeline), params });
    }

    /// Update params for a specific effect in the chain
    pub fn update_effect_params(&mut self, index: usize, params: Box<dyn std::any::Any + Send + Sync>) {
        if let Some(effect) = self.effect_chain.get_mut(index) {
            effect.params = params;
        }
    }
    
    /// Get a reference to params for a specific effect (for UI editing)
    pub fn get_effect_params_mut(&mut self, index: usize) -> Option<&mut Box<dyn std::any::Any + Send + Sync>> {
        self.effect_chain.get_mut(index).map(|e| &mut e.params)
    }
    
    /// Get number of effects in the chain
    pub fn effect_count(&self) -> usize {
        self.effect_chain.len()
    }

    pub fn get_effect(&mut self, index: usize) -> Option<&mut Effect> {
        self.effect_chain.get_mut(index)
    }
}

impl FrameRenderer for Renderer {
    fn render_frame(
        &mut self,
        frame: &media::frame::Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> wgpu::TextureView {
        let dimensions = frame.dimensions();
        let width = dimensions.width();
        let height = dimensions.height();
        let buffer = frame.raw_data();

        // Upload video frame to GPU
        let input_view = self.upload.blit_rgba(device, queue, width, height, buffer);

        if self.effect_chain.is_empty() {
            return input_view;
        }

        self.ensure_intermediate_textures(device, width, height);

        let texture_views: [wgpu::TextureView; 2] = [
            self.texture_domain[0].create_view(&wgpu::TextureViewDescriptor::default()),
            self.texture_domain[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });

        for (i, effect) in self.effect_chain.iter().enumerate() {
            let current_input = if i == 0 {
                &input_view
            } else {
                &texture_views[(i - 1) % 2]
            };
            let current_output = &texture_views[i % 2];

            if let Err(e) = effect.pipeline.apply(
                device,
                queue,
                &mut encoder,
                current_input,
                current_output,
                effect.params.as_ref(),
            ) {
                eprintln!("Pipeline {} failed: {}", effect.pipeline.name(), e);
                continue;
            }
        }

        queue.submit(Some(encoder.finish()));
        texture_views[(self.effect_chain.len() - 1) % 2].clone()
    }
}

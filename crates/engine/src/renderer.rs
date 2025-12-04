use pipelines::common::Pipeline;
pub mod pipelines;
pub mod surface;
pub mod upload_stager;
use crate::effect::Effect;
use crate::errors::EngineError;

pub struct Renderer {
    upload_stager: upload_stager::UploadStager,
    effect_chain: Vec<Effect>,
    texture_domain: Vec<wgpu::Texture>,
    texture_size: (u32, u32),
    target_format: wgpu::TextureFormat,
}

impl Renderer {
    pub fn new(format: wgpu::TextureFormat) -> Result<Self, EngineError> {
        Ok(Self {
            upload_stager: upload_stager::UploadStager::new(),
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
            self.texture_domain.clear();

            let ping = self.create_texture(device, width, height, "ping_texture");
            let pong = self.create_texture(device, width, height, "pong_texture");

            self.texture_domain.push(ping);
            self.texture_domain.push(pong);
            self.texture_size = (width, height);
        }
    }

    pub fn add_effect<P: Pipeline + 'static, T: 'static + Send + Sync>(
        &mut self,
        pipeline: P,
        params: T,
    ) {
        self.effect_chain.push(Effect::new(pipeline, params));
    }

    pub fn get_effect_mut(&mut self, index: usize) -> Option<&mut Effect> {
        self.effect_chain.get_mut(index)
    }

    pub fn get_effect(&self, index: usize) -> Option<&Effect> {
        self.effect_chain.get(index)
    }

    pub fn effect_count(&self) -> usize {
        self.effect_chain.len()
    }

    pub fn effects(&self) -> impl Iterator<Item = &Effect> {
        self.effect_chain.iter()
    }

    pub fn effects_mut(&mut self) -> impl Iterator<Item = &mut Effect> {
        self.effect_chain.iter_mut()
    }

    pub fn render_frame(
        &mut self,
        frame: &media::frame::Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<wgpu::TextureView, EngineError> {
        let dimensions = frame.dimensions();
        let width = dimensions.width();
        let height = dimensions.height();
        let buffer = frame.raw_data();

        let input_view = self
            .upload_stager
            .blit_rgba(device, queue, width, height, buffer)?;

        if self.effect_chain.is_empty() {
            return Ok(input_view);
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

            effect.pipeline().apply(
                device,
                queue,
                &mut encoder,
                current_input,
                current_output,
                effect.params_any(),
            )?;
        }

        queue.submit(Some(encoder.finish()));
        let final_index = (self.effect_chain.len() - 1) % 2;
        Ok(texture_views[final_index].clone())
    }
}

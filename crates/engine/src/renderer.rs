pub mod pipelines;
pub mod upload;
use crate::effect::Effect;
use crate::errors::RendererError;

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
            upload: upload::UploadStager::new(format),
            effect_chain: Vec::new(),
            texture_domain: Vec::new(),
            texture_size: (0, 0),
            target_format: format,
        })
    }

    pub fn render_frame_to_gpu(
        &mut self,
        frame: &media::frame::Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output_texture: &wgpu::Texture, // UI-owned texture
    ) {
        let dims = frame.dimensions();
        let width = dims.width();
        let height = dims.height();
        // Upload input frame to internal texture
        let input_view = self
            .upload
            .blit_rgba(device, queue, width, height, frame.raw_data());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });
        
        if self.effect_chain.is_empty() {
            let src_texture = match self.upload.current_texture() {
                Some(tex) => tex,
                None => {
                    eprintln!("No upload texture available");
                    return;
                }
            };
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: src_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: output_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit(Some(encoder.finish()));
            return;
        }

        // Apply effect chain using ping-pong textures
        self.ensure_intermediate_textures(device, width, height);

        let texture_views: [wgpu::TextureView; 2] = [
            self.texture_domain[0].create_view(&wgpu::TextureViewDescriptor::default()),
            self.texture_domain[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        for (i, effect) in self.effect_chain.iter().enumerate() {
            let current_input = if i == 0 {
                &input_view
            } else {
                &texture_views[(i - 1) % 2]
            };
            let current_output = &texture_views[i % 2];

            effect
                .pipeline()
                .apply(
                    device,
                    queue,
                    &mut encoder,
                    current_input,
                    current_output,
                    effect.params_any(),
                )
                .unwrap_or_else(|e| {
                    eprintln!("Pipeline {} failed: {}", effect.pipeline().name(), e)
                });
        }

        // Copy final texture into UI texture
        let final_texture = &texture_views[(self.effect_chain.len() - 1) % 2];
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture_domain[(self.effect_chain.len() - 1) % 2],
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(Some(encoder.finish()));
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
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

    pub fn add_effect(&mut self, effect: Effect) {
        self.effect_chain.push(effect);
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

    pub fn add_effect_boxed(&mut self, effect: Effect) {
        self.effect_chain.push(effect);
    }

    // I think option makes sense for replace and remove since something might not happen
    pub fn replace_effect(&mut self, index: usize, effect: Effect) -> Option<Effect> {
        if index < self.effect_chain.len() {
            Some(std::mem::replace(&mut self.effect_chain[index], effect))
        } else {
            None
        }
    }

    pub fn remove_effect(&mut self, index: usize) -> Option<Effect> {
        if index < self.effect_chain.len() {
            Some(self.effect_chain.remove(index))
        } else {
            None
        }
    }
}

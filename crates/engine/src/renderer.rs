use pipelines::common::Pipeline;
pub mod pipelines;
pub mod upload_stager;
use crate::effect::Effect;
use crate::errors::EngineError;

/// The main feature for rendering video frames with a chain of effects
pub struct Renderer {
    /// Stager for uploading textures
    upload_stager: upload_stager::UploadStager,

    effect_chain: Vec<Effect>,

    /// Intermediate textures for ping-pong rendering
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

    /// Ensure intermediate textures are created and match the given size
    fn ensure_intermediate_textures(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.texture_size != (width, height) || self.texture_domain.is_empty() {
            self.texture_domain.clear();

            // Create two textures for ping-pong rendering
            let ping = self.create_texture(device, width, height, "ping_texture");
            let pong = self.create_texture(device, width, height, "pong_texture");

            // Add the ping-pong textures to the domain
            self.texture_domain.push(ping);
            self.texture_domain.push(pong);

            // Update the stored texture size for future checks
            self.texture_size = (width, height);
        }
    }

    /// API for managing the effect chain
    /// Later this might be changed to support a more bulk approach
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

    /// This is the main rendering function that applies the effect chain to a given frame
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

        // Upload the input frame as a texture
        let input_view = self
            .upload_stager
            .cpu_to_gpu_rgba(device, queue, width, height, buffer)?;

        // If there are no effects, return the input view directly
        if self.effect_chain.is_empty() {
            return Ok(input_view);
        }

        // Ensure intermediate textures are ready
        self.ensure_intermediate_textures(device, width, height);

        // Create views for the intermediate textures
        let texture_views: [wgpu::TextureView; 2] = [
            self.texture_domain[0].create_view(&wgpu::TextureViewDescriptor::default()),
            self.texture_domain[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        // Create a command encoder for rendering from the device sent from the UI
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });

        // Apply each effect in the chain
        for (i, effect) in self.effect_chain.iter().enumerate() {
            // Determine input and output views based on ping-ponging
            let current_input = if i == 0 {
                &input_view
            } else {
                &texture_views[(i - 1) % 2]
            };

            // Determine the output view for the current effect
            let current_output = &texture_views[i % 2];

            // Apply the current effect using the pipeline
            effect.pipeline().apply(
                device,
                queue,
                &mut encoder,
                current_input,
                current_output,
                effect.params_any(),
            )?;
        }

        // Submit the commands to the GPU queue
        queue.submit(Some(encoder.finish()));

        // Return the final output view
        let final_index = (self.effect_chain.len() - 1) % 2;
        Ok(texture_views[final_index].clone())
    }
}

use crate::errors::EngineError;

pub struct UploadStager {
    tex: Option<wgpu::Texture>,
    extent: wgpu::Extent3d,
}

impl Default for UploadStager {
    fn default() -> Self {
        Self::new()
    }
}

impl UploadStager {
    pub fn new() -> Self {
        Self {
            tex: None,
            extent: wgpu::Extent3d {
                width: 0,
                height: 0,
                depth_or_array_layers: 1,
            },
        }
    }

    /// Make sure we have a texture of the right size
    /// If not, create a new one
    fn ensure_texture(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.extent.width == width && self.extent.height == height && self.tex.is_some() {
            return;
        }

        self.extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("upload_texture"),
            size: self.extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.tex = Some(tex);
    }

    /// Blit RGBA data into the texture and return a texture view
    /// This saves us from having to create a new texture every frame
    /// Here we go from RAM to VRAM
    pub fn cpu_to_gpu_rgba(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<wgpu::TextureView, EngineError> {
        let expected_size = (width * height * 4) as usize;

        if data.len() < expected_size {
            return Err(EngineError::DataSizeMismatch {
                expected: expected_size,
                actual: data.len(),
            });
        }

        self.ensure_texture(device, width, height);

        // Creates a staging buffer
        // Copies CPU data into the staging buffer
        // Schedules a GPU command, copy from the staging buffer to the texture
        // executes on the GPU once the queue is submitted
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: self
                    .tex
                    .as_ref()
                    .ok_or(EngineError::TextureNotInitialized)?,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data, // the framebuffer data
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            self.extent,
        );

        // Create and return the texture view we can render with
        // unwrap is safe here because we checked tex is Some above
        Ok(self
            .tex
            .as_ref()
            .unwrap()
            .create_view(&wgpu::TextureViewDescriptor::default()))
    }
}

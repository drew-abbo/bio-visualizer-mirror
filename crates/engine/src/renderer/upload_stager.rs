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

    pub fn blit_rgba(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<wgpu::TextureView, EngineError> {
        let expected_size = (width * height * 4) as usize; // RGBA = 4 bytes per pixel
        if data.len() < expected_size {
            return Err(EngineError::DataSizeMismatch {
                expected: expected_size,
                actual: data.len(),
            });
        }

        self.ensure_texture(device, width, height);

        const BYTES_PER_PIXEL: u32 = 4; // RGBA
        let bytes_per_row = ((width * BYTES_PER_PIXEL + 255) / 256) * 256;

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: self
                    .tex
                    .as_ref()
                    .ok_or_else(|| EngineError::TextureNotInitialized)?,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
            self.extent,
        );

        // Create view on-demand
        Ok(self
            .tex
            .as_ref()
            .ok_or_else(|| EngineError::TextureNotInitialized)?
            .create_view(&wgpu::TextureViewDescriptor::default()))
    }
}

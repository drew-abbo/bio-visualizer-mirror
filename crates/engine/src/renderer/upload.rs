pub struct UploadStager {
    tex: Option<wgpu::Texture>,
    extent: wgpu::Extent3d,
    format: wgpu::TextureFormat,
}

impl UploadStager {
    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            tex: None,
            extent: wgpu::Extent3d {
                width: 0,
                height: 0,
                depth_or_array_layers: 1,
            },
            format,
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
            format: self.format, // <-- use stored format
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
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
    ) -> wgpu::TextureView {
        self.ensure_texture(device, width, height);

        const BYTES_PER_PIXEL: u32 = 4; // RGBA8 or BGRA8 still 4 bytes
        let bytes_per_row = ((width * BYTES_PER_PIXEL + 255) / 256) * 256;

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: self.tex.as_ref().unwrap(),
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

        self.tex
            .as_ref()
            .unwrap()
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn current_texture(&self) -> Option<&wgpu::Texture> {
        self.tex.as_ref()
    }
}

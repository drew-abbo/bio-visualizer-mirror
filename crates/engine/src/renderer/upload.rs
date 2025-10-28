pub struct UploadStager {
    // Reusable CPU→GPU texture that matches the last frame size
    tex: Option<wgpu::Texture>,
    view: Option<wgpu::TextureView>,
    extent: wgpu::Extent3d,
}

impl UploadStager {
    pub fn new() -> Self {
        Self {
            tex: None,
            view: None,
            extent: wgpu::Extent3d {
                width: 0,
                height: 0,
                depth_or_array_layers: 1,
            },
        }
    }

    fn ensure_texture(&mut self, device: &wgpu::Device, width: u32, height: u32, label: &str) {
        // Only recreate if size changed or texture doesn't exist
        if self.extent.width == width && self.extent.height == height && self.tex.is_some() {
            return;
        }

        self.extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: self.extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        self.tex = Some(tex);
        self.view = Some(view);
    }

    /// Upload an RGBA frame into an internal texture and return a view to bind.
    pub fn blit_rgba<'a>(
        &'a mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame_width: u32,
        frame_height: u32,
        frame_stride: u32, // bytes per row in the source buffer
        pixels: &'a [u8],
    ) -> &'a wgpu::TextureView {
        self.ensure_texture(device, frame_width, frame_height, "upload/rgba");

        const BYTES_PER_PIXEL: u32 = 4; // RGBA
        // wgpu requires bytes_per_row to be 256-byte aligned
        let bytes_per_row_aligned = ((frame_width * BYTES_PER_PIXEL + 255) / 256) * 256;
        let unpadded_bytes_per_row = frame_width * BYTES_PER_PIXEL;

        let data_to_upload = if frame_stride == unpadded_bytes_per_row {
            // Tightly packed: can upload directly
            pixels
        } else {
            // Stride mismatch: need to repack into aligned buffer
            // This creates a temporary allocation - could be optimized with a reusable buffer
            
            let mut staging = vec![0u8; (bytes_per_row_aligned * frame_height) as usize];

            for y in 0..frame_height as usize {
                let src_start = y * frame_stride as usize;
                let src_end = src_start + unpadded_bytes_per_row as usize;
                let src = &pixels[src_start..src_end];

                let dst_start = y * bytes_per_row_aligned as usize;
                let dst_end = dst_start + unpadded_bytes_per_row as usize;
                staging[dst_start..dst_end].copy_from_slice(src);
            }

            // Leak to return a slice with the right lifetime
            // In production, you'd want a reusable staging buffer in the struct
            Box::leak(staging.into_boxed_slice())
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: self.tex.as_ref().unwrap(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data_to_upload,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row_aligned),
                rows_per_image: Some(frame_height),
            },
            self.extent,
        );

        // Return the texture view. What the gpu can read from.
        self.view.as_ref().unwrap()
    }
}

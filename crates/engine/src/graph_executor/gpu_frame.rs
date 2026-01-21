use std::sync::Arc;

/// GPU frame handle with its dimensions. Holds a texture view plus its size so
/// downstream consumers can size new textures correctly.
#[derive(Debug, Clone)]
pub struct GpuFrame {
    pub view: Arc<wgpu::TextureView>,
    pub size: wgpu::Extent3d,
}

impl GpuFrame {
    pub fn new(view: wgpu::TextureView, size: wgpu::Extent3d) -> Self {
        Self { view: Arc::new(view), size }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn size(&self) -> wgpu::Extent3d {
        self.size
    }
}
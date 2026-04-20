use std::sync::Arc;
use media::frame::Uid;

/// GPU frame handle with its dimensions. Holds a texture view plus its size so
/// downstream consumers can size new textures correctly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuFrame {
    pub view: Arc<wgpu::TextureView>,
    pub size: wgpu::Extent3d,
    pub frame_id: Uid,
}

impl GpuFrame {
    pub fn new(view: wgpu::TextureView, size: wgpu::Extent3d, frame_id: Uid) -> Self {
        Self {
            view: Arc::new(view),
            size,
            frame_id,
        }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn size(&self) -> wgpu::Extent3d {
        self.size
    }

    pub fn frame_id(&self) -> Uid {
        self.frame_id
    }
}

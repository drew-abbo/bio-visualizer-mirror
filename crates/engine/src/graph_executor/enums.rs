/// GPU frame handle with its dimensions. Holds a texture view plus its size so
/// downstream consumers can size new textures correctly.
#[derive(Debug, Clone)]
pub struct GpuFrame {
    pub view: wgpu::TextureView,
    pub size: wgpu::Extent3d,
}

impl GpuFrame {
    pub fn new(view: wgpu::TextureView, size: wgpu::Extent3d) -> Self {
        Self { view, size }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn size(&self) -> wgpu::Extent3d {
        self.size
    }
}

/// Resolved input value (after looking up connections)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ResolvedInput {
    Frame(GpuFrame),
    Bool(bool),
    Int(i32),
    Float(f32),
    Dimensions(u32, u32),
    Pixel([f32; 4]),
    Text(String),
    Enum(usize),
    File(std::path::PathBuf),
}

/// Output value from a node
#[derive(Debug, Clone)]
pub enum OutputValue {
    Frame(GpuFrame),
    Bool(bool),
    Int(i32),
    Float(f32),
    Dimensions(u32, u32),
    Pixel([f32; 4]),
    Text(String),
}

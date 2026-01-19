/// Resolved input value (after looking up connections)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ResolvedInput {
    Frame(wgpu::TextureView),
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
    Frame(wgpu::TextureView),
    Bool(bool),
    Int(i32),
    Float(f32),
    Dimensions(u32, u32),
    Pixel([f32; 4]),
    Text(String),
}

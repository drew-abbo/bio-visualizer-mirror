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
    File(String),
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

#[derive(Debug, Clone, Copy)]
pub enum NodeOutputKind {
    Frame,
    Bool,
    Int,
    Float,
    Dimensions,
    Pixel,
    Text,
}

pub enum NodeInputKind {
    Frame,
    Bool { default: bool },
    Int { default: i32 },
    Float { default: f32 },
    Dimensions { default: (u32, u32) },
    Pixel { default: [f32; 4] },
    Text { default: String },
    Enum { choices: Vec<String> },
    File { kind: FileKind },
}

pub enum FileKind {
    Any,
    Video,
    Image,
}

pub enum NodeExecutionPlan {
    Shader { file_path: std::path::PathBuf },
    BuiltIn(BuiltInHandler),
}

#[derive(Debug, Clone, Copy)]
pub enum BuiltInHandler {
    SumInputs,
    // Add more built-in handlers
}

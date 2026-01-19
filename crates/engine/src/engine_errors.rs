use thiserror::Error;

#[derive(Error, Debug)]
pub enum EngineError {
    // Pipeline errors
    #[error("Invalid parameter type for pipeline '{pipeline}': expected {expected}, got {actual}")]
    InvalidParamType {
        pipeline: String,
        expected: String,
        actual: String,
    },

    #[error("Parameter buffer size mismatch: expected {expected} bytes, got {actual} bytes")]
    BufferSizeMismatch { expected: usize, actual: usize },

    // Upload errors
    #[error("Upload failed: texture not initialized")]
    TextureNotInitialized,

    #[error("Upload failed: data size mismatch (expected {expected} bytes, got {actual} bytes)")]
    DataSizeMismatch { expected: usize, actual: usize },

    // Surface errors
    #[error("Failed to acquire swap chain texture: {0}")]
    SwapChainAcquireFailed(String),

    #[error("Failed to request GPU adapter")]
    RequestAdapter(#[from] wgpu::RequestAdapterError),

    #[error("Failed to request GPU device")]
    RequestDevice(#[from] wgpu::RequestDeviceError),

    #[error("Invalid input count for pipeline: expected {expected}, got {actual}")]
    InvalidInputCount { expected: usize, actual: usize },

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("No output node defined in the graph")]
    NoOutputNode,
}

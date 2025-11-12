use thiserror::Error;

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Invalid parameter type for pipeline '{pipeline}': expected {expected}, got {actual}")]
    InvalidParamType {
        pipeline: String,
        expected: String,
        actual: String,
    },
    
    #[error("Parameter buffer size mismatch: expected {expected} bytes, got {actual} bytes")]
    BufferSizeMismatch { expected: usize, actual: usize },
    
    #[error("Pipeline creation failed: {0}")]
    CreationFailed(String),
}

#[derive(Error, Debug)]
pub enum SurfaceError {
    #[error("Failed to acquire next swap chain texture: {0}")]
    AcquireFailed(String),

    #[error("Failed to request WGPU adapter: {source}")]
    RequestAdapterError {
        #[from]
        source: wgpu::RequestAdapterError,
    },

    #[error("Failed to request WGPU device: {source}")]
    RequestDeviceError {
        #[from]
        source: wgpu::RequestDeviceError,
    },
}

#[derive(Error, Debug)]
pub enum RendererError {
    #[error("Pipeline error: {0}")]
    Pipeline(#[from] PipelineError),
    
    #[error("Surface error: {0}")]
    Surface(#[from] SurfaceError),
}
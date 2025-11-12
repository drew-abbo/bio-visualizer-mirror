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
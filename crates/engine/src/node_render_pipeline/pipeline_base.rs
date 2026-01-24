use std::any::Any;

use crate::engine_errors::EngineError;

/// Trait for runtime pipelines; implementors provide `apply` to execute the pipeline.
#[allow(clippy::too_many_arguments)]
pub trait PipelineBase {
    /// Execute the render pipeline.
    ///
    /// - `device`/`queue` are used to prepare GPU resources [wgpu::Device], [wgpu::Queue]).
    /// - `encoder` is a [wgpu::CommandEncoder] used to record the render pass that writes into `output` [wgpu::TextureView].
    /// - `primary_input` and `additional_inputs` are input texture views [wgpu::TextureView].
    /// - `params` is expected to be a `HashMap<String, ResolvedInput>`; implementations should
    ///   validate and return [crate::engine_errors::EngineError::InvalidParamType] on mismatch.
    fn apply(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        primary_input: &wgpu::TextureView,
        additional_inputs: &[&wgpu::TextureView],
        output: &wgpu::TextureView,
        params: &dyn Any,
    ) -> Result<(), EngineError>;
}

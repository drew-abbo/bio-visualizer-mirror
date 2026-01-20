use crate::engine_errors::EngineError;
use std::any::Any;

pub trait PipelineBase {
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

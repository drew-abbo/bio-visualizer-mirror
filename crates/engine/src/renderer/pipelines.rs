pub mod color_grading;
pub mod common;
use crate::errors::PipelineError;
use common::Pipeline;

#[derive(Debug)]
pub struct Pipelines {
    pub color_grading: color_grading::ColorGradingPipeline,
}

impl Pipelines {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> Result<Self, PipelineError> {
        Ok(Self {
            color_grading: color_grading::ColorGradingPipeline::new(device, surface_format)?,
        })
    }
}

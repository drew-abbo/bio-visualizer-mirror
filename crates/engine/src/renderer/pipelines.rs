pub mod color_grading;
pub mod common;
use common::Pipeline;
use crate::errors::PipelineError;

pub struct Pipelines {
    pub color_grading: color_grading::ColorGradingPipeline,
    // Future pipelines:
    // pub blur: blur::BlurPipeline,
    // pub chromatic_aberration: ChromaticAberrationPipeline,
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

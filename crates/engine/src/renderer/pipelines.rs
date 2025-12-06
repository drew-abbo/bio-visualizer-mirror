pub mod common;
pub mod color_grading;
use crate::{errors::EngineError, renderer::pipelines::common::Pipeline};

#[derive(Debug)]
pub struct Pipelines {
    pub color_grading: color_grading::color_grading_pipeline::ColorGradingPipeline,
}

impl Pipelines {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> Result<Self, EngineError> {
        Ok(Self {
            color_grading: color_grading::color_grading_pipeline::ColorGradingPipeline::new(device, surface_format)?,
        })
    }
}

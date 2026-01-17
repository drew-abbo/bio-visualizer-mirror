pub mod brightness_pipeline;
pub mod common;
pub mod grayscale_pipeline;
pub mod invert_pipeline;
pub mod shaders;

use crate::engine_errors::EngineError;
use common::Pipeline;

/// Collection of all rendering pipelines
/// Don't know if this would be helpful, but could be used to initialize all pipelines at once
pub struct Pipelines {
    pub brightness: brightness_pipeline::BrightnessPipeline,
    pub grayscale: grayscale_pipeline::GrayscalePipeline,
    pub invert: invert_pipeline::InvertPipeline,
}

impl Pipelines {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> Result<Self, EngineError> {
        Ok(Self {
            brightness: brightness_pipeline::BrightnessPipeline::new(device, surface_format)?,
            grayscale: grayscale_pipeline::GrayscalePipeline::new(device, surface_format)?,
            invert: invert_pipeline::InvertPipeline::new(device, surface_format)?,
        })
    }
}

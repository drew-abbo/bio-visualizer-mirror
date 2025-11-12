pub mod color_grading;
pub mod common;
use common::Pipeline;

pub struct Pipelines {
    pub fullscreen: color_grading::ColorGradingPipeline,
    // Future pipelines:
    // pub blur: blur::BlurPipeline,
    // pub chromatic_aberration: ChromaticAberrationPipeline,
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        Ok(Self {
            fullscreen: color_grading::ColorGradingPipeline::new(device, surface_format)?,
        })
    }
}
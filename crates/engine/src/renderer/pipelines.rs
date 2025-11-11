pub mod fullscreen;
pub mod common;
use common::Pipeline;

pub struct Pipelines {
    pub fullscreen: fullscreen::FullScreenPipeline,
    // Future pipelines:
    // pub blur: blur::BlurPipeline,
    // pub chromatic_aberration: ChromaticAberrationPipeline,
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        Ok(Self {
            fullscreen: fullscreen::FullScreenPipeline::new(device, surface_format)?,
        })
    }
}
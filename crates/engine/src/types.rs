use std::any::Any;

/// Parameters for color grading pipeline
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColorGradingParams {
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub vignette: f32,
    pub time: f32,
    pub surface_w: f32,
    pub surface_h: f32,
    pub _pad0: f32,
}

impl Default for ColorGradingParams {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            vignette: 0.5,
            time: 0.0,
            surface_w: 0.0,
            surface_h: 0.0,
            _pad0: 0.0,
        }
    }
}

/// Parameters for blur pipeline (example)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlurParams {
    pub radius: f32,
    pub strength: f32,
    pub surface_w: f32,
    pub surface_h: f32,
}

impl Default for BlurParams {
    fn default() -> Self {
        Self {
            radius: 5.0,
            strength: 1.0,
            surface_w: 0.0,
            surface_h: 0.0,
        }
    }
}

// Helper trait to make downcasting easier
pub trait PipelineParams: Any + Copy {
    fn type_name() -> &'static str;
}

impl PipelineParams for ColorGradingParams {
    fn type_name() -> &'static str {
        "ColorGradingParams"
    }
}

impl PipelineParams for BlurParams {
    fn type_name() -> &'static str {
        "BlurParams"
    }
}
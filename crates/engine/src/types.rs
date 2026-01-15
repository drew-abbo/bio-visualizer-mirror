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

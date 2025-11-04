/// Uniform Buffer Object
pub struct ParamsUbo {
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub vignette: f32,
    pub time: f32,
    pub surface_w: f32,
    pub surface_h: f32,
    pub _pad0: f32,
}
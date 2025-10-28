use std::{sync::Arc, time::Duration};

// Public RGBA frame structure used for video frames. Will probably change depengding on ffmpeg usage.
#[derive(Clone)]
pub struct RgbaFrame {
    pub pts: Option<Duration>,
    pub width: u32,
    pub height: u32,
    pub stride: u32, // bytes per row
    pub pixels: Arc<[u8]>,
}

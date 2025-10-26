// Public RGBA frame structure used for video frames. Will probably change depengding on ffmpeg usage.
#[derive(Clone)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    // Bytes-per-row for the buffer may be padded to 256-byte alignment.
    pub bpr: u32, // already 256-aligned if possible
    // Pixel bytes, length = bpr * height
    pub data: std::sync::Arc<[u8]>,
    // Optional presentation timestamp in nanoseconds
    pub pts_ns: Option<u64>,
}

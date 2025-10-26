

#[derive(Clone)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    /// Bytes-per-row for the buffer may be padded to 256-byte alignment.
    pub bpr: u32,
    /// Pixel bytes, length = bpr * height
    pub data: std::sync::Arc<[u8]>,
    /// Optional presentation timestamp in nanoseconds
    pub pts_ns: Option<u64>,
}

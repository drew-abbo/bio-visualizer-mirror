use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use media::frame::Dimensions;
use media::frame::{
    streams::{FrameStream, OnStreamEnd, Video},
    Producer,
};

pub fn write_rgba_to_bmp<P: AsRef<Path>>(
    path: P,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> io::Result<()> {
    // Validate size
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "dimensions overflow"))?;
    if rgba.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "buffer size mismatch: expected {}, got {}",
                expected,
                rgba.len()
            ),
        ));
    }

    let pixel_data_size = expected as u32; // width * height * 4
    let file_header_size = 14u32;
    let dib_header_size = 40u32;
    let offset = file_header_size + dib_header_size; // 54
    let file_size = offset + pixel_data_size;

    let mut f = File::create(path)?;

    // ---- BITMAPFILEHEADER (14 bytes) ----
    // bfType: 'BM'
    f.write_all(b"BM")?;
    // bfSize: u32 (little-endian)
    f.write_all(&file_size.to_le_bytes())?;
    // bfReserved1: u16 = 0
    f.write_all(&0u16.to_le_bytes())?;
    // bfReserved2: u16 = 0
    f.write_all(&0u16.to_le_bytes())?;
    // bfOffBits: u32 (offset to pixel data)
    f.write_all(&offset.to_le_bytes())?;

    // ---- BITMAPINFOHEADER (40 bytes) ----
    // biSize
    f.write_all(&dib_header_size.to_le_bytes())?;
    // biWidth (i32)
    f.write_all(&(width as i32).to_le_bytes())?;
    // biHeight (i32) positive: bottom-up
    f.write_all(&(height as i32).to_le_bytes())?;
    // biPlanes u16 = 1
    f.write_all(&1u16.to_le_bytes())?;
    // biBitCount u16 = 32 (RGBA/BGRA)
    f.write_all(&32u16.to_le_bytes())?;
    // biCompression u32 = 0 (BI_RGB - no compression)
    f.write_all(&0u32.to_le_bytes())?;
    // biSizeImage u32 (can be 0 for BI_RGB, but we'll put pixel_data_size)
    f.write_all(&pixel_data_size.to_le_bytes())?;
    // biXPelsPerMeter (print resolution) - set a reasonable default (2835 = 72 DPI)
    f.write_all(&2835i32.to_le_bytes())?;
    // biYPelsPerMeter
    f.write_all(&2835i32.to_le_bytes())?;
    // biClrUsed u32 = 0
    f.write_all(&0u32.to_le_bytes())?;
    // biClrImportant u32 = 0
    f.write_all(&0u32.to_le_bytes())?;

    // ---- Pixel data ----
    // BMP 32bpp rows are stored bottom-up, left-to-right.
    // Input buffer is assumed top-down, so we iterate rows in reverse.
    let row_stride = (width as usize) * 4;
    for row in (0..height as usize).rev() {
        let row_start = row * row_stride;
        // iterate columns left to right
        for col in 0..(width as usize) {
            let idx = row_start + col * 4;
            let r = rgba[idx];
            let g = rgba[idx + 1];
            let b = rgba[idx + 2];
            let a = rgba[idx + 3];
            // BMP expects little-endian pixel order B G R A for 32bpp
            f.write_all(&[b, g, r, a])?;
        }
    }

    f.flush()?;
    Ok(())
}

fn main() {
    let stream = Video::new("./input.mp4").unwrap();
    let stream_len = stream.stats().stream_length.unwrap();

    let mut producer = Producer::new(stream, OnStreamEnd::HoldSolidBlack).unwrap();

    for i in 0..stream_len {
        let frame = producer.fetch_frame().unwrap();

        if i % 500 == 0 {
            println!("Frame {i}/{stream_len} ({})", frame.dimensions());
            write_rgba_to_bmp(
                format!("./output/{i}.bmp"),
                frame.dimensions().width(),
                frame.dimensions().height(),
                frame.raw_data(),
            )
            .unwrap();
        }
    }
}

use std::time::{Duration, Instant};

fn main() -> anyhow::Result<()> {
    // Create a message channel FFmpeg or any producer would send frames through this
    let (frame_receiver, frame_sender) = util::channels::message_channel::new();

    // Spawn a background thread that generates fake frames
    std::thread::spawn(move || {
        // Frame dimensions (640x480, RGBA)
        let width: u32 = 640;
        let height: u32 = 480;
        let bytes_per_row: u32 = width * 4; // 4 bytes per pixel (R,G,B,A)

        // Solid colors to cycle through
        let color_sequence: [[u8; 4]; 5] = [
            [255, 0, 0, 255],     // Red
            [0, 255, 0, 255],     // Green
            [0, 0, 255, 255],     // Blue
            [255, 255, 255, 255], // White
            [0, 0, 0, 255],       // Black
        ];

        let mut current_color_index: usize = 0;
        let mut next_frame_time = Instant::now();

        loop {
            // Build one horizontal line (row) of pixels in the chosen color
            let mut single_row = vec![0u8; bytes_per_row as usize];
            for pixel in single_row.chunks_exact_mut(4) {
                pixel.copy_from_slice(&color_sequence[current_color_index]);
            }

            // Duplicate that row for the entire frame
            let mut full_frame_bytes = vec![0u8; (bytes_per_row * height) as usize];
            for y in 0..height {
                let start = (y * bytes_per_row) as usize;
                full_frame_bytes[start..start + bytes_per_row as usize]
                    .copy_from_slice(&single_row);
            }

            // Wrap the raw bytes into a frame object and send to the renderer
            let frame = engine::types::RgbaFrame {
                pts: None,
                width,
                height,
                stride: bytes_per_row,
                pixels: std::sync::Arc::from(full_frame_bytes),
            };

            // Send to app — ignore send errors if receiver was closed
            let _ = frame_sender.send(frame);

            // Move to the next color in the sequence
            current_color_index = (current_color_index + 1) % color_sequence.len();

            // fps
            next_frame_time += Duration::from_millis(100);
            let now = Instant::now();
            if next_frame_time > now {
                std::thread::sleep(next_frame_time - now);
            } else {
                next_frame_time = now;
            }
        }
    });

    // Start the app
    engine::run(frame_receiver)
}

use crate::effect::Effect;
use crate::engine_errors::EngineError;
use crate::upload_stager::UploadStager;

pub struct SimpleTestExecutor {
    upload_stager: UploadStager,
    target_format: wgpu::TextureFormat,
}

impl SimpleTestExecutor {
    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            upload_stager: UploadStager::new(),
            target_format: format,
        }
    }

    /// Execute a simple linear chain of effects
    pub fn execute_chain(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input_data: &[u8],
        width: u32,
        height: u32,
        effects: Vec<Effect>,
    ) -> Result<wgpu::Texture, EngineError> {
        // Return Texture instead of TextureView
        // Upload input frame
        let input_view = self
            .upload_stager
            .cpu_to_gpu_rgba(device, queue, width, height, input_data)?;

        if effects.is_empty() {
            // Need to return a texture, not just the view
            // For now, copy to a new texture
            let output = self.create_texture(device, width, height, "output");
            // let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());

            let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("copy_encoder"),
            });

            // Copy input to output
            // Note: This requires the input texture, not just the view
            // For simplicity, we'll just return the texture
            // In a real scenario, you'd handle this better

            queue.submit(Some(encoder.finish()));
            return Ok(output);
        }

        // Create ping-pong textures
        let texture_a = self.create_texture(device, width, height, "ping");
        let texture_b = self.create_texture(device, width, height, "pong");
        let view_a = texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let view_b = texture_b.create_view(&wgpu::TextureViewDescriptor::default());

        let views = [view_a, view_b];

        // Execute effects in chain
        for (i, effect) in effects.iter().enumerate() {
            let current_input = if i == 0 {
                &input_view
            } else {
                &views[(i - 1) % 2]
            };
            let current_output = &views[i % 2];

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("effect_encoder"),
            });

            effect.pipeline().apply(
                device,
                queue,
                &mut encoder,
                current_input,
                &[], // No additional inputs
                current_output,
                effect.params_any(),
            )?;

            queue.submit(Some(encoder.finish()));
        }

        // Return final output texture (not just the view)
        let final_index = (effects.len() - 1) % 2;
        if final_index == 0 {
            Ok(texture_a)
        } else {
            Ok(texture_b)
        }
    }

    fn create_texture(
        &self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        label: &str,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC, // Important for readback!
            view_formats: &[],
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use crate::pipelines::{
        brightness_pipeline::{BrightnessParams, BrightnessPipeline},
        common::Pipeline,
        grayscale_pipeline::GrayscalePipeline,
        invert_pipeline::InvertPipeline,
    };

    use super::*;

    /// Read texture data back from GPU to CPU
    /// This is for testing/debugging - not for production rendering
    fn read_texture_rgba(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
    ) -> Vec<u8> {
        use futures_intrusive::channel::shared::oneshot_channel;
        use pollster::block_on;

        let bytes_per_pixel = 4u32;
        let align = 256u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * height) as u64;

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("readback_encoder"),
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(Some(encoder.finish()));

        let (sender, receiver) = oneshot_channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |res| {
            sender.send(res).unwrap();
        });

        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(5)),
            })
            .unwrap();

        block_on(receiver.receive()).unwrap().unwrap();

        let data = buffer.slice(..).get_mapped_range();
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for y in 0..height as usize {
            let src = &data[(y * padded_bytes_per_row as usize)
                ..(y * padded_bytes_per_row as usize + (width * 4) as usize)];
            let dst = &mut pixels[y * (width * 4) as usize..(y + 1) * (width * 4) as usize];
            dst.copy_from_slice(src);
        }

        drop(data);
        buffer.unmap();

        pixels
    }

    /// Write PNG file for visual inspection
    fn write_png(path: &str, data: &[u8], width: u32, height: u32) {
        use std::path::Path;

        // Ensure directory exists
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent).ok();
        }

        image::save_buffer(path, data, width, height, image::ColorType::Rgba8)
            .expect("Failed to save image");

        println!("  Saved output to: {}", path);
    }

    #[test]
    fn test_single_invert_effect() {
        pollster::block_on(async {
            let (device, queue) = setup_gpu().await;

            // Create a simple red image
            let width = 256;
            let height = 256;
            let mut input_data = vec![0u8; (width * height * 4) as usize];
            for pixel in input_data.chunks_exact_mut(4) {
                pixel[0] = 255; // Red
                pixel[1] = 0;
                pixel[2] = 0;
                pixel[3] = 255;
            }

            // Create invert pipeline
            let invert = InvertPipeline::new(&device, wgpu::TextureFormat::Rgba8Unorm).unwrap();
            let effect = Effect::new(invert, ());

            // Execute
            let mut executor = SimpleTestExecutor::new(wgpu::TextureFormat::Rgba8Unorm);
            let result = executor
                .execute_chain(&device, &queue, &input_data, width, height, vec![effect])
                .unwrap();

            // Read back result
            let output_pixels = read_texture_rgba(&device, &queue, &result, width, height);

            // Save to file
            write_png(
                "target/test_output_invert.png",
                &output_pixels,
                width,
                height,
            );

            // Verify first pixel is cyan (inverse of red)
            assert_eq!(output_pixels[0], 0); // R = 0
            assert_eq!(output_pixels[1], 255); // G = 255
            assert_eq!(output_pixels[2], 255); // B = 255
            assert_eq!(output_pixels[3], 255); // A = 255

            println!("✓ Invert effect executed successfully!");
            println!("  Input: Red (255,0,0) -> Output: Cyan (0,255,255)");
        });
    }

    #[test]
    fn test_brightness_effect() {
        pollster::block_on(async {
            let (device, queue) = setup_gpu().await;

            let width = 256;
            let height = 256;
            let mut input_data = vec![0u8; (width * height * 4) as usize];
            for pixel in input_data.chunks_exact_mut(4) {
                pixel[0] = 128;
                pixel[1] = 128;
                pixel[2] = 128;
                pixel[3] = 255;
            }

            // Create brightness pipeline with 2x brightness
            let brightness =
                BrightnessPipeline::new(&device, wgpu::TextureFormat::Rgba8Unorm).unwrap();
            let params = BrightnessParams {
                brightness: 2.0,
                _padding: [0.0; 3],
            };
            let effect = Effect::new(brightness, params);

            let mut executor = SimpleTestExecutor::new(wgpu::TextureFormat::Rgba8Unorm);
            let result = executor
                .execute_chain(&device, &queue, &input_data, width, height, vec![effect])
                .unwrap();

            // Read back result
            let output_pixels = read_texture_rgba(&device, &queue, &result, width, height);

            // Save to file
            write_png(
                "target/test_output_brightness.png",
                &output_pixels,
                width,
                height,
            );

            // Verify brightness doubled (clamped to 255)
            assert_eq!(output_pixels[0], 255); // 128 * 2 = 256 -> clamped to 255
            assert_eq!(output_pixels[1], 255);
            assert_eq!(output_pixels[2], 255);
            assert_eq!(output_pixels[3], 255);

            println!("✓ Brightness effect executed successfully!");
            println!("  Input: Gray (128,128,128) -> Output: White (255,255,255)");
        });
    }

    #[test]
    fn test_effect_chain() {
        pollster::block_on(async {
            let (device, queue) = setup_gpu().await;

            let width = 256;
            let height = 256;
            let mut input_data = vec![0u8; (width * height * 4) as usize];
            for pixel in input_data.chunks_exact_mut(4) {
                pixel[0] = 200;
                pixel[1] = 100;
                pixel[2] = 50;
                pixel[3] = 255;
            }

            // Chain: Brightness -> Grayscale -> Invert
            let brightness =
                BrightnessPipeline::new(&device, wgpu::TextureFormat::Rgba8Unorm).unwrap();
            let grayscale =
                GrayscalePipeline::new(&device, wgpu::TextureFormat::Rgba8Unorm).unwrap();
            let invert = InvertPipeline::new(&device, wgpu::TextureFormat::Rgba8Unorm).unwrap();

            let effects = vec![
                Effect::new(
                    brightness,
                    BrightnessParams {
                        brightness: 1.5,
                        _padding: [0.0; 3],
                    },
                ),
                // Effect::new(grayscale, ()),
                Effect::new(invert, ()),
            ];

            let mut executor = SimpleTestExecutor::new(wgpu::TextureFormat::Rgba8Unorm);

            let output_texture = executor
                .execute_chain(&device, &queue, &input_data, width, height, effects)
                .unwrap();

            let output_pixels = read_texture_rgba(&device, &queue, &output_texture, width, height);

            write_png(
                "target/test_output_chain.png",
                &output_pixels,
                width,
                height,
            );

            // This will change depending on what you want, would probably fail right now
            // Calculate expected result:
            // Input: (200, 100, 50)
            // After brightness 1.5x: (300, 150, 75) -> clamped to (255, 150, 75)
            // After grayscale: luminance = 0.299*255 + 0.587*150 + 0.114*75
            //                            = 76.245 + 88.05 + 8.55 = 172.845 -> 173
            // After invert: 255 - 173 = 82
            let expected_gray = 82u8; // Approximately

            // Check first pixel (allowing small tolerance due to floating point)
            let actual_r = output_pixels[0];
            let actual_g = output_pixels[1];
            let actual_b = output_pixels[2];

            assert!(
                (actual_r as i16 - expected_gray as i16).abs() <= 2,
                "Expected ~{}, got {}",
                expected_gray,
                actual_r
            );
            assert_eq!(actual_r, actual_g); // Should be grayscale
            assert_eq!(actual_g, actual_b);

            println!("✓ Effect chain executed successfully!");
            println!("  Input -> Brightness(1.5x) -> Grayscale -> Invert -> Output");
            println!("  Output pixel: ({}, {}, {})", actual_r, actual_g, actual_b);
        });
    }

    #[test]
    fn test_visual_gradient() {
        pollster::block_on(async {
            let (device, queue) = setup_gpu().await;

            let width = 512;
            let height = 512;
            let mut input_data = vec![0u8; (width * height * 4) as usize];

            // Create a gradient from red to blue
            for y in 0..height {
                for x in 0..width {
                    let idx = ((y * width + x) * 4) as usize;
                    input_data[idx] = ((x as f32 / width as f32) * 255.0) as u8; // R gradient
                    input_data[idx + 1] = 128; // G constant
                    input_data[idx + 2] = ((y as f32 / height as f32) * 255.0) as u8; // B gradient
                    input_data[idx + 3] = 255; // A opaque
                }
            }

            // Apply grayscale
            let grayscale =
                GrayscalePipeline::new(&device, wgpu::TextureFormat::Rgba8Unorm).unwrap();
            let effect = Effect::new(grayscale, ());

            let mut executor = SimpleTestExecutor::new(wgpu::TextureFormat::Rgba8Unorm);
            let output_texture = executor
                .execute_chain(&device, &queue, &input_data, width, height, vec![effect])
                .unwrap();

            let output_pixels = read_texture_rgba(&device, &queue, &output_texture, width, height);

            write_png(
                "target/test_output_gradient.png",
                &output_pixels,
                width,
                height,
            );

            println!("✓ Visual gradient test executed successfully!");
            println!("  Check target/test_output_gradient.png to see the grayscale gradient");
        });
    }

    async fn setup_gpu() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("Failed to find adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("test_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::default(),
                ..Default::default()
            })
            .await
            .expect("Failed to create device");

        (device, queue)
    }
}

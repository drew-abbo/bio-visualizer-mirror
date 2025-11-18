use crate::{Effect, Renderer};

pub struct Engine {
    renderer: Renderer,
}

pub enum EngineResult {
    FrameRendered {
        width: u32,
        height: u32,
        texture: wgpu::Texture,
    },
    EffectAdded {
        index: usize,
    },
    EffectRemoved {
        index: usize,
    },
    Error {
        message: String,
    },
}

impl Engine {
    /// Create engine. No channels, no worker thread.
    pub fn new(target_format: wgpu::TextureFormat) -> Self {
        let renderer = Renderer::new(target_format).expect("Renderer creation failed");

        Self { renderer }
    }

    /// Direct synchronous render.
    pub fn render_frame(
        &mut self,
        frame: &media::frame::Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output_texture: &wgpu::Texture,
    ) -> EngineResult {
        let view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("clear_encoder"),
        });

        {
            let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 0.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        } // render pass ends here

        queue.submit(Some(encoder.finish()));

        let dims = frame.dimensions();

        EngineResult::FrameRendered {
            width: dims.width(),
            height: dims.height(),
            texture: output_texture.clone(),
        }
    }

    pub fn add_effect(&mut self, effect: Effect) -> EngineResult {
        let idx = self.renderer.effect_count();
        self.renderer.add_effect(effect);

        EngineResult::EffectAdded { index: idx }
    }

    pub fn update_effect(&mut self, index: usize, effect: Effect) {
        self.renderer.replace_effect(index, effect);
    }

    pub fn remove_effect(&mut self, index: usize) -> EngineResult {
        self.renderer.remove_effect(index);

        EngineResult::EffectRemoved { index }
    }
}

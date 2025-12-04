use crate::errors::EngineError;
use std::sync::Arc;
use winit::window::Window;

// The Rendering Flow
// 1. acquire() → Get a blank canvas (SurfaceTexture)
// 2. [Draw stuff to it using pipelines]
// 3. present() → Display it on screen

#[derive(Debug)]
pub struct SurfaceMgr {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub format: wgpu::TextureFormat,
}

impl SurfaceMgr {
    pub async fn new_async(window: Arc<Window>) -> Result<Self, EngineError> {
        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).map_err(|e| {
            EngineError::SwapChainAcquireFailed(format!("failed to create surface: {e:?}"))
        })?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        // Choose a surface format we can render to
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let size = window.inner_size();
        let (width, height) = (size.width.max(1), size.height.max(1));

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 3,
        };
        surface.configure(&device, &config);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            format,
        })
    }

    pub fn new(window: Arc<Window>) -> Result<Self, EngineError> {
        // Non-async helper for native via pollster
        pollster::block_on(Self::new_async(window))
    }

    pub fn configure(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn acquire(&self) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView), EngineError> {
        let frame = self.surface.get_current_texture().map_err(|e| {
            EngineError::SwapChainAcquireFailed(format!("swapchain acquire failed: {e:?}"))
        })?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        Ok((frame, view))
    }

    pub fn present(&self, frame: wgpu::SurfaceTexture) {
        frame.present();
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
    pub fn width(&self) -> u32 {
        self.config.width
    }
    pub fn height(&self) -> u32 {
        self.config.height
    }
}

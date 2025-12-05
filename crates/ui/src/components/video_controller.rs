use eframe::wgpu;
use engine::renderer::Renderer;
use engine::renderer::pipelines::color_grading::ColorGradingPipeline;
use engine::renderer::pipelines::common::Pipeline;
use engine::types::ColorGradingParams;
use media::VideoPlayer;

/// Coordinates video playback and rendering
pub struct VideoController {
    player: Option<VideoPlayer>,
    renderer: Renderer,
}

impl VideoController {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) -> Result<Self, engine::errors::EngineError> {
        let mut renderer = Renderer::new(target_format)?;

        // Add default color grading effect
        let pipeline = ColorGradingPipeline::new(device, target_format)?;
        let params = ColorGradingParams {
            exposure: 1.0,
            contrast: 1.2,
            saturation: 1.1,
            vignette: 0.3,
            time: 0.0,
            surface_w: 0.0,
            surface_h: 0.0,
            _pad0: 0.0,
        };
        renderer.add_effect(pipeline, params);

        Ok(Self {
            player: None,
            renderer,
        })
    }

    /// Load a video file
    pub fn load_video(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let stream = media::frame::streams::Video::new(path)?;
        let producer =
            media::frame::Producer::new(stream, media::frame::streams::OnStreamEnd::Loop)?;

        self.player = Some(VideoPlayer::new(producer));
        Ok(())
    }

    /// Load from an existing producer
    pub fn load_producer(&mut self, producer: media::frame::Producer) {
        self.player = Some(VideoPlayer::new(producer));
    }

    /// Update playback and render current frame
    pub fn update_and_render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Option<RenderResult>, engine::errors::EngineError> {
        let Some(player) = &mut self.player else {
            return Ok(None);
        };

        player.update();

        // Get current frame
        let Some(current_frame) = player.current_frame() else {
            return Ok(None);
        };

        // Render with effects
        let texture_view = self.renderer.render_frame(current_frame, device, queue)?;
        let dims = current_frame.dimensions();

        Ok(Some(RenderResult {
            texture_view,
            size: [dims.width() as usize, dims.height() as usize],
        }))
    }

    pub fn player(&self) -> Option<&VideoPlayer> {
        self.player.as_ref()
    }

    pub fn player_mut(&mut self) -> Option<&mut VideoPlayer> {
        self.player.as_mut()
    }

    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    pub fn renderer_mut(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    pub fn has_video(&self) -> bool {
        self.player.is_some()
    }

    pub fn is_playing(&self) -> bool {
        self.player.as_ref().map_or(false, |p| p.is_playing())
    }
}

pub struct RenderResult {
    pub texture_view: wgpu::TextureView,
    pub size: [usize; 2],
}

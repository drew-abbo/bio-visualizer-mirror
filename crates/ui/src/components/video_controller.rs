use eframe::wgpu;
use engine::renderer::Renderer;
use media::VideoPlayer;

/// Coordinates video playback and rendering
pub struct VideoController {
    player: Option<VideoPlayer>,
    renderer: Renderer,
}

impl VideoController {
    pub fn new(
        target_format: wgpu::TextureFormat,
    ) -> Result<Self, engine::errors::EngineError> {
        let renderer = Renderer::new(target_format)?;

        Ok(Self {
            player: None,
            renderer,
        })
    }

    pub fn load_video(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let stream = media::frame::streams::Video::new(path)?;
        let producer =
            media::frame::Producer::new(stream, media::frame::streams::OnStreamEnd::Loop)?;

        self.player = Some(VideoPlayer::new(producer));
        Ok(())
    }

    pub fn load_producer(&mut self, producer: media::frame::Producer) {
        self.player = Some(VideoPlayer::new(producer));
    }

    /// Update playback and render current frame if needed
    pub fn update_and_render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        dt: f32,
    ) -> Result<Option<RenderResult>, engine::errors::EngineError> {
        let Some(player) = &mut self.player else {
            return Ok(None);
        };

        // Check if we need a new frame
        let has_new_frame = player.update_with_dt(dt);

        if !has_new_frame {
            return Ok(None);
        }

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
            frame_id: current_frame.uid(),
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
    pub frame_id: media::frame::Uid,
}
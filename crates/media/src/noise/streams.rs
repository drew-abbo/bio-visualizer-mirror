use crate::fps::Fps;
use crate::fps::SwitchTimer;
use crate::fps::consts::FPS_30;
use crate::playback_stream::PlaybackStream;

pub trait NoiseGeneratorCore {
    fn sample_at_time(&mut self, t_seconds: f64) -> Result<f32, NoiseStreamError>;
}

impl<F> NoiseGeneratorCore for F
where
    F: FnMut(f64) -> Result<f32, NoiseStreamError>,
{
    fn sample_at_time(&mut self, t_seconds: f64) -> Result<f32, NoiseStreamError> {
        self(t_seconds)
    }
}

pub trait NoiseStream: PlaybackStream<f32, NoiseStreamError> {}

impl<T> NoiseStream for T where T: PlaybackStream<f32, NoiseStreamError> {}

#[derive(Debug, thiserror::Error, Clone)]
pub enum NoiseStreamError {
    #[error("noise stream generator failed: {0}")]
    Generator(String),
}

pub struct ProceduralNoiseStream<G>
where
    G: NoiseGeneratorCore,
{
    generator: G,
    target_fps: Fps,
    switch_timer: SwitchTimer,
    paused: bool,
    playhead: usize,
    last_sample: f32,
}

impl<G> ProceduralNoiseStream<G>
where
    G: NoiseGeneratorCore,
{
    pub fn new(generator: G) -> Self {
        Self {
            generator,
            target_fps: FPS_30,
            switch_timer: SwitchTimer::new(FPS_30),
            paused: false,
            playhead: 0,
            last_sample: 0.0,
        }
    }
}

impl<G> PlaybackStream<f32, NoiseStreamError> for ProceduralNoiseStream<G>
where
    G: NoiseGeneratorCore + 'static,
{
    fn fetch(&mut self) -> Result<f32, NoiseStreamError> {
        if self.paused {
            return Ok(self.last_sample);
        }

        if !self.switch_timer.is_switch_time() {
            return Ok(self.last_sample);
        }

        let t_seconds = self.playhead as f64 / self.target_fps.as_float();
        let sample = self.generator.sample_at_time(t_seconds)?.clamp(0.0, 1.0);

        self.last_sample = sample;
        self.playhead = self.playhead.saturating_add(1);

        Ok(sample)
    }

    fn set_target_fps(&mut self, new_target_fps: Fps) {
        self.target_fps = new_target_fps;
        self.switch_timer.set_target_fps(new_target_fps);
    }

    fn target_fps(&self) -> Fps {
        self.target_fps
    }

    fn set_paused(&mut self, paused: bool) -> bool {
        if self.paused && !paused {
            self.switch_timer.reset();
        }
        self.paused = paused;
        !self.paused
    }

    fn is_paused(&self) -> bool {
        self.paused
    }

    fn seek_controls(
        &mut self,
    ) -> Option<&mut dyn crate::playback_stream::SeekablePlaybackStream<f32, NoiseStreamError>>
    {
        None
    }
}

use std::collections::HashMap;
use std::path::PathBuf;

use media::frame::{Frame, Producer, streams::OnStreamEnd, streams::Video};

use crate::gpu_frame::GpuFrame;
use crate::graph_executor::{ExecutionContext, ExecutionError, NodeValue};
use crate::node::handler::node_handler::NodeHandler;
use crate::upload_stager::UploadStager;

/// Video source with frame caching (must be kept alive between executions)
pub struct VideoSourceHandler {
    producer_cache: HashMap<PathBuf, Producer>,
    playback_state: HashMap<PathBuf, PlaybackState>,
}

#[derive(Default)]
struct PlaybackState {
    accumulator: f64,
    last_gpu_frame: Option<GpuFrame>,
    /// CPU frame pending recycling back to the producer.
    /// We hold onto it until we need to fetch the next frame.
    pending_recycle: Option<Frame>,
}

impl Default for VideoSourceHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoSourceHandler {
    pub fn new() -> Self {
        Self {
            producer_cache: HashMap::new(),
            playback_state: HashMap::new(),
        }
    }

    pub fn clear_cache(&mut self) {
        self.producer_cache.clear();
        self.playback_state.clear();
    }

    fn get_or_create_producer(&mut self, path: &PathBuf) -> Result<&mut Producer, ExecutionError> {
        if !self.producer_cache.contains_key(path) {
            let stream = Video::new(path).map_err(|e| {
                ExecutionError::VideoStreamError(
                    path.clone(),
                    format!("Failed to open video stream: {:?}", e),
                )
            })?;

            let producer = Producer::new(stream, OnStreamEnd::Loop).map_err(|e| {
                ExecutionError::ProducerCreateError(
                    path.clone(),
                    format!("Failed to create producer: {:?}", e),
                )
            })?;

            self.producer_cache.insert(path.clone(), producer);
        }

        Ok(self.producer_cache.get_mut(path).unwrap())
    }

    pub fn fetch_frame(&mut self, path: &PathBuf) -> Result<Frame, ExecutionError> {
        // First, recycle any pending frame back to the producer
        if let Some(playback_state) = self.playback_state.get_mut(path) {
            if let Some(frame_to_recycle) = playback_state.pending_recycle.take() {
                if let Some(producer) = self.producer_cache.get_mut(path) {
                    producer.recycle_frame(frame_to_recycle);
                }
            }
        }

        let producer = self.get_or_create_producer(path)?;
        producer.fetch_frame().map_err(|e| {
            ExecutionError::VideoFetchError(path.clone(), format!("Failed to fetch frame: {:?}", e))
        })
    }

    pub fn get_stats(&mut self, path: &PathBuf) -> Result<(f32, f64), ExecutionError> {
        let producer = self.get_or_create_producer(path)?;
        let fps = producer.stats().fps as f32;
        let duration_secs = producer
            .stats()
            .stream_duration()
            .map(|d: std::time::Duration| d.as_secs_f64())
            .unwrap_or(0.0);
        Ok((fps, duration_secs))
    }
}

impl NodeHandler for VideoSourceHandler {
    fn execute(
        &mut self,
        inputs: &HashMap<String, NodeValue>,
        inputs: &HashMap<String, NodeValue>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        upload_stager: &mut UploadStager,
        context: &ExecutionContext,
    ) -> Result<Vec<NodeValue>, ExecutionError> {
        // Find the File input there should only be one
        let path = inputs
            .values()
            .find_map(|v| match v {
                NodeValue::File(p) => Some(p),
                _ => None,
            })
            .ok_or(ExecutionError::InvalidInputType)?;

        // Find the playback rate (Float input, defaults to 1.0 if not provided)
        // I think this is fine since these are not user defined nodes
        let playback_rate = inputs
            .values()
            .find_map(|v| match v {
                NodeValue::Float(rate) => Some(*rate),
                _ => None,
            })
            .unwrap_or(1.0);

        let (native_fps, duration_secs) = self.get_stats(path)?;
        let sampling_rate_hz = if context.sampling_rate_hz > 0.0 {
            context.sampling_rate_hz
        } else {
            30.0
        };

        let effective_fps = (native_fps as f64) * (playback_rate as f64);
        let advance_ratio = if sampling_rate_hz > 0.0 {
            effective_fps / sampling_rate_hz
        } else {
            0.0
        };

        let (mut frames_to_advance, cached_frame) = {
<<<<<<< HEAD
            let playback_state = self.playback_state.entry(path.clone()).or_default();
=======
            let playback_state = self
                .playback_state
                .entry(path.clone())
                .or_insert_with(PlaybackState::default);
>>>>>>> 4e14061 (fps control and some more fixes)

            if context.advance_frame {
                playback_state.accumulator += advance_ratio;
            }

            let frames_to_advance = playback_state.accumulator.floor() as u32;
            playback_state.accumulator -= frames_to_advance as f64;

            let cached_frame = if frames_to_advance == 0 {
                playback_state.last_gpu_frame.clone()
            } else {
                None
            };

            (frames_to_advance, cached_frame)
        };

        if let Some(gpu_frame) = cached_frame {
            return Ok(vec![
                NodeValue::Frame(gpu_frame),
                NodeValue::Float(effective_fps as f32),
                NodeValue::Float(duration_secs as f32),
            ]);
        }

        if frames_to_advance == 0 {
            frames_to_advance = 1;
        }

        let mut frame = self.fetch_frame(path)?;
        for _ in 1..frames_to_advance {
            frame = self.fetch_frame(path)?;
        }

        let width = frame.dimensions().width();
        let height = frame.dimensions().height();

        // Upload frame to GPU
        let texture_view = upload_stager
            .cpu_to_gpu_rgba(device, queue, width, height, frame.raw_data())
            .map_err(|e| {
                ExecutionError::TextureUploadError(format!("Failed to upload texture: {:?}", e))
            })?;

        let gpu_frame = GpuFrame::new(
            texture_view,
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        // Store GPU frame for reuse when playback rate is < 1.0, and
        // store CPU frame for recycling back to the producer on next fetch
        if let Some(playback_state) = self.playback_state.get_mut(path) {
            playback_state.last_gpu_frame = Some(gpu_frame.clone());
            playback_state.pending_recycle = Some(frame);
        }

        // Return outputs in the order defined in node.json: Output, Fps, Duration
        // Again this is not user defined so this is acceptable
        Ok(vec![
            NodeValue::Frame(gpu_frame),
            NodeValue::Float(effective_fps as f32),
            NodeValue::Float(duration_secs as f32),
        ])
    }
}

use std::collections::HashMap;
use std::path::PathBuf;

use media::frame::{Frame, Uid, streams::FrameStream};

use crate::gpu_frame::GpuFrame;
use crate::graph_executor::{ExecutionContext, ExecutionError, NodeValue};
use crate::node::handler::node_handler::NodeHandler;
use crate::upload_stager::UploadStager;

#[derive(Default)]
struct VideoPlaybackState {
    last_frame_uid: Option<Uid>,
    last_gpu_frame: Option<GpuFrame>,
    is_playing: bool,
    cached_fps: Option<f32>,
    cached_duration_secs: Option<f64>,
}

/// Video source with stream caching (must be kept alive between executions)
pub struct VideoSourceHandler {
    stream_cache: HashMap<PathBuf, Box<dyn FrameStream>>,
    playback_state: HashMap<PathBuf, VideoPlaybackState>,
}

impl Default for VideoSourceHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoSourceHandler {
    pub fn new() -> Self {
        Self {
            stream_cache: HashMap::new(),
            playback_state: HashMap::new(),
        }
    }

    pub fn clear_cache(&mut self) {
        self.stream_cache.clear();
        self.playback_state.clear();
    }

    fn get_or_create_stream(
        &mut self,
        path: &PathBuf,
    ) -> Result<&mut Box<dyn FrameStream>, ExecutionError> {
        if !self.stream_cache.contains_key(path) {
            // Create a new VideoFrameStream with looping enabled
            let mut request = media::frame::streams::VideoFrameStream::builder()
                .set_loop(true)
                .build(path);

            let stream = request.wait().map_err(|e| {
                ExecutionError::VideoStreamError(
                    path.clone(),
                    format!("Failed to create video stream: {:?}", e),
                )
            })?;

            let stream = stream.map_err(|e| {
                ExecutionError::VideoStreamError(
                    path.clone(),
                    format!("Failed to open video stream: {:?}", e),
                )
            })?;

            self.stream_cache.insert(path.clone(), Box::new(stream));
        }

        Ok(self.stream_cache.get_mut(path).unwrap())
    }

    pub fn fetch_frame(&mut self, path: &PathBuf) -> Result<Frame, ExecutionError> {
        let stream = self.get_or_create_stream(path)?;
        stream.fetch().map_err(|e| {
            ExecutionError::VideoFetchError(path.clone(), format!("Failed to fetch frame: {:?}", e))
        })
    }

    pub fn get_stats(&mut self, path: &PathBuf) -> Result<(f32, f64), ExecutionError> {
        let stream = self.get_or_create_stream(path)?;
        let fps = stream.target_fps().as_float();

        let duration_secs = stream
            .seek_controls()
            .map(|seek| {
                if fps > 0.0 {
                    seek.unclipped_stream_duration() as f64 / fps
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);

        Ok((fps as f32, duration_secs))
    }

    fn get_cached_stats(&mut self, path: &PathBuf) -> Result<(f32, f64), ExecutionError> {
        if let Some(state) = self.playback_state.get(path)
            && let (Some(fps), Some(duration_secs)) = (state.cached_fps, state.cached_duration_secs)
        {
            return Ok((fps, duration_secs));
        }

        let (fps, duration_secs) = self.get_stats(path)?;
        let state = self.playback_state.entry(path.clone()).or_default();
        state.cached_fps = Some(fps);
        state.cached_duration_secs = Some(duration_secs);
        Ok((fps, duration_secs))
    }

    fn recycle_frame(&mut self, path: &PathBuf, frame: Frame) {
        if let Ok(stream) = self.get_or_create_stream(path) {
            stream.recycle(frame);
        }
    }
}

impl NodeHandler for VideoSourceHandler {
    fn execute(
        &mut self,
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

        // Apply play/pause only on transitions to avoid expensive churn.
        let needs_mode_update = self
            .playback_state
            .get(path)
            .map(|state| state.is_playing != context.advance_frame)
            .unwrap_or(true);

        if needs_mode_update {
            let stream = self.get_or_create_stream(path)?;
            if context.advance_frame {
                stream.play();
            } else {
                stream.pause();
            }

            let state = self.playback_state.entry(path.clone()).or_default();
            state.is_playing = context.advance_frame;
        }

        let (native_fps, duration_secs) = self.get_cached_stats(path)?;

        // If we are not advancing playback, return the already-uploaded frame.
        if !context.advance_frame
            && let Some(state) = self.playback_state.get(path)
            && let Some(frame) = &state.last_gpu_frame
        {
            return Ok(vec![
                NodeValue::Frame(frame.clone()),
                NodeValue::Float(native_fps),
                NodeValue::Float(duration_secs as f32),
            ]);
        }

        // Fetch and upload only when we need a new frame.
        let frame = self.fetch_frame(path)?;
        let frame_uid = frame.uid();

        // If the stream gave us the same frame again, avoid re-uploading.
        if let Some(cached_gpu_frame) = self
            .playback_state
            .get(path)
            .and_then(|state| {
                (state.last_frame_uid == Some(frame_uid))
                    .then(|| state.last_gpu_frame.clone())
                    .flatten()
            })
        {
            self.recycle_frame(path, frame);
            return Ok(vec![
                NodeValue::Frame(cached_gpu_frame),
                NodeValue::Float(native_fps),
                NodeValue::Float(duration_secs as f32),
            ]);
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

        self.recycle_frame(path, frame);

        let state = self.playback_state.entry(path.clone()).or_default();
        state.last_frame_uid = Some(frame_uid);
        state.last_gpu_frame = Some(gpu_frame.clone());

        // Return outputs in the order defined in node.json: Output, Fps, Duration
        Ok(vec![
            NodeValue::Frame(gpu_frame),
            NodeValue::Float(native_fps),
            NodeValue::Float(duration_secs as f32),
        ])
    }
}

use std::collections::HashMap;
use std::path::PathBuf;

use media::fps::Fps;
use media::frame::Frame;
use media::frame::streams::{FrameStream, FrameStreamError, VideoFrameStream};
use util::channels::request_channel::Request;

use crate::gpu_frame::GpuFrame;
use crate::graph_executor::{ExecutionContext, ExecutionError, NodeValue};
use crate::node::handler::node_handler::NodeHandler;
use crate::upload_stager::UploadStager;

#[derive(Default)]
struct VideoPlaybackState {
    last_gpu_frame: Option<GpuFrame>,
    is_playing: bool,
}

/// Video source with stream caching (must be kept alive between executions)
pub struct VideoSourceHandler {
    stream_cache: HashMap<PathBuf, Box<dyn FrameStream>>,
    pending_stream_requests: HashMap<PathBuf, Request<Result<VideoFrameStream, FrameStreamError>>>,
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
            pending_stream_requests: HashMap::new(),
            playback_state: HashMap::new(),
        }
    }

    pub fn clear_cache(&mut self) {
        self.stream_cache.clear();
        self.pending_stream_requests.clear();
        self.playback_state.clear();
    }

    fn try_get_or_create_stream(
        &mut self,
        path: &PathBuf,
    ) -> Result<Option<&mut Box<dyn FrameStream>>, ExecutionError> {
        if self.stream_cache.contains_key(path) {
            return Ok(self.stream_cache.get_mut(path));
        }

        self.pending_stream_requests
            .entry(path.clone())
            .or_insert_with(|| VideoFrameStream::builder().set_loop(true).build(path));

        let ready_stream = {
            let request = self.pending_stream_requests.get_mut(path).unwrap();
            request.check_non_blocking().map_err(|e| {
                ExecutionError::VideoStreamError(
                    path.clone(),
                    format!("Failed to create video stream: {:?}", e),
                )
            })?
        };

        match ready_stream {
            Some(stream_result) => {
                self.pending_stream_requests.remove(path);

                let stream = stream_result.map_err(|e| {
                    ExecutionError::VideoStreamError(
                        path.clone(),
                        format!("Failed to open video stream: {:?}", e),
                    )
                })?;

                self.stream_cache.insert(path.clone(), Box::new(stream));
                Ok(self.stream_cache.get_mut(path))
            }
            None => Ok(None),
        }
    }

    pub fn fetch_frame(&mut self, path: &PathBuf) -> Result<(Frame, bool), ExecutionError> {
        let Some(stream) = self.try_get_or_create_stream(path)? else {
            return Err(ExecutionError::VideoStreamNotReady(path.clone()));
        };

        let frame = stream.fetch().map_err(|e| {
            ExecutionError::VideoFetchError(path.clone(), format!("Failed to fetch frame: {:?}", e))
        })?;
        let is_distinct = stream.last_frame_is_distinct_from_previous();

        Ok((frame, is_distinct))
    }

    pub fn get_target_fps(&mut self, path: &PathBuf) -> Result<Fps, ExecutionError> {
        let Some(stream) = self.try_get_or_create_stream(path)? else {
            return Err(ExecutionError::VideoStreamNotReady(path.clone()));
        };

        Ok(stream.target_fps())
    }

    fn recycle_frame(&mut self, path: &PathBuf, frame: Frame) {
        if let Ok(Some(stream)) = self.try_get_or_create_stream(path) {
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

        // Poll stream creation without blocking the frame loop.
        let stream_ready = self.try_get_or_create_stream(path)?.is_some();

        if !stream_ready {
            if let Some(state) = self.playback_state.get(path)
                && let Some(frame) = &state.last_gpu_frame
            {
                return Ok(vec![NodeValue::Frame(frame.clone())]);
            }

            return Err(ExecutionError::VideoStreamNotReady(path.clone()));
        }

        // Apply play/pause only on transitions to avoid expensive churn.
        let needs_mode_update = self
            .playback_state
            .get(path)
            .map(|state| state.is_playing != context.playback_running)
            .unwrap_or(true);

        if needs_mode_update {
            let stream = self
                .try_get_or_create_stream(path)?
                .expect("stream checked above");
            if context.playback_running {
                stream.play();
            } else {
                stream.pause();
            }

            let state = self.playback_state.entry(path.clone()).or_default();
            state.is_playing = context.playback_running;
        }

        // If we are not advancing playback, return the already-uploaded frame.
        if !context.advance_frame
            && let Some(state) = self.playback_state.get(path)
            && let Some(frame) = &state.last_gpu_frame
        {
            return Ok(vec![NodeValue::Frame(frame.clone())]);
        }

        // Fetch and upload only when we need a new frame.
        let (frame, is_distinct) = self.fetch_frame(path)?;

        // Stream says this frame is unchanged from the previous one, so reuse
        // the cached GPU upload when available.
        if !is_distinct
            && let Some(cached_gpu_frame) = self
                .playback_state
                .get(path)
                .and_then(|state| state.last_gpu_frame.clone())
        {
            self.recycle_frame(path, frame);
            return Ok(vec![NodeValue::Frame(cached_gpu_frame)]);
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
        state.last_gpu_frame = Some(gpu_frame.clone());

        Ok(vec![NodeValue::Frame(gpu_frame)])
    }
}

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use media::fps::Fps;
use media::frame::Frame;
use media::frame::streams::{FrameStream, FrameStreamError, VideoFrameStream};
use util::channels::request_channel::Request;

use super::NodeHandler;
use crate::gpu_frame::GpuFrame;
use crate::graph_executor::{ExecutionContext, ExecutionError, NodeValue};
use crate::node_graph::EngineNodeId;
use crate::upload_stager::UploadStager;

#[derive(Default)]
struct VideoPlaybackState {
    last_gpu_frame: Option<GpuFrame>,
    is_playing: bool,
}

/// Video source with stream caching (must be kept alive between executions).
///
/// Keyed by [EngineNodeId] so two nodes loading the same path get independent
/// streams and playback state.
pub struct VideoSourceHandler {
    stream_cache: HashMap<EngineNodeId, Box<dyn FrameStream>>,
    pending_stream_requests:
        HashMap<EngineNodeId, Request<Result<VideoFrameStream, FrameStreamError>>>,
    playback_state: HashMap<EngineNodeId, VideoPlaybackState>,
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

    /// Evict cache entries for nodes no longer present in the graph.
    pub fn retain_nodes(&mut self, active_nodes: &HashSet<EngineNodeId>) {
        self.stream_cache.retain(|id, _| active_nodes.contains(id));
        self.pending_stream_requests
            .retain(|id, _| active_nodes.contains(id));
        self.playback_state
            .retain(|id, _| active_nodes.contains(id));
    }

    /// Apply play/pause to all cached streams based on whether their node is
    /// in the active execution set.
    pub fn set_playback_for_nodes(
        &mut self,
        active_nodes: &HashSet<EngineNodeId>,
        playback_running: bool,
    ) {
        let cached_ids: Vec<EngineNodeId> = self.stream_cache.keys().copied().collect();

        for id in cached_ids {
            let should_play = active_nodes.contains(&id) && playback_running;

            if let Some(stream) = self.stream_cache.get_mut(&id) {
                if should_play {
                    stream.play();
                } else {
                    stream.pause();
                }
            }

            self.playback_state.entry(id).or_default().is_playing = should_play;
        }
    }

    fn try_get_or_create_stream(
        &mut self,
        node_id: EngineNodeId,
        path: &PathBuf,
    ) -> Result<Option<&mut Box<dyn FrameStream>>, ExecutionError> {
        if self.stream_cache.contains_key(&node_id) {
            return Ok(self.stream_cache.get_mut(&node_id));
        }

        self.pending_stream_requests
            .entry(node_id)
            .or_insert_with(|| VideoFrameStream::builder().set_loop(true).build(path));

        let ready_stream = {
            let request = self.pending_stream_requests.get_mut(&node_id).unwrap();
            request.check_non_blocking().map_err(|e| {
                ExecutionError::VideoStreamError(
                    path.clone(),
                    format!("Failed to create video stream: {:?}", e),
                )
            })?
        };

        match ready_stream {
            Some(stream_result) => {
                self.pending_stream_requests.remove(&node_id);

                let stream = stream_result.map_err(|e| {
                    ExecutionError::VideoStreamError(
                        path.clone(),
                        format!("Failed to open video stream: {:?}", e),
                    )
                })?;

                self.stream_cache.insert(node_id, Box::new(stream));
                Ok(self.stream_cache.get_mut(&node_id))
            }
            None => Ok(None),
        }
    }

    pub fn fetch_frame(
        &mut self,
        node_id: EngineNodeId,
        path: &PathBuf,
    ) -> Result<(Frame, bool), ExecutionError> {
        let Some(stream) = self.try_get_or_create_stream(node_id, path)? else {
            return Err(ExecutionError::VideoStreamNotReady(path.clone()));
        };

        let frame = stream.fetch().map_err(|e| {
            ExecutionError::VideoFetchError(path.clone(), format!("Failed to fetch frame: {:?}", e))
        })?;
        let is_distinct = stream.last_frame_is_distinct_from_previous();

        Ok((frame, is_distinct))
    }

    pub fn get_target_fps(
        &mut self,
        node_id: EngineNodeId,
        path: &PathBuf,
    ) -> Result<Fps, ExecutionError> {
        let Some(stream) = self.try_get_or_create_stream(node_id, path)? else {
            return Err(ExecutionError::VideoStreamNotReady(path.clone()));
        };

        Ok(stream.target_fps())
    }

    fn recycle_frame(&mut self, node_id: EngineNodeId, path: &PathBuf, frame: Frame) {
        if let Ok(Some(stream)) = self.try_get_or_create_stream(node_id, path) {
            stream.recycle(frame);
        }
    }

    pub fn execute_for_node(
        &mut self,
        node_id: EngineNodeId,
        inputs: &HashMap<String, NodeValue>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        upload_stager: &mut UploadStager,
        context: &ExecutionContext,
    ) -> Result<Vec<NodeValue>, ExecutionError> {
        let path = inputs
            .values()
            .find_map(|v| match v {
                NodeValue::File(p) => Some(p),
                _ => None,
            })
            .ok_or(ExecutionError::InvalidInputType)?;

        let stream_ready = self.try_get_or_create_stream(node_id, path)?.is_some();

        if !stream_ready {
            if let Some(frame) = self
                .playback_state
                .get(&node_id)
                .and_then(|s| s.last_gpu_frame.clone())
            {
                return Ok(vec![NodeValue::Frame(frame)]);
            }
            return Err(ExecutionError::VideoStreamNotReady(path.clone()));
        }

        let needs_mode_update = self
            .playback_state
            .get(&node_id)
            .map(|s| s.is_playing != context.playback_running)
            .unwrap_or(true);

        if needs_mode_update {
            let stream = self
                .try_get_or_create_stream(node_id, path)?
                .expect("stream checked above");
            if context.playback_running {
                stream.play();
            } else {
                stream.pause();
            }
            self.playback_state.entry(node_id).or_default().is_playing = context.playback_running;
        }

        if !context.advance_frame {
            if let Some(frame) = self
                .playback_state
                .get(&node_id)
                .and_then(|s| s.last_gpu_frame.clone())
            {
                return Ok(vec![NodeValue::Frame(frame)]);
            }
        }

        let (frame, is_distinct) = self.fetch_frame(node_id, path)?;

        if !is_distinct {
            if let Some(cached) = self
                .playback_state
                .get(&node_id)
                .and_then(|s| s.last_gpu_frame.clone())
            {
                self.recycle_frame(node_id, path, frame);
                return Ok(vec![NodeValue::Frame(cached)]);
            }
        }

        let width = frame.dimensions().width();
        let height = frame.dimensions().height();

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

        self.recycle_frame(node_id, path, frame);

        self.playback_state
            .entry(node_id)
            .or_default()
            .last_gpu_frame = Some(gpu_frame.clone());

        Ok(vec![NodeValue::Frame(gpu_frame)])
    }
}

impl NodeHandler for VideoSourceHandler {
    fn execute(
        &mut self,
        node_id: EngineNodeId,
        inputs: &HashMap<String, NodeValue>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        upload_stager: &mut UploadStager,
        context: &ExecutionContext,
    ) -> Result<Vec<NodeValue>, ExecutionError> {
        let path = inputs
            .values()
            .find_map(|v| match v {
                NodeValue::File(p) => Some(p),
                _ => None,
            })
            .ok_or(ExecutionError::InvalidInputType)?;

        let stream_ready = self.try_get_or_create_stream(node_id, path)?.is_some();

        if !stream_ready {
            if let Some(state) = self.playback_state.get(&node_id)
                && let Some(frame) = &state.last_gpu_frame
            {
                return Ok(vec![NodeValue::Frame(frame.clone())]);
            }

            return Err(ExecutionError::VideoStreamNotReady(path.clone()));
        }

        let needs_mode_update = self
            .playback_state
            .get(&node_id)
            .map(|state| state.is_playing != context.playback_running)
            .unwrap_or(true);

        if needs_mode_update {
            let stream = self
                .try_get_or_create_stream(node_id, path)?
                .expect("stream checked above");
            if context.playback_running {
                stream.play();
            } else {
                stream.pause();
            }

            self.playback_state.entry(node_id).or_default().is_playing = context.playback_running;
        }

        if !context.advance_frame
            && let Some(state) = self.playback_state.get(&node_id)
            && let Some(frame) = &state.last_gpu_frame
        {
            return Ok(vec![NodeValue::Frame(frame.clone())]);
        }

        let (frame, is_distinct) = self.fetch_frame(node_id, path)?;

        if !is_distinct
            && let Some(cached_gpu_frame) = self
                .playback_state
                .get(&node_id)
                .and_then(|state| state.last_gpu_frame.clone())
        {
            self.recycle_frame(node_id, path, frame);
            return Ok(vec![NodeValue::Frame(cached_gpu_frame)]);
        }

        let width = frame.dimensions().width();
        let height = frame.dimensions().height();

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

        self.recycle_frame(node_id, path, frame);

        self.playback_state
            .entry(node_id)
            .or_default()
            .last_gpu_frame = Some(gpu_frame.clone());

        Ok(vec![NodeValue::Frame(gpu_frame)])
    }
}

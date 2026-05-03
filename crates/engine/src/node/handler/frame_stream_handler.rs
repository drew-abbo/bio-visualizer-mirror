use crate::node_graph::EngineNodeId;
use crate::{gpu_frame::GpuFrame, graph_executor::NodeValue, upload_stager::UploadStager};
use media::fps::{Fps, consts::FPS_30};
use media::frame::streams::{FrameStream, FrameStreamError, StillFrameStream, VideoFrameStream};
use media::frame::{Frame, FromImgFileError};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use util::channels::message_channel::Outbox;

use super::timed_stream_handler::TimedStreamHandler;

/// Status messages sent from the engine to the app about stream loading progress.
#[derive(Clone, Debug)]
pub enum StreamLoadingStatus {
    /// A video stream is starting to load
    LoadingStarted { path: PathBuf },
    /// A video stream has finished loading successfully
    LoadingCompleted { path: PathBuf },
    /// A video stream failed to load
    LoadingFailed { path: PathBuf, error: String },
}

#[derive(Debug, thiserror::Error)]
pub enum FrameStreamHandlerError {
    #[error("Failed to open video stream for '{path}': {source}")]
    VideoStream {
        path: PathBuf,
        #[source]
        source: FrameStreamError,
    },
    #[error("Failed to load image for '{path}': {source}")]
    ImageStream {
        path: PathBuf,
        #[source]
        source: FromImgFileError,
    },
    #[error("Failed to fetch frame for '{path}': {source}")]
    FetchStream {
        path: PathBuf,
        #[source]
        source: FrameStreamError,
    },
    #[error("Failed to upload frame texture for '{path}': {error}")]
    TextureUpload { path: PathBuf, error: String },
    #[error("Stream for '{path}' is still loading, will be available next frame")]
    StreamStillLoading { path: PathBuf },
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub enum StreamKind {
    Image,
    Video,
}

pub struct NodeFrameStreamRequest {
    pub node_id: EngineNodeId,
    pub file_path: PathBuf,
    pub stream_kind: StreamKind,
}

#[derive(Clone, Hash, Eq, PartialEq)]
struct NodeFrameStreamKey {
    node_id: EngineNodeId,
    file_path: PathBuf,
    stream_kind: StreamKind,
}

pub struct FrameStreamHandler {
    /// Cache of video and image streams keyed by node id + file path + stream kind.
    stream_cache: HashMap<NodeFrameStreamKey, Box<dyn FrameStream>>,
    /// Streams currently loading in background threads, keyed by (node_id, file_path, kind).
    /// The Arc<Mutex> contains Some(stream) when loading completes, None while loading.
    loading_streams: HashMap<NodeFrameStreamKey, Arc<Mutex<Option<Box<dyn FrameStream>>>>>,
    /// Optional outbox for sending status messages to the app.
    status_outbox: Arc<Mutex<Option<Outbox<StreamLoadingStatus>>>>,
    paused: bool,
}

impl Default for FrameStreamHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameStreamHandler {
    pub fn new() -> Self {
        Self {
            stream_cache: HashMap::new(),
            loading_streams: HashMap::new(),
            status_outbox: Arc::new(Mutex::new(None)),
            paused: false,
        }
    }

    /// Set the outbox for receiving stream loading status messages.
    pub fn set_status_outbox(&mut self, outbox: Outbox<StreamLoadingStatus>) {
        *self.status_outbox.lock().expect("mutex lock failed") = Some(outbox);
    }

    pub fn pause_all_streams(&mut self) {
        <Self as TimedStreamHandler>::pause_all_streams(self);
    }

    pub fn play_all_streams(&mut self) {
        <Self as TimedStreamHandler>::play_all_streams(self);
    }

    pub fn clear_cache(&mut self) {
        <Self as TimedStreamHandler>::clear_cache(self);
    }

    pub fn set_target_fps_all(&mut self, target_fps: Fps) {
        <Self as TimedStreamHandler>::set_target_fps_all(self, target_fps);
    }

    /// Apply target FPS to non-video frame streams only.
    ///
    /// Video streams keep their own internal timing/resampling behavior.
    pub fn set_target_fps_all_non_video(&mut self, target_fps: Fps) {
        for (key, stream) in self.stream_cache.iter_mut() {
            if matches!(key.stream_kind, StreamKind::Video) {
                continue;
            }
            stream.set_target_fps(target_fps);
        }
    }

    pub fn set_target_fps_for_nodes(
        &mut self,
        target_fps: Fps,
        active_nodes: &HashSet<EngineNodeId>,
    ) {
        <Self as TimedStreamHandler>::set_target_fps_for_nodes(self, target_fps, active_nodes);
    }

    /// Apply target FPS to active non-video frame streams only.
    pub fn set_target_fps_for_nodes_non_video(
        &mut self,
        target_fps: Fps,
        active_nodes: &HashSet<EngineNodeId>,
    ) {
        for (key, stream) in self.stream_cache.iter_mut() {
            if !active_nodes.contains(&key.node_id) {
                continue;
            }
            if matches!(key.stream_kind, StreamKind::Video) {
                continue;
            }
            stream.set_target_fps(target_fps);
        }
    }

    pub fn set_playback_for_nodes(&mut self, active_nodes: &HashSet<EngineNodeId>) {
        <Self as TimedStreamHandler>::set_playback_for_nodes(self, active_nodes);
    }

    pub fn get_target_fps(
        &mut self,
        request: &NodeFrameStreamRequest,
    ) -> Result<Fps, FrameStreamHandlerError> {
        let stream = self.create_stream(request)?;
        Ok(stream.target_fps())
    }

    pub fn get_recommended_fps(
        &mut self,
        request: &NodeFrameStreamRequest,
    ) -> Result<Fps, FrameStreamHandlerError> {
        let stream = self.create_stream(request)?;
        let any = stream.as_ref() as &dyn std::any::Any;

        if let Some(video_stream) = any.downcast_ref::<VideoFrameStream>() {
            return Ok(video_stream.native_fps());
        }

        Ok(stream.target_fps())
    }

    /// Single API for both image and video stream creation with explicit stream kind.
    fn create_stream(
        &mut self,
        request: &NodeFrameStreamRequest,
    ) -> Result<&mut Box<dyn FrameStream>, FrameStreamHandlerError> {
        let key = NodeFrameStreamKey {
            node_id: request.node_id,
            file_path: request.file_path.clone(),
            stream_kind: request.stream_kind,
        };

        // A node should only keep one active source stream at a time.
        // If its input path (or kind) changes, drop the stale stream entry.
        let stale_keys: Vec<NodeFrameStreamKey> = self
            .stream_cache
            .keys()
            .filter(|cached_key| cached_key.node_id == request.node_id && **cached_key != key)
            .cloned()
            .collect();
        for stale_key in stale_keys {
            self.stream_cache.remove(&stale_key);
            self.loading_streams.remove(&stale_key);
        }

        // Check if already cached
        if self.stream_cache.contains_key(&key) {
            return Ok(self
                .stream_cache
                .get_mut(&key)
                .expect("stream inserted above"));
        }

        // Check if currently loading
        if self.loading_streams.contains_key(&key) {
            let is_loaded = {
                if let Some(stream_holder) = self.loading_streams.get(&key) {
                    if let Ok(mutex) = stream_holder.lock() {
                        mutex.as_ref().is_some()
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            if is_loaded {
                // Loading completed, move stream from loading_streams to cache
                if let Some(stream_holder) = self.loading_streams.remove(&key) {
                    if let Ok(mut mutex) = stream_holder.lock() {
                        if let Some(mut stream) = mutex.take() {
                            if self.paused {
                                stream.pause();
                            } else {
                                stream.play();
                            }
                            self.stream_cache.insert(key.clone(), stream);
                            return Ok(self
                                .stream_cache
                                .get_mut(&key)
                                .expect("stream was just inserted"));
                        }
                    }
                }
            } else {
                // Still loading
                return Err(FrameStreamHandlerError::StreamStillLoading {
                    path: request.file_path.clone(),
                });
            }
        }

        // Not loading yet - spawn background thread to load it
        self.spawn_background_load(request, &key)?;

        Err(FrameStreamHandlerError::StreamStillLoading {
            path: request.file_path.clone(),
        })
    }

    /// Spawn a background thread to load a stream without blocking the main thread.
    fn spawn_background_load(
        &mut self,
        request: &NodeFrameStreamRequest,
        key: &NodeFrameStreamKey,
    ) -> Result<(), FrameStreamHandlerError> {
        let stream_holder = Arc::new(Mutex::new(None));
        let stream_holder_clone = Arc::clone(&stream_holder);
        let outbox_clone = Arc::clone(&self.status_outbox);
        let file_path = request.file_path.clone();
        let stream_kind = request.stream_kind;
        let node_id = request.node_id;

        // Send loading started message
        if let Ok(mutex) = outbox_clone.lock() {
            if let Some(ref outbox) = *mutex {
                let _ = outbox.send(StreamLoadingStatus::LoadingStarted {
                    path: file_path.clone(),
                });
            }
        }

        // Spawn background thread to load stream
        thread::spawn(move || {
            let request_clone = NodeFrameStreamRequest {
                node_id,
                file_path: file_path.clone(),
                stream_kind,
            };

            match Self::build_stream_in_background(&request_clone) {
                Ok(stream) => {
                    *stream_holder_clone.lock().expect("mutex lock failed") = Some(stream);
                    if let Ok(mutex) = outbox_clone.lock() {
                        if let Some(ref outbox) = *mutex {
                            let _ = outbox.send(StreamLoadingStatus::LoadingCompleted {
                                path: file_path,
                            });
                        }
                    }
                }
                Err(e) => {
                    if let Ok(mutex) = outbox_clone.lock() {
                        if let Some(ref outbox) = *mutex {
                            let _ = outbox.send(StreamLoadingStatus::LoadingFailed {
                                path: file_path,
                                error: format!("{:?}", e),
                            });
                        }
                    }
                }
            }
        });

        self.loading_streams.insert(key.clone(), stream_holder);
        Ok(())
    }

    /// Execute stream retrieval for a built-in source node.
    ///
    /// Uses the cached stream when available, otherwise creates it, then
    /// fetches, uploads to GPU, and returns node outputs.
    pub fn execute_handler(
        &mut self,
        request: &NodeFrameStreamRequest,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        upload_stager: &mut UploadStager,
    ) -> Result<Vec<NodeValue>, FrameStreamHandlerError> {
        let stream = self.create_stream(request)?;

        let frame = stream
            .fetch()
            .map_err(|source| FrameStreamHandlerError::FetchStream {
                path: request.file_path.clone(),
                source,
            })?;

        let width = frame.dimensions().width();
        let height = frame.dimensions().height();

        let texture_view = upload_stager
            .cpu_to_gpu_rgba(device, queue, width, height, frame.raw_data())
            .map_err(|error| FrameStreamHandlerError::TextureUpload {
                path: request.file_path.clone(),
                error: format!("{error:?}"),
            })?;

        let gpu_frame = GpuFrame::new(
            texture_view,
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            frame.uid()
        );

        stream.recycle(frame);

        Ok(vec![NodeValue::Frame(gpu_frame)])
    }

    fn build_stream_in_background(
        request: &NodeFrameStreamRequest,
    ) -> Result<Box<dyn FrameStream>, FrameStreamHandlerError> {
        match request.stream_kind {
            StreamKind::Video => {
                let mut video_request = VideoFrameStream::builder()
                    .set_loop(true)
                    .build(&request.file_path);

                // This .wait() is called in a background thread, not blocking the main thread
                let stream = video_request
                    .wait()
                    .map_err(|e| FrameStreamHandlerError::VideoStream {
                        path: request.file_path.clone(),
                        source: FrameStreamError::from(e),
                    })?
                    .map_err(|source| FrameStreamHandlerError::VideoStream {
                        path: request.file_path.clone(),
                        source,
                    })?;

                Ok(Box::new(stream))
            }
            StreamKind::Image => {
                let frame = Frame::from_img_file(&request.file_path).map_err(|source| {
                    FrameStreamHandlerError::ImageStream {
                        path: request.file_path.clone(),
                        source,
                    }
                })?;

                let frame = Self::cap_image_frame_dimensions(frame);

                Ok(Box::new(StillFrameStream::new(frame, FPS_30)))
            }
        }
    }

    fn cap_image_frame_dimensions(frame: Frame) -> Frame {
        const MAX_IMAGE_DIMENSION: u32 = 2048;

        let width = frame.dimensions().width();
        let height = frame.dimensions().height();

        if width <= MAX_IMAGE_DIMENSION && height <= MAX_IMAGE_DIMENSION {
            return frame;
        }

        let width_scale = MAX_IMAGE_DIMENSION as f32 / width as f32;
        let height_scale = MAX_IMAGE_DIMENSION as f32 / height as f32;
        let scale = width_scale.min(height_scale);

        let new_width = (width as f32 * scale).round().max(1.0) as u32;
        let new_height = (height as f32 * scale).round().max(1.0) as u32;
        let new_dimensions = media::frame::Dimensions::new(new_width, new_height)
            .expect("downscaled image dimensions should be valid");

        frame.rescale(new_dimensions, media::frame::RescaleMethod::NearestNeighbor)
    }
}

impl TimedStreamHandler for FrameStreamHandler {
    type Stream = Box<dyn FrameStream>;

    fn for_each_stream_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(EngineNodeId, &mut Self::Stream),
    {
        for (key, stream) in self.stream_cache.iter_mut() {
            f(key.node_id, stream);
        }
    }

    fn set_paused_state(&mut self, paused: bool) {
        self.paused = paused;
    }

    fn is_paused_state(&self) -> bool {
        self.paused
    }

    fn clear_stream_cache(&mut self) {
        self.stream_cache.clear();
    }

    fn stream_pause(stream: &mut Self::Stream) {
        stream.pause();
    }

    fn stream_play(stream: &mut Self::Stream) {
        stream.play();
    }

    fn stream_set_target_fps(stream: &mut Self::Stream, target_fps: Fps) {
        stream.set_target_fps(target_fps);
    }
}

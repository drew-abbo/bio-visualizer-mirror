use crate::node_graph::EngineNodeId;
use crate::{gpu_frame::GpuFrame, graph_executor::NodeValue, upload_stager::UploadStager};
use media::fps::{Fps, consts::FPS_30};
use media::frame::streams::{FrameStream, FrameStreamError, StillFrameStream, VideoFrameStream};
use media::frame::{Frame, FromImgFileError};
use std::collections::HashMap;
use std::path::PathBuf;
use util::channels::ChannelError;

#[derive(Debug, thiserror::Error)]
pub enum FrameStreamHandlerError {
    #[error("Failed waiting for video stream request for '{path}': {source}")]
    VideoRequest {
        path: PathBuf,
        #[source]
        source: ChannelError,
    },
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
    paused: bool,
}

impl FrameStreamHandler {
    pub fn new() -> Self {
        Self {
            stream_cache: HashMap::new(),
            paused: false,
        }
    }

    pub fn pause_all_streams(&mut self) {
        self.paused = true;
        for stream in self.stream_cache.values_mut() {
            stream.pause();
        }
    }

    pub fn play_all_streams(&mut self) {
        self.paused = false;
        for stream in self.stream_cache.values_mut() {
            stream.play();
        }
    }

    pub fn clear_cache(&mut self) {
        self.stream_cache.clear();
    }

    pub fn set_target_fps_all(&mut self, target_fps: Fps) {
        for stream in self.stream_cache.values_mut() {
            stream.set_target_fps(target_fps);
        }
    }

    pub fn get_target_fps(
        &mut self,
        request: &NodeFrameStreamRequest,
    ) -> Result<Fps, FrameStreamHandlerError> {
        let stream = self.create_stream(request)?;
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
        }

        if !self.stream_cache.contains_key(&key) {
            let mut stream = Self::build_stream(request)?;
            if self.paused {
                stream.pause();
            } else {
                stream.play();
            }
            self.stream_cache.insert(key.clone(), stream);
        }

        Ok(self
            .stream_cache
            .get_mut(&key)
            .expect("stream inserted above"))
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
        );

        stream.recycle(frame);

        Ok(vec![NodeValue::Frame(gpu_frame)])
    }

    fn build_stream(
        request: &NodeFrameStreamRequest,
    ) -> Result<Box<dyn FrameStream>, FrameStreamHandlerError> {
        match request.stream_kind {
            StreamKind::Video => {
                let mut video_request = VideoFrameStream::builder()
                    .set_loop(true)
                    .build(&request.file_path);

                let stream = video_request
                    .wait()
                    .map_err(|source| FrameStreamHandlerError::VideoRequest {
                        path: request.file_path.clone(),
                        source,
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

                Ok(Box::new(StillFrameStream::new(frame, FPS_30)))
            }
        }
    }
}

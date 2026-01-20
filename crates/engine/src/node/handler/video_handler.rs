use crate::graph_executor::enums::{OutputValue, ResolvedInput};
use crate::graph_executor::errors::ExecutionError;
use crate::node::handler::node_handler::NodeHandler;
use crate::upload_stager::UploadStager;
use media::frame::{Frame, Producer, streams::OnStreamEnd, streams::Video};
use std::collections::HashMap;
use std::path::PathBuf;

/// Video source with frame caching (must be kept alive between executions)
pub struct VideoSourceHandler {
    producer_cache: HashMap<PathBuf, Producer>,
}

impl VideoSourceHandler {
    pub fn new() -> Self {
        Self {
            producer_cache: HashMap::new(),
        }
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

    pub fn clear_cache(&mut self) {
        self.producer_cache.clear();
    }

    pub fn fetch_frame(&mut self, path: &PathBuf) -> Result<Frame, ExecutionError> {
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
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        Ok((fps, duration_secs))
    }
}

impl NodeHandler for VideoSourceHandler {
    fn execute(
        &self,
        _inputs: &HashMap<String, ResolvedInput>,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _upload_stager: &mut UploadStager,
    ) -> Result<HashMap<String, OutputValue>, ExecutionError> {
        // VideoSourceHandler is handled specially in execute_builtin_node
        // because it requires mutable access to the producer cache
        Err(ExecutionError::InvalidInputType)
    }
}

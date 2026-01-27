use std::collections::HashMap;
use std::path::PathBuf;

use media::frame::{Frame, Producer, streams::OnStreamEnd, streams::Video};

use crate::gpu_frame::GpuFrame;
use crate::graph_executor::{ExecutionError, OutputValue, ResolvedInput};
use crate::node::handler::node_handler::NodeHandler;
use crate::upload_stager::UploadStager;

/// Video source with frame caching (must be kept alive between executions)
pub struct VideoSourceHandler {
    producer_cache: HashMap<PathBuf, Producer>,
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
        }
    }

    pub fn clear_cache(&mut self) {
        self.producer_cache.clear();
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
        inputs: &HashMap<String, ResolvedInput>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        upload_stager: &mut UploadStager,
    ) -> Result<HashMap<String, OutputValue>, ExecutionError> {
        let path = inputs
            .get("path")
            .and_then(|v| match v {
                ResolvedInput::File(p) => Some(p),
                _ => None,
            })
            .ok_or(ExecutionError::InvalidInputType)?;

        let frame = self.fetch_frame(path)?;
        let (fps, duration_secs) = self.get_stats(path)?;

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

        // Prepare outputs
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), OutputValue::Frame(gpu_frame));
        outputs.insert("fps".to_string(), OutputValue::Float(fps));
        outputs.insert(
            "duration".to_string(),
            OutputValue::Float(duration_secs as f32),
        );

        Ok(outputs)
    }
}

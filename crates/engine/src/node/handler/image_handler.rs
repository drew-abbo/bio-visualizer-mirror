use std::collections::HashMap;
use std::path::PathBuf;

use crate::gpu_frame::GpuFrame;
use crate::graph_executor::{ExecutionError, OutputValue, ResolvedInput};
use crate::node::handler::node_handler::NodeHandler;
use crate::upload_stager::UploadStager;

/// Loads an image file to GPU texture with caching
pub struct ImageSourceHandler {
    pub frame_cache: HashMap<PathBuf, GpuFrame>,
}

impl ImageSourceHandler {
    pub fn new() -> Self {
        Self {
            frame_cache: HashMap::new(),
        }
    }

    pub fn clear_cache(&mut self) {
        self.frame_cache.clear();
    }
}

impl Default for ImageSourceHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeHandler for ImageSourceHandler {
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

        if !self.frame_cache.contains_key(path) {
            // Load image from disk (CPU)
            let frame = media::frame::Frame::from_img_file(path).map_err(|e| {
                ExecutionError::TextureUploadError(format!("Failed to load image: {:?}", e))
            })?;

            let width = frame.dimensions().width();
            let height = frame.dimensions().height();

            // Upload to GPU on this thread
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

            self.frame_cache.insert(path.clone(), gpu_frame);
        }

        let gpu_frame = self.frame_cache.get(path).unwrap().clone();
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), OutputValue::Frame(gpu_frame));
        Ok(outputs)
    }
}

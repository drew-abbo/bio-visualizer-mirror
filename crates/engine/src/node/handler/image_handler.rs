use crate::graph_executor::enums::{OutputValue, ResolvedInput};
use crate::graph_executor::errors::ExecutionError;
use crate::graph_executor::gpu_frame::GpuFrame;
use crate::node::handler::node_handler::NodeHandler;
use crate::upload_stager::UploadStager;
use std::collections::HashMap;
use std::path::PathBuf;

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

impl NodeHandler for ImageSourceHandler {
    fn execute(
        &self,
        inputs: &HashMap<String, ResolvedInput>,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _upload_stager: &mut UploadStager,
    ) -> Result<HashMap<String, OutputValue>, ExecutionError> {
        let _path = inputs
            .get("path")
            .and_then(|v| match v {
                ResolvedInput::File(p) => Some(p),
                _ => None,
            })
            .ok_or(ExecutionError::InvalidInputType)?;

        // ImageSourceHandler is handled specially in execute_builtin_node
        // because it requires mutable access to the frame cache
        Err(ExecutionError::InvalidInputType)
    }
}

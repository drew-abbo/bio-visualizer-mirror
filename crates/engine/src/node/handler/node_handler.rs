use std::collections::HashMap;

use crate::graph_executor::{ExecutionError, NodeValue};
use crate::upload_stager::UploadStager;

pub trait NodeHandler {
    fn execute(
        &mut self,
        inputs: &HashMap<String, NodeValue>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        upload_stager: &mut UploadStager,
    ) -> Result<HashMap<String, NodeValue>, ExecutionError>;
}

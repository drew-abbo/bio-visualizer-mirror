use std::collections::HashMap;

use crate::graph_executor::{ExecutionError, NodeValue};
use crate::upload_stager::UploadStager;

pub trait NodeHandler {
    /// Execute the node handler and return outputs in the order defined by the node definition.
    /// The executor will map these to the actual output names.
    fn execute(
        &mut self,
        inputs: &HashMap<String, NodeValue>,
        inputs: &HashMap<String, NodeValue>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        upload_stager: &mut UploadStager,
    ) -> Result<HashMap<String, NodeValue>, ExecutionError>;
}

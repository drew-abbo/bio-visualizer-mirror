use std::collections::HashMap;

use crate::graph_executor::{ExecutionContext, ExecutionError, NodeValue};
use crate::node_graph::EngineNodeId;
use crate::upload_stager::UploadStager;

pub trait NodeHandler {
    /// Execute the node handler and return outputs in the order defined by the node definition.
    /// The executor will map these to the actual output names.
    fn execute(
        &mut self,
        node_id: EngineNodeId,
        inputs: &HashMap<String, NodeValue>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        upload_stager: &mut UploadStager,
        context: &ExecutionContext,
    ) -> Result<Vec<NodeValue>, ExecutionError>;
}

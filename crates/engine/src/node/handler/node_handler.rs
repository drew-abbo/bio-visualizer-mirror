use crate::graph_executor::ExecutionError;
use crate::graph_executor::{OutputValue, ResolvedInput};
use crate::upload_stager::UploadStager;
use std::collections::HashMap;

pub trait NodeHandler {
    fn execute(
        &mut self,
        inputs: &HashMap<String, ResolvedInput>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        upload_stager: &mut UploadStager,
    ) -> Result<HashMap<String, OutputValue>, ExecutionError>;
}

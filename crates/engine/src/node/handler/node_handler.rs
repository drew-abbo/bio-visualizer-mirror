use crate::graph_executor::enums::{OutputValue, ResolvedInput};
use crate::graph_executor::errors::ExecutionError;
use crate::upload_stager::UploadStager;
use std::collections::HashMap;

pub trait NodeHandler {
    fn execute(
        &self,
        inputs: &HashMap<String, ResolvedInput>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        upload_stager: &mut UploadStager,
    ) -> Result<HashMap<String, OutputValue>, ExecutionError>;
}

use std::collections::HashMap;

use crate::graph_executor::{ExecutionError, OutputValue, ResolvedInput};
use crate::upload_stager::UploadStager;

pub trait NodeHandler {
    fn execute(
        &mut self,
        inputs: &HashMap<String, ResolvedInput>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        upload_stager: &mut UploadStager,
    ) -> Result<HashMap<String, OutputValue>, ExecutionError>;
}

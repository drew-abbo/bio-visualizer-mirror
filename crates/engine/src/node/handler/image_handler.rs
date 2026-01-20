use crate::graph_executor::enums::{OutputValue, ResolvedInput};
use crate::graph_executor::errors::ExecutionError;
use crate::node::handler::node_handler::NodeHandler;
use crate::upload_stager::UploadStager;
use media::frame::Frame;
use std::collections::HashMap;

/// Loads an image file to GPU texture
pub struct ImageSourceHandler;

impl NodeHandler for ImageSourceHandler {
    fn execute(
        &self,
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

        let frame = Frame::from_img_file(path).unwrap(); // TODO: proper error handling

        let width = frame.dimensions().width();
        let height = frame.dimensions().height();

        let texture_view = upload_stager
            .cpu_to_gpu_rgba(device, queue, width, height, frame.raw_data())
            .unwrap(); // TODO: proper error handling

        let gpu_frame = crate::graph_executor::enums::GpuFrame::new(
            texture_view,
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), OutputValue::Frame(gpu_frame));
        Ok(outputs)
    }
}

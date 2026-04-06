use crate::node::engine_node::NodeInputKind;

pub fn create_linear_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("sampler/linear"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    })
}

pub const fn align_to(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment > 0);
    (value + (alignment - 1)) & !(alignment - 1)
}

pub const fn uniform_param_size(kind: &NodeInputKind) -> usize {
    match kind {
        NodeInputKind::Bool { .. } => 4,
        NodeInputKind::Int { .. } => 4,
        NodeInputKind::Float { .. } => 4,
        NodeInputKind::Dimensions { .. } => 8,
        NodeInputKind::Pixel { .. } => 16,
        NodeInputKind::Enum { .. } => 4,
        NodeInputKind::Text { .. } => 0,
        NodeInputKind::File { .. } => 0,
        NodeInputKind::Midi => 0,
        NodeInputKind::Frame => 0,
    }
}

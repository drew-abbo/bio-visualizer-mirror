struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((vid << 1u) & 2u);
    let y = f32(vid & 2u);
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

struct Params {
    strength: f32,
}

@group(0) @binding(0) var input_sampler: sampler;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> params: Params;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate offset based on strength (convert to screen-space)
    let offset = params.strength * 0.005;
    
    // Sample each channel at different positions
    // Red shifts right, Blue shifts left, Green stays centered
    let r = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(offset, 0.0)).r;
    let g = textureSample(input_texture, input_sampler, in.uv).g;
    let b = textureSample(input_texture, input_sampler, in.uv - vec2<f32>(offset, 0.0)).b;
    
    return vec4<f32>(r, g, b, 1.0);
}
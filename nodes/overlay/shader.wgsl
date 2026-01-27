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
    opacity: f32,
    _pad: vec3<f32>,
}

@group(0) @binding(0) var input_sampler: sampler;
@group(0) @binding(1) var background_texture: texture_2d<f32>;
@group(0) @binding(2) var foreground_texture: texture_2d<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let bg = textureSample(background_texture, input_sampler, in.uv);
    let fg = textureSample(foreground_texture, input_sampler, in.uv);
    return mix(bg, fg, params.opacity);
}

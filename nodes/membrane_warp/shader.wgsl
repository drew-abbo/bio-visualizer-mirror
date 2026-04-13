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
    phase: f32,
    amplitude: f32,
    frequency: f32,
    drift: f32,
}

@group(0) @binding(0) var input_sampler: sampler;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> params: Params;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let amp = max(params.amplitude, 0.0);
    let freq = max(params.frequency, 0.001);
    let phase = params.phase * max(params.drift, 0.0) * 6.2831853;

    let uv = in.uv;
    let uv_sum = uv.x + uv.y;
    let wave_a = sin((uv.y * freq) + phase);
    let wave_b = cos((uv.x * (freq * 0.82)) - phase * 0.77);
    let wave_c = sin((uv_sum * (freq * 0.61)) + phase * 1.19);

    let dx = (wave_a * 0.6 + wave_c * 0.4) * amp;
    let dy = (wave_b * 0.6 - wave_c * 0.4) * amp;

    let warped_uv = clamp(uv + vec2<f32>(dx, dy), vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(input_texture, input_sampler, warped_uv);
}

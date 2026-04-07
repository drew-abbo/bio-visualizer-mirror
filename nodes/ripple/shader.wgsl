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
    center_x: f32,
    center_y: f32,
    amplitude: f32,
    frequency: f32,
    decay: f32,
    phase: f32,
    midi_strength: f32,
}

@group(0) @binding(0) var input_sampler: sampler;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> params: Params;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tau = 6.2831853;
    let center = vec2<f32>(params.center_x, params.center_y);
    let delta = in.uv - center;
    let radius = length(delta);

    // Avoid unstable normalization at the center pixel.
    let dir = select(vec2<f32>(0.0, 0.0), delta / radius, radius > 0.0001);

    // Scale phase so MIDI values in 0..1 still produce visible motion.
    let phase = params.phase * tau * 2.0;

    // Layer two radial waves for a richer water feel across the whole frame.
    let primary = sin(radius * params.frequency * tau - phase);
    let secondary = sin(radius * (params.frequency * 0.55) * tau + phase * 1.3);
    let wave = primary * 0.72 + secondary * 0.28;

    // Keep decay gentle so ripples span the full image instead of only near center.
    let envelope = exp(-params.decay * radius * 0.35);

    // Preserve a base effect and let MIDI push intensity further.
    let intensity = 0.35 + params.midi_strength * 0.65;
    let displacement = params.amplitude * intensity * wave * envelope;

    let sample_uv = clamp(in.uv + (dir * displacement), vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(input_texture, input_sampler, sample_uv);
}

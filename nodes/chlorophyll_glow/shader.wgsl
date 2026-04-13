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
    pulse: f32,
    glow_strength: f32,
    green_bias: f32,
    vein_contrast: f32,
}

@group(0) @binding(0) var input_sampler: sampler;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> params: Params;

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let base = textureSample(input_texture, input_sampler, in.uv);

    let pulse = clamp(params.pulse, 0.0, 1.0);
    let green_bias = clamp(params.green_bias, 0.0, 1.0);
    let glow_strength = max(params.glow_strength, 0.0);
    let vein_contrast = max(params.vein_contrast, 0.0);

    let uv = in.uv;
    let chlorophyll = max(base.g - max(base.r, base.b), 0.0);

    let vein_phase = uv.x * 26.0 + uv.y * 18.0 + pulse * 6.2831853 * 1.6;
    let vein_wave = sin(vein_phase) * 0.5 + 0.5;
    let veins = smoothstep(0.25, 0.85, vein_wave);

    let organic_glow = pow(chlorophyll + 1e-4, 0.6) * glow_strength;
    let pulse_boost = 0.25 * pulse;
    let tint = vec3<f32>(0.18, 0.95, 0.38);

    let lit = base.rgb + tint * (organic_glow + pulse_boost) * (0.55 + 0.45 * veins);

    let local_luma = luma(lit);
    let contrast_centered = (lit - vec3<f32>(local_luma)) * (1.0 + (vein_contrast * 0.35) * veins);
    let contrasted = vec3<f32>(local_luma) + contrast_centered;

    let biased = mix(contrasted, contrasted * vec3<f32>(0.95, 1.0 + green_bias * 0.2, 0.97), green_bias);

    return vec4<f32>(clamp(biased, vec3<f32>(0.0), vec3<f32>(1.0)), base.a);
}

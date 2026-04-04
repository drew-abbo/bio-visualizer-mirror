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
    threshold: f32,
    strength: f32,
    scan_step: f32,
    direction: u32,
    sort_by: u32,
}

@group(0) @binding(0) var input_sampler: sampler;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var metric_texture: texture_2d<f32>;
@group(0) @binding(3) var<uniform> params: Params;

fn scan_axis(direction: u32) -> vec2<f32> {
    if (direction == 0u) {
        return vec2<f32>(1.0, 0.0);
    }
    return vec2<f32>(0.0, 1.0);
}

fn packed_metric(sample: vec4<f32>) -> f32 {
    // mask pass packs: r=hue, g=saturation, b=luminance
    if (params.sort_by == 1u) {
        return sample.r;
    }
    if (params.sort_by == 2u) {
        return sample.g;
    }
    return sample.b;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let axis = scan_axis(params.direction);
    let source = textureSample(input_texture, input_sampler, in.uv);
    let source_metric = packed_metric(textureSample(metric_texture, input_sampler, in.uv));

    // Dark areas stay mostly untouched; bright areas get shifted toward the
    // previous threshold boundary to mimic a pixel-sort segment collapse.
    if (source_metric < params.threshold) {
        return source;
    }

    var segment_len = 0.0;
    var probe_uv = in.uv;

    // Fixed loop bounds keep the shader predictable for the pipeline.
    for (var i: i32 = 0; i < 128; i = i + 1) {
        probe_uv = probe_uv - axis * params.scan_step;
        if (any(probe_uv < vec2<f32>(0.0)) || any(probe_uv > vec2<f32>(1.0))) {
            break;
        }
        let probe_metric = packed_metric(textureSample(metric_texture, input_sampler, probe_uv));
        if (probe_metric < params.threshold) {
            break;
        }
        segment_len = segment_len + 1.0;
    }

    let shift = segment_len * params.scan_step * params.strength;
    let sample_uv = clamp(in.uv - axis * shift, vec2<f32>(0.0), vec2<f32>(1.0));
    let sorted = textureSample(input_texture, input_sampler, sample_uv);

    return vec4<f32>(sorted.rgb, source.a);
}

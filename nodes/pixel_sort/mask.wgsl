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
@group(0) @binding(2) var<uniform> params: Params;

fn luminance(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn hsv_metric(rgb: vec3<f32>) -> vec3<f32> {
    let max_c = max(rgb.r, max(rgb.g, rgb.b));
    let min_c = min(rgb.r, min(rgb.g, rgb.b));
    let delta = max_c - min_c;

    var hue = 0.0;
    if (delta > 0.00001) {
        if (max_c == rgb.r) {
            hue = (rgb.g - rgb.b) / delta;
            if (rgb.g < rgb.b) {
                hue = hue + 6.0;
            }
        } else if (max_c == rgb.g) {
            hue = ((rgb.b - rgb.r) / delta) + 2.0;
        } else {
            hue = ((rgb.r - rgb.g) / delta) + 4.0;
        }
        hue = hue / 6.0;
    }

    var saturation = 0.0;
    if (max_c > 0.0) {
        saturation = delta / max_c;
    }
    return vec3<f32>(hue, saturation, max_c);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let source = textureSample(input_texture, input_sampler, in.uv);
    let hsv = hsv_metric(source.rgb);
    let lum = luminance(source.rgb);

    // Pack reusable metrics so sort pass can choose Brightness/Hue/Saturation
    // without requiring a dedicated metric bake path.
    // r = hue, g = saturation, b = luminance
    return vec4<f32>(hsv.x, hsv.y, lum, source.a);
}

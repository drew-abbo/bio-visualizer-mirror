// Single-pass pixel sort compute shader
// Adapted from the working render shader to run as compute

struct Params {
    threshold: f32,
    strength: f32,
    scan_step: f32,       // Reserved for future stride control; currently unused (scans pixel-by-pixel)
    scan_direction: u32,  // 0 = horizontal, 1 = vertical
    metric_type: u32,     // 0 = brightness, 1 = hue, 2 = saturation
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> params: Params;

fn brightness(color: vec4<f32>) -> f32 {
    return dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));
}

fn hue(color: vec4<f32>) -> f32 {
    let r = color.r;
    let g = color.g;
    let b = color.b;
    let max_c = max(max(r, g), b);
    let min_c = min(min(r, g), b);
    let delta = max_c - min_c;
    
    var h = 0.0;
    if delta > 0.0 {
        if max_c == r {
            h = (g - b) / delta;
        } else if max_c == g {
            h = (b - r) / delta + 2.0;
        } else {
            h = (r - g) / delta + 4.0;
        }
        h = h / 6.0;
    }
    return fract(h);
}

fn saturation(color: vec4<f32>) -> f32 {
    let max_c = max(max(color.r, color.g), color.b);
    let min_c = min(min(color.r, color.g), color.b);
    if max_c < 0.00001 { return 0.0; }
    return (max_c - min_c) / max_c;
}

fn get_metric(color: vec4<f32>) -> f32 {
    if params.metric_type == 1u {
        return hue(color);
    } else if params.metric_type == 2u {
        return saturation(color);
    }
    return brightness(color);
}

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let tex_dims = textureDimensions(input_texture);
    let pos = vec2<i32>(global_id.xy);
    
    // global_id is u32, so only check upper bounds
    if pos.x >= i32(tex_dims.x) || pos.y >= i32(tex_dims.y) {
        return;
    }
    
    let source = textureLoad(input_texture, vec2<u32>(pos), 0);
    let source_metric = get_metric(source);
    
    // Threshold check: if pixel is below threshold, output unchanged
    if source_metric < params.threshold {
        textureStore(output_texture, vec2<u32>(pos), source);
        return;
    }
    
    // For bright areas: scan backwards along the direction and measure segment length
    let scan_axis_i = select(vec2<i32>(1, 0), vec2<i32>(0, 1), params.scan_direction == 1u);
    var probe_pos = pos;
    var segment_len = 0.0;
    
    // Scan backwards to find segment boundary (working in integer pixel coordinates)
    for (var i: i32 = 0; i < 128; i = i + 1) {
        probe_pos = probe_pos - scan_axis_i;
        if any(probe_pos < vec2<i32>(0)) || any(probe_pos >= vec2<i32>(tex_dims)) {
            break;
        }
        
        let probe_color = textureLoad(input_texture, vec2<u32>(probe_pos), 0);
        let probe_metric = get_metric(probe_color);
        
        if probe_metric < params.threshold {
            break;
        }
        segment_len = segment_len + 1.0;
    }
    
    // Shift pixel position towards the segment boundary
    // Note: shift is unclamped against segment_len, so strength > 1.0 can sample outside the measured segment.
    // This allows for more aggressive effects and is intentional.
    let shift_distance = segment_len * params.strength;
    let shift_pixels = i32(round(shift_distance));
    let sample_pos = pos - scan_axis_i * shift_pixels;
    let sample_pos_clamped = clamp(sample_pos, vec2<i32>(0), vec2<i32>(tex_dims) - vec2<i32>(1));
    let sorted = textureLoad(input_texture, vec2<u32>(sample_pos_clamped), 0);
    
    textureStore(output_texture, vec2<u32>(pos), vec4<f32>(sorted.rgb, source.a));
}

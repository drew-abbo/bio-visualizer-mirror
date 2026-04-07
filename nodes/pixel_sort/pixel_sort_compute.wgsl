// Pass 1: label contiguous thresholded segments for pixel sort.
// Stores the start/end coordinate along the scan axis in a float label texture.

struct Params {
    threshold: f32,
    strength: f32,
    pixel_stride: f32,
    scan_direction: u32,  // 0 = horizontal, 1 = vertical
    metric_type: u32,      // 0 = brightness, 1 = hue, 2 = saturation
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba16float, write>;
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
    if max_c < 0.00001 {
        return 0.0;
    }
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

const EMPTY_LABEL: vec4<f32> = vec4<f32>(-1.0, -1.0, 0.0, 0.0);

@compute @workgroup_size(1, 1, 1)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let tex_dims = textureDimensions(input_texture);
    let pos = vec2<i32>(global_id.xy);

    if pos.x >= i32(tex_dims.x) || pos.y >= i32(tex_dims.y) {
        return;
    }

    if params.scan_direction == 0u {
        // Horizontal sort: one invocation handles a full row.
        // With dispatch_override (1, height, 1), each thread at (0, y, 0) processes row y.
        let line_y = pos.y;
        if line_y >= i32(tex_dims.y) {
            return;
        }

        var x = 0;
        loop {
            if x >= i32(tex_dims.x) {
                break;
            }

            let current_color = textureLoad(input_texture, vec2<u32>(u32(x), u32(line_y)), 0);
            if get_metric(current_color) < params.threshold {
                textureStore(output_texture, vec2<u32>(u32(x), u32(line_y)), EMPTY_LABEL);
                x = x + 1;
                continue;
            }

            var segment_start = x;
            var segment_end = x;
            let max_scan = i32(tex_dims.x);
            var back_steps = 0;

            loop {
                if back_steps >= max_scan || segment_start == 0 {
                    break;
                }

                let candidate = segment_start - 1;
                let candidate_color = textureLoad(input_texture, vec2<u32>(u32(candidate), u32(line_y)), 0);
                if get_metric(candidate_color) < params.threshold {
                    break;
                }

                segment_start = candidate;
                back_steps = back_steps + 1;
            }

            var forward_steps = 0;

            loop {
                if forward_steps >= max_scan || segment_end + 1 >= i32(tex_dims.x) {
                    break;
                }

                let candidate = segment_end + 1;
                let candidate_color = textureLoad(input_texture, vec2<u32>(u32(candidate), u32(line_y)), 0);
                if get_metric(candidate_color) < params.threshold {
                    break;
                }

                segment_end = candidate;
                forward_steps = forward_steps + 1;
            }

            let label = vec4<f32>(f32(segment_start), f32(segment_end), 0.0, 0.0);
            var write_x = segment_start;
            loop {
                if write_x > segment_end {
                    break;
                }

                textureStore(output_texture, vec2<u32>(u32(write_x), u32(line_y)), label);
                write_x = write_x + 1;
            }

            x = segment_end + 1;
        }
    } else {
        // Vertical sort: one invocation handles a full column.
        // With dispatch_override (width, 1, 1), each thread at (x, 0, 0) processes column x.
        let line_x = pos.x;
        if line_x >= i32(tex_dims.x) {
            return;
        }

        var y = 0;
        loop {
            if y >= i32(tex_dims.y) {
                break;
            }

            let current_color = textureLoad(input_texture, vec2<u32>(u32(line_x), u32(y)), 0);
            if get_metric(current_color) < params.threshold {
                textureStore(output_texture, vec2<u32>(u32(line_x), u32(y)), EMPTY_LABEL);
                y = y + 1;
                continue;
            }

            var segment_start = y;
            var segment_end = y;
            let max_scan = i32(tex_dims.y);
            var back_steps = 0;

            loop {
                if back_steps >= max_scan || segment_start == 0 {
                    break;
                }

                let candidate = segment_start - 1;
                let candidate_color = textureLoad(input_texture, vec2<u32>(u32(line_x), u32(candidate)), 0);
                if get_metric(candidate_color) < params.threshold {
                    break;
                }

                segment_start = candidate;
                back_steps = back_steps + 1;
            }

            var forward_steps = 0;

            loop {
                if forward_steps >= max_scan || segment_end + 1 >= i32(tex_dims.y) {
                    break;
                }

                let candidate = segment_end + 1;
                let candidate_color = textureLoad(input_texture, vec2<u32>(u32(line_x), u32(candidate)), 0);
                if get_metric(candidate_color) < params.threshold {
                    break;
                }

                segment_end = candidate;
                forward_steps = forward_steps + 1;
            }

            let label = vec4<f32>(f32(segment_start), f32(segment_end), 0.0, 0.0);
            var write_y = segment_start;
            loop {
                if write_y > segment_end {
                    break;
                }

                textureStore(output_texture, vec2<u32>(u32(line_x), u32(write_y)), label);
                write_y = write_y + 1;
            }

            y = segment_end + 1;
        }
    }
}

// Pass 2: resolve labeled segments by sorting samples within each segment.

struct Params {
    threshold: f32,
    strength: f32,
    pixel_stride: f32,
    scan_direction: u32,  // 0 = horizontal, 1 = vertical
    metric_type: u32,      // 0 = brightness, 1 = hue, 2 = saturation
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var segment_labels: texture_2d<f32>;
@group(0) @binding(2) var output_texture: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<uniform> params: Params;

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

const MAX_SORT_SAMPLES: u32 = 64u;

@compute @workgroup_size(1, 1, 1)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let tex_dims = textureDimensions(input_texture);
    let pos = vec2<i32>(global_id.xy);

    if pos.x >= i32(tex_dims.x) || pos.y >= i32(tex_dims.y) {
        return;
    }

    let mix_amount = clamp(params.strength, 0.0, 1.0);
    let sample_stride = max(1, i32(round(params.pixel_stride)));

    if params.scan_direction == 0u {
        // Horizontal sort: one invocation handles a full row.
        // Any invocation with x != 0 exits so each row is processed exactly once.
        if pos.x != 0 {
            return;
        }
        let line_y = pos.y;
        if line_y >= i32(tex_dims.y) {
            return;
        }

        if params.threshold <= 0.0 {
            var copy_x = 0;
            loop {
                if copy_x >= i32(tex_dims.x) {
                    break;
                }
                let source = textureLoad(input_texture, vec2<u32>(u32(copy_x), u32(line_y)), 0);
                textureStore(output_texture, vec2<u32>(u32(copy_x), u32(line_y)), source);
                copy_x = copy_x + 1;
            }
            return;
        }

        var x = 0;
        loop {
            if x >= i32(tex_dims.x) {
                break;
            }

            let label = textureLoad(segment_labels, vec2<u32>(u32(x), u32(line_y)), 0);
            if label.x < 0.0 {
                let source = textureLoad(input_texture, vec2<u32>(u32(x), u32(line_y)), 0);
                textureStore(output_texture, vec2<u32>(u32(x), u32(line_y)), source);
                x = x + 1;
                continue;
            }

            let segment_start = i32(label.x + 0.5);
            let segment_end = i32(label.y + 0.5);

            var sample_metrics: array<f32, MAX_SORT_SAMPLES>;
            var sample_colors: array<vec4<f32>, MAX_SORT_SAMPLES>;
            var sample_count: u32 = 0u;
            var sample_x = segment_start;

            loop {
                if sample_x > segment_end || sample_count >= MAX_SORT_SAMPLES {
                    break;
                }

                let sample_color = textureLoad(input_texture, vec2<u32>(u32(sample_x), u32(line_y)), 0);
                sample_metrics[sample_count] = get_metric(sample_color);
                sample_colors[sample_count] = sample_color;
                sample_count = sample_count + 1u;
                sample_x = sample_x + sample_stride;
            }

            if sample_count == 0u {
                let source = textureLoad(input_texture, vec2<u32>(u32(x), u32(line_y)), 0);
                textureStore(output_texture, vec2<u32>(u32(x), u32(line_y)), source);
                x = x + 1;
                continue;
            }

            var i: u32 = 1u;
            loop {
                if i >= sample_count {
                    break;
                }

                let key_metric = sample_metrics[i];
                let key_color = sample_colors[i];
                var j: i32 = i32(i) - 1;

                loop {
                    if j < 0 {
                        break;
                    }

                    let j_index = u32(j);
                    if sample_metrics[j_index] <= key_metric {
                        break;
                    }

                    sample_metrics[j_index + 1u] = sample_metrics[j_index];
                    sample_colors[j_index + 1u] = sample_colors[j_index];
                    j = j - 1;
                }

                let insert_index = u32(j + 1);
                sample_metrics[insert_index] = key_metric;
                sample_colors[insert_index] = key_color;
                i = i + 1u;
            }

            var write_x = segment_start;
            loop {
                if write_x > segment_end {
                    break;
                }

                let sample_index = clamp((write_x - segment_start) / sample_stride, 0, i32(sample_count) - 1);
                let sorted = sample_colors[u32(sample_index)];
                let original = textureLoad(input_texture, vec2<u32>(u32(write_x), u32(line_y)), 0);
                textureStore(output_texture, vec2<u32>(u32(write_x), u32(line_y)), mix(original, vec4<f32>(sorted.rgb, original.a), mix_amount));
                write_x = write_x + 1;
            }

            x = segment_end + 1;
        }
    } else {
        // Vertical sort: one invocation handles a full column.
        // Any invocation with y != 0 exits so each column is processed exactly once.
        if pos.y != 0 {
            return;
        }
        let line_x = pos.x;
        if line_x >= i32(tex_dims.x) {
            return;
        }

        if params.threshold <= 0.0 {
            var copy_y = 0;
            loop {
                if copy_y >= i32(tex_dims.y) {
                    break;
                }
                let source = textureLoad(input_texture, vec2<u32>(u32(line_x), u32(copy_y)), 0);
                textureStore(output_texture, vec2<u32>(u32(line_x), u32(copy_y)), source);
                copy_y = copy_y + 1;
            }
            return;
        }

        var y = 0;
        loop {
            if y >= i32(tex_dims.y) {
                break;
            }

            let label = textureLoad(segment_labels, vec2<u32>(u32(line_x), u32(y)), 0);
            if label.x < 0.0 {
                let source = textureLoad(input_texture, vec2<u32>(u32(line_x), u32(y)), 0);
                textureStore(output_texture, vec2<u32>(u32(line_x), u32(y)), source);
                y = y + 1;
                continue;
            }

            let segment_start = i32(label.x + 0.5);
            let segment_end = i32(label.y + 0.5);

            var sample_metrics: array<f32, MAX_SORT_SAMPLES>;
            var sample_colors: array<vec4<f32>, MAX_SORT_SAMPLES>;
            var sample_count: u32 = 0u;
            var sample_y = segment_start;

            loop {
                if sample_y > segment_end || sample_count >= MAX_SORT_SAMPLES {
                    break;
                }

                let sample_color = textureLoad(input_texture, vec2<u32>(u32(line_x), u32(sample_y)), 0);
                sample_metrics[sample_count] = get_metric(sample_color);
                sample_colors[sample_count] = sample_color;
                sample_count = sample_count + 1u;
                sample_y = sample_y + sample_stride;
            }

            if sample_count == 0u {
                let source = textureLoad(input_texture, vec2<u32>(u32(line_x), u32(y)), 0);
                textureStore(output_texture, vec2<u32>(u32(line_x), u32(y)), source);
                y = y + 1;
                continue;
            }

            var i: u32 = 1u;
            loop {
                if i >= sample_count {
                    break;
                }

                let key_metric = sample_metrics[i];
                let key_color = sample_colors[i];
                var j: i32 = i32(i) - 1;

                loop {
                    if j < 0 {
                        break;
                    }

                    let j_index = u32(j);
                    if sample_metrics[j_index] <= key_metric {
                        break;
                    }

                    sample_metrics[j_index + 1u] = sample_metrics[j_index];
                    sample_colors[j_index + 1u] = sample_colors[j_index];
                    j = j - 1;
                }

                let insert_index = u32(j + 1);
                sample_metrics[insert_index] = key_metric;
                sample_colors[insert_index] = key_color;
                i = i + 1u;
            }

            var write_y = segment_start;
            loop {
                if write_y > segment_end {
                    break;
                }

                let sample_index = clamp((write_y - segment_start) / sample_stride, 0, i32(sample_count) - 1);
                let sorted = sample_colors[u32(sample_index)];
                let original = textureLoad(input_texture, vec2<u32>(u32(line_x), u32(write_y)), 0);
                textureStore(output_texture, vec2<u32>(u32(line_x), u32(write_y)), mix(original, vec4<f32>(sorted.rgb, original.a), mix_amount));
                write_y = write_y + 1;
            }

            y = segment_end + 1;
        }
    }
}

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
    border_mode: u32,  // 0=clamp, 1=wrap, 2=transparent
    angle: f32,
    center_x: f32,
    center_y: f32,
}

@group(0) @binding(0) var input_sampler: sampler;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> params: Params;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = vec2<f32>(params.center_x, params.center_y);
    let uv_centered = in.uv - center;
    
    let cos_angle = cos(params.angle);
    let sin_angle = sin(params.angle);
    
    let rotated_uv = vec2<f32>(
        uv_centered.x * cos_angle - uv_centered.y * sin_angle,
        uv_centered.x * sin_angle + uv_centered.y * cos_angle
    );
    
    let final_uv = rotated_uv + center;
    
    // Handle different border modes
    var sample_uv = final_uv;
    
    if (params.border_mode == 0u) {
        // Clamp (stretches)
        sample_uv = clamp(final_uv, vec2<f32>(0.0), vec2<f32>(1.0));
    } else if (params.border_mode == 1u) {
        // Wrap (tiles)
        sample_uv = fract(final_uv);
    } else {
        // Transparent (shows black for out of bounds)
        if (final_uv.x < 0.0 || final_uv.x > 1.0 || final_uv.y < 0.0 || final_uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }
    
    return textureSample(input_texture, input_sampler, sample_uv);
}
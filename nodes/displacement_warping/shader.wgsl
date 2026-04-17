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
    intensity: f32,
    direction_x: f32,
    direction_y: f32,
    use_red_channel: u32,
    use_green_channel: u32,
    center_neutral: u32,
    _pad: vec2<f32>,
}

@group(0) @binding(0) var input_sampler: sampler;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var displacement_map: texture_2d<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the displacement map
    let disp = textureSample(displacement_map, input_sampler, in.uv);
    
    // Extract displacement values
    var offset_x = disp.r;
    var offset_y = disp.g;
    
    // If center_neutral is enabled, remap from [0,1] to [-0.5, 0.5]
    if (params.center_neutral > 0u) {
        offset_x = offset_x - 0.5;
        offset_y = offset_y - 0.5;
    }
    
    // Apply channel selection
    offset_x = offset_x * f32(params.use_red_channel);
    offset_y = offset_y * f32(params.use_green_channel);
    
    // Apply direction multipliers and intensity
    offset_x = offset_x * params.direction_x * params.intensity;
    offset_y = offset_y * params.direction_y * params.intensity;
    
    // Calculate displaced UV coordinates
    let displaced_uv = in.uv + vec2<f32>(offset_x, offset_y);
    
    // Sample the input texture at the displaced position
    // Clamp to edges to avoid sampling outside texture bounds
    let clamped_uv = clamp(displaced_uv, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(input_texture, input_sampler, clamped_uv);
}
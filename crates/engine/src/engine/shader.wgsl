// struct VertexOutput {
//     @builtin(position) clip_position: vec4<f32>,
//     @location(0) vert_pos: vec3<f32>,
// }

// @vertex
// fn vs_main(
//     @builtin(vertex_index) in_vertex_index: u32,
// ) -> VertexOutput {
//     var out: VertexOutput;
//     let x = f32(1 - i32(in_vertex_index)) * 0.5;
//     let y = f32(i32(in_vertex_index & 1u) * 2 - 1) * 0.5;
//     out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
//     out.vert_pos = out.clip_position.xyz;
//     return out;
// }
 

// @fragment
// fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
//     return vec4<f32>(0.3, 0.2, 0.1, 1.0);
// }


struct VSOut {
  @builtin(position) pos : vec4<f32>,
  @location(0) uv : vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VSOut {
  // Fullscreen triangle (no vertex buffers)
  var pos = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -3.0),
    vec2<f32>( 3.0,  1.0),
    vec2<f32>(-1.0,  1.0),
  );
  var uv  = array<vec2<f32>, 3>(
    vec2<f32>(0.0, 2.0),
    vec2<f32>(2.0, 0.0),
    vec2<f32>(0.0, 0.0),
  );

  var out: VSOut;
  out.pos = vec4<f32>(pos[vid], 0.0, 1.0);
  out.uv  = uv[vid];
  return out;
}

@group(0) @binding(0) var tex : texture_2d<f32>;
@group(0) @binding(1) var smp : sampler;

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  // Just sample; no color-space correction for now
  return textureSample(tex, smp, in.uv);
}

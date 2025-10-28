struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> VsOut {
  // Fullscreen triangle
  var pos = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -3.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>( 3.0,  1.0),
  );
  let p = pos[vi];
  var out: VsOut;
  out.pos = vec4<f32>(p, 0.0, 1.0);
  // Map NDC [-1,1] to UV [0,1]
  out.uv = 0.5 * (p + vec2<f32>(1.0, 1.0));
  return out;
}

@group(0) @binding(0) var tex0: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_blit(in: VsOut) -> @location(0) vec4<f32> {
  // For UNORM source texture; if your source is SRGB, adjust formats accordingly.
  return textureSampleLevel(tex0, samp, in.uv, 0.0);
}
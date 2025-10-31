struct Params {
  exposure:  f32,
  contrast:  f32,
  saturation:f32,
  vignette:  f32,
  time:      f32,
  surface_w: f32,   // window/swapchain width (pixels)
  surface_h: f32,   // window/swapchain height (pixels)
  _pad0:     f32,
};

struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var out: VsOut;
    
    // Generate a fullscreen triangle
    // Vertex 0: (-1, -1) bottom-left
    // Vertex 1: ( 3, -1) bottom-right (off-screen)
    // Vertex 2: (-1,  3) top-left (off-screen)
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index & 2u) * 2 - 1);
    
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    
    // UV coordinates (flipped Y for upside-down fix)
    out.uv = vec2<f32>(
        (x + 1.0) * 0.5,
        1.0 - (y + 1.0) * 0.5
    );
    
    return out;
}

@group(0) @binding(0) var samp: sampler;
@group(0) @binding(1) var vid_tex: texture_2d<f32>;
@group(0) @binding(2) var<uniform> params: Params;

@fragment
fn fs_blit(in: VsOut) -> @location(0) vec4<f32> {
  return textureSampleLevel(vid_tex, samp, in.uv, 0.0);
}
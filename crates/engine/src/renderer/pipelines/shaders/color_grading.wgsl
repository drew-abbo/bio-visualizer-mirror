struct Params {
  exposure:  f32,
  contrast:  f32,
  saturation:f32,
  vignette:  f32,
  time:      f32,
  surface_w: f32,
  surface_h: f32,
  _pad0:     f32,
};

struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn main_vs(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var out: VsOut;
    
    // Generate a fullscreen triangle
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index & 2u) * 2 - 1);
    
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    
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
fn main_fs(in: VsOut) -> @location(0) vec4<f32> {
  // Sample the texture
  var color = textureSampleLevel(vid_tex, samp, in.uv, 0.0);
  
  // Apply exposure
  color = vec4<f32>(color.rgb * params.exposure, color.a);
  
  // Apply contrast
  let contrasted = (color.rgb - 0.5) * params.contrast + 0.5;
  color = vec4<f32>(contrasted, color.a);
  
  // Apply saturation
  let luminance = dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));
  let saturated = mix(vec3<f32>(luminance), color.rgb, params.saturation);
  color = vec4<f32>(saturated, color.a);
  
  // Apply vignette
  let dist = length(in.uv - vec2<f32>(0.5, 0.5));
  let vignette_factor = smoothstep(0.8, 0.4, dist * params.vignette);
  color = vec4<f32>(color.rgb * vignette_factor, color.a);
  
  return color;
}
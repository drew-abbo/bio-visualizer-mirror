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

@group(0) @binding(0) var samp: sampler;
@group(0) @binding(1) var vid_tex: texture_2d<f32>;
@group(0) @binding(2) var<uniform> params: Params;

struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
  // Fullscreen triangle (covers whole NDC)
  let V = array<vec4<f32>, 3>(
    vec4<f32>(-1.0, -1.0, 0.0, 1.0),
    vec4<f32>( 3.0, -1.0, 0.0, 1.0),
    vec4<f32>(-1.0,  3.0, 0.0, 1.0)
  );
  // UV in 0..2; we’ll remap to 0..1 in fragment
  let UV = array<vec2<f32>, 3>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(2.0, 0.0),
    vec2<f32>(0.0, 2.0)
  );

  var out: VsOut;
  out.pos = V[vid];
  out.uv  = UV[vid];
  return out;
}

// helpers
fn apply_exposure(c: vec3<f32>, exp: f32) -> vec3<f32> { return c * pow(2.0, exp); }
fn apply_contrast(c: vec3<f32>, k: f32) -> vec3<f32> { let m = vec3<f32>(0.5); return (c - m)*k + m; }
fn apply_saturation(c: vec3<f32>, s: f32) -> vec3<f32> {
  let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
  return mix(vec3<f32>(luma), c, s);
}
fn apply_vignette(c: vec3<f32>, uv01: vec2<f32>, strength: f32) -> vec3<f32> {
  if (strength <= 0.0) { return c; }
  let d = distance(uv01, vec2<f32>(0.5, 0.5));
  let v = clamp(1.0 - d * (2.0 * strength), 0.0, 1.0);
  return c * v;
}

@fragment
fn fs_main(@location(0) uv_in: vec2<f32>) -> @location(0) vec4<f32> {
  // Remap 0..2 -> 0..1 and flip vertically
  var uv01 = uv_in * 0.5;
  uv01.y = 1.0 - uv01.y;

  // Compute “fit” to center video with aspect preserved
  let vw_vh = vec2<f32>(textureDimensions(vid_tex)); // video size (pixels)
  let sw = params.surface_w;
  let sh = params.surface_h;

  let va = vw_vh.x / vw_vh.y;   // video aspect
  let sa = sw / sh;             // surface aspect

  var sample_uv = uv01;
  if (va > sa) {
    // letterbox: fit to width, scale Y
    let scale = sa / va;                                  // 0..1 visible band height
    let band_min = (1.0 - scale) * 0.5;
    let band_max = 1.0 - band_min;
    // Re-normalize uv01.y from [band_min, band_max] to [0,1]
    sample_uv.y = (uv01.y - band_min) / (band_max - band_min);
  } else {
    // pillarbox: fit to height, scale X
    let scale = va / sa;                                  // 0..1 visible band width
    let band_min = (1.0 - scale) * 0.5;
    let band_max = 1.0 - band_min;
    sample_uv.x = (uv01.x - band_min) / (band_max - band_min);
  }

  // Clamp to avoid sampling outside the video
  sample_uv = clamp(sample_uv, vec2<f32>(0.0), vec2<f32>(1.0));

  // Sample and grade (you can keep/disable the RGB split/wobble you had earlier)
  var rgba = textureSample(vid_tex, samp, sample_uv);
  var c = rgba.rgb;

  c = apply_exposure(c, params.exposure);
  c = apply_contrast(c, params.contrast);
  c = apply_saturation(c, params.saturation);
  c = apply_vignette(c, uv01, params.vignette);

  return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), rgba.a);
}

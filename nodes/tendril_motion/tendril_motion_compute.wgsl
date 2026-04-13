// Tendril Motion compute stage.
// Writes X/Y/Rotation-like values into RGBA at pixel (0,0), which the executor
// reads back into scalar outputs.

struct Params {
    phase: f32,
    speed: f32,
    curl: f32,
    chaos: f32,
    scale: f32,
    seed: f32,
}

@group(0) @binding(0) var output_texture: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(1) var<uniform> params: Params;

fn hash11(x: f32) -> f32 {
    return fract(sin(x * 127.1 + 311.7) * 43758.5453123);
}

@compute @workgroup_size(1, 1, 1)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    if global_id.x != 0u || global_id.y != 0u {
        return;
    }

    let chaos = clamp(params.chaos, 0.0, 1.0);
    let curl = max(params.curl, 0.0);
    let scale = max(params.scale, 0.0);

    let seed_phase = params.seed * 0.173;
    let t = params.phase * max(params.speed, 0.001) * 6.28318530718 + seed_phase;

    let drift_x = hash11(params.seed + floor(params.phase * (0.25 + chaos))) * 2.0 - 1.0;
    let drift_y = hash11(params.seed + 19.0 + floor(params.phase * (0.31 + chaos))) * 2.0 - 1.0;

    let base_x = sin(t * (1.0 + curl * 1.7));
    let base_y = cos(t * (0.8 + curl * 1.3));

    // Blend periodic motion with low-frequency chaotic drift for a tendril feel.
    let x = ((base_x * (1.0 - chaos * 0.6)) + drift_x * chaos) * scale;
    let y = ((base_y * (1.0 - chaos * 0.6)) + drift_y * chaos) * scale;

    // Normalize to [0,1] since this writes into an rgba8unorm texture.
    let x01 = clamp(x * 0.5 + 0.5, 0.0, 1.0);
    let y01 = clamp(y * 0.5 + 0.5, 0.0, 1.0);

    let rotation = atan2(y, x); // [-pi, pi]
    let rotation01 = clamp(rotation / 6.28318530718 + 0.5, 0.0, 1.0);

    textureStore(output_texture, vec2<u32>(0u, 0u), vec4<f32>(x01, y01, rotation01, 1.0));
}

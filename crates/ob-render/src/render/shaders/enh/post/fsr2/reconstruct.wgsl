//!pass reconstruct
// ffx_fsr2_reconstruct_dilated_velocity_and_previous_depth.h
@group(0) @binding(2) var input_depth: texture_depth_2d;
@group(0) @binding(3) var input_color: texture_2d<f32>;
@group(0) @binding(4) var motion: texture_2d<f32>;
@group(0) @binding(5) var out_dilated_motion: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var out_dilated_depth: texture_storage_2d<r32float, write>;
@group(0) @binding(7) var out_lock_luma: texture_storage_2d<r32float, write>;
@group(0) @binding(8) var<storage, read_write> reconstructed: array<atomic<u32>>;

const NEAREST_OFFSETS = array<vec2<i32>, 9>(
    vec2<i32>(0, 0), vec2<i32>(1, 0), vec2<i32>(0, 1), vec2<i32>(0, -1), vec2<i32>(-1, 0),
    vec2<i32>(-1, 1), vec2<i32>(1, 1), vec2<i32>(-1, -1), vec2<i32>(1, -1));

// Stored as the bits of the distance from the far plane's depth (larger
// is nearer in both depth directions; a cleared buffer is the far plane).
fn store_reconstructed(p: vec2<i32>, d: f32) {
    let i = u32(p.y) * u32(render_size().x) + u32(p.x);
    atomicMax(&reconstructed[i], bitcast<u32>(max(abs(d - far_depth()), 0.0)));
}

fn reconstruct_prev_depth(p: vec2<i32>, d: f32, mv0: vec2<f32>, size: vec2<i32>) {
    var mv = mv0;
    if length(mv * display_size()) <= 0.1 {
        mv = vec2<f32>(0.0);
    }
    let uv = (vec2<f32>(p) + 0.5) / vec2<f32>(size);
    let b = bilinear_data(uv + mv, render_size());
    for (var i = 0; i < 4; i++) {
        if b.weights[i] > RECONSTRUCTED_DEPTH_THRESHOLD {
            let s = b.base + BILINEAR_OFFSETS[i];
            if on_screen(s, size) {
                store_reconstructed(s, d);
            }
        }
    }
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = vec2<i32>(id.xy);
    let size = vec2<i32>(render_size());
    if !on_screen(p, size) {
        return;
    }
    var nearest = p;
    var nearest_d = textureLoad(input_depth, p, 0);
    for (var i = 1; i < 9; i++) {
        let s = p + NEAREST_OFFSETS[i];
        if on_screen(s, size) {
            let d = textureLoad(input_depth, s, 0);
            if (reversed() && d > nearest_d) || (!reversed() && d < nearest_d) {
                nearest = s;
                nearest_d = d;
            }
        }
    }
    let mv = textureLoad(motion, nearest, 0);
    textureStore(out_dilated_depth, p, vec4<f32>(nearest_d, 0.0, 0.0, 0.0));
    textureStore(out_dilated_motion, p, mv);
    reconstruct_prev_depth(p, nearest_d, mv.xy, size);
    // ComputeLockInputLuma (exposure 1)
    var rgb = max(vec3<f32>(0.0), textureLoad(input_color, p, 0).rgb);
    if hdr() {
        rgb = tonemap(rgb);
    }
    textureStore(out_lock_luma, p, vec4<f32>(pow(perceived_luma(rgb), 1.0 / 6.0), 0.0, 0.0, 0.0));
}


//!pass composition
// What SSR added to the scene (after it, against the snapshot taken
// before it): FSR 2's transparency-and-composition mask (reflections move
// with what they reflect, not with the surface).
@group(0) @binding(2) var input_color: texture_2d<f32>;
@group(0) @binding(3) var opaque_color: texture_2d<f32>;
@group(0) @binding(4) var out_composition: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = vec2<i32>(id.xy);
    if !on_screen(p, vec2<i32>(render_size())) {
        return;
    }
    let post = max(textureLoad(input_color, p, 0).rgb, vec3<f32>(0.0));
    let pre = max(textureLoad(opaque_color, p, 0).rgb, vec3<f32>(0.0));
    let delta = rgb_to_ycocg(tonemap(post)) - rgb_to_ycocg(tonemap(pre));
    let tc = clamp((length(delta) - 0.01) * u.misc.y, 0.0, 1.0);
    textureStore(out_composition, p, vec4<f32>(tc, 0.0, 0.0, 0.0));
}


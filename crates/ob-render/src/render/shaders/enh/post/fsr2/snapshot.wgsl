//!pass snapshot
// The scene before SSR: the "opaque only" image of the composition mask.
@group(0) @binding(2) var input_color: texture_2d<f32>;
@group(0) @binding(3) var out_opaque: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = vec2<i32>(id.xy);
    if on_screen(p, vec2<i32>(render_size())) {
        textureStore(out_opaque, p, textureLoad(input_color, p, 0));
    }
}


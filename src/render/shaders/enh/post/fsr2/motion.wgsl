//!pass motion
// The motion of each render pixel's own surface (uv, FSR 2's convention:
// previous uv = uv + motion), from the depth and the drawn cars (the TAA pass's
// temporal.wgsl), and its masks: xy motion, z reactive (a spinning wheel:
// its texture moves where the rigid motion does not; 1 on a car without a
// last pose), w transparency-and-composition (what SSR added, the
// composition pass).
@group(0) @binding(2) var input_depth: texture_depth_2d;
@group(0) @binding(3) var out_motion: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var composition: texture_2d<f32>;
// The world's animation mask (R reactive, G composition) and its depth
@group(0) @binding(5) var anim_mask: texture_2d<f32>;
@group(0) @binding(6) var anim_depth: texture_depth_2d;

// How far the mask's surface may lie behind the final scene's (share of the
// view distance, plus 0.3 m) for the mark to hold: a car in front of a
// barrier is not marked; a blended barrier (no depth of its own in the
// scene) in front of what shows through it is; Animated Water's waves move
// its surface a little.
const ANIM_DEPTH_TOLERANCE: f32 = 0.015;

// docs/notes/graphics/fsr2.md "Animation mask": (reactive, composition) of the
// world's moving textures at p (the mask holds the draws' values), where
// the final scene shows them.
fn animation(p: vec2<i32>, d: f32) -> vec2<f32> {
    if u.anim.x < 0.5 {
        return vec2<f32>(0.0);
    }
    let m = textureLoad(anim_mask, p, 0).xy;
    if m.x + m.y <= 0.0 {
        return vec2<f32>(0.0);
    }
    let z = view_depth(d);
    let mz = view_depth(textureLoad(anim_depth, p, 0));
    if mz > z + abs(z) * ANIM_DEPTH_TOLERANCE + 0.3 && u.misc.x < 6.5 {
        return vec2<f32>(0.0);
    }
    return m;
}

fn motion_of(rel: vec3<f32>) -> vec4<f32> {
    var flags = vec2<f32>(0.0);
    var p = vec4<f32>(rel, 1.0);
    let count = i32(u.params.x);
    for (var i = 0; i < count; i++) {
        let sphere = cars[i].sphere;
        let to = rel - sphere.xyz;
        if dot(to, to) > sphere.w {
            continue;
        }
        let local = (cars[i].inv * vec4<f32>(rel, 1.0)).xyz;
        let lo = cars[i].lo;
        let hi = cars[i].hi;
        if all(local >= lo.xyz) && all(local <= hi.xyz) {
            p = cars[i].motion * vec4<f32>(rel, 1.0);
            flags.y = lo.w;
            let hw = cars[i].wheel_hw;
            for (var w = 0; w < 4; w++) {
                let wheel = cars[i].wheel[w];
                let d = local - wheel.xyz;
                if abs(d.y) < hw[w] && dot(d.xz, d.xz) < wheel.w * wheel.w {
                    flags.x = 1.0;
                }
            }
            break;
        }
    }
    let clip = u.prev_rel_to_clip * p;
    let ndc = clip.xy / max(clip.w, 1e-6);
    var prev_uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if clip.w <= 0.0 {
        prev_uv = vec2<f32>(-10.0);
    }
    return vec4<f32>(prev_uv, flags);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = vec2<i32>(id.xy);
    let size = vec2<i32>(render_size());
    if !on_screen(p, size) {
        return;
    }
    let d = textureLoad(input_depth, p, 0);
    let uv = (vec2<f32>(p) + 0.5) * u.render.zw;
    let h = u.clip_to_rel * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    let rel = h.xyz / h.w;
    let m = motion_of(rel);
    // the content's (unjittered) uv
    let content = (vec2<f32>(p) + 0.5 - jitter()) * u.render.zw;
    var mv = m.xy - content;
    if u.misc.x > 5.5 && u.misc.x < 6.5 {
        mv = vec2<f32>(0.0);
    }
    var reactive = m.z * u.misc.z;
    if m.w > 0.5 {
        reactive = 1.0;
    }
    var tc = 0.0;
    if u.extra.x > 0.5 {
        tc = textureLoad(composition, p, 0).x;
    }
    let anim = animation(p, d);
    reactive = max(reactive, anim.x);
    tc = max(tc, anim.y);
    textureStore(out_motion, p, vec4<f32>(mv, reactive, tc));
}


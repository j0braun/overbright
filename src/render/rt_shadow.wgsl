// Clone deviation, Modern + RT only: the strongest
// lights' ray-traced shadows at half the scene's resolution, after the main
// view's depth pre-pass and before its colour pass (render/rt_shadow.rs,
// docs/perf-budget.md, the new effects' costs). Before this pass every pixel traced its rays inside
// the world's colour shader (soft shadows: 2 + 1 + 1 + 1 rays per pixel, the shader's
// registers and occupancy paid by every world pixel). Now one thread per
// 2 × 2 pixel block: the scene point and face normal from the pre-pass's
// depth, the same light ranking as `ml_relight` (`ml_now`, its share floor,
// no ceilings), and for the two strongest lights the area-light rays
// (`ml_ray`: the block's quarter of the Vogel disc set). Written per block:
// (light + 1, visibility) for both, the block's view depth and its normal;
// the colour pass gathers a 2 × 2 box of blocks around each pixel (all four
// quarters of every light's disc set, as the quad mean), weighted by the
// pixel's own plane and normal (`ml_vis` in world.wgsl).
// Prepended by rt_shadow.rs: world.wgsl's Frame and Lamp structs and its
// RTVIS A / B / D blocks (the Modern Lighting constants, params, pools,
// `ml_light_radius`, `ml_now`).
@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> lamp_list: array<Lamp>;
@group(0) @binding(2) var<storage, read> clusters: array<u32>;
@group(0) @binding(3) var ml_tlas: acceleration_structure;
// The pre-pass's depth (DEPTH_TEXTURE: texture_depth_2d or its
// multisampled form; sample 0).
@group(0) @binding(4) var scene_depth: DEPTH_TEXTURE;
@group(0) @binding(5) var vis_out: texture_storage_2d<rgba32uint, write>;
struct RtVis {
    // Clip → world position relative to the eye (the main view's
    // view-projection with the eye moved to the origin, inverted in f64).
    inv: mat4x4<f32>,
    // xy the scene's size (pixels), zw the blocks'.
    sizes: vec4<f32>,
    // x the cleared depth (1, 0 with Reversed-Z), yz the scene pixels per
    // grid pixel (1, or the supersampling's: `rt_shadow::grid`).
    misc: vec4<f32>,
};
@group(0) @binding(6) var<uniform> rv: RtVis;
// The dispatch's first block (xy): 0 for the frame's pass; the characters'
// box for their own pass after they are in the depth (shadow.rs
// `Plan::record_region`).
@group(0) @binding(7) var<uniform> rv_first: vec4<u32>;
// The block (half-resolution) the rays interleave over (`ml_ray`).
var<private> g_ml_pix: vec2<f32>;
// `ml_ray` / `ml_ray_one`: world.wgsl's RTVIS C block (prepended).
// The scene's depth at pixel q (sample 0).
fn rv_depth(q: vec2<i32>) -> f32 {
    let c = clamp(q, vec2<i32>(0), vec2<i32>(rv.sizes.xy) - 1);
    return textureLoad(scene_depth, c, 0);
}
// The point (relative to the eye) a pixel's depth shows.
fn rv_point(q: vec2<i32>, d: f32) -> vec3<f32> {
    let uv = (vec2<f32>(q) + 0.5) / rv.sizes.xy;
    let c = rv.inv * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    return c.xyz / c.w;
}
// Octahedral 2 × 16-bit unit normal.
fn rv_oct(n: vec3<f32>) -> u32 {
    var e = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    if n.z < 0.0 {
        e = (1.0 - abs(e.yx)) * select(vec2<f32>(-1.0), vec2<f32>(1.0), e >= vec2<f32>(0.0));
    }
    return pack2x16snorm(e);
}
// (light + 1) << 16 | visibility (16 bits); 0: no light.
fn rv_slot(light: u32, vis: f32) -> u32 {
    return ((light + 1u) << 16u) | u32(round(clamp(vis, 0.0, 1.0) * 65535.0));
}

@compute @workgroup_size(8, 8)
fn cs_rt_vis(@builtin(global_invocation_id) gid: vec3<u32>) {
    let id = vec3<u32>(gid.xy + rv_first.xy, gid.z);
    if f32(id.x) >= rv.sizes.z || f32(id.y) >= rv.sizes.w { return; }
    // The block's scene pixel: under its first grid pixel (2 id when the
    // grid is the scene's).
    let q = vec2<i32>(floor((vec2<f32>(id.xy) * 2.0 + 0.5) * rv.misc.yz));
    let d = rv_depth(q);
    if d == rv.misc.x {
        textureStore(vis_out, vec2<i32>(id.xy), vec4<u32>(0u));
        return;
    }
    let pr = rv_point(q, d);
    // The face normal: per axis the neighbour on the same surface (the
    // nearer in depth), as cross(dpdx, dpdy) on the drawn triangle.
    let fwd = frame.forward.xyz;
    let z = dot(pr, fwd);
    let xa = rv_point(q + vec2<i32>(1, 0), rv_depth(q + vec2<i32>(1, 0)));
    let xb = rv_point(q - vec2<i32>(1, 0), rv_depth(q - vec2<i32>(1, 0)));
    let ya = rv_point(q + vec2<i32>(0, 1), rv_depth(q + vec2<i32>(0, 1)));
    let yb = rv_point(q - vec2<i32>(0, 1), rv_depth(q - vec2<i32>(0, 1)));
    let dx = select(pr - xb, xa - pr, abs(dot(xa, fwd) - z) < abs(dot(xb, fwd) - z));
    let dy = select(pr - yb, ya - pr, abs(dot(ya, fwd) - z) < abs(dot(yb, fwd) - z));
    var ng = cross(dx, dy);
    ng = ng * inverseSqrt(max(dot(ng, ng), 1e-20));
    let v = -pr * inverseSqrt(max(dot(pr, pr), 1e-12));
    if dot(ng, v) < 0.0 { ng = -ng; }
    let p = pr + frame.camera.xyz;
    let out_zn = vec2<u32>(bitcast<u32>(z), rv_oct(ng));
    // The cluster (as ml_relight).
    let ndc = vec2<f32>((f32(q.x) + 0.5) / rv.sizes.x * 2.0 - 1.0, 1.0 - (f32(q.y) + 0.5) / rv.sizes.y * 2.0);
    let tiles = frame.cluster.xy;
    let tx = u32(clamp(floor((ndc.x * 0.5 + 0.5) * tiles.x), 0.0, tiles.x - 1.0));
    let ty = u32(clamp(floor((ndc.y * 0.5 + 0.5) * tiles.y), 0.0, tiles.y - 1.0));
    let slice = u32(clamp(floor(log(max(z, frame.cluster_near.x) / frame.cluster_near.x) * frame.cluster.w), 0.0, frame.cluster.z - 1.0));
    let c = (slice * u32(tiles.y) + ty) * u32(tiles.x) + tx;
    let first = clusters[c * 2u];
    let count = clusters[c * 2u + 1u];
    let P = ml_params();
    // Ceilings and undersides keep their fallback (as ml_relight).
    if P.rays == 0u || ng.z < -0.3 {
        textureStore(vis_out, vec2<i32>(id.xy), vec4<u32>(0u, 0u, out_zn));
        return;
    }
    // The two lights that light p most (ml_relight's ranking, the face
    // normal for the shading one) and p's whole relit light (the share floor).
    var sel = vec2<u32>(0xffffffffu);
    var selw = vec2<f32>(0.02);
    var total = 0.0;
    let n_lights = min(count, P.lights);
    for (var i = 0u; i < n_lights; i++) {
        let l = lamp_list[clusters[first + i]];
        let colour = l.shadow.yzw;
        if all(colour <= vec3<f32>(0.0)) { continue; }
        let pack = l.colour.w >= 0.0;
        if !pack && P.emit <= 0.0 { continue; }
        let rel = l.position.xyz - p;
        let d2 = dot(rel, rel);
        let dir = rel * inverseSqrt(max(d2, 1e-12));
        let bn = ml_now(l, rel, dir, ng, ng, P.soft, false);
        if max(bn.x, bn.y) < ML_TAIL { continue; }
        let w = bn.y * dot(colour, vec3<f32>(0.33));
        total += w;
        if l.colour.w > -1.5 && w > selw.y {
            if w > selw.x {
                sel = vec2<u32>(i, sel.x);
                selw = vec2<f32>(w, selw.x);
            } else {
                sel.y = i;
                selw.y = w;
            }
        }
    }
    g_ml_pix = vec2<f32>(id.xy);
    let floor_w = ML_RAY_SHARE * total;
    var slots = vec2<u32>(0u);
    for (var j = 0u; j < min(P.rays, 2u); j++) {
        let i = sel[j];
        if i == 0xffffffffu || selw[j] < floor_w { break; }
        let k = clusters[first + i];
        if k >= 0xFFFFu { continue; }
        let l = lamp_list[k];
        let vis = ml_ray(p, ng, l.position.xyz, ml_light_radius(l), 0x03u, k, select(ML_RAY_SAMPLES_REST, ML_RAY_SAMPLES, j == 0u));
        slots[j] = rv_slot(k, vis);
    }
    textureStore(vis_out, vec2<i32>(id.xy), vec4<u32>(slots, out_zn));
}

// Modern + RT: the frame's traced scene (render/rt.rs: the world's low LOD
// and the cars; instance mask 1 the world, 2 the cars). For speed, traced by the
// half-resolution pass before the colour pass (rt_shadow.wgsl, after the
// depth pre-pass); kept bound here.
@group(0) @binding(11) var ml_tlas: acceleration_structure;
// That pass's blocks (2 × 2 pixels each): x, y (light + 1) << 16 | 1 << 15
// for a light of the rest's kind (an emitter) | visibility (15 bits),
// z the rest's visibility (16 bits; its history's frames in the
// next 8, 0: no rest), w the block's view depth (f16, low half) and its
// face normal (octahedral 2 × 8 bits, high half).
@group(0) @binding(15) var ml_vis_blocks: texture_2d<u32>;
// That pass's uniform (rt_shadow.wgsl `RtVis`): misc.yz the scene pixels
// per grid pixel (1, or the supersampling's: the blocks then cover 2 × 2
// screen pixels each, `rt_shadow::grid`).
struct MlRtVis {
    inv: mat4x4<f32>,
    sizes: vec4<f32>,
    misc: vec4<f32>,
};
@group(0) @binding(16) var<uniform> ml_vis_grid: MlRtVis;
// The scene position (pixel centre) block `c` sampled.
fn ml_vis_at(c: vec2<i32>) -> vec2<f32> {
    return floor((vec2<f32>(c) * 2.0 + 0.5) * ml_vis_grid.misc.yz) + 0.5;
}
// The NIS characters' pipelines (`LF_ML_CHAR`) trace their rays here
// in the colour shader (they are not in the depth pre-pass); every other world draw
// reads the blocks (the ray code is folded out of its pipelines).
override D9_ML_CHAR: bool = false;
// BEGIN RTVIS C
// The fraction of a light's disc (centre l, radius `radius`, facing p)
// that nothing of the scene hides from p (face normal ng); `mask` 1 for a
// car's own light (the world only). Soft shadows (no harsh, hard
// edges): an area light, not a point — `samples` rays per pixel (block) to
// points of a fixed Vogel disc set; the 2×2 pixels (blocks) take interleaved
// quarters of it (samples × 4 points, `g_ml_pix`) and each light its own
// turn of the set (`key`); nothing depends on time: a still view is still.
fn ml_ray(p: vec3<f32>, ng: vec3<f32>, l: vec3<f32>, radius: f32, mask: u32, key: u32, samples: u32) -> f32 {
    // Off the surface by more where the ray grazes it (the traced world is
    // the low LOD: its faces sit a little off the drawn ones).
    let d0 = normalize(l - p);
    let o = p + ng * (0.08 + 0.3 * (1.0 - max(dot(ng, d0), 0.0)));
    if length(l - o) < 1.3 { return 1.0; }
    // Walls start farther out (the low LOD's facades lie up to ~0.5 m
    // off the drawn curved ones).
    let tmin = mix(0.6, 0.15, smoothstep(0.3, 0.7, ng.z));
    // The disc's axes (across the direction to its centre).
    let t = normalize(cross(d0, select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(d0.z) > 0.9)));
    let b = cross(d0, t);
    let block = vec2<u32>(g_ml_pix) & vec2<u32>(1u);
    let quarter = block.x + 2u * block.y;
    let turn = f32(key & 1023u) * ML_GOLDEN;
    // Stop short of the light's own body (a lamp's head, a sign's card).
    let stop = 0.6 + radius;
    var vis = 0.0;
    for (var j = 0u; j < samples; j++) {
        let m = quarter + 4u * j;
        let r = sqrt((f32(m) + 0.5) / f32(4u * samples)) * radius;
        let a = f32(m) * ML_GOLDEN + turn;
        let to = l + (t * cos(a) + b * sin(a)) * r - o;
        let len = length(to);
        vis += ml_ray_one(o, to / len, tmin, max(len - stop, tmin + 0.01), mask);
    }
    return vis / f32(samples);
}
// One ray: 1 when it reaches tmax.
fn ml_ray_one(o: vec3<f32>, d: vec3<f32>, tmin: f32, tmax: f32, mask: u32) -> f32 {
    var rq: ray_query;
    rayQueryInitialize(&rq, ml_tlas, RayDesc(RAY_FLAG_FORCE_OPAQUE | RAY_FLAG_TERMINATE_ON_FIRST_HIT, mask, tmin, tmax, o, d));
    while rayQueryProceed(&rq) {}
    let h = rayQueryGetCommittedIntersection(&rq);
    return select(1.0, 0.0, h.kind != RAY_QUERY_INTERSECTION_NONE);
}

// END RTVIS C
// The blocks around this pixel (`ml_vis_taps`): the lights their centre
// block and the next one diagonally toward the pixel traced (+ 1; 0 none),
// and per light the weighted visibility and weight over the 3 × 3 blocks;
// over all of them on this pixel's surface the weight, and the rest's
// weighted visibility and weight; the centre block.
var<private> g_vis_ids: vec4<u32>;
// Whether each of those lights is of the rest's kind (1: an emitter).
var<private> g_vis_rk: vec4<u32>;
var<private> g_vis_sum: vec4<f32>;
var<private> g_vis_wt: vec4<f32>;
var<private> g_vis_all: f32;
var<private> g_vis_rest: vec2<f32>;
var<private> g_vis_c: vec4<u32>;
var<private> g_vis_cb: vec2<i32>;
// The octahedral encoding of a unit normal (as rt_shadow.wgsl stores it).
fn ml_vis_oct(n: vec3<f32>) -> vec2<f32> {
    var e = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    if n.z < 0.0 {
        e = (1.0 - abs(e.yx)) * select(vec2<f32>(-1.0), vec2<f32>(1.0), e >= vec2<f32>(0.0));
    }
    return e;
}
// A block's view depth.
fn ml_vis_z(t: vec4<u32>) -> f32 {
    return unpack2x16float(t.w).x;
}
// Block `t`'s weight for this pixel (filter weight `fw`): on the pixel's
// plane (`want` its view depth there, `tol`) and facing as the pixel (`e`
// its normal's octahedral code; ~0.15 apart is ~15°); 0 outside the grid.
fn ml_vis_weight(t: vec4<u32>, q: vec2<i32>, dims: vec2<i32>, fw: f32, want: f32, tol: f32, e: vec2<f32>) -> f32 {
    let de = unpack4x8snorm(t.w >> 16u).xy - e;
    let wd = 1.0 - smoothstep(tol, 2.0 * tol, abs(ml_vis_z(t) - want));
    let wn = 1.0 - smoothstep(0.1, 0.2, abs(de.x) + abs(de.y));
    let inside = all(q >= vec2<i32>(0)) && all(q < dims);
    return select(0.0, fw * wd * wn, inside && t.w != 0u);
}
// One block `q` of the pixel's into the sums.
fn ml_vis_add(q: vec2<i32>, dims: vec2<i32>, fw: f32, want: f32, tol: f32, e: vec2<f32>) {
    let t = textureLoad(ml_vis_blocks, clamp(q, vec2<i32>(0), dims - 1), 0);
    let w = ml_vis_weight(t, q, dims, fw, want, tol, e);
    let mx = select(vec4<f32>(0.0), vec4<f32>(w), g_vis_ids == vec4<u32>(t.x >> 16u));
    let my = select(vec4<f32>(0.0), vec4<f32>(w), g_vis_ids == vec4<u32>(t.y >> 16u));
    g_vis_sum += mx * (f32(t.x & 0x7FFFu) / 32767.0) + my * (f32(t.y & 0x7FFFu) / 32767.0);
    g_vis_wt += mx + my;
    g_vis_all += w;
    let r = select(0.0, w, (t.z & 0xFF0000u) != 0u);
    g_vis_rest += vec2<f32>(r * (f32(t.z & 0xFFFFu) / 65535.0), r);
}
// The uniform case's tap (`ml_vis_taps`): its weight, as `ml_vis_add`
// makes it, into the running weight sum and the rest's sums, and whether it
// holds the centre block's lights.
var<private> g_vis_same: bool;
fn ml_vis_add_uniform(q: vec2<i32>, dims: vec2<i32>, fw: f32, want: f32, tol: f32, e: vec2<f32>) {
    let t = textureLoad(ml_vis_blocks, clamp(q, vec2<i32>(0), dims - 1), 0);
    let w = ml_vis_weight(t, q, dims, fw, want, tol, e);
    g_vis_all += w;
    let r = select(0.0, w, (t.z & 0xFF0000u) != 0u);
    g_vis_rest += vec2<f32>(r * (f32(t.z & 0xFFFFu) / 65535.0), r);
    g_vis_same = g_vis_same && all(t.xy == g_vis_c.xy);
}
// The pixel's blocks: the four whose centres are around it, weighted
// bilinearly (the weights move smoothly from pixel to pixel: no 2-pixel
// steps). Each block's record holds its whole disc set and its 2 × 2 of
// pixels already (the shadow pass turns them over the frames and keeps a
// history), so no wider tent is needed to mix the quarters. Each block
// counts only on this pixel's plane (view depth z, its screen slopes dz)
// and facing as its face normal ng.
fn ml_vis_taps(pix: vec2<f32>, z: f32, dz: vec2<f32>, ng: vec3<f32>) {
    let dims = vec2<i32>(textureDimensions(ml_vis_blocks));
    let u = (pix / ml_vis_grid.misc.yz - 0.5) * 0.5;
    let k = floor(u);
    let f = u - k;
    let near = f >= vec2<f32>(0.5);
    // The nearest block (the centre) and the one diagonally across.
    let c = vec2<i32>(k) + select(vec2<i32>(0), vec2<i32>(1), near);
    let side = select(vec2<i32>(1), vec2<i32>(-1), near);
    // The weights of the blocks at c and at c + side, per axis.
    let wc = select(1.0 - f, f, near);
    let ws = 1.0 - wc;
    g_vis_cb = c;
    g_vis_c = textureLoad(ml_vis_blocks, clamp(c, vec2<i32>(0), dims - 1), 0);
    let tol = 0.004 * z + 0.02;
    let e = ml_vis_oct(ng);
    // The view depth at block (c + o) on this pixel's plane: z0 + dz·2so
    // (the blocks 2s scene pixels apart).
    let z0 = z + dot(dz, ml_vis_at(c) - pix);
    let sx = dz.x * 2.0 * ml_vis_grid.misc.y * f32(side.x);
    let sy = dz.y * 2.0 * ml_vis_grid.misc.z * f32(side.y);
    let ox = vec2<i32>(side.x, 0);
    let oy = vec2<i32>(0, side.y);
    g_vis_all = 0.0;
    g_vis_rest = vec2<f32>(0.0);
    // A short cut for what most pixels see (the full gather below makes the
    // same sums up to rounding): every block holds the centre's lights, each
    // light's visibility 0 or 1; a light's sum is then its weight sum or 0
    // (the rest's sums are taken as in the full gather).
    let vx = g_vis_c.x & 0x7FFFu;
    let vy = g_vis_c.y & 0x7FFFu;
    if (vx == 0u || vx == 0x7FFFu) && (vy == 0u || vy == 0x7FFFu) && (g_vis_c.x >> 16u) != (g_vis_c.y >> 16u) {
        g_vis_same = true;
        ml_vis_add_uniform(c, dims, wc.x * wc.y, z0, tol, e);
        ml_vis_add_uniform(c + ox, dims, ws.x * wc.y, z0 + sx, tol, e);
        ml_vis_add_uniform(c + oy, dims, wc.x * ws.y, z0 + sy, tol, e);
        ml_vis_add_uniform(c + side, dims, ws.x * ws.y, z0 + sx + sy, tol, e);
        if g_vis_same {
            let ids = vec2<u32>(g_vis_c.x >> 16u, g_vis_c.y >> 16u);
            g_vis_ids = vec4<u32>(ids, ids);
            g_vis_rk = (vec4<u32>(g_vis_c.xy, g_vis_c.xy) >> vec4<u32>(15u)) & vec4<u32>(1u);
            let s = select(vec2<f32>(0.0), vec2<f32>(g_vis_all), vec2<u32>(vx, vy) == vec2<u32>(0x7FFFu));
            g_vis_sum = vec4<f32>(s, s);
            g_vis_wt = vec4<f32>(g_vis_all);
            return;
        }
        g_vis_all = 0.0;
        g_vis_rest = vec2<f32>(0.0);
    }
    let d = textureLoad(ml_vis_blocks, clamp(c + side, vec2<i32>(0), dims - 1), 0);
    g_vis_ids = vec4<u32>(g_vis_c.x >> 16u, g_vis_c.y >> 16u, d.x >> 16u, d.y >> 16u);
    g_vis_rk = (vec4<u32>(g_vis_c.xy, d.xy) >> vec4<u32>(15u)) & vec4<u32>(1u);
    g_vis_sum = vec4<f32>(0.0);
    g_vis_wt = vec4<f32>(0.0);
    if (g_vis_c.x | g_vis_c.y | g_vis_c.z | d.x | d.y | d.z) == 0u { return; }
    ml_vis_add(c, dims, wc.x * wc.y, z0, tol, e);
    ml_vis_add(c + ox, dims, ws.x * wc.y, z0 + sx, tol, e);
    ml_vis_add(c + oy, dims, wc.x * ws.y, z0 + sy, tol, e);
    ml_vis_add(c + side, dims, ws.x * ws.y, z0 + sx + sy, tol, e);
}
// The blocks for this pixel at p (face normal ng, view depth z, its
// screen slopes dp / dz). An overlay the pre-pass does not hold (a blended
// decal on the road: the start line) lies a little over the surface the
// blocks traced; there the blocks of the surface point under p are taken
// (their screen position from p's plane), as long as it is within 25 cm.
fn ml_vis_find(p: vec3<f32>, z: f32, dpx: vec3<f32>, dpy: vec3<f32>, dz: vec2<f32>, ng: vec3<f32>) {
    ml_vis_taps(g_ml_pix, z, dz, ng);
    if g_vis_all >= 0.025 || ng.z < 0.5 || (g_vis_c.x | g_vis_c.y | g_vis_c.z) == 0u { return; }
    let b = g_vis_cb;
    let behind = ml_vis_z(g_vis_c) - (z + dot(dz, ml_vis_at(b) - g_ml_pix));
    let ray = normalize(p - frame.camera.xyz);
    let rf = dot(ray, frame.forward.xyz);
    if behind <= 0.0 || rf <= 0.0 { return; }
    let t = behind / rf;
    let h = t * max(-dot(ray, ng), 0.0);
    if h > 0.25 { return; }
    // From the surface point the pixel sees (p + ray t) to the one under
    // p (p − ng h): in the plane, in pixels along dpx / dpy.
    let w = -(ray * t + ng * h);
    let aa = dot(dpx, dpx);
    let ab = dot(dpx, dpy);
    let bb = dot(dpy, dpy);
    let det = aa * bb - ab * ab;
    if abs(det) < 1e-12 { return; }
    let ds = vec2<f32>(bb * dot(dpx, w) - ab * dot(dpy, w), aa * dot(dpy, w) - ab * dot(dpx, w)) / det;
    if dot(ds, ds) > 4096.0 { return; }
    let r = p - ng * h;
    ml_vis_taps(g_ml_pix + ds, dot(r - frame.camera.xyz, frame.forward.xyz), dz, ng);
}
// The traced visibility of lamp `light` at this pixel: over the blocks on
// this surface, its own where a block traced it and the rest's where not
// (the rest's mean visibility here; its own mean when no block has a rest);
// -1 when too few blocks lie on this surface or none traced it.
// (Below a tenth of the full weight: too few blocks on this surface.)
fn ml_vis(light: u32, rest: f32) -> f32 {
    let m = g_vis_ids == vec4<u32>(light + 1u);
    let w = dot(select(vec4<f32>(0.0), g_vis_wt, m), vec4<f32>(1.0));
    if w < 0.025 || g_vis_all < 0.025 { return -1.0; }
    let s = dot(select(vec4<f32>(0.0), g_vis_sum, m), vec4<f32>(1.0));
    if rest < 0.0 {
        return s / w;
    }
    return (s + max(g_vis_all - w, 0.0) * rest) / g_vis_all;
}
// The traced blocks for this pixel, gathered at the top of the fragment
// shader (`ml_vis_early`, called first by the world's colour functions),
// where little else is live: inside `ml_relight` the gather's sums
// were held across the whole light loop and took registers (waves) from
// every Modern + RT world pixel. What the light loop needs is kept: the
// slots' light ids (below 2^16, two to a register), each slot's traced
// visibility (`ml_vis`; -1 none; as halves, two to a register) and the
// rest's (-1 none). The same face normal, view depth and screen slopes as
// `ml_relight` uses.
var<private> g_rt_ids: vec2<u32>;
var<private> g_rt_vis: vec2<u32>;
var<private> g_rt_rest: f32;
fn ml_vis_early(p: vec3<f32>, ng_in: vec3<f32>, clip: vec2<f32>) {
    g_rt_ids = vec2<u32>(0u);
    g_rt_vis = vec2<u32>(pack2x16float(vec2<f32>(-1.0)));
    g_rt_rest = -1.0;
    // The view depth's screen slope (this pixel's plane, for the blocks'
    // weights; in uniform control flow).
    let dpx = dpdxFine(p);
    let dpy = dpdyFine(p);
    let dz = vec2<f32>(dot(dpx, frame.forward.xyz), dot(dpy, frame.forward.xyz));
    // Modern Lighting relights this pixel (`lamp_shade`) only from 1.5.
    if frame.lamps.x <= 1.5 { return; }
    let P = ml_params();
    let z = dot(p - frame.camera.xyz, frame.forward.xyz);
    let v = normalize(frame.camera.xyz - p);
    var ng = ng_in * inverseSqrt(max(dot(ng_in, ng_in), 1e-20));
    if dot(ng, v) < 0.0 { ng = -ng; }
    // As `ml_relight`'s `rt_blocks` (the characters trace their own).
    if D9_ML_CHAR || P.rays == 0u || ng.z < -0.3 { return; }
    g_ml_pix = clip;
    ml_vis_find(p, z, dpx, dpy, dz, ng);
    // The rest's mean here (enough of its blocks on this surface).
    if g_vis_rest.y >= 0.025 && g_vis_all >= 0.025 {
        g_rt_rest = g_vis_rest.x / g_vis_rest.y;
    }
    g_rt_ids = vec2<u32>(g_vis_ids.x | (g_vis_ids.y << 16u), g_vis_ids.z | (g_vis_ids.w << 16u));
    g_rt_vis = vec2<u32>(
        pack2x16float(vec2<f32>(ml_vis_rest(g_vis_ids.x, g_vis_rk.x), ml_vis_rest(g_vis_ids.y, g_vis_rk.y))),
        pack2x16float(vec2<f32>(ml_vis_rest(g_vis_ids.z, g_vis_rk.z), ml_vis_rest(g_vis_ids.w, g_vis_rk.w))),
    );
}
// `ml_vis` of slot id `id` (light + 1): the rest stands in where a block did
// not trace it only for a light of the rest's kind (`rk` 1: an emitter, no shadow
// layer); a lamp with a layer takes the blocks that traced it alone (its
// layer is no part of the rest's estimate).
fn ml_vis_rest(id: u32, rk: u32) -> f32 {
    if id == 0u { return -1.0; }
    return ml_vis(id - 1u, select(-1.0, g_rt_rest, rk == 1u));
}
// How deep the rest's shadow goes (of the traced lights' depth): its ray
// goes to an emitter's centre (rt_shadow.wgsl `rv_ray_centre`), while a sign
// or a neon tube lights from its whole face; at 1 the wet road before the
// casino's neon lost its glow (luma p95 −4 levels, the look rule allows 3).
const ML_REST_DEPTH: f32 = 0.5;
// The traced visibility of light `key` - 1 at this pixel (`ml_vis`'s, made
// in `ml_vis_early`); `found` false when no slot holds it. A light no slot
// holds that belongs to the rest (`rest`: an emitter, not a car's)
// takes the rest's visibility (-1: none here).
fn ml_vis_slot(key: u32, rest: bool, found: ptr<function, bool>) -> f32 {
    let m = vec4<u32>(g_rt_ids.x & 0xFFFFu, g_rt_ids.x >> 16u, g_rt_ids.y & 0xFFFFu, g_rt_ids.y >> 16u) == vec4<u32>(key);
    *found = any(m);
    let a = unpack2x16float(g_rt_vis.x);
    let b = unpack2x16float(g_rt_vis.y);
    let own = select(select(select(b.y, b.x, m.z), a.y, m.y), a.x, m.x);
    return select(select(-1.0, g_rt_rest, rest), own, *found);
}

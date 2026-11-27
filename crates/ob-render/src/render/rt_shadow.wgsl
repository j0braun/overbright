// Clone deviation, Modern + RT only: the strongest
// lights' ray-traced shadows at half the scene's resolution, after the main
// view's depth pre-pass and before its colour pass (render/rt_shadow.rs,
// docs/notes/performance/budget.md, the new effects' costs). Before this pass every pixel traced its rays inside
// the world's colour shader (soft shadows: 2 + 1 + 1 + 1 rays per pixel, the shader's
// registers and occupancy paid by every world pixel). Now one thread per
// 2 × 2 pixel block: the scene point and face normal from the pre-pass's
// depth, the same light ranking as `ml_relight` (`ml_now`, its share floor,
// no ceilings), and for the two strongest lights the area-light rays
// (`ml_ray`: the block's quarter of the Vogel disc set). Written per block:
// (light + 1, visibility) for both, the rest's visibility, the block's view
// depth and its normal; the colour pass gathers a 3 × 3 tent of blocks around
// each pixel (all four quarters of every light's disc set, as the quad mean),
// weighted by the pixel's own plane and normal (`ml_vis` in mlrt.wgsl).
// The rest (docs/notes/graphics/shadow-softness.md "Every light: the rest resampled"): the
// emitters past those two (neon, signs: no shadow layer; the lamps keep
// their fallback) share one visibility, the ratio of their shadowed to
// their unshadowed light at the block. It is estimated by resampled
// importance sampling: one of them is picked with a probability in
// proportion to its unshadowed light there (its exact share: every light of
// the cluster weighed, the pick by the inverse of their running sum; a
// golden-ratio sequence per block over the frames stratifies the picks), and
// one ray traced to its centre; its visibility is an unbiased estimate of the ratio,
// averaged over the frames in the block's history (reprojected by the
// camera, kept while the surface, its normal and the rest's light hold).
// The two traced lights keep a history of their own, by light: a light the
// block traced last frame too blends its visibility with that one.
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
    // World position relative to this frame's eye → the last frame's clip.
    prev: mat4x4<f32>,
    // x 1 when the history holds the last frame's blocks (the same grid, no
    // cut), y the rest's least blend toward this frame and w the traced
    // lights' (both by the world time passed: the same at any frame rate),
    // z this frame's number (the blocks' sample pixel, disc quarter and the
    // rest's pick).
    hist: vec4<f32>,
    // The NIS characters' box of blocks (x0, y0, x1, y1) the first pass
    // leaves to the characters' pass (the last frame's, joined with this
    // frame's there): their blocks are traced once a frame.
    skip: vec4<f32>,
};
@group(0) @binding(6) var<uniform> rv: RtVis;
// The dispatch's first block (xy): 0 for the frame's pass; the characters'
// box for their own pass after they are in the depth (shadow.rs
// `Plan::record_region`), z 1 then.
@group(0) @binding(7) var<uniform> rv_first: vec4<u32>;
// The last frame's blocks (the history).
@group(0) @binding(8) var vis_prev: texture_2d<u32>;
// The rest's history frames at most (its blend never below 1 / this).
const RV_REST_FRAMES: f32 = 64.0;
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
// The block's view depth (f16, low half) and its normal (octahedral,
// 2 × 8 bits, high half).
fn rv_oct(n: vec3<f32>) -> vec2<f32> {
    var e = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    if n.z < 0.0 {
        e = (1.0 - abs(e.yx)) * select(vec2<f32>(-1.0), vec2<f32>(1.0), e >= vec2<f32>(0.0));
    }
    return e;
}
fn rv_zn(z: f32, n: vec3<f32>) -> u32 {
    return (pack2x16float(vec2<f32>(z, 0.0)) & 0xFFFFu) | (pack4x8snorm(vec4<f32>(rv_oct(n), 0.0, 0.0)) << 16u);
}
// The rest's word: its visibility (16 bits), its history's frames (8) and
// its unshadowed light's log2 (8; a change of 2× drops the history).
fn rv_rest(s: f32, frames: f32, lw: f32) -> u32 {
    return u32(round(clamp(s, 0.0, 1.0) * 65535.0)) | (u32(frames) << 16u) | (u32(lw) << 24u);
}
// A block's hash, for the rest's pick.
fn rv_hash(q: vec2<u32>) -> f32 {
    var h = (q.x * 0x8DA6B343u) ^ (q.y * 0xD8163841u);
    h = (h ^ (h >> 16u)) * 0x7FEB352Du;
    h = (h ^ (h >> 15u)) * 0x846CA68Bu;
    return f32(h ^ (h >> 16u)) * (1.0 / 4294967296.0);
}
// A traced light's word: (light + 1) << 16, 1 << 15 when it is of the rest's
// kind (an emitter: the colour pass may stand the rest in for it),
// its visibility (15 bits); 0: no light.
fn rv_slot(light: u32, rest_kind: bool, vis: f32) -> u32 {
    return ((light + 1u) << 16u) | select(0u, 0x8000u, rest_kind) | u32(round(clamp(vis, 0.0, 1.0) * 32767.0));
}
fn rv_vis(word: u32) -> f32 {
    return f32(word & 0x7FFFu) / 32767.0;
}

// This frame's work at one block (`rv_block`), before the history's blend.
struct RvBlock {
    // Write a record (inside the grid).
    store: bool,
    // The record's depth and normal word (0: nothing there).
    zn: u32,
    // This frame's rays: `rv_slot` words per traced light.
    slots: vec2<u32>,
    // This frame's rest sample: its visibility (16 bits), 1 << 16 when this
    // block traced one, 1 << 17 when there is a rest, the rest's light
    // (log2, 8 bits) << 24; 0: no rest.
    rest: u32,
    // The history of the traced lights' visibility (by light; -1 none) and
    // of the rest (its estimate, its frames; frames 0: none).
    hist_slots: vec2<f32>,
    hist_rest: vec2<f32>,
}

// The blocks of the workgroup: this frame's rays, then their means over
// each block's 3 × 3 (the history's clamp).
var<workgroup> wg_rays: array<vec4<u32>, 64>;
var<workgroup> wg_means: array<vec3<f32>, 64>;
// The history's clamp: the means' range around the block, widened by this
// much (this frame's rays are few: their means still scatter).
const RV_CLAMP_MARGIN: f32 = 0.15;

// The mean of this frame's rays over the 3 × 3 blocks around (x, y) of the
// workgroup (those inside it that traced) for traced light `key` (0: the
// rest).
fn rv_mean(x: i32, y: i32, key: u32) -> f32 {
    var sum = 0.0;
    var n = 0.0;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let nx = x + dx;
            let ny = y + dy;
            if nx < 0 || nx > 7 || ny < 0 || ny > 7 { continue; }
            let r = wg_rays[u32(ny) * 8u + u32(nx)];
            if key == 0u {
                if (r.z & 0x10000u) != 0u {
                    sum += f32(r.z & 0xFFFFu) / 65535.0;
                    n += 1.0;
                }
            } else {
                if (r.x >> 16u) == key {
                    sum += rv_vis(r.x);
                    n += 1.0;
                }
                if (r.y >> 16u) == key {
                    sum += rv_vis(r.y);
                    n += 1.0;
                }
            }
        }
    }
    return select(-1.0, sum / n, n > 0.0);
}

// The range of the 3 × 3 means around (x, y) for light `key` (0: the rest):
// the history is held to it.
fn rv_range(x: i32, y: i32, key: u32) -> vec2<f32> {
    var lo = 2.0;
    var hi = -1.0;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let nx = x + dx;
            let ny = y + dy;
            if nx < 0 || nx > 7 || ny < 0 || ny > 7 { continue; }
            let i = u32(ny) * 8u + u32(nx);
            let r = wg_rays[i];
            let m = wg_means[i];
            var v = -1.0;
            if key == 0u {
                v = m.z;
            } else if (r.x >> 16u) == key {
                v = m.x;
            } else if (r.y >> 16u) == key {
                v = m.y;
            }
            if v >= 0.0 {
                lo = min(lo, v);
                hi = max(hi, v);
            }
        }
    }
    return vec2<f32>(lo - RV_CLAMP_MARGIN, hi + RV_CLAMP_MARGIN);
}

@compute @workgroup_size(8, 8)
fn cs_rt_vis(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let id = gid.xy + rv_first.xy;
    let b = rv_block(id);
    let x = i32(li % 8u);
    let y = i32(li / 8u);
    wg_rays[li] = vec4<u32>(b.slots, b.rest, 0u);
    workgroupBarrier();
    wg_means[li] = vec3<f32>(rv_mean(x, y, b.slots.x >> 16u), rv_mean(x, y, b.slots.y >> 16u), rv_mean(x, y, 0u));
    workgroupBarrier();
    if !b.store {
        return;
    }
    var out = vec4<u32>(0u, 0u, 0u, b.zn);
    // Each traced light's visibility: its own history (the block traced it
    // last time too) held to the range of this frame's means of that light
    // around the block (no trail of a moving shadow), then blended.
    for (var j = 0u; j < 2u; j++) {
        let s = b.slots[j];
        if s == 0u { continue; }
        let key = s >> 16u;
        // This frame's rays of that light over the 3 × 3 blocks (spatially
        // resolved first, then blended with the history: the blocks' disc
        // quarters mix here, not in the colour pass).
        let m = wg_means[li][j];
        var vis = select(rv_vis(s), m, m >= 0.0);
        let was = b.hist_slots[j];
        if was >= 0.0 {
            let r = rv_range(x, y, key);
            vis = mix(clamp(was, r.x, r.y), vis, rv.hist.w);
        }
        out[j] = (s & 0xFFFF8000u) | u32(round(clamp(vis, 0.0, 1.0) * 32767.0));
    }
    // The rest: its estimate's history, held likewise, averaged with this
    // frame's sample.
    if b.rest != 0u {
        // (The rest's samples over the 3 × 3 blocks, as the lights'.)
        let m = wg_means[li].z;
        let sampled = m >= 0.0;
        let s = m;
        let lw = f32(b.rest >> 24u);
        if b.hist_rest.y > 0.0 {
            let r = rv_range(x, y, 0u);
            let held = clamp(b.hist_rest.x, r.x, r.y);
            if sampled {
                let frames = min(b.hist_rest.y + 1.0, RV_REST_FRAMES);
                out.z = rv_rest(mix(held, s, max(1.0 / frames, rv.hist.y)), frames, lw);
            } else {
                out.z = rv_rest(held, b.hist_rest.y, lw);
            }
        } else if sampled {
            out.z = rv_rest(s, 1.0, lw);
        }
    }
    textureStore(vis_out, vec2<i32>(id), out);
}

// This frame's work at block `id`: its point (under one pixel of its 2 × 2,
// a different one each frame), face normal and cluster, its history, the
// two strongest lights' rays (the disc set's quarter turning each frame)
// and the rest's sample.
fn rv_block(id: vec2<u32>) -> RvBlock {
    var out: RvBlock;
    let fid = vec2<f32>(id);
    let left = rv_first.z == 0u && all(fid >= rv.skip.xy) && all(fid < rv.skip.zw);
    out.store = fid.x < rv.sizes.z && fid.y < rv.sizes.w && !left;
    out.hist_slots = vec2<f32>(-1.0);
    if !out.store { return out; }
    let f = u32(rv.hist.z);
    // (Its pixel, its disc quarter and its rest pick turn by the frame.
    // Updating half the blocks a frame, a checkerboard, was tried: in motion
    // the two halves' histories drift apart into a visible checker.)
    let g = f;
    // This frame's pixel of the block's 2 × 2 (of the grid), and the one the
    // record stands for (its first: the colour pass's `ml_vis_at`).
    let jit = vec2<f32>(f32(g & 1u), f32((g >> 1u) & 1u));
    let q = vec2<i32>(floor((vec2<f32>(id) * 2.0 + 0.5 + jit) * rv.misc.yz));
    let q0 = vec2<i32>(floor((vec2<f32>(id) * 2.0 + 0.5) * rv.misc.yz));
    let d = rv_depth(q);
    if d == rv.misc.x {
        return out;
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
    // The record's depth at the pixel it stands for (on this plane).
    let o = vec2<f32>(q0 - q);
    out.zn = rv_zn(z + dot(dx, fwd) * o.x + dot(dy, fwd) * o.y, ng);
    let P = ml_params();
    // Ceilings and undersides keep their fallback (as ml_relight).
    if P.rays == 0u || ng.z < -0.3 {
        return out;
    }
    // The block's history: where the last frame saw this point, the four
    // blocks around it there (bilinear: a nearest block's record taken
    // frame after frame under a slow motion repeats rows and columns), each
    // on the same surface (its view depth, its normal).
    var taps = array<vec4<u32>, 4>(vec4<u32>(0u), vec4<u32>(0u), vec4<u32>(0u), vec4<u32>(0u));
    var tw = vec4<f32>(0.0);
    if rv.hist.x > 0.5 {
        let pc = rv.prev * vec4<f32>(pr, 1.0);
        if pc.w > 0.05 {
            let uv = vec2<f32>(pc.x / pc.w * 0.5 + 0.5, 0.5 - pc.y / pc.w * 0.5);
            let fb = uv * rv.sizes.xy / rv.misc.yz * 0.5 - 0.5;
            let b0 = vec2<i32>(floor(fb));
            let fr = fb - floor(fb);
            // (The record's depth is its first pixel's: up to a block and a
            // half of slope from this point.)
            let tol = 0.02 * pc.w + 0.05 + 3.0 * (abs(dot(dx, fwd)) + abs(dot(dy, fwd))) * max(rv.misc.y, rv.misc.z);
            let e = rv_oct(ng);
            for (var t = 0; t < 4; t++) {
                let to = vec2<i32>(t & 1, t >> 1);
                let hb = b0 + to;
                if any(hb < vec2<i32>(0)) || any(vec2<f32>(hb) >= rv.sizes.zw) { continue; }
                let h = textureLoad(vis_prev, hb, 0);
                let de = unpack4x8snorm(h.w >> 16u).xy - e;
                if h.w == 0u || abs(unpack2x16float(h.w).x - pc.w) > tol || abs(de.x) + abs(de.y) >= 0.2 { continue; }
                taps[t] = h;
                tw[t] = select(1.0 - fr.x, fr.x, to.x == 1) * select(1.0 - fr.y, fr.y, to.y == 1) + 1e-4;
            }
        }
    }
    // The cluster (as ml_relight).
    let ndc = vec2<f32>((f32(q.x) + 0.5) / rv.sizes.x * 2.0 - 1.0, 1.0 - (f32(q.y) + 0.5) / rv.sizes.y * 2.0);
    let tiles = frame.cluster.xy;
    let tx = u32(clamp(floor((ndc.x * 0.5 + 0.5) * tiles.x), 0.0, tiles.x - 1.0));
    let ty = u32(clamp(floor((ndc.y * 0.5 + 0.5) * tiles.y), 0.0, tiles.y - 1.0));
    let slice = u32(clamp(floor(log(max(z, frame.cluster_near.x) / frame.cluster_near.x) * frame.cluster.w), 0.0, frame.cluster.z - 1.0));
    let c = (slice * u32(tiles.y) + ty) * u32(tiles.x) + tx;
    let first = clusters[c * 2u];
    let count = clusters[c * 2u + 1u];
    // The two lights that light p most (ml_relight's ranking, the face
    // normal for the shading one) and p's whole relit light (the share
    // floor); in the same loop the rest's pick: a weighted reservoir over
    // the lights of the rest's kind (emitters, not a car's) as they
    // leave the running top two or pass it by (each exactly once, the top
    // two's left-overs at the end), one uniform number rescaled at each
    // step (the reservoir does not depend on the order).
    var sel = vec2<u32>(0xffffffffu);
    var selw = vec2<f32>(0.02);
    var selr = vec2<f32>(0.0);
    var total = 0.0;
    g_rv_u = fract(rv_hash(id) + f32(g) * 0.618034);
    g_rv_sum = 0.0;
    g_rv_pick = 0xffffffffu;
    let n_lights = min(count, P.lights);
    for (var i = 0u; i < n_lights; i++) {
        let k = clusters[first + i];
        let w = rv_light(k, p, ng, P);
        total += w.x;
        if w.z > 0.5 && w.x > selw.y {
            let out_i = sel.y;
            let out_r = selr.y;
            if w.x > selw.x {
                sel = vec2<u32>(i, sel.x);
                selw = vec2<f32>(w.x, selw.x);
                selr = vec2<f32>(w.y, selr.x);
            } else {
                sel.y = i;
                selw.y = w.x;
                selr.y = w.y;
            }
            if out_i != 0xffffffffu {
                rv_stream(clusters[first + out_i], out_r);
            }
        } else {
            rv_stream(k, w.y);
        }
    }
    // (The disc set's quarter by the block and its update: over four updates
    // every block takes all four.)
    g_ml_pix = vec2<f32>(id) + jit;
    let floor_w = ML_RAY_SHARE * total;
    // One ray each an update: the history gathers the disc set's quarters
    // and the block's pixels.
    for (var j = 0u; j < 2u; j++) {
        let i = sel[j];
        if i == 0xffffffffu { continue; }
        let k = clusters[first + i];
        if j < min(P.rays, 2u) && selw[0] >= floor_w && selw[j] >= floor_w && k < 0xFFFFu {
            let l = lamp_list[k];
            let vis = ml_ray(p, ng, l.position.xyz, ml_light_radius(l), 0x03u, k, ML_RAY_SAMPLES_REST);
            out.slots[j] = rv_slot(k, selr[j] > 0.0, vis);
        } else {
            // (Not traced on its own: one of the rest.)
            rv_stream(k, selr[j]);
        }
    }
    out.hist_slots = rv_hist_slots(taps, tw, out.slots.x >> 16u, out.slots.y >> 16u);
    // The rest: the picked light's ray.
    let rest_w = g_rv_sum;
    if rest_w > 0.02 * total && rest_w > 1e-6 {
        let lw = clamp(round(log2(rest_w) * 8.0 + 128.0), 0.0, 255.0);
        out.hist_rest = rv_hist_rest(taps, tw, lw);
        out.rest = 0x20000u | (u32(lw) << 24u);
        let pick = g_rv_pick;
        if pick < 0xFFFFu {
            let l = lamp_list[pick];
            let s = rv_ray_centre(p, ng, l.position.xyz, ml_light_radius(l));
            out.rest |= u32(round(s * 65535.0)) | 0x10000u;
        }
    }
    return out;
}

// The history's visibility of the lights `a`, `b` (light + 1; 0 none):
// over the taps (weights `tw`, 0 not on this surface) that traced them.
fn rv_hist_slots(taps: array<vec4<u32>, 4>, tw: vec4<f32>, a: u32, b: u32) -> vec2<f32> {
    var sw = vec2<f32>(0.0);
    var sv = vec2<f32>(0.0);
    for (var t = 0; t < 4; t++) {
        let h = taps[t];
        let w = tw[t];
        if w <= 0.0 { continue; }
        for (var j = 0; j < 2; j++) {
            let key = select(b, a, j == 0);
            if key == 0u { continue; }
            if (h.x >> 16u) == key {
                sw[j] += w;
                sv[j] += w * rv_vis(h.x);
            } else if (h.y >> 16u) == key {
                sw[j] += w;
                sv[j] += w * rv_vis(h.y);
            }
        }
    }
    return select(vec2<f32>(-1.0), sv / max(sw, vec2<f32>(1e-6)), sw > vec2<f32>(0.0));
}

// The history's rest (its estimate, its frames; frames 0: none) over the
// taps whose rest's light (log2 `lw`) is within 2× of this one's.
fn rv_hist_rest(taps: array<vec4<u32>, 4>, tw: vec4<f32>, lw: f32) -> vec2<f32> {
    var rw = 0.0;
    var rs = 0.0;
    var rf = 255.0;
    for (var t = 0; t < 4; t++) {
        let h = taps[t];
        let hf = f32((h.z >> 16u) & 0xFFu);
        if tw[t] <= 0.0 || hf <= 0.0 || abs(f32(h.z >> 24u) - lw) > 8.0 { continue; }
        rw += tw[t];
        rs += tw[t] * f32(h.z & 0xFFFFu) / 65535.0;
        rf = min(rf, hf);
    }
    return select(vec2<f32>(0.0), vec2<f32>(rs / max(rw, 1e-6), rf), rw > 0.0);
}

// One ray from p (face normal ng) toward an emitter's centre `l`, stopping
// short of its body (`radius`) as `ml_ray` does: 1 when nothing hides it.
// To the centre, not to points of a disc around it: an emitter sits on the
// wall or sign that carries it, and a disc across the line to it reaches
// behind that surface (the whole road read as shadowed from a facade's
// neon). The softness comes from the many emitters and the history.
fn rv_ray_centre(p: vec3<f32>, ng: vec3<f32>, l: vec3<f32>, radius: f32) -> f32 {
    let d0 = normalize(l - p);
    let o = p + ng * (0.08 + 0.3 * (1.0 - max(dot(ng, d0), 0.0)));
    let to = l - o;
    let len = length(to);
    let stop = 0.6 + radius;
    if len <= stop + 0.7 { return 1.0; }
    let tmin = mix(0.6, 0.15, smoothstep(0.3, 0.7, ng.z));
    return ml_ray_one(o, to / len, tmin, max(len - stop, tmin + 0.01), 0x03u);
}

// The rest's reservoir (`rv_block`): its uniform number, the weight it
// has seen and its pick.
var<private> g_rv_u: f32;
var<private> g_rv_sum: f32;
var<private> g_rv_pick: u32;
// Light `k` of weight `w` (0: not of the rest) into the reservoir: taken
// with the probability of its share of the weight seen so far, the uniform
// number rescaled to the part of [0, 1) it fell in.
fn rv_stream(k: u32, w: f32) {
    if w <= 0.0 { return; }
    g_rv_sum += w;
    let q = w / g_rv_sum;
    if g_rv_u < q {
        g_rv_pick = k;
        g_rv_u = g_rv_u / q;
    } else {
        g_rv_u = (g_rv_u - q) / max(1.0 - q, 1e-6);
    }
}

// Light `k`'s weight at p (face normal ng): x its relit light (0 where it
// does not reach p), y the same when it belongs to the rest (an emitter:
// neon, a sign; no shadow layer, not a car's light), z 1 when it may be
// traced (not a car's).
fn rv_light(k: u32, p: vec3<f32>, ng: vec3<f32>, P: MlParams) -> vec3<f32> {
    let l = lamp_list[k];
    let colour = l.shadow.yzw;
    if all(colour <= vec3<f32>(0.0)) { return vec3<f32>(0.0); }
    let pack = l.colour.w >= 0.0;
    if !pack && P.emit <= 0.0 { return vec3<f32>(0.0); }
    let rel = l.position.xyz - p;
    let d2 = dot(rel, rel);
    let dir = rel * inverseSqrt(max(d2, 1e-12));
    let bn = ml_now(l, rel, dir, ng, ng, P.soft, false);
    if max(bn.x, bn.y) < ML_TAIL { return vec3<f32>(0.0); }
    let w = bn.y * dot(colour, vec3<f32>(0.33));
    let may = l.colour.w > -1.5;
    return vec3<f32>(w, select(0.0, w, may && l.shadow.x < 0.0 && !pack), select(0.0, 1.0, may));
}

// ---- march ----

// The renderer's own look, fixed when its pipelines are made (`VolumeFx::new`): a branch its look never takes is
// left out of its march altogether (fewer registers, more pixels in flight); the values the branches read are
// the uniform's as before. FINE: `Look::fine` > 0; DIRECTIONAL: `Look::directional` > 0; SHADOW: `Look::shadow` > 0.
override FINE: bool = true;
override DIRECTIONAL: bool = true;
override SHADOW: bool = true;

@group(0) @binding(0) var<uniform> mv: Vol;
@group(0) @binding(1) var mdens: texture_3d<f32>;
@group(0) @binding(2) var mlight: texture_3d<f32>;
@group(0) @binding(3) var msamp: sampler;
@group(0) @binding(4) var mnoise: texture_3d<f32>;
@group(0) @binding(5) var mnoise_samp: sampler;
@group(0) @binding(6) var<storage, read> mbricks: array<u32>;
// the scene's depth (DEPTH_TEXTURE: texture_depth_2d or its multisampled
// form, sample 0)
@group(0) @binding(7) var scene_depth_m: DEPTH_TEXTURE;
@group(0) @binding(8) var<storage, read> mbounds: array<u32>;
// 16 × 16 low-res pixel tiles that hold any of it (the composite draws
// only those)
@group(0) @binding(9) var<storage, read_write> mtiles: array<atomic<u32>>;
// The scene's block averages (volume_fx_ambient.wgsl), then their mean
@group(0) @binding(10) var<storage, read> mamb: array<vec4<f32>>;
// The light volume's dominant light direction (cs_light)
@group(0) @binding(11) var mldir: texture_3d<f32>;

// The scene ambient's shares: the blocks round the pixel, the road
// under it (the rest: the view's mean).
const SA_NEAR: f32 = 0.5;
const SA_ROAD: f32 = 0.25;
const AW: i32 = 32;
const AH: i32 = 18;

fn amb_block(c: vec2<i32>) -> vec3<f32> {
    let q = clamp(c, vec2(0), vec2(AW - 1, AH - 1));
    return mamb[q.y * AW + q.x].rgb;
}

// The scene's light around a target position `uv` (0..1): its blocks
// bilinearly.
fn amb_at(uv: vec2<f32>) -> vec3<f32> {
    let g = clamp(uv, vec2(0.0), vec2(1.0)) * vec2(f32(AW), f32(AH)) - 0.5;
    let i = vec2<i32>(floor(g));
    let f = g - floor(g);
    let a = mix(amb_block(i), amb_block(i + vec2(1, 0)), f.x);
    let b = mix(amb_block(i + vec2(0, 1)), amb_block(i + vec2(1, 1)), f.x);
    return mix(a, b, f.y);
}

// The ambient light on the medium seen through low-res pixel `px`:
// the scene drawn around it (what lies behind it and the road under it,
// and the whole view's mean for the light from beside and behind the
// camera) × the gain — dark in a dark street, warm under sodium lamps;
// without a readable target the flat ambient.
fn scene_ambient(px: vec2<f32>) -> vec3<f32> {
    if (mv.look5.w < 0.5) {
        return mv.ambient.rgb;
    }
    let uv = px * mv.depth.w / mv.sizes.zw;
    let mean = mamb[AW * AH].rgb;
    // (Wide: the light round the cloud, not the pattern behind each pixel
    // — smoke in front of a dark doorway is not itself dark.)
    var near = vec3(0.0);
    for (var k = 0; k < 4; k++) {
        let o = vec2(select(-0.09, 0.09, (k & 1) == 1), select(-0.07, 0.07, k >= 2));
        near += amb_at(uv + o);
    }
    // (The light round the cloud weighs more than the whole view's
    // mean — 0.35 / 0.2 / 0.45 weights once lifted smoke in a dark corner to
    // the bright lamps' and sky's level: milky blacks.)
    let c = SA_NEAR * near * 0.25 + SA_ROAD * amb_at(uv + vec2(0.0, 0.15)) + (1.0 - SA_NEAR - SA_ROAD) * mean;
    // (Light gathered from all round is less saturated than any one
    // surface's: the purple sky or a yellow wall tints, it does not dye.)
    return mv.look5.x * mix(vec3(dot(c, vec3(0.3, 0.59, 0.11))), c, 0.6);
}

// The native fog's factor (world.wgsl: 1 = none) at view depth z.
fn fog_keep(z: f32) -> f32 {
    if (mv.fog_value.w < 0.5) {
        return 1.0;
    }
    return clamp((mv.fog_value.x - z) * mv.fog_value.y, mv.fog_value.z, 1.0);
}

// The medium's shadow on the surface a low-res pixel sees (view
// depth zs): its optical depth straight up over 3.5 m (the lamps and the
// sky are overhead at night), as the light left: 1 none.
fn surface_shadow(px: vec2<f32>, zs: f32) -> f32 {
    if (!SHADOW || mv.look5.z <= 0.0) {
        return 1.0;
    }
    let ndc = vec2((px.x - mv.viewport.x) / mv.viewport.z * 2.0 - 1.0, 1.0 - (px.y - mv.viewport.y) / mv.viewport.w * 2.0);
    let h = mv.inv_vp * vec4(ndc, 0.5, 1.0);
    let dir = normalize(h.xyz / h.w);
    let p = dir * (zs / max(dot(dir, mv.forward.xyz), 1e-3));
    let bmin = mv.origin.xyz - mv.eye.xyz;
    let ext = mv.origin.w * vec3<f32>(mv.dims.xyz);
    var tau = 0.0;
    for (var i = 0; i < 4; i++) {
        let q = p + vec3(0.0, 0.0, 0.15 + 0.85 * f32(i) + 0.4);
        let uvw = (q - bmin) / ext;
        // (Empty bricks hold zeros: no sample.)
        if (all(uvw > vec3(0.0)) && all(uvw < vec3(1.0)) && mbrick_at((q - bmin) / mv.origin.w) != 0u) {
            tau += textureSampleLevel(mdens, msamp, uvw, 0.0).r * 0.85;
        }
    }
    return 1.0 - mv.look5.z * (1.0 - exp(-0.8 * tau));
}

const TILE: u32 = 16u;

@vertex
fn vs_march(@builtin(vertex_index) vi: u32) -> FsIn {
    var o: FsIn;
    let lo = vec3(f32(mbounds[12]), f32(mbounds[13]), f32(mbounds[14]));
    let hi = vec3(f32(mbounds[15]), f32(mbounds[16]), f32(mbounds[17]));
    o.pos = rect_vertex(vi, mv, lo, hi, mv.sizes.xy, 1.0, 3.0);
    return o;
}

fn mbrick_at(g: vec3<f32>) -> u32 {
    let nb = vec3<i32>(mv.dims.xyz) / i32(BRICK);
    let b = vec3<i32>(floor(g / f32(BRICK)));
    if (any(b < vec3(0)) || any(b >= nb)) {
        return 0u;
    }
    return mbricks[u32((b.z * nb.y + b.y) * nb.x + b.x)];
}

// The density texel `d` at world `wp` eroded by the detail: two octaves
// drifting with the wind and rising, the second warped by the first.
// Thin parts are eroded hardest (round billows at the edge, an opaque
// core), old ones more (they break up), thin (wet) ones a little more.
// `fine` the third octave's weight (0: two octaves, as before): a
// finer, curl-warped octave (~0.35 m) that only the samples near the eye
// get (the caller fades it by the march's step, so it never aliases).
fn eroded(d: vec4<f32>, wp: vec3<f32>, time: f32, fine: f32) -> f32 {
    let q = (wp - mv.wind.xyz * time * 0.8 - vec3(0.0, 0.0, 0.35 * time)) * mv.look2.y;
    let n1 = textureSampleLevel(mnoise, mnoise_samp, q, 0.0);
    let n2s = textureSampleLevel(mnoise, mnoise_samp, q * 2.7 + (n1.gba - 0.5) * 0.25 + vec3(0.0, 0.0, time * 0.06), 0.0);
    var n = n1.r * 0.6 + n2s.r * 0.4;
    if (FINE && fine > 0.0) {
        // (Warped by both coarser octaves' gradients: the fine billows
        // curl round the larger ones instead of a fixed texture.)
        let n3 = textureSampleLevel(mnoise, mnoise_samp, q * 4.6 + (n1.gba + n2s.gba - 1.0) * 0.3 + vec3(0.0, 0.0, time * 0.12), 0.0).r;
        n = mix(n, n * 0.78 + n3 * 0.22, fine);
    }
    let cov = 1.0 - exp(-d.r * 0.9);
    let e = mv.look2.x * (0.15 + 0.85 * (1.0 - cov)) * (0.75 + 0.5 * d.b) * (1.0 + 0.3 * d.g);
    // The edge's contrast about its middle (a crisper billow outline, the
    // same mass on average).
    let x = clamp((n - e) / max(1.0 - e, 0.05), 0.0, 1.0);
    let edge = clamp((x - 0.5) * mv.look4.x + 0.5, 0.0, 1.0);
    return d.r * edge * (1.0 + 0.6 * e);
}

// The ray march: from the eye (or the grid's entry) to the scene's depth
// (or the grid's exit), skipping empty bricks, energy-conserving
// integration (Hillaire 2015) of the eroded density lit from the light
// volume and shaded per billow (the eroded density a step up). Out:
// premultiplied colour and coverage.
struct MarchOut {
    @location(0) colour: vec4<f32>,
    // the nearest view depth over the pixel's block (the composite's
    // bilateral weights)
    @location(1) depth: vec4<f32>,
}

fn mlin(d: f32) -> f32 {
    let n = mv.depth.x;
    let fa = mv.depth.y;
    if (mv.depth.z > 0.5) {
        return n * fa / (n + d * (fa - n));
    }
    return n * fa / (fa - d * (fa - n));
}

fn mdepth(p: vec2<i32>) -> f32 {
    return mlin(textureLoad(scene_depth_m, clamp(p, vec2(0), vec2<i32>(mv.sizes.zw) - 1), 0));
}

@fragment
fn fs_march(in: FsIn) -> MarchOut {
    var o: MarchOut;
    // The block's nearest view depth (its four corners).
    let div = i32(mv.depth.w);
    let base = vec2<i32>(in.pos.xy) * div;
    let e = div - 1;
    let zs = min(min(mdepth(base), mdepth(base + vec2(e, 0))), min(mdepth(base + vec2(0, e)), mdepth(base + vec2(e, e))));
    let sh = surface_shadow(in.pos.xy, zs);
    let m = march(in.pos.xy, zs);
    o.colour = m.colour;
    // (b: the medium's mean distance along the ray, the temporal
    // resolve's reprojection depth.)
    o.depth = vec4(zs, 1.0 - sh, m.dist, 0.0);
    if (o.colour.a > 0.001 || sh < 0.995) {
        let t = vec2<u32>(in.pos.xy) / TILE;
        atomicStore(&mtiles[t.y * ((u32(mv.sizes.x) + TILE - 1u) / TILE) + t.x], 1u);
    }
    return o;
}

struct Marched {
    colour: vec4<f32>,
    // the medium's contribution-weighted distance along the ray (m)
    dist: f32,
}

fn march(px: vec2<f32>, zs: f32) -> Marched {
    var o: Marched;
    o.colour = vec4(0.0);
    o.dist = zs;
    let ndc = vec2((px.x - mv.viewport.x) / mv.viewport.z * 2.0 - 1.0, 1.0 - (px.y - mv.viewport.y) / mv.viewport.w * 2.0);
    let h = mv.inv_vp * vec4(ndc, 0.5, 1.0);
    let dir = normalize(h.xyz / h.w);
    let cosf = max(dot(dir, mv.forward.xyz), 1e-3);
    var t1 = min(zs / cosf, mv.forward.w);
    let vs = mv.origin.w;
    let bmin = mv.origin.xyz - mv.eye.xyz;
    // The box of the occupied bricks (none: nothing to march).
    let bs = f32(BRICK) * vs;
    let lo_b = vec3(f32(mbounds[12]), f32(mbounds[13]), f32(mbounds[14]));
    let hi_b = vec3(f32(mbounds[15]), f32(mbounds[16]), f32(mbounds[17]));
    if (any(lo_b > hi_b)) {
        return o;
    }
    let inv = 1.0 / select(dir, vec3(1e-6), abs(dir) < vec3(1e-6));
    let ta = (bmin + lo_b * bs) * inv;
    let tb = (bmin + (hi_b + 1.0) * bs) * inv;
    let tmin = max(max(min(ta.x, tb.x), min(ta.y, tb.y)), min(ta.z, tb.z));
    let tmax = min(min(max(ta.x, tb.x), max(ta.y, tb.y)), max(ta.z, tb.z));
    var t = max(tmin, 0.3);
    t1 = min(t1, tmax);
    if (t >= t1) {
        return o;
    }
    // Interleaved gradient noise (Jimenez 2014): the start jittered within
    // a step (a fixed pattern: stable, no shimmer). With the
    // temporal resolve the pattern turns by the golden ratio each frame
    // (its history integrates the steps; Hillaire 2015 / Schneider 2015).
    var ign = fract(52.9829189 * fract(dot(px, vec2(0.06711056, 0.00583715))));
    if (mv.temporal.z > 0.5) {
        ign = fract(ign + mv.temporal.y * 0.618034);
    }
    var wsum = 0.0;
    var dsum = 0.0;
    let time = mv.eye.w;
    let dims = vec3<f32>(mv.dims.xyz);
    var trans = 1.0;
    var rgb = vec3(0.0);
    var first = true;
    var maxs = 0.0;
    var steps = 0;
    let max_steps = i32(mv.wind.w);
    // (The ambient once a pixel, at its first lit sample.)
    var amb = vec3(0.0);
    var amb_done = false;
    for (var i = 0; i < max_steps; i++) {
        steps = i;
        if (t >= t1 || trans < 0.02) {
            break;
        }
        // (× the step scale, look4.w: longer steps where the temporal
        // resolve integrates their turning offsets.)
        let ss = select(1.0, mv.look4.w, mv.look4.w > 0.0);
        let dt = clamp(0.02 * t, 0.2, 0.8) * ss * select(1.0, mv.look6.x, mv.look6.x > 0.0);
        let p = dir * t;
        let g = (p - bmin) / vs;
        if (mbrick_at(g) == 0u) {
            let bs = f32(BRICK);
            let lo = floor(g / bs) * bs * vs + bmin;
            let hi = lo + bs * vs;
            let a = (lo - p) * inv;
            let b = (hi - p) * inv;
            let exit = min(min(max(a.x, b.x), max(a.y, b.y)), max(a.z, b.z));
            t += max(exit, 0.0) + 0.02;
            first = true;
            continue;
        }
        if (first) {
            // (look6.y > 0 sets the jitter's share to look6.y − 1,
            // `VolumeFx::set_jitter`; 0 keeps it whole.)
            t += dt * select(ign, (ign - 0.5) * (mv.look6.y - 1.0) + 0.5, mv.look6.y > 0.0);
            first = false;
        }
        let ps = dir * t;
        let uvw = (ps - bmin) / (vs * dims);
        let d = textureSampleLevel(mdens, msamp, uvw, 0.0);
        maxs = max(maxs, d.r);
        if (d.r > 1e-3) {
            // (Faded in near the eye.)
            // (The fine octave where the step resolves it: full at
            // 0.2 m steps, gone from 0.4 m — about 20 m from the eye.)
            // (Without a history to integrate it — a still, the first frame
            // after a cut — a third of it: one frame's samples would grain.)
            let fine = mv.look4.y * clamp((0.4 - dt / ss) / 0.2, 0.0, 1.0) * select(0.3, 1.0, mv.prev_eye.w > 0.5);
            let sigma = eroded(d, ps + mv.eye.xyz, time, fine) * smoothstep(0.3, max(mv.look5.y, 0.31), t);
            if (sigma > 1e-4) {
                let l = textureSampleLevel(mlight, msamp, uvw, 0.0);
                // The billow shading's direction: straight up (the
                // sky), turned toward the dominant direct light by
                // how much of this sample's light is direct and one-sided.
                var sdir = vec3(0.0, 0.0, 1.0);
                if (DIRECTIONAL && mv.look4.z > 0.0) {
                    if (!amb_done) {
                        amb = scene_ambient(px);
                        amb_done = true;
                    }
                    let ld = textureSampleLevel(mldir, msamp, uvw, 0.0).xyz;
                    let dl = dot(l.rgb, vec3(0.3, 0.59, 0.11));
                    let share = dl / max(dl + l.a * dot(amb, vec3(0.3, 0.59, 0.11)), 1e-4);
                    let k = mv.look4.z * clamp(length(ld) * share * 1.5, 0.0, 1.0);
                    sdir = normalize(mix(sdir, ld / max(length(ld), 1e-3), k) + vec3(0.0, 0.0, 1e-3));
                }
                let pu = ps + sdir * 0.6;
                let du = textureSampleLevel(mdens, msamp, (pu - bmin) / (vs * dims), 0.0);
                let su = eroded(du, pu + mv.eye.xyz, time, 0.0);
                let shade = 1.0 - mv.look3.x * (1.0 - exp(-su * 0.9));
                // The direct light shaded per billow, the ambient
                // half as much (an underside still sees the lit road),
                // then the native fog at the sample's depth.
                if (!amb_done) {
                    amb = scene_ambient(px);
                    amb_done = true;
                }
                let lv = l.rgb * shade + l.a * amb * (0.5 + 0.5 * shade);
                let f = fog_keep(t * cosf);
                let st = exp(-sigma * dt);
                let wgt = trans * (1.0 - st);
                rgb += wgt * (lv * f + mv.fog_color.rgb * (1.0 - f));
                wsum += wgt;
                dsum += wgt * t;
                trans *= st;
            }
        }
        t += dt;
    }
    let a = 1.0 - trans;
    if (wsum > 1e-5) {
        o.dist = dsum / wsum;
    }
    if (mv.look2.w == 1.0) {
        o.colour = vec4(a, maxs * 0.1, f32(steps) / f32(max_steps), 1.0);
        return o;
    }
    o.colour = vec4(rgb, a);
    return o;
}


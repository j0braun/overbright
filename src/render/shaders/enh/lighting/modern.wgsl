// Modern Lighting (phase 1 and the stronger phase 2, clone deviation, opt-in:
// the Graphics → Lighting page's Lighting row, or the hidden switch
// NFSU2_MODERN_LIGHTING; docs/modern-lighting.md, render/modern_light.rs,
// render/modern_light_bold.rs): frame.lamps.x ≥ 2. The street lamps' share
// of the bake is modelled per pixel as physical point lights fitted to the
// bake (`ml_pool`: inverse square × cosine with a softening, windowed to
// the authored outer radius; the colour per lamp in shadow.yzw = the
// authored colour × the fitted channel gains, 0 for a lamp outside the
// district). That share is taken out of the baked light and put back
// relit: through the shading normal, shadowed by the lamp's depth layer
// and the cars, with a specular lobe. Phase 2 (`MlParams`, the strength):
// the rest of the bake × the ambient ("painted darkness"), the relit
// pools sharper, brighter and less bound to the bake, plus the neon /
// signs (colour.w = −1) and the cars' lights (colour.w = −2, shadow.x =
// −2 − the owner car) as lights of their own, and with Modern + RT ray
// queries for the strongest lights' shadows. Strength 0 is phase 1.
// BEGIN RTVIS A
const ML_POOL_REF: f32 = 9.0;
const ML_POOL_SOFT: f32 = 150.0;
// At most this share of the bake (per channel) is taken as lamp light.
const ML_SHARE_MAX: f32 = 0.85;
// How much of a lamp's share a full shadow takes (the rest: its bounce).
const ML_SHADOW_DEPTH: f32 = 0.85;
// The cars' depth in the lamps' light (soft ellipsoids).
const ML_CAR_DEPTH: f32 = 0.75;
// The lamps' specular (output units per vertex-colour unit) and the
// roughness of a surface without Surface Detail's material (dry, wet).
const ML_SPEC: f32 = 0.5;
const ML_ROUGH_DRY: f32 = 0.55;
const ML_ROUGH_WET: f32 = 0.22;
const ML_TAIL: f32 = 0.01;
// Look rule: the baked luminance (vertex-colour units, 0.5 = the texture at 1x)
// from which the strength's darkening fades out (darken only where the original is dark).
const ML_KEEP_LO: f32 = 0.06;
const ML_KEEP_HI: f32 = 0.22;
// Phase 2: a dry road's roughness at full strength.
const ML_ROUGH_BOLD: f32 = 0.38;
// The params record (`modern_light_bold::PARAMS_INDEX`: after the lamps
// and the beams).
const ML_PARAMS: u32 = 1040u;
// The tier in force (1 Low, 2 Modern, 3 Modern + RT; 0 unknown: the
// params record decides; it holds the same tier wherever ml_relight runs)
// and whether the debug view can be on (NFSU2_MODERN_LIGHTING_DEBUG).
override D9_ML_TIER: u32 = 0u;
override D9_ML_DEBUG: bool = true;
struct MlParams {
    s: f32,
    ambient: f32,
    boost: f32,
    soft: f32,
    tier: f32,
    emit: f32,
    spec: f32,
    uncap: f32,
    shadowed: u32,
    rays: u32,
    lights: u32,
};
fn ml_params() -> MlParams {
    let r = lamp_list[ML_PARAMS];
    return MlParams(r.position.x, r.position.y, r.position.z, r.position.w, r.colour.x, r.colour.y, r.colour.z, r.colour.w, u32(r.direction.x), u32(r.direction.y), u32(r.direction.w));
}
fn ml_pool_soft(rel: vec3<f32>, n: vec3<f32>, outer: f32, soft: f32) -> f32 {
    let d2 = dot(rel, rel);
    if outer <= 0.0 || d2 >= outer * outer { return 0.0; }
    let d = max(sqrt(d2), 1e-3);
    let c = max(dot(n, rel) / d, 0.0);
    let x = d2 / (outer * outer);
    let w = max(1.0 - x * x, 0.0);
    return (ML_POOL_REF * ML_POOL_REF + soft) * c / (d2 + max(soft, 1.0)) * w * w;
}
fn ml_pool(rel: vec3<f32>, n: vec3<f32>, outer: f32) -> f32 {
    return ml_pool_soft(rel, n, outer, ML_POOL_SOFT);
}
// A light of its own (neon, a car's light): 1 at its source, the inverse
// square softened by (outer / 4)² (`modern_light_bold::light_pool`).
fn ml_near_pool(rel: vec3<f32>, n: vec3<f32>, outer: f32) -> f32 {
    let d2 = dot(rel, rel);
    if outer <= 0.0 || d2 >= outer * outer { return 0.0; }
    let d = max(sqrt(d2), 1e-3);
    let c = max(dot(n, rel) / d, 0.0);
    let s = outer * outer * 0.0625;
    let x = d2 / (outer * outer);
    let w = max(1.0 - x * x, 0.0);
    // Not on the surface it sits on (a sign's own facade): faded in from
    // ML_NEAR_FADE metres.
    return s / (d2 + s) * c * w * w * smoothstep(ML_NEAR_FADE.x, ML_NEAR_FADE.y, d);
}
const ML_NEAR_FADE: vec2<f32> = vec2<f32>(1.0, 3.5);
// The other lights' most (vertex-colour units, per channel) and the
// baked light level above which they add less (the neon tints the dark:
// a lit facade does not flood).
const ML_EXTRA_MAX: f32 = 0.45;
// A sign's light on walls (vs the ground's 1).
const ML_EMIT_WALL: f32 = 0.15;
const ML_EXTRA_PEAK: f32 = 0.8;
// Against harsh shadows and a flickering casino canopy:
// Modern + RT's rays per pixel for the strongest traced light and for the
// others (area lights, `ml_ray`; × 4 over the quad), the golden
// angle of their disc set, a street lamp's radius (its head) and a sign's
// (from its reach, `modern_light_bold::emitter_light`: reach 7 + 3 × the
// side of its area), and the least of a pixel's unshadowed light a shadow
// leaves (the bounce and the painted fill: nothing goes black).
const ML_RAY_SAMPLES: u32 = 2u;
const ML_RAY_SAMPLES_REST: u32 = 1u;
const ML_GOLDEN: f32 = 2.3999632;
const ML_LAMP_RADIUS: f32 = 0.35;
const ML_UMBRA_FLOOR: f32 = 0.4;
const ML_RAY_SHARE: f32 = 0.25;
// END RTVIS A
// The pixel (render target units) for the rays' interleaving.
var<private> g_ml_pix: vec2<f32>;
// Against harsh lines on the flag girl: the NIS characters'
// vertex colours are the clone's half-Lambert (`character::shade`), not a
// lamp bake, and the world's pixels have only the face normal: on a
// skinned body the relight followed every triangle. Characters take no
// lamp share out, the lights' pool without the face's cosine (× the
// hemisphere's mean), their shadows, no specular.
const ML_CHARACTER_MARK: f32 = 0.25;
const ML_CHARACTER_WRAP: f32 = 0.5;
var<private> g_ml_char: bool;
// The share of its unshadowed light a pixel keeps (the contact shadow's
// darker-of, `lamp_shade`).
var<private> g_ml_ratio: f32;
// The lights of this pixel the traced blocks shadow (/ 4; the debug view).
var<private> g_ml_traced: f32;
// The mean over the pixel's 2×2 quad (fine derivatives: the quad's pairs'
// differences), so the quad's interleaved ray sets (`ml_ray`) make one
// smooth penumbra per pixel: no dither, nothing over time.
fn ml_quad_mean(v: vec3<f32>) -> vec3<f32> {
    let px = select(-0.5, 0.5, (u32(g_ml_pix.x) & 1u) == 0u);
    let h = v + dpdxFine(v) * px;
    let py = select(-0.5, 0.5, (u32(g_ml_pix.y) & 1u) == 0u);
    return h + dpdyFine(h) * py;
}
// BEGIN RTVIS B
fn ml_light_radius(l: Lamp) -> f32 {
    if l.colour.w >= 0.0 { return ML_LAMP_RADIUS; }
    return clamp((l.position.w - 7.0) / 6.0, 0.3, 2.5);
}
// END RTVIS B
// A lamp layer's visibility (soft edges whatever Lamp Shadows' own
// quality; the Low tier one compare).
fn ml_shadow(layer: i32, p: vec3<f32>, n: vec3<f32>, taps: u32) -> f32 {
    let v = shadow_views[layer];
    let dist = length(p - v.eye.xyz);
    let q = p + n * (v.eye.w * (1.0 + dist * 0.02)) + (v.eye.xyz - p) / max(dist, 1e-3) * (v.eye.w + dist * 0.01);
    let c = v.view_proj * vec4<f32>(q, 1.0);
    if c.w <= 0.0 { return 1.0; }
    let ndc = c.xyz / c.w;
    if any(abs(ndc.xy) > vec2<f32>(1.0)) || ndc.z > 1.0 || ndc.z < 0.0 { return 1.0; }
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let texel = 1.0 / f32(textureDimensions(shadow_maps).x);
    let edge = 1.0 - smoothstep(0.8, 1.0, max(abs(ndc.x), abs(ndc.y)));
    if taps < 4u {
        return mix(1.0, textureSampleCompareLevel(shadow_maps, shadow_samp, uv, layer, ndc.z), edge);
    }
    // Four bilinear compares 1.5 texels off the centre (a soft 4×4 texel
    // footprint at a third of a 3×3 grid's cost).
    let o = texel * 1.5;
    var sum = textureSampleCompareLevel(shadow_maps, shadow_samp, uv + vec2<f32>(-o, -o), layer, ndc.z);
    sum += textureSampleCompareLevel(shadow_maps, shadow_samp, uv + vec2<f32>(o, -o), layer, ndc.z);
    sum += textureSampleCompareLevel(shadow_maps, shadow_samp, uv + vec2<f32>(-o, o), layer, ndc.z);
    sum += textureSampleCompareLevel(shadow_maps, shadow_samp, uv + vec2<f32>(o, o), layer, ndc.z);
    return mix(1.0, sum * 0.25, edge);
}
// The cars near p (within 12 m: past that a car shades p from no lamp),
// gathered once per pixel; ml_cars then tests only these.
const ML_NEAR_CARS: u32 = 4u;
// frame.lamps.x = 2 + the cars Modern Lighting considers (the nearest to
// the eye, `modern_light::frame_x`), + 64 for the debug view.
fn ml_car_count() -> u32 {
    let x = frame.lamps.x - 2.0;
    return u32(x - 64.0 * floor(x / 64.0));
}
// The near cars' indices, a byte each (no indexed private array: those
// spill to scratch memory on some GPUs).
var<private> g_ml_near: u32;
var<private> g_ml_count: u32;
fn ml_gather_cars(p: vec3<f32>, most: u32) {
    g_ml_count = 0u;
    g_ml_near = 0u;
    let count = ml_car_count();
    for (var i = 0u; i < count && g_ml_count < most; i++) {
        let rel = p - occluders[i].centre.xyz;
        if dot(rel, rel) <= 144.0 {
            g_ml_near |= i << (8u * g_ml_count);
            g_ml_count += 1u;
        }
    }
}
// The near cars between p and a light at l (as car_visibility, at
// ML_CAR_DEPTH), but car `skip` (the light's own) and the cars the
// light's cluster entry leaves out (`cars`, lamps::car_masks: they give
// exactly 1 here).
fn ml_cars(p: vec3<f32>, l: vec3<f32>, skip: u32, cars: u32) -> f32 {
    var vis = 1.0;
    for (var j = 0u; j < g_ml_count; j++) {
        let k = (g_ml_near >> (8u * j)) & 0xFFu;
        if k == skip || (cars & (1u << k)) == 0u { continue; }
        let o = occluders[k];
        let rel0 = p - o.centre.xyz;
        let rel1 = l - o.centre.xyz;
        let s = vec3<f32>(o.x.w, o.y.w, o.z.w) * 1.15;
        let a = vec3<f32>(dot(rel0, o.x.xyz), dot(rel0, o.y.xyz), dot(rel0, o.z.xyz)) / s;
        let b = vec3<f32>(dot(rel1, o.x.xyz), dot(rel1, o.y.xyz), dot(rel1, o.z.xyz)) / s;
        if dot(a, a) < 1.0 { continue; }
        let d = b - a;
        let t = clamp(-dot(a, d) / max(dot(d, d), 1e-6), 0.0, 1.0);
        vis *= mix(1.0 - ML_CAR_DEPTH, 1.0, smoothstep(0.7, 1.3, length(a + d * t)));
    }
    return vis;
}
#if MLRT
// BEGIN MLRT
#include "enh/lighting/mlrt.wgsl"
// END MLRT
#endif
// A light's pool at p: (the bake's modelled share, the relit light) —
// `ml_relight`'s per light, also its rays' ranking.
// BEGIN RTVIS D
fn ml_now(l: Lamp, rel: vec3<f32>, dir: vec3<f32>, ng: vec3<f32>, n: vec3<f32>, soft: f32, low: bool) -> vec2<f32> {
    let pack = l.colour.w >= 0.0;
    var baked = 0.0;
    var now = 0.0;
    if pack {
        baked = ml_pool(rel, ng, l.position.w);
        // Low: the fitted pool relit as it is (one evaluation).
        if low {
            now = baked;
        } else {
            now = ml_pool_soft(rel, n, l.position.w, soft);
        }
    } else {
        now = ml_near_pool(rel, n, l.position.w);
        // A sign's facing: the half-space in front of it only.
        if l.colour.w > -1.5 {
            if dot(l.direction.xyz, l.direction.xyz) > 0.25 {
                now *= smoothstep(0.05, 0.4, dot(-dir, l.direction.xyz));
            }
            // Mostly on the ground: the signs hang on facades (lit
            // panels around them would wash out), the pavement and the
            // road under them take their colour.
            now *= mix(ML_EMIT_WALL, 1.0, smoothstep(0.3, 0.7, ng.z));
        }
        // A car's headlight: its cone.
        if l.direction.w > -1.5 {
            let f = dot(-dir, l.direction.xyz);
            now *= smoothstep(l.direction.w, mix(l.direction.w, 1.0, 0.6), f);
        }
    }
    return vec2<f32>(baked, now);
}
// END RTVIS D
// The baked light at p relit (vertex-colour units): `bake` = v0 (with the
// native headlight projector's light), `ng` the face normal, `n` the
// shading normal. Adds the lights' specular to g_spec.
fn ml_relight(p: vec3<f32>, bake: vec3<f32>, ng_in: vec3<f32>, n_in: vec3<f32>, screen: vec4<f32>) -> vec3<f32> {
    let P = ml_params();
    g_ml_ratio = 1.0;
    g_ml_traced = 0.0;
    let ndc = screen.xy / screen.w;
    let tiles = frame.cluster.xy;
    let tx = u32(clamp(floor((ndc.x * 0.5 + 0.5) * tiles.x), 0.0, tiles.x - 1.0));
    let ty = u32(clamp(floor((ndc.y * 0.5 + 0.5) * tiles.y), 0.0, tiles.y - 1.0));
    let z = dot(p - frame.camera.xyz, frame.forward.xyz);
    #if MLRT
    // BEGIN MLRT
    // The view depth's screen slope (this pixel's plane, for the
    // traced blocks' weights; here, in uniform control flow).
    let dpx = dpdxFine(p);
    let dpy = dpdyFine(p);
    let dz = vec2<f32>(dot(dpx, frame.forward.xyz), dot(dpy, frame.forward.xyz));
    // END MLRT
    #endif
    let slice = u32(clamp(floor(log(max(z, frame.cluster_near.x) / frame.cluster_near.x) * frame.cluster.w), 0.0, frame.cluster.z - 1.0));
    let c = (slice * u32(tiles.y) + ty) * u32(tiles.x) + tx;
    let first = clusters[c * 2u];
    let count = clusters[c * 2u + 1u];
    let v = normalize(frame.camera.xyz - p);
    var ng = ng_in * inverseSqrt(max(dot(ng_in, ng_in), 1e-20));
    if dot(ng, v) < 0.0 { ng = -ng; }
    var n = n_in * inverseSqrt(max(dot(n_in, n_in), 1e-20));
    if dot(n, v) < 0.0 { n = -n; }
    let wet = frame.wet.x > 0.5;
    var rough = select(mix(ML_ROUGH_DRY, ML_ROUGH_BOLD, P.s), ML_ROUGH_WET, wet);
    var f0 = 0.04;
    if g_surface.on > 0.5 {
        rough = g_surface.rough;
        f0 = max(g_surface.f0, 0.02);
    }
    // Phase 1: the specular on a wet road or with Surface Detail only; with
    // strength, on every surface facing up (the roads) too.
    let low = select(P.tier < 1.5, D9_ML_TIER == 1u, D9_ML_TIER != 0u);
    // (Low: the dry specular off.)
    let spec_on = (wet || g_surface.on > 0.5 || (P.s > 0.0 && !low && ng.z > 0.7)) && !g_ml_char;
    let taps = select(4u, 1u, low);
    // The near cars, gathered at the first light that lights p.
    var gathered = false;
    var share = vec3<f32>(0.0);
    var lit = vec3<f32>(0.0);
    var extra = vec3<f32>(0.0);
    var spec = vec3<f32>(0.0);
    var layers = 0.0;
    var shaded = 0u;
    // The unshadowed relit light (the umbra's floor, the debug view).
    var lit0 = vec3<f32>(0.0);
    var extra0 = vec3<f32>(0.0);
    var spec0 = vec3<f32>(0.0);
    // Low: the first few of the cluster's lights only (the nearest to the
    // eye first), however many reach p.
    let n_lights = min(count, P.lights);
    // Modern + RT: the rays go to the P.rays lights that light p most (at first
    // they went to the first in the cluster's list, nearest the eye: which
    // lights were traced changed with the view and the traffic, and a
    // light's shadow flipped between its ray and its fallback — the
    // flicker under the casino's canopy). Ranked in the loop below (every
    // light takes its fallback shadow there), the 4 strongest then traced
    // and their fallback swapped for the rays' (a separate ranking pass
    // over the cluster cost ~10 ms at 4K under the canopy). Per slot:
    // the list index, the light at p, the fallback's visibility.
    var sel = vec4<u32>(0xffffffffu);
    var selw = vec4<f32>(0.02);
    var selv = vec4<f32>(1.0);
    #if MLRT
    // BEGIN MLRT
    // The half-resolution pass traced the two strongest lights of
    // each block (ranked as here, with the share floor; not on ceilings):
    // the blocks around this pixel, and the lights they hold (two corner
    // blocks'). Those lights' traced visibility replaces their fallback in
    // the loop below. The characters' pipelines rank and trace here.
    let rt_blocks = !D9_ML_CHAR && P.rays > 0u && ng.z >= -0.3;
    var rt_ids = vec4<u32>(0u);
    if rt_blocks {
        ml_vis_find(p, z, dpx, dpy, dz, ng);
        rt_ids = g_vis_ids;
    }
    // END MLRT
    #endif
    for (var i = 0u; i < n_lights; i++) {
        let l = lamp_list[clusters[first + i]];
        let colour = l.shadow.yzw;
        if all(colour <= vec3<f32>(0.0)) { continue; }
        let pack = l.colour.w >= 0.0;
        if !pack && P.emit <= 0.0 { continue; }
        let rel = l.position.xyz - p;
        let d2 = dot(rel, rel);
        let dir = rel * inverseSqrt(max(d2, 1e-12));
        let ch = g_ml_char;
        let bn = ml_now(l, rel, dir, select(ng, dir, ch), select(n, dir, ch), P.soft, low);
        let baked = select(bn.x, 0.0, ch);
        let now = select(bn.y, bn.y * ML_CHARACTER_WRAP, ch);
        // The pool's tail (below 1 % of its peak) is left to the bake.
        if max(baked, now) < ML_TAIL { continue; }
        if !gathered {
            ml_gather_cars(p, select(ML_NEAR_CARS, 1u, low));
            gathered = true;
        }
        // The owner car of a car's light (skipped by its own shadow).
        let owner = select(0xffffffffu, u32(-2.0 - l.shadow.x), l.shadow.x <= -1.5);
        var vis = 1.0;
        if shaded < P.shadowed {
            vis = ml_cars(p, l.position.xyz, owner, clusters[first + i + LAMP_CAR_MASKS]);
            if l.shadow.x >= 0.0 {
                // The layer looks straight down over 140°: faces the light
                // grazes (a wall beside a wall lamp) and the frustum's outer
                // ring (past ~55° off the vertical) self-shadow there, so the
                // map's shadow fades out over both.
                let trust = select(smoothstep(0.1, 0.35, dot(ng, dir)), 1.0, g_ml_char) * smoothstep(0.45, 0.6, dir.z);
                vis *= mix(1.0, ml_shadow(i32(l.shadow.x), p, ng, taps), ML_SHADOW_DEPTH * trust * (1.0 - fract(l.shadow.x)));
            }
            shaded += 1u;
        }
        // The fallback's (the layers' measure and Surface Detail's light
        // direction keep it; Modern + RT's traced lights replace it below).
        let fb = vis;
        #if MLRT
        // BEGIN MLRT
        // The cars' own lights keep their ellipsoid shadows (low and near:
        // the traced low LOD would shade them).
        let w = now * dot(colour, vec3<f32>(0.33));
        if rt_blocks && l.colour.w > -1.5 {
            let key = clusters[first + i] + 1u;
            if any(rt_ids == vec4<u32>(key)) {
                let traced = ml_vis(key - 1u);
                if traced >= 0.0 {
                    g_ml_traced += 0.25;
                    // Faces the light grazes are left lit (the traced low
                    // LOD of a curved facade shades the drawn one there).
                    vis = mix(1.0, traced, ML_SHADOW_DEPTH * smoothstep(0.1, 0.35, dot(ng, dir)));
                }
            }
        }
        if D9_ML_CHAR && l.colour.w > -1.5 && w > selw.w {
            if w > selw.x {
                sel = vec4<u32>(i, sel.xyz);
                selw = vec4<f32>(w, selw.xyz);
                selv = vec4<f32>(vis, selv.xyz);
            } else if w > selw.y {
                sel = vec4<u32>(sel.x, i, sel.yz);
                selw = vec4<f32>(selw.x, w, selw.yz);
                selv = vec4<f32>(selv.x, vis, selv.yz);
            } else if w > selw.z {
                sel = vec4<u32>(sel.xy, i, sel.z);
                selw = vec4<f32>(selw.xy, w, selw.z);
                selv = vec4<f32>(selv.xy, vis, selv.z);
            } else {
                sel.w = i;
                selw.w = w;
                selv.w = vis;
            }
        }
        // END MLRT
        #endif
        if pack {
            share += colour * baked;
            layers += (1.0 - fb) * 4.0 * dot(colour * baked, vec3<f32>(0.33));
            lit += colour * (now * vis);
            lit0 += colour * now;
        } else {
            extra += colour * (now * vis);
            extra0 += colour * now;
        }
        // The specular (a dry surface without a material shows next to none
        // in phase 1: F0 0.04 at roughness 0.55).
        if spec_on {
            let window = select(ml_near_pool(rel, dir, l.position.w), ml_pool(rel, dir, l.position.w), pack);
            let gain = select(P.emit, 1.0, pack);
            let sp = colour * (gain * window * surface_spec(n, v, dir, rough, f0, 0.3 * inverseSqrt(max(d2, 0.09))));
            spec += sp * vis;
            spec0 += sp;
        }
        if g_surface.on > 0.5 {
            g_dir += vec4<f32>(dir, 1.0) * (now * fb * dot(colour, vec3<f32>(0.2126, 0.7152, 0.0722)));
        }
    }
    #if MLRT
    // BEGIN MLRT
    // The strongest lights' rays in place of their fallback shadows.
    // Only lights that hold a real part of p's light (ML_RAY_SHARE of it:
    // a faint sign among dozens throws a shadow no one sees, at a ray's
    // full cost).
    let floor_w = ML_RAY_SHARE * dot(lit0 + extra0, vec3<f32>(0.33));
    // Not on ceilings and undersides (an awning's or the canopy's: lit
    // from below, nothing to shade them; their fallback stays).
    // The characters' pipelines only (the others took the blocks'
    // visibility in the loop).
    let n_rays = select(0u, select(min(P.rays, 4u), 0u, ng.z < -0.3 && !g_ml_char), D9_ML_CHAR);
    for (var j = 0u; j < n_rays; j++) {
        let i = sel[j];
        if i == 0xffffffffu || selw[j] < floor_w { break; }
        let l = lamp_list[clusters[first + i]];
        let colour = l.shadow.yzw;
        let pack = l.colour.w >= 0.0;
        let rel = l.position.xyz - p;
        let d2 = dot(rel, rel);
        let dir = rel * inverseSqrt(max(d2, 1e-12));
        let ch = g_ml_char;
        let bn = ml_now(l, rel, dir, select(ng, dir, ch), select(n, dir, ch), P.soft, low);
        let now = select(bn.y, bn.y * ML_CHARACTER_WRAP, ch);
        // Faces the light grazes are left lit (the traced low LOD of a
        // curved facade shades the drawn one there).
        let graze = select(smoothstep(0.1, 0.35, dot(ng, dir)), 1.0, ch);
        let vis = mix(1.0, ml_ray(p, ng, l.position.xyz, ml_light_radius(l), 0xFFu, clusters[first + i], select(ML_RAY_SAMPLES_REST, ML_RAY_SAMPLES, j == 0u)), ML_SHADOW_DEPTH * graze);
        let dv = vis - selv[j];
        if pack {
            lit += colour * (now * dv);
        } else {
            extra += colour * (now * dv);
        }
        if spec_on {
            let window = select(ml_near_pool(rel, dir, l.position.w), ml_pool(rel, dir, l.position.w), pack);
            let gain = select(P.emit, 1.0, pack);
            spec += colour * (gain * window * surface_spec(n, v, dir, rough, f0, 0.3 * inverseSqrt(max(d2, 0.09)))) * dv;
        }
    }
    // The rays' shadows over the quad (their interleaved sets resolved;
    // the characters' only, the blocks' gather resolves the others).
    if D9_ML_CHAR {
        lit = max(lit0 - ml_quad_mean(lit0 - lit), vec3<f32>(0.0));
        extra = max(extra0 - ml_quad_mean(extra0 - extra), vec3<f32>(0.0));
        spec = max(spec0 - ml_quad_mean(spec0 - spec), vec3<f32>(0.0));
    }
    // END MLRT
    #endif
    // The share the bake can hold (per channel), the relit part in step
    // (phase 2: freed from that cap by `uncap`).
    let k = min(vec3<f32>(1.0), ML_SHARE_MAX * max(bake, vec3<f32>(0.0)) / max(share, vec3<f32>(1e-6)));
    g_spec += spec * mix(k, vec3<f32>(1.0), P.uncap * smoothstep(0.3, 0.7, ng.z)) * (ML_SPEC * P.spec / SURFACE_SPEC);
    // NFSU2_MODERN_LIGHTING_DEBUG=1 (lamps.x ≥ 66): red the bake's modelled
    // lamp share, green the relit share (+ the other lights), blue the
    // shadowed share × 4 (a review aid).
    // NFSU2_MODERN_LIGHTING_DEBUG=2 (lamps.x ≥ 130): the light the
    // shadows take, red the lamps', green the other lights', blue the
    // traced lights' count / 4.
    if D9_ML_DEBUG && frame.lamps.x >= 130.0 {
        let floor_d = ML_RAY_SHARE * dot(lit0 + extra0, vec3<f32>(0.33));
        let traced = g_ml_traced + dot(vec4<f32>((sel != vec4<u32>(0xffffffffu)) & (selw >= vec4<f32>(floor_d)) & (vec4<u32>(0u, 1u, 2u, 3u) < vec4<u32>(P.rays))), vec4<f32>(0.25));
        return vec3<f32>(dot((lit0 - lit) * k, vec3<f32>(0.33)) * 4.0, dot(extra0 - extra, vec3<f32>(0.33)) * 4.0 * P.emit, traced);
    }
    if D9_ML_DEBUG && frame.lamps.x >= 66.0 {
        return vec3<f32>(dot(share * k, vec3<f32>(0.33)), dot(lit * k, vec3<f32>(0.33)), dot(extra * P.emit, vec3<f32>(0.33)));
    }
    // Uncapped on the ground only: the walls' and the foliage's light is
    // the artists' (a lamp beside a facade would wash it out).
    let kl = mix(k, vec3<f32>(1.0), P.uncap * smoothstep(0.3, 0.7, ng.z));
    let headroom = clamp(1.0 - dot(bake, vec3<f32>(0.2126, 0.7152, 0.0722)) / ML_EXTRA_PEAK, 0.25, 1.0);
    // Look rule (docs/look-direction.md, darken only where the original is dark): the strength's darkening
    // deepens the dark between the lights only. Where the bake itself is
    // bright (a lit facade, a sign, the garage's turntable, a pool the lamp
    // model does not hold) the artists' light is kept: the darkening fades
    // out between ML_KEEP_LO and ML_KEEP_HI of baked luminance. Halving it
    // everywhere took the highlights and the glow with it (p95 -34, the
    // garage's lit floor -100 at Bold).
    // The same holds for the boost and the emitters' extra light: on a
    // brightly baked surface the relit share is put back at the bake's own
    // level (phase 1: redistributed, not added), so the frame's highlights
    // and the native glow they feed stay the original's.
    let keep = smoothstep(ML_KEEP_LO, ML_KEEP_HI, dot(bake, vec3<f32>(0.2126, 0.7152, 0.0722)));
    let amb = mix(P.ambient, 1.0, keep);
    let boost = mix(P.boost, 1.0, keep);
    let emit = P.emit * (1.0 - keep);
    let rest = (bake - share * k) * amb;
    let lightened = rest + lit * kl * boost + min(extra * emit, vec3<f32>(ML_EXTRA_MAX)) * headroom;
    // A shadow leaves at least ML_UMBRA_FLOOR of the light p has
    // unshadowed (the bounce and the painted fill; several lights' shadows
    // and the strength's darkening stacked went near black).
    let open = rest + lit0 * kl * boost + min(extra0 * emit, vec3<f32>(ML_EXTRA_MAX)) * headroom;
    let lit_out = max(lightened, open * ML_UMBRA_FLOOR);
    let luma = vec3<f32>(0.2126, 0.7152, 0.0722);
    g_ml_ratio = clamp(dot(lit_out, luma) / max(dot(open, luma), 1e-4), ML_UMBRA_FLOOR, 1.0);
    return lit_out;
}

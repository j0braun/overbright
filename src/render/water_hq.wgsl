// Animated Water, Realistic (clone deviation, opt-in; docs/water.md
// "Realistic"). Appended to water.wgsl (its Frame, noise, waves, splash
// rings, rain rings, lamps, cube, fog and spray depth are shared); the On
// tier's module is water.wgsl alone, unchanged.
//
// Passes (render/water_hq.rs), in this order, into the scene before the
// temporal pass:
//   1. wet stone and underwater caustics: one screen rectangle over the
//      near water bodies' boxes, the scene's own pixels in them darkened and glossed where they lie
//      near the water's edge (a top-down distance field per body), lit
//      with moving caustics where they lie under the water;
//   2. a half-size copy of the scene (colour, and linear depth) as it is
//      behind the water;
//   3. the surfaces: screen-space reflections of that copy (the car cube
//      where the march misses), refraction of what lies under the water
//      with depth-based absorption (a virtual floor with caustics where
//      nothing lies under it), flow-mapped ripples, splash and rain rings,
//      lamp glints; opaque, depth written as the native draw;
//   4. the sheets and jets: glassy (premultiplied: the scene through
//      them), aerated where the native art is white, streaked along the
//      flow, lit by the lamps, with a Fresnel rim.

struct Hq {
    // x, y = the target's size (px), z, w = 1 / size
    size: vec4<f32>,
    // x = wet bodies, y = sun height (−1…1, the sky's), z = the sky's
    // brightness (0 night … 1 day), w = rain
    misc: vec4<f32>,
    // x, y = tan of the half field of view across, up (the pixel rays)
    tanv: vec4<f32>,
    // per wet body: (camera-relative origin xy, cell (m), layer),
    // (box min camera-relative, base z camera-relative) → box max in the next
    wet: array<vec4<f32>, 48>,
}

@group(2) @binding(0) var<uniform> hq: Hq;
@group(2) @binding(1) var behind_tex: texture_2d<f32>;
@group(2) @binding(2) var behind_depth: texture_2d<f32>;
@group(2) @binding(3) var lin_samp: sampler;
@group(2) @binding(4) var wet_tex: texture_2d_array<f32>;

const WET_CELLS: f32 = 96.0;

fn to_screen(rel: vec3<f32>) -> vec3<f32> {
    let c = f.view_proj * vec4<f32>(rel, 1.0);
    let ndc = c.xy / c.w;
    return vec3<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5, c.w);
}

fn behind_lin(uv: vec2<f32>) -> f32 {
    let dims = vec2<f32>(textureDimensions(behind_depth));
    let px = clamp(vec2<i32>(uv * dims), vec2<i32>(0), vec2<i32>(dims) - 1);
    return textureLoad(behind_depth, px, 0).r;
}

fn behind_rgb(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(behind_tex, lin_samp, uv, 0.0).rgb;
}

// Moving caustics (a few iterations of warped trig; bright thin lines).
fn caustics(p: vec2<f32>, t: f32) -> f32 {
    var q = p * 1.9;
    var c = 0.0;
    var w = 1.0;
    for (var i = 0; i < 3; i++) {
        let a = q + vec2<f32>(cos(t * 0.7 + q.y * 1.3), sin(t * 0.9 + q.x * 1.1)) * 0.55;
        let n = noised(a + vec2<f32>(t * 0.35, -t * 0.27) + f32(i) * 7.3).x;
        c += (1.0 - abs(n * 2.0 - 1.0)) * w;
        q = q * 1.7 + vec2<f32>(3.1, 1.7);
        w *= 0.55;
    }
    let s = clamp(c / 1.85, 0.0, 1.0);
    return pow(s, 5.0);
}

// The light-pack lamps at one point in one loop: their light, and their
// glints at two sharpnesses (as water.wgsl's lamp_light / lamp_glints).
struct LampTerms {
    light: vec3<f32>,
    sharp: vec3<f32>,
    soft: vec3<f32>,
}

fn lamp_terms(p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, sharp: f32, soft: f32) -> LampTerms {
    var o: LampTerms;
    for (var i = 0u; i < f.counts.x; i++) {
        let l = f.lamps[i * 2u];
        let col = f.lamps[i * 2u + 1u];
        let to = l.xyz - p;
        let d2 = dot(to, to);
        let w = lamp_weight(d2, l.w, col.w);
        let far = lamp_weight(d2, l.w * 2.0, col.w * 2.0) * 0.25;
        // A lamp out of reach adds exactly nothing (skipped; w <= far's reach).
        if max(w, far) <= 0.0 {
            continue;
        }
        let h = normalize(to * inverseSqrt(max(d2, 1e-4)) + v);
        let nh = max(dot(n, h), 0.0);
        let k = col.rgb * max(w, far);
        o.light += col.rgb * w;
        o.sharp += k * pow(nh, sharp) * (sharp + 8.0) / 64.0;
        o.soft += k * pow(nh, soft) * (soft + 8.0) / 64.0;
    }
    return o;
}

// The sky's light on the water (the cube looking up), never darker than a
// little of the fog's colour.
fn sky_light() -> vec3<f32> {
    return max(environment(vec3<f32>(0.0, 0.0, 1.0)), f.fog_color.rgb * 0.35);
}

// ---- 1. wet stone and underwater caustics ----

struct WOut {
    @builtin(position) clip: vec4<f32>,
}

// One rectangle over the bodies' boxes on screen (all of it when a box
// reaches behind the eye); the fragments loop over the bodies.
@vertex
fn vs_wet(@builtin(vertex_index) vi: u32) -> WOut {
    var o: WOut;
    var lo = vec2<f32>(1.0);
    var hi = vec2<f32>(-1.0);
    let bodies = u32(hq.misc.x);
    for (var b = 0u; b < bodies; b++) {
        let a = hq.wet[b * 3u + 1u].xyz;
        let c = hq.wet[b * 3u + 2u].xyz;
        for (var k = 0u; k < 8u; k++) {
            let q = mix(a, c, vec3<f32>(f32(k & 1u), f32((k >> 1u) & 1u), f32((k >> 2u) & 1u)));
            let cl = f.view_proj * vec4<f32>(q, 1.0);
            if cl.w <= 0.05 {
                lo = vec2<f32>(-1.0);
                hi = vec2<f32>(1.0);
            } else {
                lo = min(lo, cl.xy / cl.w);
                hi = max(hi, cl.xy / cl.w);
            }
        }
    }
    lo = clamp(lo, vec2<f32>(-1.0), vec2<f32>(1.0));
    hi = clamp(hi, vec2<f32>(-1.0), vec2<f32>(1.0));
    let corner = CORNERS[vi] * 0.5 + 0.5;
    o.clip = vec4<f32>(mix(lo, hi, corner), 0.0, 1.0);
    if bodies == 0u || any(hi <= lo) {
        o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    }
    return o;
}

struct Wet {
    // 0 dry … 1 wet
    wet: f32,
    // 0 … 1 under the water
    under: f32,
    // the scene point (camera-relative)
    q: vec3<f32>,
    depth_under: f32,
}

fn wet_at(in: WOut) -> Wet {
    var w: Wet;
    let px = vec2<i32>(in.clip.xy);
    let lin = linear_depth(textureLoad(depth_tex, px, 0));
    let ndc = vec2<f32>(in.clip.x * hq.size.z * 2.0 - 1.0, 1.0 - in.clip.y * hq.size.w * 2.0);
    let dir = f.forward.xyz + f.right.xyz * ndc.x * hq.tanv.x + f.up.xyz * ndc.y * hq.tanv.y;
    let q = dir * lin;
    w.q = q;
    let bodies = u32(hq.misc.x);
    for (var b = 0u; b < bodies; b++) {
        let o = hq.wet[b * 3u];
        let lo = hq.wet[b * 3u + 1u];
        let hi = hq.wet[b * 3u + 2u];
        if any(q < lo.xyz) || any(q > hi.xyz) {
            continue;
        }
        let uv = (q.xy - o.xy) / (o.z * WET_CELLS);
        if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
            continue;
        }
        let s = textureSampleLevel(wet_tex, lin_samp, uv, i32(o.w), 0.0);
        let d = s.r;
        let wz = s.g + lo.w;
        let h = q.z - wz;
        // Under the water: inside its outline, below its level.
        let under = (1.0 - smoothstep(0.05, 0.2, d)) * smoothstep(0.005, -0.01, h);
        if under > w.under {
            w.under = under;
            w.depth_under = max(-h, 0.0);
        }
        // Wet: near the edge (splashed; a ragged line), up to a little
        // above the water's level, down the outer walls to the ground.
        // (Nothing to add past the widest reach, 1.3 m, or 0.7 m
        // above the level, or where it is wetter already: skipped.)
        if d >= 1.3 || h >= 0.7 || w.wet >= 1.0 {
            continue;
        }
        let wp = q.xy + f.eye.xy;
        let n = noised(wp * 1.7).x * 0.6 + noised(wp * 5.3).x * 0.4;
        let reach = 0.35 + 0.95 * n;
        let near = 1.0 - smoothstep(reach * 0.6, reach, d);
        let above = 1.0 - smoothstep(0.25, 0.7, h);
        w.wet = max(w.wet, near * above);
    }
    w.wet *= 1.0 - w.under;
    return w;
}

// One pass, as rgb + dst · a: the wet stone darker (a) and glossy (the
// lamps and the cube on it, from the depth's normal), the caustics' light
// on what lies under the water (rgb).
@fragment
fn fs_wet(in: WOut) -> @location(0) vec4<f32> {
    let w = wet_at(in);
    var n = normalize(cross(dpdx(w.q), dpdy(w.q)));
    if w.wet <= 0.001 && w.under <= 0.001 {
        discard;
    }
    let t = f.eye.w;
    let v = normalize(-w.q);
    if dot(n, v) < 0.0 {
        n = -n;
    }
    var rgb = vec3<f32>(0.0);
    if w.wet > 0.001 {
        let ndv = clamp(dot(n, v), 0.0, 1.0);
        let fres = 0.02 + 0.98 * pow(1.0 - ndv, 5.0);
        rgb += (environment(reflect(-v, n)) * fres * 0.35 + lamp_glints(w.q, n, v, 90.0) * 0.5) * w.wet;
    }
    if w.under > 0.001 {
        // Brighter on shallow floors, fading with depth; the sky's and
        // the lamps' light focused by the ripples.
        let c = caustics(w.q.xy + f.eye.xy, t) * w.under * exp(-max(w.depth_under, 0.3) * 1.2);
        rgb += c * (vec3<f32>(0.55, 0.75, 0.8) * (0.2 + 0.5 * hq.misc.z) + lamp_light(w.q) * 0.6);
    }
    let fog = fog_factor(dot(w.q, f.forward.xyz));
    return vec4<f32>(rgb * fog, 1.0 - 0.42 * w.wet * fog);
}

// ---- 2. the scene copied behind the water (half size) ----

// The scene (with MSAA: the multisampled one, its first sample; the
// module's multisampled variant swaps the type).
@group(3) @binding(0) var scene_tex: texture_2d<f32>;

struct BOut {
    @builtin(position) clip: vec4<f32>,
}

@vertex
fn vs_blit(@builtin(vertex_index) vi: u32) -> BOut {
    var o: BOut;
    let p = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    o.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

struct BlitOut {
    @location(0) color: vec4<f32>,
    @location(1) depth: vec4<f32>,
}

// One texel of each 2 × 2 block (with MSAA its first sample): reading a
// block (and all samples) cost 3–4× as much for no visible gain under the
// ripples' distortion.
@fragment
fn fs_blit(in: BOut) -> BlitOut {
    var o: BlitOut;
    let px = min(vec2<i32>(in.clip.xy) * 2, vec2<i32>(textureDimensions(scene_tex)) - 1);
    o.color = textureLoad(scene_tex, px, 0);
    o.depth = vec4<f32>(linear_depth(textureLoad(depth_tex, px, 0)), 0.0, 0.0, 0.0);
    return o;
}

// ---- 3. the surfaces ----

// Screen-space reflection of the copy behind the water: (colour, weight).
fn ssr(p: vec3<f32>, r: vec3<f32>) -> vec4<f32> {
    if r.z < -0.2 {
        return vec4<f32>(0.0);
    }
    // The ray's clip position and view depth are linear in t
    // (clip(p + r t) = clip(p) + clip(r) t): one product for the march
    // instead of one a step (the same points, rounding apart).
    let c0 = f.view_proj * vec4<f32>(p, 1.0);
    let cd = f.view_proj * vec4<f32>(r, 0.0);
    let z0 = dot(p, f.forward.xyz);
    let zd = dot(r, f.forward.xyz);
    var prev = 0.0;
    var hit = -1.0;
    var t = 0.0;
    for (var i = 1; i <= 16; i++) {
        t = 0.15 * f32(i) + 0.06 * f32(i * i) * sqrt(f32(i));
        let s = clip_uv(c0 + cd * t);
        if s.z <= 0.05 || any(s.xy < vec2<f32>(0.0)) || any(s.xy > vec2<f32>(1.0)) {
            break;
        }
        let ray = z0 + zd * t;
        let scene = behind_lin(s.xy);
        let diff = ray - scene;
        if diff > 0.02 {
            if diff < max(0.5, t * 0.12) {
                hit = t;
                break;
            }
        }
        prev = t;
    }
    if hit < 0.0 {
        return vec4<f32>(0.0);
    }
    // Refined between the last two steps.
    var a = prev;
    var b = hit;
    for (var k = 0; k < 5; k++) {
        let m = 0.5 * (a + b);
        let s = clip_uv(c0 + cd * m);
        if z0 + zd * m > behind_lin(s.xy) {
            b = m;
        } else {
            a = m;
        }
    }
    let s = clip_uv(c0 + cd * b);
    let edge = min(min(s.x, 1.0 - s.x), min(s.y, 1.0 - s.y));
    let w = smoothstep(0.0, 0.07, edge) * (1.0 - smoothstep(30.0, 60.0, b));
    return vec4<f32>(behind_rgb(s.xy), w);
}

// to_screen of a clip position.
fn clip_uv(c: vec4<f32>) -> vec3<f32> {
    let ndc = c.xy / c.w;
    return vec3<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5, c.w);
}

// The surface's inputs (a vertex's varyings: the native triangles' or,
// at Next-Gen, the simulated grid's).
struct SurfIn {
    clip: vec4<f32>,
    rel: vec3<f32>,
    kind: u32,
    uv: vec2<f32>,
    aux: vec2<f32>,
    color: vec4<f32>,
    world_xy: vec2<f32>,
    view_z: f32,
    // Next-Gen: the simulation's (height, slope x, slope y, foam); w < 0: none
    sim: vec4<f32>,
}

fn surf_in(in: VOut) -> SurfIn {
    return SurfIn(in.clip, in.rel, in.kind, in.uv, in.aux, in.color, in.world_xy, in.view_z, vec4<f32>(0.0, 0.0, 0.0, -1.0));
}

fn hq_waves(in: SurfIn, t: f32, churn: ptr<function, f32>) -> vec2<f32> {
    let p = in.rel;
    if in.sim.w >= 0.0 {
        // Next-Gen: the simulated waves (impacts, rain), the flow's
        // ripples and the capillaries on top (finer than its cells).
        var g = in.sim.yz + flow_waves(in.world_xy, in.aux, t) * 0.45;
        let d = noised(in.world_xy * 13.0 + vec2<f32>(t * 1.9, -t * 1.4));
        g += d.yz * 13.0 * 0.0022;
        if f.anim.z > 0.001 {
            g += rain_rings(in.world_xy * 1.63 + vec2<f32>(5.1, 2.3), t * 1.21 + 0.37) * 0.8;
        }
        *churn += in.sim.w;
        if in.kind == KIND_POOL {
            g *= 0.6;
        }
        return g;
    }
    var g = flow_waves(in.world_xy, in.aux, t);
    // Fine capillary ripples on top (small, fast).
    let d = noised(in.world_xy * 13.0 + vec2<f32>(t * 1.9, -t * 1.4));
    g += d.yz * 13.0 * 0.0022;
    g += splashes(p, t, churn) * 1.25;
    if f.anim.z > 0.001 {
        g += rain_rings(in.world_xy, t) * 1.1 + rain_rings(in.world_xy * 1.63 + vec2<f32>(5.1, 2.3), t * 1.21 + 0.37) * 0.8;
    }
    if in.kind == KIND_POOL {
        g *= 0.5;
    }
    return g;
}

@fragment
fn fs_surface_hq(v_in: VOut) -> FOut {
    return surface_hq(surf_in(v_in));
}

fn surface_hq(in: SurfIn) -> FOut {
    let t = f.eye.w;
    let p = in.rel;
    var ng = normalize(cross(dpdx(p), dpdy(p)));
    if dot(ng, -p) < 0.0 {
        ng = -ng;
    }
    if in.sim.w >= 0.0 {
        // The simulated grid: its displacement's slope comes smooth from
        // the simulation (the triangles' facets would show).
        ng = vec3<f32>(0.0, 0.0, 1.0);
    }
    var churn = 0.0;
    let g = hq_waves(in, t, &churn);
    churn = clamp(churn, 0.0, 1.0);
    let n = normalize(ng + vec3<f32>(-g.x, -g.y, 0.0));
    let v = normalize(-p);
    let layer = layer_of(in.kind, t);
    // The native shading: the water's body colour (its bake and tint).
    let tex = textureSample(water_tex, water_samp, in.uv + native_offset(layer, t) + g * 0.03, i32(layer));
    let body = clamp(2.0 * tex.rgb * in.color.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let uv0 = in.clip.xy * hq.size.zw;
    let surf = in.view_z;
    // What lies under the water, refracted by the slope.
    let nv = (f.view * vec4<f32>(n - ng, 0.0)).xy;
    let under0 = behind_lin(uv0) - surf;
    let cosv = max(dot(normalize(p), f.forward.xyz), 0.1);
    let real = under0 > -0.02 && under0 < 2.5 * cosv / max(v.z, 0.25);
    var path = 0.0;
    var floor_rgb = vec3<f32>(0.0);
    let shallow_k = select(0.45, 1.4, in.kind == KIND_POOL);
    if real {
        // The basins' floors lie just under the water plane in the art
        // (a few cm): seen as at least the basin's depth.
        path = max(max(under0, 0.0) / cosv, shallow_k / max(v.z, 0.2));
        var uv = uv0 + vec2<f32>(nv.x, -nv.y) * 0.9 * min(path, 1.0) / max(surf, 1.0);
        if behind_lin(uv) < surf {
            uv = uv0;
        }
        floor_rgb = behind_rgb(uv);
    } else {
        // Nothing modelled under it: a floor at the basin's depth, the
        // native texture darkened as its tiles, with caustics.
        let depth = shallow_k;
        path = depth / max(v.z, 0.2);
        let ref_uv = in.uv + native_offset(layer, t) + g * 0.08 * depth;
        let ft = textureSample(water_tex, water_samp, ref_uv, i32(layer)).rgb;
        let fl = clamp(2.0 * ft * in.color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)) * 0.85;
        let floor_p = in.world_xy - v.xy / max(v.z, 0.2) * depth * 0.75;
        let c = caustics(floor_p, t) * exp(-depth * 1.2);
        floor_rgb = fl * (1.0 + c * (1.0 + 0.6 * hq.misc.z)) + lamp_light(p - vec3<f32>(0.0, 0.0, depth)) * c * 0.3;
    }
    // Absorption (clear fountain water: red first) and the body colour
    // scattered back.
    let sigma = vec3<f32>(1.1, 0.5, 0.4) * select(1.0, 0.6, in.kind == KIND_POOL);
    let trans = exp(-sigma * path);
    let scatter = body * 0.88 + sky_light() * 0.03;
    // The art's own colour keeps a share (it is the lit water the
    // original shows: the pools' lamps, the fountains' tint).
    var refr = mix(floor_rgb * trans + scatter * (1.0 - trans), body, 0.4);
    // White water where the sheets land; a thin line at the edge.
    let lum = dot(in.color.rgb, vec3<f32>(0.3, 0.59, 0.11));
    let foam_n = noised(in.world_xy * 7.0 + vec2<f32>(t * 1.7, -t * 1.3)).x;
    var foam = churn * (0.55 + 0.45 * foam_n);
    if real {
        foam += smoothstep(0.06, 0.0, path * v.z) * 0.35 * foam_n;
    }
    // The art's white caps (the flip-book's flowing highlights, the pools'
    // bright patterns, less) stay on the surface as foam over the clear
    // water.
    {
        // Texels above the texture's mean (its 4 × 4 mip; the basins'
        // flip-book spans about 0.5–1.1 of it, measured at the B basin).
        let w3 = vec3<f32>(0.3, 0.59, 0.11);
        let mean = dot(textureSampleLevel(water_tex, water_samp, in.uv, i32(layer), 6.0).rgb, w3);
        let art = dot(tex.rgb, w3) / max(mean, 1e-3);
        foam = max(foam, smoothstep(1.0, 1.35, art) * (0.25 + 0.25 * foam_n) * select(1.0, 0.6, in.kind == KIND_POOL));
    }
    foam = clamp(foam, 0.0, 1.0);
    let lamps = lamp_terms(p, n, v, 420.0, 50.0);
    let foam_rgb = vec3<f32>(min(lum * 2.2, 1.0)) * 0.85 + lamps.light * 0.25;
    refr = mix(refr, foam_rgb, foam * 0.75);
    // Light caught in the crests (as On): ripples, splash and rain rings
    // read on the body as well as in the reflection.
    refr *= 0.9 + 0.32 * clamp(length(g) * 1.5, 0.0, 1.0);
    // Reflection: the scene where the march finds it, else the cube.
    let ndv = clamp(dot(n, v), 0.0, 1.0);
    // Rippled water: the grazing reflection held below a mirror's.
    let fresnel = 0.02 + 0.6 * pow(1.0 - ndv, 5.0);
    let r = reflect(-v, n);
    let env = environment(r);
    // The march only where the reflection shows (steep views reflect 2 %).
    var s = vec4<f32>(0.0);
    if fresnel > 0.035 {
        s = ssr(p, r);
    }
    let refl = mix(env, s.rgb, s.w);
    var rgb = mix(refr, refl, fresnel * (1.0 - foam * 0.8));
    // The lamps: a sharp glint and a softer sheen around it.
    rgb += (lamps.sharp * 1.3 + lamps.soft * 0.12) * (1.0 - foam * 0.7);
    var o: FOut;
    o.color = vec4<f32>(apply_fog(rgb, fog_factor(in.view_z)), clamp(tex.a * in.color.a, 0.0, 1.0));
    return o;
}

// ---- 4. the sheets and jets ----

@fragment
fn fs_sheet_hq(in: VOut) -> FOut {
    return sheet_hq(in, SheetLook(0.0, 0.14, 0.35, 0.25));
}

// What the tiers draw differently on the same sheets.
struct SheetLook {
    // how much of the water leaves the sheet as drops toward its landing
    // (Next-Gen draws those drops as particles and thins the sheet there)
    breakup: f32,
    // the clear water's cover and the share of the art's light it shows
    glass_a: f32,
    glass_light: f32,
    // how far the aerated water's light goes from the art's tint toward a
    // neutral white
    neutral: f32,
}

fn sheet_hq(in: VOut, look: SheetLook) -> FOut {
    let t = f.eye.w;
    let p = in.rel;
    let h = in.aux.x;
    let arc = in.aux.y < 0.0;
    let layer = layer_of(in.kind, t);
    let s = f.scroll[layer];
    // The native layer and a faster one (as On).
    let t1 = textureSample(water_tex, water_samp, in.uv + native_offset(layer, t), i32(layer));
    let off2 = s.yz * t * 1.43;
    let scale2 = select(vec2<f32>(1.0, 0.71), vec2<f32>(1.3, 3.0), arc);
    let t2 = textureSample(water_tex, water_samp, in.uv * scale2 + vec2<f32>(0.37, 0.0) + (off2 - trunc(off2)), i32(layer));
    // Streaks along the flow, two speeds (motion streaks).
    let flow_v = s.yz * vec2<f32>(0.0, 2.2) * t * 1.2;
    let streak = noised(vec2<f32>(in.uv.x * 11.0, in.uv.y * 2.2) + flow_v).x;
    let fine = noised(vec2<f32>(in.uv.x * 47.0, in.uv.y * 1.1) + flow_v * 1.9).x;
    var a = (0.62 * t1.a + 0.38 * t2.a) * (0.84 + 0.32 * streak);
    if arc {
        a = t1.a * (0.7 + 0.55 * t2.a) * (0.8 + 0.4 * streak);
        let pulse = noised(vec2<f32>(in.uv.x * 1.7, h * 5.0 + t * 3.1)).x;
        a *= 0.62 + 0.62 * pulse;
        let e = clamp(min(in.uv.x, 1.0 - in.uv.x), 0.0, 0.5);
        let shimmer = noised(vec2<f32>(in.uv.x * 31.0, h * 47.0 + t * 11.0)).x;
        a *= mix(smoothstep(0.3, 0.75, shimmer), 1.0, smoothstep(0.0, 0.2, e));
    }
    let bottom = smoothstep(0.22, 0.0, h);
    let drops = noised(vec2<f32>(in.uv.x * 23.0, in.uv.y * 9.0 + t * 6.0)).x;
    a *= mix(1.0, smoothstep(0.25, 0.65, drops) * 1.25, bottom * 0.6);
    let brk = smoothstep(0.5, 0.05, h);
    a *= 1.0 - look.breakup * brk * smoothstep(0.2, 0.7, drops + 0.3 * brk);
    let cover = clamp(a * in.color.a * 1.15, 0.0, 1.0);
    if cover < 0.004 {
        discard;
    }
    // Aeration: where the art is white the water is broken and bright;
    // elsewhere it is glassy and shows what is behind it.
    let art = dot(mix(t1.rgb, t2.rgb, 0.3), vec3<f32>(0.3, 0.59, 0.11));
    let aer = clamp(0.3 + smoothstep(0.3, 0.8, art) * 0.6 + bottom * 0.3 + smoothstep(0.55, 0.9, fine) * 0.25, 0.0, 1.0);
    // The rippled normal of the moving water.
    var ng = normalize(cross(dpdx(p), dpdy(p)));
    if dot(ng, -p) < 0.0 {
        ng = -ng;
    }
    let gn = noised(vec2<f32>(in.uv.x * 6.0, in.uv.y * 3.0) + flow_v * 1.1);
    let n = normalize(ng + vec3<f32>(gn.y, gn.z, 0.0) * 0.18);
    let v = normalize(-p);
    // Lit water: the native bake's colour (2 · tex · v0) for the aerated
    // part, the lamps through the body, a Fresnel rim of the cube.
    let lit = clamp(2.0 * mix(t1.rgb, t2.rgb, 0.3) * in.color.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let lum = dot(in.color.rgb, vec3<f32>(0.3, 0.59, 0.11));
    let lamps = lamp_terms(p, n, v, 140.0, 140.0);
    let white = mix(lit, vec3<f32>(min(lum * 2.0, 1.0)), look.neutral + bottom * 0.2) + lamps.light * 0.18;
    // Premultiplied: the aerated water over the scene, the glassy part
    // a faint tint of it (it lets the scene through), the light on top.
    let glass_a = look.glass_a;
    var rgb = white * aer + lit * look.glass_light * glass_a * (1.0 - aer);
    let ndv = clamp(dot(n, v), 0.0, 1.0);
    let fresnel = 0.02 + 0.98 * pow(1.0 - ndv, 5.0);
    rgb += environment(reflect(-v, n)) * fresnel * 0.45;
    rgb += lamps.sharp * (0.9 + aer) * 0.6;
    // Sparkles moving down the streaks.
    rgb += vec3<f32>(pow(fine, 9.0) * 0.6 * (0.35 + lum)) * (1.0 - bottom * 0.5);
    // (rgb is premultiplied by the local alpha la; fogged as such.)
    let fog = fog_factor(in.view_z);
    let la = aer + (1.0 - aer) * glass_a;
    let lit_rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)) * fog + f.fog_color.rgb * (1.0 - fog) * la;
    var o: FOut;
    o.color = vec4<f32>(lit_rgb * cover, la * cover);
    return o;
}

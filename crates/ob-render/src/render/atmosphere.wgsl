// Next-Gen atmosphere (clone deviation, opt-in: Volumetric Fog "Next-Gen", docs/notes/graphics/atmosphere.md).
// Nothing here is in SPEED2.EXE: its fog is world.efx's per-vertex fog (FogValue / FogColor) and the
// horizon fog grid; the clone draws those unchanged and this pass adds a participating medium on top.
//
// A froxel volume (view-aligned cells: DIMS.x × DIMS.y screen tiles × DIMS.z exponential depth slices
// from NEAR to FAR) in three passes:
//   cs_inject    per cell: the haze's extinction (height fog that thickens in rain, drifting noise)
//                and the light it scatters toward the eye: the street lamps (the native weight² falloff,
//                a downward lobe, their Lamp Shadows layers: light shafts), the neon / signs (Modern
//                Lighting's emitters: coloured glows), spot lights (the player's headlight; the Next-Gen car lights' cones
//                go here), the city's ambient in the native fog colour, a Henyey–Greenstein phase;
//                blended with the last frame's cell (reprojected) over a jittered sample position.
//   cs_integrate per column, front to back: the in-scattered light and the transmittance up to each
//                slice (energy-conserving step, Hillaire 2015). The volumetric smoke's density
//                (volume_fx) takes part in the extinction of the light scattered behind it, not in the
//                transmittance applied to the scene (the smoke is already composited there).
//   fs_apply     per pixel: the volume at the pixel's depth, scene × T + S, a ±0.5/255 dither.
//
// Algorithms (own code): froxel volumetric fog (B. Wronski, "Volumetric Fog", SIGGRAPH 2014;
// S. Hillaire, "Towards Unified and Physically-Based Volumetric Lighting in Frostbite", SIGGRAPH 2015),
// Henyey–Greenstein phase (1941), interleaved gradient noise (J. Jimenez, SIGGRAPH 2014).

struct U {
    // clip -> camera-relative world (unjittered)
    clip_to_rel: mat4x4<f32>,
    // camera-relative (this frame's eye) -> last frame's clip
    rel_to_prev: mat4x4<f32>,
    // world eye xyz, time (s)
    eye: vec4<f32>,
    // view forward xyz, reversed depth (0/1)
    forward: vec4<f32>,
    // x: the share of the haze's extinction veiling the scene (look rule: no double fog), y 1: every light
    // tested in every cell (`set_plain_inject`, the comparison with the workgroup's cull),
    // z: the veil floor's quantile (one volume: the energy bound), history valid (0/1)
    prev_forward: vec4<f32>,
    // dims xyz, frame index
    dims: vec4<f32>,
    // near, far, ln(far / near), z jitter 0..1
    depth: vec4<f32>,
    // haze extinction at the ground (1/m), height falloff (m), ground z (world), rain 0..1
    medium: vec4<f32>,
    // ambient (city) light rgb, noise amount 0..1
    ambient: vec4<f32>,
    // light from the ground (the lit streets) rgb, its height (m)
    glow: vec4<f32>,
    // light count, spot count, shadows on (0/1), smoke on (0/1)
    counts: vec4<f32>,
    // phase g, history blend, lamp gain, emitter gain
    look: vec4<f32>,
    // smoke grid: min corner (camera-relative) xyz, voxel edge
    smoke: vec4<f32>,
    // smoke grid voxels xyz, the lights' soft knee
    smoke_dims: vec4<f32>,
    // scene size, 1 / size
    size: vec4<f32>,
    // wind (m/s) xy, spot gain, debug (1: the scattered light alone × 4)
    wind: vec4<f32>,
    // one volume: the headlight cones' aureole cosines (car_lights::Look::aureole), the car lamps' gain,
    // the local floor's share (`tile_floor`)
    shape: vec4<f32>,
    // The rain's tint folded in (`fs_apply_tint`): its factors (xyz), 0
    tint: vec4<f32>,
};

struct Light {
    // camera-relative position xyz, outer radius
    position: vec4<f32>,
    // rgb colour × strength, shadow layer (-1 none)
    colour: vec4<f32>,
    // xyz: the lobe's axis (0: all around), w: kind (0 lamp head, 1 emitter half-space, 2 a car's
    // tail / brake lamp, the axis its facing)
    axis: vec4<f32>,
};

struct Spot {
    // camera-relative eye xyz, reach (m)
    eye: vec4<f32>,
    // right axis xyz, across scale
    right: vec4<f32>,
    // up axis xyz, down scale
    up: vec4<f32>,
    // direction xyz, shadow layer (-1 none)
    dir: vec4<f32>,
    // colour rgb, unused
    colour: vec4<f32>,
};

struct ShadowView {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> lights: array<Light>;
@group(0) @binding(2) var<storage, read> spots: array<Spot>;
@group(0) @binding(3) var<storage, read> shadow_views: array<ShadowView>;
@group(0) @binding(4) var shadow_maps: texture_depth_2d_array;
@group(0) @binding(5) var shadow_samp: sampler_comparison;
@group(0) @binding(6) var noise_tex: texture_3d<f32>;
@group(0) @binding(7) var rep_samp: sampler;
@group(0) @binding(8) var history: texture_3d<f32>;
@group(0) @binding(9) var clamp_samp: sampler;
@group(0) @binding(10) var cells_out: texture_storage_3d<rgba16float, write>;
@group(0) @binding(11) var smoke_tex: texture_3d<f32>;

const PI: f32 = 3.14159265;
// The lights' soft source size (m) for the inverse-square falloff (inferred).
const R0_LAMP: f32 = 3.0;
// A car lamp's soft source size (m): a small bulb behind glass (inferred).
const R0_CAR: f32 = 0.6;
// The headlight cookie's lit part across / down in the projector's uv (car_lights.wgsl COOKIE_X / _Y):
// the bright centre of the beam the air shows.
const COOKIE_X: f32 = 0.38;
const COOKIE_Y: f32 = 0.40;
// The car lights' air phase (car_lights::Look::g) and its share (the rest isotropic), as their own pass had it.
const CAR_G: f32 = 0.3;
const CAR_PHASE_SHARE: f32 = 0.6;
const SKY_DEPTH: f32 = 200.0;
const SKY_SCATTER: f32 = 0.5;

fn cell_dims() -> vec3<u32> {
    return vec3<u32>(u.dims.xyz);
}

// The view ray through screen uv (0..1), camera-relative, normalised.
fn ray(uv: vec2<f32>) -> vec3<f32> {
    let h = u.clip_to_rel * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.5, 1.0);
    return normalize(h.xyz / h.w);
}

// View depth (along forward) of slice coordinate s (0..1).
fn slice_depth(s: f32) -> f32 {
    return u.depth.x * exp(s * u.depth.z);
}

fn hg(g: f32, c: f32) -> f32 {
    let d = 1.0 + g * g - 2.0 * g * c;
    return (1.0 - g * g) / (4.0 * PI * d * sqrt(d));
}

// HeadlightClipTex (0x5BC5E0) as world.wgsl and car_lights.wgsl sample it: the native beam's near cut-off
// (about 5 m) and reach (about 48 m) along the projector's axis.
fn clip_texel(i: f32) -> f32 {
    if i < 33.0 || i > 47.0 {
        return 0.0;
    }
    return trunc(255.0 * sqrt(1.0 - (i - 33.0) * (1.0 / 15.0))) / 255.0;
}

fn headlight_clip(z: f32) -> f32 {
    let x = clamp(z * 0.005 + 0.5, 0.0, 1.0) * 64.0 - 0.5;
    let i = floor(x);
    return mix(clip_texel(clamp(i, 0.0, 63.0)), clip_texel(clamp(i + 1.0, 0.0, 63.0)), x - i);
}

// The car lights' phase: Henyey–Greenstein at CAR_G × 4π, CAR_PHASE_SHARE of it (1 = isotropic).
fn car_phase(c: f32) -> f32 {
    return mix(1.0, hg(CAR_G, c) * 4.0 * PI, CAR_PHASE_SHARE);
}

fn shadow(layer: i32, p: vec3<f32>) -> f32 {
    if layer < 0 || u.counts.z < 0.5 {
        return 1.0;
    }
    let v = shadow_views[layer];
    // p is camera-relative: the views are in world space.
    let c = v.view_proj * vec4<f32>(p + u.eye.xyz, 1.0);
    if c.w <= 0.0 {
        return 1.0;
    }
    let ndc = c.xyz / c.w;
    if any(abs(ndc.xy) > vec2<f32>(1.0)) || ndc.z > 1.0 || ndc.z < 0.0 {
        return 1.0;
    }
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let edge = 1.0 - smoothstep(0.85, 1.0, max(abs(ndc.x), abs(ndc.y)));
    let s = textureSampleCompareLevel(shadow_maps, shadow_samp, uv, layer, ndc.z - 0.0005);
    return mix(1.0, s, edge);
}

// Haze extinction (1/m) at world point w.
fn haze(w: vec3<f32>) -> f32 {
    let above = max(w.z - u.medium.z, 0.0);
    var sigma = u.medium.x * exp(-above / u.medium.y);
    // Drifting low-frequency noise (two octaves, rotated): patchy haze that moves with the wind.
    let t = u.eye.w;
    let q = w * (1.0 / 38.0) + vec3<f32>(u.wind.xy * t * (1.0 / 38.0), t * 0.004);
    let n1 = textureSampleLevel(noise_tex, rep_samp, q, 0.0).r;
    let q2 = vec3<f32>(0.8 * q.x - 0.6 * q.y, 0.6 * q.x + 0.8 * q.y, q.z) * 2.7 + vec3<f32>(0.31, 0.17, t * 0.011);
    let n2 = textureSampleLevel(noise_tex, rep_samp, q2, 0.0).r;
    let n = n1 * 0.65 + n2 * 0.35;
    sigma *= mix(1.0, 0.25 + 1.5 * n, u.ambient.w);
    return sigma;
}

// The lights that reach a workgroup's cells (one bit a light, `MAX_LIGHT_WORDS` words): the workgroup's
// 8 × 8 cells share one slice, so they lie in the quad its corner rays cut from the plane of that view
// depth; a light whose sphere misses the quad's box (with a margin over the cells' own test) reaches none
// of them. The cells then walk the set bits in order: the same lights, in the same order, as the plain loop
// over every light (Hillaire 2015: the volume tiles reuse a culled light list).
const MAX_LIGHT_WORDS: u32 = 4u;
var<workgroup> reach_bits: array<atomic<u32>, 4>;

// Corner `k` (0..3) of the quad: the point of slice coordinate `s` on the ray through the cell centre
// `lo` / `hi` (x by bit 0, y by bit 1); w 1 where that ray's cosine is clamped (the cells' positions then
// leave the plane: no box).
fn group_corner(k: u32, lo: vec2<u32>, hi: vec2<u32>, fd: vec3<f32>, s: f32) -> vec4<f32> {
    let c = vec2<u32>(select(lo.x, hi.x, (k & 1u) == 1u), select(lo.y, hi.y, k >= 2u));
    let dir = ray((vec2<f32>(c) + 0.5) / fd.xy);
    let cd = dot(dir, u.forward.xyz);
    return vec4<f32>(dir * (slice_depth(s) / max(cd, 0.05)), select(0.0, 1.0, cd <= 0.05));
}
var<workgroup> group_corners: array<vec4<f32>, 4>;

@compute @workgroup_size(8, 8, 1)
fn cs_inject(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    let dims = cell_dims();
    let fd = vec3<f32>(dims);
    let n = min(u32(u.counts.x), MAX_LIGHT_WORDS * 32u);
    // The workgroup's lights: four threads its corners, then every thread a light at a time against
    // their box.
    if li < MAX_LIGHT_WORDS {
        atomicStore(&reach_bits[li], 0u);
        let lo = wid.xy * 8u;
        group_corners[li] = group_corner(li, lo, min(lo + 7u, dims.xy - 1u), fd, (f32(wid.z) + u.depth.w) / fd.z);
    }
    workgroupBarrier();
    {
        let k = array<vec4<f32>, 4>(group_corners[0], group_corners[1], group_corners[2], group_corners[3]);
        var mn = min(min(k[0].xyz, k[1].xyz), min(k[2].xyz, k[3].xyz));
        var mx = max(max(k[0].xyz, k[1].xyz), max(k[2].xyz, k[3].xyz));
        // (The cells' rays are normalised one by one: a margin of their rounding.)
        let m = (mx - mn) * 1e-3 + vec3<f32>(1e-3 * abs(dot(k[0].xyz, u.forward.xyz)) + 1e-3);
        mn -= m;
        mx += m;
        if k[0].w + k[1].w + k[2].w + k[3].w > 0.0 {
            mn = vec3<f32>(-1e30);
            mx = vec3<f32>(1e30);
        }
        for (var i = li; i < n; i += 64u) {
            let l = lights[i];
            let q = clamp(l.position.xyz, mn, mx) - l.position.xyz;
            let r2 = l.position.w * l.position.w;
            // (The cells skip a light at d² ≥ r²: a light is left out here only well past that.)
            if dot(q, q) < r2 * 1.001 + 1e-3 || u.prev_forward.y > 0.5 {
                atomicOr(&reach_bits[i / 32u], 1u << (i % 32u));
            }
        }
    }
    workgroupBarrier();
    if any(id >= dims) {
        return;
    }
    let uv = (vec2<f32>(id.xy) + 0.5) / fd.xy;
    let dir = ray(uv);
    let fwd = u.forward.xyz;
    let cosf = max(dot(dir, fwd), 0.05);
    // A jittered position within the slice (the history averages it).
    let s = (f32(id.z) + u.depth.w) / fd.z;
    let vz = slice_depth(s);
    let p = dir * (vz / cosf);
    let w = p + u.eye.xyz;
    let sigma = haze(w);

    // The city's ambient: the native fog colour's light, brighter low over the lit streets.
    let above = max(w.z - u.medium.z, 0.0);
    var light = u.ambient.rgb + u.glow.rgb * exp(-above / max(u.glow.w, 1.0));

    let g = u.look.x;
    for (var word = 0u; word < MAX_LIGHT_WORDS; word++) {
    var bits = atomicLoad(&reach_bits[word]);
    while bits != 0u {
        let i = word * 32u + firstTrailingBit(bits);
        bits &= bits - 1u;
        let l = lights[i];
        let d = l.position.xyz - p;
        let d2 = dot(d, d);
        let r2 = l.position.w * l.position.w;
        if d2 >= r2 {
            continue;
        }
        let dist = sqrt(d2);
        let ld = d / max(dist, 1e-3);
        // The native weight (0x4893A0, inner radius left out), squared as Dynamic Lamps lights the road.
        var wgt = 1.0 - d2 / r2;
        // The window keeps the native reach (nothing beyond the outer radius); inside it the light falls
        // as from a source of ~R0_LAMP m (inverse square, softened): halos at the heads, cones fading down.
        wgt *= wgt * (R0_LAMP * R0_LAMP) / (d2 + R0_LAMP * R0_LAMP);
        let kind = l.axis.w;
        var lobe = 1.0;
        if kind < 0.5 {
            // A lamp head: it lights downward (-ld points from the lamp to p). No halo all
            // round (0.35 · e^(−0.6 d²) before): the native glow sprite is the lamp's halo (no second glow).
            let down = -ld.z;
            lobe = smoothstep(-0.05, 0.55, down);
            wgt *= u.look.z;
        } else if kind < 1.5 {
            // A sign / neon: the half-space in front of it. No glow all round close by
            // (0.5 · e^(−0.15 d²) before): the native sign's own glow and overbright draw it.
            let ax = l.axis.xyz;
            let front = select(1.0, smoothstep(-0.15, 0.35, dot(-ld, ax)), dot(ax, ax) > 0.25);
            lobe = front;
            wgt *= u.look.w;
        } else {
            // A car's tail / brake lamp (the car lights' lamps fed into the one medium): a small source,
            // brightest behind the car (its facing), the car lights' own phase.
            let r0 = R0_CAR * R0_CAR;
            wgt = (1.0 - d2 / r2) * (1.0 - d2 / r2) * r0 / (d2 + r0) * u.shape.z;
            lobe = 0.25 + 0.75 * smoothstep(-0.3, 0.7, dot(l.axis.xyz, -ld));
            light += l.colour.rgb * (wgt * lobe * car_phase(dot(ld, dir)));
            continue;
        }
        // Forward scattering: the eye sees the halo when looking toward the light (ld ≈ dir).
        let ph = hg(g, dot(ld, dir)) * 4.0 * PI;
        let vis = shadow(i32(l.colour.w), p);
        light += l.colour.rgb * (wgt * lobe * ph * vis);
    }
    }
    let ns = u32(u.counts.y);
    for (var i = 0u; i < ns; i++) {
        let sp = spots[i];
        let q = p - sp.eye.xyz;
        let z = dot(q, sp.dir.xyz);
        if z <= 0.3 || z > sp.eye.w {
            continue;
        }
        // The projector's axes and scales (view 9, `headlight_uniform`): the beam's lit centre (the
        // cookie's bright part) is the light the air shows, as the car lights' own march had it.
        let x = dot(q, sp.right.xyz) * sp.right.w / (z * COOKIE_X);
        let y = dot(q, sp.up.xyz) * sp.up.w / (z * COOKIE_Y);
        let f = max(1.0 - (x * x + y * y), 0.0);
        let clip = headlight_clip(z);
        if f <= 0.0 || clip <= 0.0 {
            continue;
        }
        let dq = length(q);
        // cos of the angle between the light's way (lamp → p) and the way to the eye.
        let c = dot(-q / max(dq, 1e-3), dir);
        // The aureole: an eye in the beam looking back toward the lamps sees their glare, which the
        // native glow sprite (and the lens core) draw; the medium leaves that light to them (it once
        // added it a second time: oncoming fronts in rain went under a white haze).
        let ph = car_phase(c) * (1.0 - smoothstep(u.shape.x, u.shape.y, c));
        let fall = clip / (1.0 + z * z * (1.0 / 100.0));
        let vis = shadow(i32(sp.dir.w), p);
        light += sp.colour.rgb * (f * f * fall * ph * vis * u.wind.z);
    }

    // A soft knee on the summed light: overlapping halos saturate instead of blowing out (the grade's
    // highlights stay the scene's own).
    let lum = max(max(light.r, light.g), light.b);
    let knee = u.smoke_dims.w;
    light *= 1.0 / (1.0 + max(lum - knee, 0.0) / knee);
    var cell = vec4<f32>(light * sigma, sigma);
    // Temporal: the last frame's cell at this point (its uvw from its view), blended.
    if u.prev_forward.w > 0.5 {
        let c = u.rel_to_prev * vec4<f32>(p, 1.0);
        if c.w > 0.0 {
            let ndc = c.xy / c.w;
            let puv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
            // A perspective's clip w is the view depth: the last frame's slice.
            let ps = log(max(c.w, u.depth.x) / u.depth.x) / u.depth.z;
            if all(puv > vec2<f32>(0.0)) && all(puv < vec2<f32>(1.0)) && ps < 1.0 {
                let h = textureSampleLevel(history, clamp_samp, vec3<f32>(puv, ps), 0.0);
                cell = mix(h, cell, u.look.y);
            }
        }
    }
    textureStore(cells_out, id, cell);
}

@group(0) @binding(12) var cells_in: texture_3d<f32>;
@group(0) @binding(13) var integ_out: texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn cs_integrate(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = cell_dims();
    if any(id.xy >= dims.xy) {
        return;
    }
    let fd = vec3<f32>(dims);
    let uv = (vec2<f32>(id.xy) + 0.5) / fd.xy;
    let dir = ray(uv);
    let cosf = max(dot(dir, u.forward.xyz), 0.05);
    var acc = vec3<f32>(0.0);
    var t_fog = 1.0;
    var t_all = 1.0;
    var z0 = 0.0;
    let smoke_on = u.counts.w > 0.5;
    for (var k = 0u; k < dims.z; k++) {
        let z1 = slice_depth(f32(k + 1u) / fd.z);
        let ds = (z1 - z0) / cosf;
        let c = textureLoad(cells_in, vec3<u32>(id.xy, k), 0);
        let sigma = max(c.a, 1e-7);
        var smoke = 0.0;
        if smoke_on {
            let p = dir * (0.5 * (z0 + z1) / cosf);
            let g = (p - u.smoke.xyz) / (u.smoke.w * u.smoke_dims.xyz);
            if all(g > vec3<f32>(0.0)) && all(g < vec3<f32>(1.0)) {
                smoke = textureSampleLevel(smoke_tex, clamp_samp, g, 0.0).r;
            }
        }
        let ext = sigma + smoke;
        let step_all = exp(-ext * ds);
        // ∫ S e^(−σx) dx over the step = S (1 − e^(−σ ds)) / σ, S = the cell's scattered light (L σ).
        acc += t_all * c.rgb * (1.0 - step_all) / ext;
        t_all *= step_all;
        t_fog *= exp(-sigma * ds);
        textureStore(integ_out, vec3<u32>(id.xy, k), vec4<f32>(acc, t_fog));
        z0 = z1;
    }
    // The column's light over its whole depth into the veil histogram (`cs_floor`), and its screen
    // tile's least (non-negative floats order as their bits).
    let lum = max(max(acc.r, acc.g), acc.b);
    atomicAdd(&veil_bins[min(u32(lum * VEIL_BINS_PER_UNIT), VEIL_BINS - 1u)], 1u);
    let tile = min(id.xy * VEIL_TILES / dims.xy, VEIL_TILES - vec2<u32>(1u));
    atomicMin(&veil_bins[VEIL_BINS + tile.y * VEIL_TILES.x + tile.x], bitcast<u32>(max(lum, 0.0)));
}

// ---- the veil floor (one volume: the energy bound) ----
// The haze's light shared by the whole frame: the `u.prev_forward.z` quantile of the froxel columns'
// light over their whole depth (cs_integrate; a column holds at least what any pixel in it gets; per
// pixel the near geometry, which gets next to none, made any pixel quantile 0). fs_apply takes it off
// every pixel: what the whole frame shares is the native fog's veil (world.efx already fades the scene
// to FogColor), the volume adds only the light that is local (the lamps' cones, the shafts, the neon's
// glow).
const VEIL_BINS: u32 = 256u;
// The local floor: the screen in VEIL_TILES tiles, each tile's least column light (`cs_floor` writes
// them after the frame's floor in `veil_out`, texel (1 + x, y)).
const VEIL_TILES: vec2<u32> = vec2<u32>(16u, 9u);
// Bins per unit of light (scene 0..1): 1/4 of an 8-bit code each, up to 64 codes.
const VEIL_BINS_PER_UNIT: f32 = 1020.0;
@group(0) @binding(16) var<storage, read_write> veil_bins: array<atomic<u32>, 400>;
@group(0) @binding(17) var veil_out: texture_storage_2d<r32float, write>;
@group(0) @binding(18) var veil_tex: texture_2d<f32>;

// One workgroup, a thread a bin: the bins' running sum (a Hillis–Steele scan in workgroup memory), the bin
// whose share of the running sum holds the quantile, then the tiles' minima (a thread a tile). (A single
// thread walking the bins took ~0.07 ms of latency a frame.)
var<workgroup> scan: array<u32, 256>;
var<workgroup> found: atomic<u32>;

@compute @workgroup_size(256, 1, 1)
fn cs_floor(@builtin(local_invocation_index) i: u32) {
    let n = atomicLoad(&veil_bins[i]);
    scan[i] = n;
    if i == 0u {
        atomicStore(&found, VEIL_BINS);
    }
    workgroupBarrier();
    for (var off = 1u; off < VEIL_BINS; off = off << 1u) {
        var v = scan[i];
        if i >= off {
            v += scan[i - off];
        }
        workgroupBarrier();
        scan[i] = v;
        workgroupBarrier();
    }
    let want = u32(f32(scan[VEIL_BINS - 1u]) * u.prev_forward.z);
    // The first bin whose running sum passes `want` (seen + n > want, seen the sum before it).
    if scan[i] - n <= want && want < scan[i] {
        atomicMin(&found, i);
    }
    atomicStore(&veil_bins[i], 0u);
    workgroupBarrier();
    if i == 0u {
        let b = atomicLoad(&found);
        // The bin's lower edge (the floor never takes more than the quantile's light); none found: 0.
        let veil = select(f32(b) / VEIL_BINS_PER_UNIT, 0.0, b >= VEIL_BINS);
        textureStore(veil_out, vec2<i32>(0, 0), vec4<f32>(veil, 0.0, 0.0, 0.0));
    }
    if i < VEIL_TILES.x * VEIL_TILES.y {
        let m = atomicLoad(&veil_bins[VEIL_BINS + i]);
        // Never written (no column: the first frame's zeroed buffer reads 0) → no local floor.
        let v = select(bitcast<f32>(m), 0.0, m == 0xFFFFFFFFu);
        textureStore(veil_out, vec2<i32>(i32(1u + i % VEIL_TILES.x), i32(i / VEIL_TILES.x)), vec4<f32>(v, 0.0, 0.0, 0.0));
        atomicStore(&veil_bins[VEIL_BINS + i], 0xFFFFFFFFu);
    }
}

// The local floor at screen uv: the tiles' least column light, bilinear between the tiles' centres.
fn tile_floor(uv: vec2<f32>) -> f32 {
    let n = vec2<f32>(VEIL_TILES);
    let t = clamp(uv * n - 0.5, vec2<f32>(0.0), n - 1.0);
    let i = vec2<i32>(floor(t));
    let j = min(i + vec2<i32>(1), vec2<i32>(VEIL_TILES) - 1);
    let f = t - floor(t);
    let a = textureLoad(veil_tex, vec2<i32>(1 + i.x, i.y), 0).r;
    let b = textureLoad(veil_tex, vec2<i32>(1 + j.x, i.y), 0).r;
    let c = textureLoad(veil_tex, vec2<i32>(1 + i.x, j.y), 0).r;
    let d = textureLoad(veil_tex, vec2<i32>(1 + j.x, j.y), 0).r;
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// ---- apply ----
@group(0) @binding(14) var depth_tex: texture_depth_2d;
@group(0) @binding(15) var integ_tex: texture_3d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_apply(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return apply(pos);
}

// The apply with the rain's tint folded in (PostFx::fold_tint): the light it adds × the tint, and the scene ×
// the tint by the blend (its constant): (scene + light) × tint in one pass, the tint pass left out.
@fragment
fn fs_apply_tint(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let o = apply(pos);
    return vec4<f32>(o.rgb * u.tint.xyz, o.a);
}

fn apply(pos: vec4<f32>) -> vec4<f32> {
    let uv = pos.xy * u.size.zw;
    let d = textureLoad(depth_tex, vec2<i32>(pos.xy), 0);
    let h = u.clip_to_rel * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    let rel = h.xyz / h.w;
    let far_plane = select(d >= 0.99999, d <= 0.00001, u.forward.w > 0.5);
    // The sky (the far plane) takes the haze of SKY_DEPTH m: the dome keeps its own colours above the haze.
    var vz = dot(rel, u.forward.xyz);
    // (The SKYDOME is 6334 m and more away: past 3 km a pixel is sky.)
    vz = select(vz, SKY_DEPTH, far_plane || vz > 3000.0);
    vz = clamp(vz, u.depth.x, u.depth.y);
    let s = log(vz / u.depth.x) / u.depth.z * u.dims.z;
    // Texel k holds the light up to slice k's far side (s = k + 1).
    let w = (s - 0.5) / u.dims.z;
    var v = textureSampleLevel(integ_tex, clamp_samp, vec3<f32>(uv, w), 0.0);
    if far_plane || dot(rel, u.forward.xyz) > 3000.0 {
        // Over the sky the native fog already veils the dome: half the haze's light (no white-out under
        // the lamps' halos in rain).
        v = vec4<f32>(v.rgb * SKY_SCATTER, v.a);
    }
    if s < 1.0 {
        // Nearer than the first slice's far side: a share of it.
        v = vec4<f32>(v.rgb * s, mix(1.0, v.a, s));
    }
    // Look rule: the native fog owns the scene's attenuation; only `veil` of the haze's is applied again.
    v = vec4<f32>(v.rgb, mix(1.0, v.a, u.prev_forward.x));
    if u.wind.w > 0.5 {
        return vec4<f32>(v.rgb * 4.0, 0.0);
    }
    // The energy bound (one volume): the veil every pixel shares (the frame's floor of the
    // haze's light, `cs_floor`) is already the native fog's (world.efx fades to FogColor): only the light
    // above it is added, so the night keeps its black level and the neon its saturation.
    // The local floor (`tile_floor`): what a whole screen tile shares is its veil too, a share of it.
    let veil = max(textureLoad(veil_tex, vec2<i32>(0, 0), 0).r, tile_floor(uv) * u.shape.w);
    let vl = max(max(v.r, v.g), v.b);
    v = vec4<f32>(v.rgb * (max(vl - veil, 0.0) / max(vl, 1e-6)), v.a);
    let ign = fract(52.9829189 * fract(dot(pos.xy, vec2<f32>(0.06711056, 0.00583715))));
    return vec4<f32>(v.rgb + (ign - 0.5) / 255.0, v.a);
}

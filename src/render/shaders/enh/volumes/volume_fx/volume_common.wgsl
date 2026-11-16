// Volumetric effects (docs/smoke-volumetric.md; clone deviation,
// opt-in). A reusable particle → density volume → lit ray march renderer
// and a GPU particle pool, shared by the Cinematic smoke and later
// effects (spray, mist, steam, fire). Sections (volume_fx.rs `sections`):
//   common    the structs, the phase function, the full-screen triangle;
//   pool      the generic simulation: spawn from emits, update (drag,
//             temperature buoyancy and cooling, divergence-free
//             turbulence, wind, car wakes), export to particles;
//   splat     particles → fixed-point accumulators (lumpy density balls);
//   light     resolve accumulators → density / albedo volumes, then the
//             half-resolution light volume (scene lights, self-shadow,
//             multiple scattering, sky; plus emission);
//   march     the reduced-resolution ray march (detail erosion, billow
//             self-shadow), and the nearest-depth downsample;
//   composite the depth-aware upsample over the scene.
// volume_fx.rs mirrors `hg`, `multi_scatter`, `wake`, `grid_origin`.

// A particle the renderer splats (80 bytes; volume_fx.rs `Particle`).
struct Particle {
    // world position, radius (m; 0: none)
    pos_r: vec4<f32>,
    // albedo, peak extinction at the centre (1/m)
    albedo_sigma: vec4<f32>,
    // emitted radiance (per unit extinction; fire, embers), lumpiness 0..1
    emit_lump: vec4<f32>,
    // noise offset (volume texels / 64), spin about the vertical (rad)
    noise: vec4<f32>,
    // age fraction 0..1 (old breaks up), thinness 0..1 (wet / steam:
    // wispier, whiter), 0, 0
    misc: vec4<f32>,
}

struct Vol {
    // the grid's world origin (min corner), voxel size (m)
    origin: vec4<f32>,
    // voxels x, y, z, particles to splat
    dims: vec4<u32>,
    // world eye, time (s)
    eye: vec4<f32>,
    // ambient rgb, phase g of the lamps
    ambient: vec4<f32>,
    // erosion, detail frequency (1/m), whiteness of thin (wet) particles, debug
    look2: vec4<f32>,
    // the view-to-eye-relative-world inverse (rotation and projection)
    inv_vp: mat4x4<f32>,
    // forward, max ray distance
    forward: vec4<f32>,
    // low-res viewport x, y, w, h (low-res pixels)
    viewport: vec4<f32>,
    // low-res size, full-res size
    sizes: vec4<f32>,
    // near, far, reversed (0/1), divisor
    depth: vec4<f32>,
    // billow self-shadow (0..1), lamp gain, spot gain, point gain
    look3: vec4<f32>,
    // wind (m/s; the detail drifts with it), max steps
    wind: vec4<f32>,
    // the eye-relative view-projection (the screen rect of the occupied
    // bricks)
    vp: mat4x4<f32>,
    // edge contrast (1: linear), fine octave weight, directional
    // billow shading (0..1), the march's step scale (0: 1)
    look4: vec4<f32>,
    // The native fog (FogValue x, y, z; on 0/1) and its colour
    fog_value: vec4<f32>,
    fog_color: vec4<f32>,
    // Scene ambient gain, near fade (m), shadow strength, scene
    // ambient read this frame (0/1)
    look5: vec4<f32>,
    // The last frame's eye-relative view-projection, its eye
    // relative to this one (w: a last frame to reproject into, 0/1), its
    // low-res viewport; the temporal resolve: history weight, frame index,
    // on (0/1), 0
    prev_vp: mat4x4<f32>,
    prev_eye: vec4<f32>,
    prev_view: vec4<f32>,
    temporal: vec4<f32>,
    // The march's step scale (0: 1), the jitter's share + 1 (0: full)
    // (`VolumeFx::set_step_scale`, `set_jitter`); 1: the resolve clears and
    // marks voxel by voxel (`set_plain_resolve`, the comparison test), 0
    look6: vec4<f32>,
}

// The scene's lights, eye-relative (volume_fx.rs `SceneLights`).
struct Lights {
    // lamps, spots, points, Modern Lighting (0/1)
    counts: vec4<u32>,
    // position, outer radius; colour
    lamps: array<vec4<f32>, 128>,
    // position, reach; direction, cos outer; colour, cos inner
    spots: array<vec4<f32>, 24>,
    // position, radius; colour
    points: array<vec4<f32>, 64>,
}

const SCALE: f32 = 65536.0;
const CH: u32 = 9u;
const BRICK: u32 = 8u;

// Henyey-Greenstein phase (1/sr), volume_fx.rs `hg`.
fn hg(g: f32, c: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / (12.566371 * pow(max(1.0 + g2 - 2.0 * g * c, 1e-4), 1.5));
}

// Light through `tau` of medium, scattered at cosine `c`: single
// scattering plus two octaves of the multiple-scattering approximation
// (Wrenninge 2013; Hillaire 2016): each octave sees 0.35× the extinction,
// adds 0.55× and scatters half as forward. volume_fx.rs `multi_scatter`.
fn multi_scatter(tau: f32, c: f32, g: f32) -> f32 {
    var s = 0.0;
    var a = 1.0;
    var b = 1.0;
    var gk = g;
    for (var k = 0; k < 3; k++) {
        s += b * exp(-a * tau) * hg(gk, c);
        a *= 0.35;
        b *= 0.55;
        gk *= 0.5;
    }
    return s * 12.566371 / 2.0;
}

fn rot2(v: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2(c * v.x - s * v.y, s * v.x + c * v.y);
}

struct FsIn {
    @builtin(position) pos: vec4<f32>,
}

// A vertex of the quad over the screen rect of the occupied bricks
// `lo`..`hi` (brick coordinates; empty: a degenerate quad, nothing
// drawn), in the clip space of a target `size` px where the scene's view
// lands in `viewport` (`scale` × the uniform's low-res viewport), with a
// margin of `margin` px. A box reaching behind the eye covers it all.
fn rect_vertex(vi: u32, u: Vol, lo: vec3<f32>, hi: vec3<f32>, size: vec2<f32>, scale: f32, margin: f32) -> vec4<f32> {
    if (any(lo > hi)) {
        return vec4(-2.0, -2.0, 0.0, 1.0);
    }
    let bs = 8.0 * u.origin.w;
    let bmin = u.origin.xyz - u.eye.xyz;
    var mn = vec2(1e9);
    var mx = vec2(-1e9);
    var behind = false;
    for (var k = 0u; k < 8u; k++) {
        let f = vec3(f32(k & 1u), f32((k >> 1u) & 1u), f32((k >> 2u) & 1u));
        let c = u.vp * vec4(bmin + mix(lo, hi + 1.0, f) * bs, 1.0);
        if (c.w < 0.05) {
            behind = true;
        } else {
            let ndc = c.xy / c.w;
            mn = min(mn, ndc);
            mx = max(mx, ndc);
        }
    }
    if (behind) {
        mn = vec2(-1e3);
        mx = vec2(1e3);
    }
    // Scene NDC to the target's pixels (y down), with the margin.
    let vp = u.viewport * scale;
    var p0 = vp.xy + vec2(mn.x * 0.5 + 0.5, 0.5 - mx.y * 0.5) * vp.zw - margin;
    var p1 = vp.xy + vec2(mx.x * 0.5 + 0.5, 0.5 - mn.y * 0.5) * vp.zw + margin;
    p0 = clamp(p0, vec2(0.0), size);
    p1 = clamp(p1, vec2(0.0), size);
    let corner = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0))[vi];
    let px = mix(p0, p1, corner);
    return vec4(px.x / size.x * 2.0 - 1.0, 1.0 - px.y / size.y * 2.0, 0.0, 1.0);
}

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> FsIn {
    var o: FsIn;
    let p = vec2(f32((vi << 1u) & 2u), f32(vi & 2u));
    o.pos = vec4(p * 2.0 - 1.0, 0.0, 1.0);
    return o;
}


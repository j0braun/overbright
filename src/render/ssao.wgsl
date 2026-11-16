// Ambient occlusion (the Graphics page's, a clone deviation; see ssao.rs).

struct Params {
    size: vec4<f32>,   // scene w, h; occlusion target w, h
    depth: vec4<f32>,  // near, far, reversed, strength
    proj: vec4<f32>,   // m00, m11, radius (m), scene texels per screen texel
    fog: vec4<f32>,    // FogValue x, y, z; fog on
    glow_count: vec4<f32>,          // x: the cars (underglow; body mask)
    glows: array<GlowBox, 8>,       // ssao::glow_boxes
    // Each box's screen rectangle (scene pixels: min x, min y, max x, max
    // y; ssao::glow_rect): every pixel where the box's body or footprint
    // term below can be other than 0 is inside it. And their union.
    glow_rects: array<vec4<f32>, 8>,
    glow_union: vec4<f32>,
    // The occlusion grid (ssao.rs `Grid`): xy the scene's texels per
    // occlusion texel (2: half the scene; under SSAA half the screen), zw
    // the offset to the scene texel holding the footprint's centre, where
    // the depth is read (just under 1 at 2: the top-left texel of each
    // 2 × 2).
    grid: vec4<f32>,
    // xy: 1 / grid.xy; z: the reach in occlusion texels per half-scene
    // texel (1 at 2).
    grid_inv: vec4<f32>,
}

// A car with an underglow in view space (ssao.rs `GlowBox`).
struct GlowBox {
    centre: vec4<f32>, // xyz, w the glow's light 0..1
    x: vec4<f32>,      // axis, w half extent
    y: vec4<f32>,
    z: vec4<f32>,
}

@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var ao_tex: texture_2d<f32>;
@group(0) @binding(2) var<uniform> p: Params;
// Nearest, clamped to the edge (the taps read whole texels, as loads at
// clamped coordinates would).
@group(0) @binding(3) var nearest: sampler;

const TAPS: i32 = 12;
const BIAS: f32 = 0.15;
const INTENSITY: f32 = 1.0;
const MAX_PIXELS: f32 = 48.0;
// The largest 16-bit float below 1 (1 − 2⁻¹¹). An unoccluded texel of the
// blur holds it or 1: its average of ones over their weights comes out a
// hair under 1, and the target keeps 16 bits rounded toward zero.
const UNOCCLUDED: f32 = 0.99951171875;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
}

fn raw_depth(c: vec2<i32>) -> f32 {
    let s = vec2<i32>(p.size.xy);
    return textureLoad(depth_tex, clamp(c, vec2<i32>(0), s - 1), 0);
}

fn is_sky(d: f32) -> bool {
    return select(d >= 1.0, d <= 0.0, p.depth.z > 0.5);
}

// The view-space distance along the view (world.efx's viewZ).
fn linear(d: f32) -> f32 {
    let n = p.depth.x;
    let f = p.depth.y;
    if p.depth.z > 0.5 {
        return n * f / (n + d * (f - n));
    }
    return n * f / (f - d * (f - n));
}

// Pass 1: the depth at the occlusion's size, linear (−1 for the sky), so
// the taps read a small single-sample texture rather than the scene's
// depth. Half the scene: the top-left texel of each 2 × 2; another grid:
// the texel holding the footprint's centre.
@fragment
fn fs_depth(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let d = raw_depth(vec2<i32>(floor(floor(frag.xy) * p.grid.xy + p.grid.zw)));
    return vec4<f32>(select(linear(d), -1.0, is_sky(d)), 0.0, 0.0, 0.0);
}

// The spiral's taps: k (cos, sin) of k·2π·φ² for k = (i + 0.5) / TAPS
// (the angle of tap i before the pixel's turn).
const SPIRAL = array<vec2<f32>, 12>(
    vec2<f32>(3.225691989e-02, 2.637427114e-02),
    vec2<f32>(-5.832057819e-02, 1.105608866e-01),
    vec2<f32>(-1.999057233e-01, -5.865558609e-02),
    vec2<f32>(2.488060668e-02, -2.906034887e-01),
    vec2<f32>(3.725407422e-01, -4.287653416e-02),
    vec2<f32>(1.418182701e-01, 4.358406067e-01),
    vec2<f32>(-4.715201557e-01, 2.665924430e-01),
    vec2<f32>(-4.095620811e-01, -4.721058011e-01),
    vec2<f32>(4.321729243e-01, -5.612153411e-01),
    vec2<f32>(7.106969953e-01, 3.487778008e-01),
    vec2<f32>(-2.217532247e-01, 8.464339972e-01),
    vec2<f32>(-9.568186998e-01, -5.385816470e-02),
);

// The view-space x, y over z of occlusion texel `t` (whole texels, any
// grid): the scene texel the depth was read at (`fs_depth`), so position
// and depth agree.
fn picked_xy(t: vec2<f32>) -> vec2<f32> {
    let c = floor(t * p.grid.xy + p.grid.zw) + 0.5;
    let ndc = (c + 0.5) / p.size.xy * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    return ndc / p.proj.xy;
}

// The occlusion's sum over the twelve taps (each tap's elevation over the
// pixel's plane, its cosine to the normal, less a bias for the meshes'
// facets: scale-free, so a body near the eye occludes no more than a wall
// far off; times its falloff). A sky tap adds nothing: chosen, not branched
// over, so that no tap's load waits for the tap before it. `two`: the 2:1
// grid, where texel t's x, y over z is A t + B, one multiply-add from the
// pixel's; another grid (SSAA: half the screen) takes the picked texel's
// own place per tap (a linear map would beat against the picks).
fn taps(t: vec2<f32>, pos: vec3<f32>, n: vec3<f32>, turn: vec2<f32>, A: vec2<f32>, B: vec2<f32>, two: bool) -> f32 {
    let inv = 1.0 / p.size.zw;
    let uv_t = (t + 0.5) * inv;
    let xy_t = A * t + B;
    let inv_r2 = 1.0 / (p.proj.z * p.proj.z);
    var sum = 0.0;
    for (var i = 0; i < TAPS; i++) {
        let d = SPIRAL[i];
        let r = round(vec2<f32>(d.x * turn.x - d.y * turn.y, d.y * turn.x + d.x * turn.y));
        let zq = textureSampleLevel(ao_tex, nearest, fma(r, inv, uv_t), 0.0).r;
        var xy: vec2<f32>;
        if two {
            xy = fma(A, r, xy_t);
        } else {
            xy = picked_xy(t + r);
        }
        let v = vec3<f32>(fma(xy, vec2<f32>(zq), -pos.xy), zq - pos.z);
        let vv = dot(v, v);
        let falloff = max(0.0, 1.0 - vv * inv_r2);
        let elevation = dot(v, n) * inverseSqrt(max(vv, 1e-6));
        sum += select(max(0.0, elevation - BIAS) * falloff, 0.0, zq < 0.0);
    }
    return sum;
}

// Pass 2: the occlusion at the occlusion's size (R; G the linear depth).
@fragment
fn fs_occlusion(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let t = floor(frag.xy);
    let inv = 1.0 / p.size.zw;
    // At 2:1 texel t's centre is scene pixel 2t + 1: x, y over z = A t + B.
    let two = all(p.grid.xy == vec2<f32>(2.0));
    let A = vec2<f32>(4.0 / (p.size.x * p.proj.x), -4.0 / (p.size.y * p.proj.y));
    let B = vec2<f32>((2.0 / p.size.x - 1.0) / p.proj.x, (1.0 - 2.0 / p.size.y) / p.proj.y);
    // The centre and its four neighbours from two gathers: at the corner
    // before t, (t.x - 1, t), t, (t.x, t.y - 1), t - 1; after it, (t.x, t.y + 1),
    // t + 1, (t.x + 1, t.y), t.
    let g0 = textureGather(0, ao_tex, nearest, t * inv);
    let g1 = textureGather(0, ao_tex, nearest, (t + 1.0) * inv);
    let z = g0.y;
    if z < 0.0 {
        return vec4<f32>(1.0, -1.0, 0.0, 0.0);
    }
    let x = vec2<f32>(1.0, 0.0);
    let y = vec2<f32>(0.0, 1.0);
    var xy = array<vec2<f32>, 5>(A * t + B, A * (t + x) + B, A * (t - x) + B, A * (t + y) + B, A * (t - y) + B);
    if !two {
        xy = array<vec2<f32>, 5>(picked_xy(t), picked_xy(t + x), picked_xy(t - x), picked_xy(t + y), picked_xy(t - y));
    }
    let pos = vec3<f32>(xy[0] * z, z);
    let r = vec3<f32>(xy[1] * g1.z, g1.z) - pos;
    let l = pos - vec3<f32>(xy[2] * g0.x, g0.x);
    let dn = vec3<f32>(xy[3] * g1.x, g1.x) - pos;
    let up = pos - vec3<f32>(xy[4] * g0.z, g0.z);
    // The normal from the nearer neighbour each way (no smearing across
    // silhouettes).
    let dx = select(l, r, abs(r.z) < abs(l.z));
    let dy = select(up, dn, abs(dn.z) < abs(up.z));
    var n = normalize(cross(dx, dy));
    if dot(n, pos) > 0.0 {
        n = -n;
    }
    let radius = p.proj.z;
    // The reach on screen: the cap is MAX_PIXELS of the screen's half-size
    // texels (SSAA / Render Scale: the same part of the screen); both in
    // half-scene texels, then in the occlusion's.
    let reach = min(radius * p.proj.y * 0.25 * p.size.y / pos.z, MAX_PIXELS * p.proj.w) * p.grid_inv.z;
    if reach < 1.0 {
        return vec4<f32>(1.0, pos.z, 0.0, 0.0);
    }
    // Interleaved gradient noise turns the spiral per pixel: one turn of
    // the whole table (cos, sin once), not a sine and cosine per tap.
    let noise = fract(52.9829189 * fract(dot(frag.xy, vec2<f32>(0.06711056, 0.00583715))));
    let turn = vec2<f32>(cos(noise * 6.2831853), sin(noise * 6.2831853)) * reach;
    var sum: f32;
    if two {
        sum = taps(t, pos, n, turn, A, B, true);
    } else {
        sum = taps(t, pos, n, turn, A, B, false);
    }
    // Each tap's share over 1 − BIAS, taken once for the sum.
    let ao = clamp(1.0 - INTENSITY * sum / (f32(TAPS) * (1.0 - BIAS)), 0.0, 1.0);
    return vec4<f32>(ao, pos.z, 0.0, 0.0);
}

// The occlusion blurred over 4 × 4 of its texels, each weighed by how near
// its depth is to the centre's (sky texels, z < 0, left out). Where all
// sixteen are unoccluded the blur is 1 without its weights (the weighed sum
// would give 1 or the 16-bit step below it, both unoccluded to fs_apply).
@fragment
fn fs_blur(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let lim = vec2<i32>(p.size.zw) - 1;
    let c = vec2<i32>(frag.xy);
    let centre = textureLoad(ao_tex, c, 0).rg;
    if centre.y < 0.0 {
        return vec4<f32>(1.0, -1.0, 0.0, 0.0);
    }
    var t: array<vec2<f32>, 16>;
    var lowest = 1.0;
    for (var i = 0; i < 16; i++) {
        t[i] = textureLoad(ao_tex, clamp(c + vec2<i32>(i % 4 - 2, i / 4 - 2), vec2<i32>(0), lim), 0).rg;
        lowest = min(lowest, t[i].x);
    }
    if lowest >= 1.0 {
        return vec4<f32>(1.0, centre.y, 0.0, 0.0);
    }
    var total = 0.0;
    var weight = 0.0;
    for (var i = 0; i < 16; i++) {
        let w = select(1.0 / (1e-3 + abs(t[i].y - centre.y) / centre.y * 50.0), 0.0, t[i].y < 0.0);
        total += t[i].x * w;
        weight += w;
    }
    return vec4<f32>(total / max(weight, 1e-6), centre.y, 0.0, 0.0);
}

// Outside a box's rectangle (ssao::glow_rect) its two terms in glow_share are
// exactly 0 (each smoothstep at its end: the share times exactly 1), so the
// rectangles leave out only factors of 1: the same share, and no loop where
// no car's terms reach on screen.
fn outside_rect(c: vec2<f32>, r: vec4<f32>) -> bool {
    return c.x < r.x || c.y < r.y || c.x > r.z || c.y > r.w;
}

// The share of the occlusion kept at a pixel (scene pixel `c`,
// linear depth `z`) under the cars' underglow: the glow lights what the car
// shades. The footprint and height as the Contact Shadows' footprint (wider than the
// lamps.wgsl one). Every car's box (`glow_boxes` takes them
// all, the nearest MAX_GLOWS): the car's own body takes no occlusion (its
// paint, glass and lamps are lit by the car material; screen-space
// self-occlusion only dimmed them, street car box -2 %), the ground under
// and round it keeps its contact darkening; under a lit underglow the
// footprint takes none (the glow's pool is light, not a surface: -6.8 %).
fn glow_share(c: vec2<f32>, z: f32) -> f32 {
    let count = u32(p.glow_count.x);
    if count == 0u || outside_rect(c, p.glow_union) {
        return 1.0;
    }
    let ndc = c / p.size.xy * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    let pos = vec3<f32>(ndc.x / p.proj.x * z, ndc.y / p.proj.y * z, z);
    var share = 1.0;
    for (var i = 0u; i < min(count, 8u); i++) {
        if outside_rect(c, p.glow_rects[i]) { continue; }
        let o = p.glows[i];
        let rel = pos - o.centre.xyz;
        if dot(rel, rel) > 16.0 { continue; }
        let q = vec3<f32>(dot(rel, o.x.xyz), dot(rel, o.y.xyz), dot(rel, o.z.xyz));
        let h = vec3<f32>(o.x.w, o.y.w, o.z.w);
        // The body: inside the box (a soft 8 cm margin, the depth's
        // reconstruction is half size), from 10 cm over its floor up (the
        // ground under the car is not the car).
        let outside = max(max(abs(q.x) - h.x, abs(q.y) - h.y), q.z - h.z);
        let body = (1.0 - smoothstep(0.0, 0.08, outside)) * smoothstep(-h.z + 0.04, -h.z + 0.14, q.z);
        share *= 1.0 - body;
        if o.centre.w <= 0.0 { continue; }
        // The glow's reach (the grid is the box × 1.05, its light spreads
        // past it): the Contact Shadows' original footprint, to 0.35 m + past it.
        let e = max(abs(q.xy) - h.xy * vec2<f32>(0.85, 0.8), vec2<f32>(0.0));
        let footprint = 1.0 - smoothstep(0.0, 0.35 + 0.15 * h.y, length(e));
        let above = q.z + h.z;
        let vertical = (1.0 - smoothstep(0.0, h.z, above)) * (1.0 - smoothstep(0.0, 0.6, -above - 0.3));
        share *= 1.0 - footprint * vertical;
    }
    return share;
}

// The blurred occlusion taken to the scene's size (the 2 × 2 texels
// around the pixel, weighed by bilinear position and depth likeness),
// then mixed in by the strength and the pixel's fog factor
// (oFog = clamp((x − viewZ)·y, z, 1)). A pixel with nothing to darken is
// not blended at all: the sky, and a pixel whose four texels are all
// unoccluded (a multiply by at least 1 − 2⁻¹¹ there, about 40 % of a
// racing frame's pixels; skipping it leaves them at most one 16-bit step
// brighter, and an 8-bit scene exactly as it was).
@fragment
fn fs_apply(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let d = raw_depth(vec2<i32>(frag.xy));
    if is_sky(d) {
        discard;
    }
    let z = linear(d);
    let h = frag.xy * p.grid_inv.xy - 0.5;
    let base = vec2<i32>(floor(h));
    let f = h - floor(h);
    let lim = vec2<i32>(p.size.zw) - 1;
    var total = 0.0;
    var weight = 0.0;
    var lowest = 1.0;
    for (var y = 0; y < 2; y++) {
        for (var x = 0; x < 2; x++) {
            let t = textureLoad(ao_tex, clamp(base + vec2<i32>(x, y), vec2<i32>(0), lim), 0).rg;
            lowest = min(lowest, t.x);
            let bw = select(1.0 - f.x, f.x, x == 1) * select(1.0 - f.y, f.y, y == 1);
            let w = select((bw + 1e-3) / (1e-3 + abs(t.y - z) / z * 50.0), 0.0, t.y < 0.0);
            total += t.x * w;
            weight += w;
        }
    }
    if lowest >= UNOCCLUDED {
        discard;
    }
    let ao = select(total / weight, 1.0, weight <= 1e-6);
    var fog = 1.0;
    if p.fog.w > 0.5 {
        fog = clamp((p.fog.x - z) * p.fog.y, p.fog.z, 1.0);
    }
    let k = 1.0 - (1.0 - ao) * p.depth.w * fog * glow_share(frag.xy, z);
    return vec4<f32>(k, k, k, 1.0);
}

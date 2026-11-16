// Screen-space reflections on the wet road (the Graphics page's, a clone
// deviation; see ssr.rs and docs/wet-roads-ssr.md).

struct Params {
    size: vec4<f32>,     // scene w, h; trace w, h
    depth: vec4<f32>,    // near, far, reversed, the pyramid's top level
    proj: vec4<f32>,     // m00, m11, max distance (m), steps
    right: vec4<f32>,    // the view's basis in the world (x right,
    up: vec4<f32>,       // y up, z ahead); right.w the debug view,
    forward: vec4<f32>,  // up.w 1 with the glow sprites, forward.w 1
                         // with Enhanced Reflections' true mirror
    fog: vec4<f32>,      // the main view's FogValue (x, y, z), w 1 when on
    fog_color: vec4<f32>, // w 1 on a wet road (Wet Roads)
    // The ray-traced tier's quarter-rate trace (ssr.rs, docs/rt-reflections.md):
    // xy this frame's pixel of each 2 × 2 (0 or 1), z 1 when the trace runs at
    // half size, w 1 when the history holds the last frame; the last frame's
    // eye (relative to this one's), basis and projection scales (xy).
    jitter: vec4<f32>,
    prev_eye: vec4<f32>,
    prev_right: vec4<f32>,
    prev_up: vec4<f32>,
    prev_forward: vec4<f32>,
    prev_proj: vec4<f32>,
}

@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var hiz_tex: texture_2d<f32>;
@group(0) @binding(2) var color_tex: texture_2d<f32>;
@group(0) @binding(3) var term_tex: texture_2d<f32>;
@group(0) @binding(4) var surface_tex: texture_2d<f32>;
@group(0) @binding(5) var ssr_tex: texture_2d<f32>;
@group(0) @binding(6) var samp: sampler;
@group(0) @binding(7) var<uniform> p: Params;
// The mask's tint (rgb: the term's per-channel weight over term.a) and
// this frame's glow sprites (the streak buffer's add, ssr.rs). The
// surface's z holds its kind with the roughness: road r, a car facing up
// 1 + r, facing down 2 + r (r < 1).
@group(0) @binding(8) var tint_tex: texture_2d<f32>;
@group(0) @binding(9) var glow_tex: texture_2d<f32>;
// The mask's kind (surface z) and view depth (surface w) under each
// trace pixel, as fs_linear read them (the filter's taps).
@group(0) @binding(10) var kinds_tex: texture_2d<f32>;

// Where a ray finds no surface: past anything on screen.
const FAR_AWAY: f32 = 1.0e6;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
}

// The trace and the filter run only where the mask needs them: the
// early depth test against fs_linear's mask depth (0: trace and filter,
// 0.5: filter only, 1: neither; compare Greater) skips the rest, which
// keep the cleared 0 the shaders return there.
@vertex
fn vs_trace(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(q * 2.0 - 1.0, 0.25, 1.0);
}

@vertex
fn vs_filter(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(q * 2.0 - 1.0, 0.75, 1.0);
}

// The scene-sized copy and composite against the world's mask depth: at
// its clear value (compare NotEqual), so only pixels the mask drew run;
// the others keep the scene as it is (what the composite returned there).
@vertex
fn vs_mask(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(q * 2.0 - 1.0, select(1.0, 0.0, p.depth.z > 0.5), 1.0);
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

// The linear depth of a depth value; the sky's FAR_AWAY.
fn lin_or_far(d: f32) -> f32 {
    return select(linear(d), FAR_AWAY, is_sky(d));
}

// The depth of texels c, c + (1, 0), c + (0, 1), c + (1, 1) (in some
// order), clamped to the edge as raw_depth (the sampler's clamp; a
// gather returns the texels unfiltered). The multisampled variant takes
// sample 0 of each instead (ssr.rs).
fn depth4(c: vec2<i32>) -> vec4<f32> {
    return textureGather(depth_tex, samp, (vec2<f32>(c) + 1.0) / p.size.xy); // MULTISAMPLE GATHER
}

// The scene's colour into the copy the passes read.
@fragment
fn fs_copy(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(color_tex, vec2<i32>(frag.xy), 0);
}

// The pyramid's base at the trace's size: the nearest linear depth of the
// scene's pixels under each texel (sky: FAR_AWAY). The scene may be any
// multiple of the trace (SSAA, Render Scale): the texels it covers, at
// most 8 a side.
struct Linear {
    @location(0) z: vec4<f32>,
    @location(1) kinds: vec4<f32>,
    @builtin(frag_depth) mask: f32,
}

@fragment
fn fs_linear(@builtin(position) frag: vec4<f32>) -> Linear {
    let ratio = p.size.xy / p.size.zw;
    let cell = floor(frag.xy);
    let lo = vec2<i32>(floor(cell * ratio + 1e-3));
    let hi = max(vec2<i32>(ceil((cell + 1.0) * ratio - 1e-3)), lo + 1);
    let n = min(hi - lo, vec2<i32>(8));
    var z = FAR_AWAY;
    if n.x >= 2 && n.y >= 2 {
        // The cell's texels four at a time (the last pair of an odd
        // side shifted back onto ones already taken: the same set).
        for (var y = 0; y < n.y; y = y + 2) {
            for (var x = 0; x < n.x; x = x + 2) {
                let d = depth4(lo + min(vec2<i32>(x, y), n - 2));
                z = min(z, min(min(lin_or_far(d.x), lin_or_far(d.y)), min(lin_or_far(d.z), lin_or_far(d.w))));
            }
        }
    } else {
        for (var y = 0; y < n.y; y = y + 1) {
            for (var x = 0; x < n.x; x = x + 1) {
                z = min(z, lin_or_far(raw_depth(lo + vec2<i32>(x, y))));
            }
        }
    }
    // The mask under this trace pixel, as fs_trace and fs_denoise
    // read it (their early outs: the same tests), and for the filter's
    // taps its kind and depth.
    let m = mask_texel(frag.xy);
    let term = textureLoad(term_tex, m, 0);
    let surface = textureLoad(surface_tex, m, 0);
    var mask = 1.0;
    if !(term.a <= 0.002) {
        mask = select(0.5, 0.0, !(term.a <= 0.002 || surface.w <= 0.0));
    }
    if p.right.w > 0.5 {
        // The debug views: every pixel.
        mask = 0.0;
    }
    return Linear(vec4<f32>(z, 0.0, 0.0, 0.0), vec4<f32>(surface.z, surface.w, 0.0, 0.0), mask);
}

// A level from the one below (bound alone): the nearest of its 2 × 2 (3
// wide or high at an odd edge).
@fragment
fn fs_down(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let src = vec2<i32>(textureDimensions(hiz_tex));
    let c = vec2<i32>(frag.xy) * 2;
    let lim = src - 1;
    var ext = vec2<i32>(1, 1);
    if (src.x & 1) == 1 && c.x + 2 == lim.x { ext.x = 2; }
    if (src.y & 1) == 1 && c.y + 2 == lim.y { ext.y = 2; }
    var z = FAR_AWAY;
    for (var y = 0; y <= ext.y; y = y + 1) {
        for (var x = 0; x <= ext.x; x = x + 1) {
            z = min(z, textureLoad(hiz_tex, min(c + vec2<i32>(x, y), lim), 0).r);
        }
    }
    return vec4<f32>(z, 0.0, 0.0, 0.0);
}

fn bayer4(q: vec2<f32>) -> f32 {
    let x = u32(q.x) & 3u;
    let y = u32(q.y) & 3u;
    let m = array<u32, 16>(0u, 8u, 2u, 10u, 12u, 4u, 14u, 6u, 3u, 11u, 1u, 9u, 15u, 7u, 13u, 5u);
    return (f32(m[y * 4u + x]) + 0.5) / 16.0;
}

// A view-space point on the trace's pixel grid (x right, y down).
fn to_trace(q: vec3<f32>) -> vec2<f32> {
    let ndc = vec2<f32>(q.x * p.proj.x, q.y * p.proj.y) / q.z;
    return vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * p.size.zw;
}

// The mask's texel under a trace pixel.
fn mask_texel(frag: vec2<f32>) -> vec2<i32> {
    let s = vec2<i32>(p.size.xy) - 1;
    return clamp(vec2<i32>(frag * p.size.xy / p.size.zw), vec2<i32>(0), s);
}

// A mask texel's kind from its surface's z: true for a car.
fn is_car(kind: f32) -> bool {
    return kind >= 1.0;
}

// The roughness and the world normal the mask holds (x, y; z's sign from
// the kind).
fn mask_rough(surface: vec4<f32>) -> f32 {
    return fract(surface.z);
}

fn mask_normal(surface: vec4<f32>) -> vec3<f32> {
    let z = sqrt(max(1.0 - dot(surface.xy, surface.xy), 0.0));
    return vec3<f32>(surface.xy, select(z, -z, surface.z >= 2.0));
}

// An overlay (the GPS arrow; the mask's tint.a, mod.rs `overlay_draws`)
// at or in front of view depth z on the mask texel c. The game draws the
// arrow in the player's view only (0x631767), so no reflection holds it.
fn overlay_on(c: vec2<i32>, z: f32) -> bool {
    let s = vec2<i32>(p.size.xy) - 1;
    let q = clamp(c, vec2<i32>(0), s);
    if textureLoad(tint_tex, q, 0).a < 0.5 {
        return false;
    }
    return textureLoad(surface_tex, q, 0).w <= z + max(0.35, 0.04 * z);
}

// A hit whose colour would be (or, through the bilinear lookup and the
// resolve's edge, take in) an overlay's: the texel and its neighbours one
// trace texel away.
fn overlay_hit(c: vec2<i32>, z: f32) -> bool {
    let r = max(vec2<i32>(ceil(p.size.xy / p.size.zw)), vec2<i32>(1));
    return overlay_on(c, z) || overlay_on(c + vec2<i32>(r.x, 0), z) || overlay_on(c - vec2<i32>(r.x, 0), z)
        || overlay_on(c + vec2<i32>(0, r.y), z) || overlay_on(c - vec2<i32>(0, r.y), z);
}

// world.efx's fog factor at view depth z (1 with the fog off).
fn fog_at(z: f32) -> f32 {
    if p.fog.w < 0.5 {
        return 1.0;
    }
    return clamp((p.fog.x - z) * p.fog.y, p.fog.z, 1.0);
}

// The streak buffer's steady state for a glow drawn every frame: the
// glow / (1 − the per-frame decay, 0x5CBB6A: 120, 90, 170 of 255).
const GLOW_STEADY: vec3<f32> = vec3<f32>(255.0 / 135.0, 255.0 / 165.0, 255.0 / 85.0);

// The trace pixel a fragment of the trace pass stands for: itself, or at the
// quarter rate (a half-size pass, p.jitter.z) this frame's pixel of its 2 × 2.
fn trace_frag(f: vec4<f32>) -> vec4<f32> {
    if p.jitter.z < 0.5 {
        return f;
    }
    return vec4<f32>(floor(f.xy) * 2.0 + 0.5 + p.jitter.xy, f.zw);
}

// The ray march (the trace's size): from the road's (or a car's) point
// along the reflected view, a stackless walk of the depth pyramid — a
// whole cell the ray stays in front of is skipped and the next one taken
// a level up, one it may enter is looked at a level down; at the base
// level a ray behind the surface by less than its thickness hits. rgb: the
// colour at the hit as the reflection sees it (the scene's, its fog taken
// off, plus the glow sprites there; a hit on a car as drawn); a: the
// confidence (0: a miss; faded at the screen's edges, toward the ray's
// reach and for rays turning to the camera).
@fragment
fn fs_trace(@builtin(position) frag_in: vec4<f32>) -> @location(0) vec4<f32> {
    let frag = trace_frag(frag_in);
    let m = mask_texel(frag.xy);
    let term = textureLoad(term_tex, m, 0);
    let surface = textureLoad(surface_tex, m, 0);
    let car = is_car(surface.z);
    let z = surface.w;
    if term.a <= 0.002 || z <= 0.0 {
        return vec4<f32>(0.0);
    }
    let uv = frag.xy / p.size.zw;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let origin = vec3<f32>(ndc.x / p.proj.x * z, ndc.y / p.proj.y * z, z);
    let nw = mask_normal(surface);
    var n = normalize(vec3<f32>(dot(nw, p.right.xyz), dot(nw, p.up.xyz), dot(nw, p.forward.xyz)));
    let v = normalize(origin);
    // An interpolated car normal may lean away from the eye at a
    // silhouette: turned to face it.
    n = normalize(n - v * max(dot(n, v) + 0.05, 0.0));
    let r = reflect(v, n);
    // The ray's reach: the distance, and not behind the near plane.
    var reach = p.proj.z;
    if r.z < 0.0 {
        reach = min(reach, (origin.z - p.depth.x * 1.5) / -r.z);
    }
    if reach <= 0.05 {
        return vec4<f32>(0.0);
    }
    // A ray turning this much to the camera has no confidence whatever it
    // meets (`facing` below is 0) and the filter takes every tap by its
    // confidence: it is not marched. (After the reach: the ray list's
    // `rt_ray` takes the head above it as written; rt/trace.rs.)
    if p.right.w < 1.5 && r.z <= -0.4 {
        return vec4<f32>(0.0);
    }
    let end = origin + r * reach;
    let s0 = to_trace(origin);
    let s1 = to_trace(end);
    let iz0 = 1.0 / origin.z;
    let iz1 = 1.0 / end.z;
    let delta = s1 - s0;
    let dist = length(delta);
    if dist < 1.0 {
        return vec4<f32>(0.0);
    }
    let dir = delta / dist;
    // Where the line leaves the screen.
    let size = p.size.zw;
    var t_max = dist;
    if dir.x > 1e-6 { t_max = min(t_max, (size.x - s0.x) / dir.x); }
    if dir.x < -1e-6 { t_max = min(t_max, -s0.x / dir.x); }
    if dir.y > 1e-6 { t_max = min(t_max, (size.y - s0.y) / dir.y); }
    if dir.y < -1e-6 { t_max = min(t_max, -s0.y / dir.y); }
    let top = i32(p.depth.w);
    let steps = i32(p.proj.w);
    var t = 1.0 + bayer4(frag.xy);
    var level = 0;
    // The ray's depth at t, carried from the step before when t moved on
    // to its t_next (the same expression's value).
    var za = 1.0 / mix(iz0, iz1, t / dist);
    var hit = false;
    var hit_car = false;
    // The hit's mask texel is a car's, whatever its depth (a car's lower
    // edge against the pyramid's nearest depth, its neon glow over it).
    var near_car = false;
    for (var i = 0; i < steps; i = i + 1) {
        if t >= t_max {
            break;
        }
        let pos = s0 + dir * t;
        let cell_size = f32(1 << u32(level));
        let cell = floor(pos / cell_size);
        let bound = (cell + select(vec2<f32>(0.0), vec2<f32>(1.0), dir > vec2<f32>(0.0))) * cell_size;
        var step = 1e9;
        if abs(dir.x) > 1e-6 { step = min(step, (bound.x - pos.x) / dir.x); }
        if abs(dir.y) > 1e-6 { step = min(step, (bound.y - pos.y) / dir.y); }
        let t_next = min(t + max(step, 0.0) + 0.01, t_max);
        let zb = 1.0 / mix(iz0, iz1, t_next / dist);
        let ray_near = min(za, zb);
        let ray_far = max(za, zb);
        // The level's size (as textureDimensions: the base's halved, at
        // least 1), without a query per step.
        let cells = max(vec2<i32>(p.size.zw) >> vec2<u32>(u32(level)), vec2<i32>(1)) - 1;
        let cell_z = textureLoad(hiz_tex, clamp(vec2<i32>(cell), vec2<i32>(0), cells), level).r;
        if ray_far < cell_z {
            // The whole segment in front of the cell's nearest surface.
            t = t_next;
            za = zb;
            level = min(level + 1, top);
        } else if level > 0 {
            level = level - 1;
        } else {
            // Crossing the surface, or behind it by less than its
            // thickness: a hit; further behind, the ray passes behind
            // something thin and goes on.
            if ray_near - cell_z < max(0.35, 0.04 * cell_z) {
                let mid = (t + t_next) * 0.5;
                let hm = mask_texel(s0 + dir * mid);
                let hsurf = textureLoad(surface_tex, hm, 0);
                let hs = hsurf.w;
                let hk = hsurf.z;
                // The mask's own surface where the ray is (its depth).
                let on_mask = hs > 0.0 && abs(hs - cell_z) <= 0.04 * hs + 0.2;
                // From the road: the road itself (grazing rays against the
                // pyramid's nearest depth) is no hit; a car on it is.
                let road_self = !car && on_mask && !is_car(hk) && textureLoad(term_tex, hm, 0).a > 0.002;
                // From a car: its own body next to the start is no hit (an
                // interpolated normal's ray grazing it); the road and
                // other cars are.
                let car_self = car && on_mask && is_car(hk) && abs(cell_z - z) < 2.0;
                // An overlay (the GPS arrow) is no hit: the ray goes on
                // past it, as past something thin (docs/gps-reflection.md).
                // (Looked up only when it decides: no side effects.)
                if !road_self && !car_self && !overlay_hit(hm, cell_z) {
                    t = mid;
                    hit = true;
                    hit_car = on_mask && is_car(hk);
                    near_car = hs > 0.0 && is_car(hk);
                    break;
                }
            }
            t = t_next;
            za = zb;
        }
    }
    if p.right.w > 1.5 {
        return vec4<f32>(select(0.0, 1.0, hit), t / dist, dist / 64.0, t_max / 64.0);
    }
    if !hit {
        return vec4<f32>(0.0);
    }
    let at = (s0 + dir * t) / size;
    let edge = smoothstep(0.0, 0.07, min(min(at.x, 1.0 - at.x), min(at.y, 1.0 - at.y)));
    let travel = 1.0 - smoothstep(0.7, 1.0, t / dist);
    let facing = smoothstep(-0.4, -0.05, r.z);
    var colour = textureSampleLevel(color_tex, samp, at, 0.0).rgb;
    let hit_z = 1.0 / mix(iz0, iz1, t / dist);
    if !hit_car && p.fog.w > 0.5 {
        // The scene's colour carries the fog of the hit's own distance
        // from the eye; the reflection gets its fog once, where it is
        // seen: the planar capture it replaces is unfogged (worldnofog),
        // and the composite's weight holds the road's own fog. The floor
        // keeps far, additive (unfogged) surfaces from being blown up.
        let f = fog_at(hit_z);
        colour = max((colour - p.fog_color.rgb * (1.0 - f)) / max(f, 0.35), vec3<f32>(0.0));
    }
    // The hit in view space (for its path length).
    let hn = vec2<f32>(at.x * 2.0 - 1.0, 1.0 - at.y * 2.0);
    let hit_p = vec3<f32>(hn.x / p.proj.x * hit_z, hn.y / p.proj.y * hit_z, hit_z);
    let path = length(hit_p - origin);
    if p.up.w > 0.5 && !near_car {
        // The glow sprites there, as the streak buffer adds them. Not on a
        // car (docs/chrome-flicker.md): the sprites over a car are its
        // own (neon tubes, lamps), which the native road capture never
        // holds; and only from 2–4 m on: a road ray reaching up under a
        // car (its neon tube, an additive mesh outside the mask) took the
        // underglow's sprites and the vertical filter drew them as magenta
        // streaks behind the car. The world's lamp glows are further off.
        colour += min(textureSampleLevel(glow_tex, samp, at, 0.0).rgb * GLOW_STEADY, vec3<f32>(1.0)) * smoothstep(2.0, 4.0, path);
    }
    if !car && !hit_car && p.fog_color.w < 0.5 {
        // A dry road (docs/chrome-flicker.md): the native road shows its
        // planar capture at the texture's authored strength, rain or not.
        // The traced scene is brighter than that capture (1.24–1.48× the
        // native reflection's mean in explore / race start / night), so it
        // is held to the planar term's luminance here; darker detail, and
        // the cars it hits (not in the native capture), stay as traced.
        let lp = dot(term.rgb, vec3<f32>(0.299, 0.587, 0.114));
        let lc = dot(colour, vec3<f32>(0.299, 0.587, 0.114));
        colour = colour * min(1.0, lp / max(lc, 1e-5));
    }
    if car && p.fog.w > 0.5 {
        // A car's reflection: the cube it replaces is fogged by the
        // distance from the car (its views' fog); the traced path's.
        let f = fog_at(path);
        colour = colour * f + p.fog_color.rgb * (1.0 - f);
    }
    return vec4<f32>(colour, edge * travel * facing);
}

// The spatial filter (no history): an ellipse of taps around each trace
// pixel, wider the rougher the surface and stretched along the screen's
// vertical, weighted by the taps' confidence, depth and kind (road taps
// never blur into a car's, nor the other way).
@fragment
fn fs_denoise(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let lim = vec2<i32>(p.size.zw) - 1;
    let c = vec2<i32>(frag.xy);
    let centre = textureLoad(ssr_tex, c, 0);
    let m = mask_texel(frag.xy);
    let surface = textureLoad(surface_tex, m, 0);
    if textureLoad(term_tex, m, 0).a <= 0.002 {
        return centre;
    }
    if p.right.w > 1.5 {
        return centre;
    }
    let car = is_car(surface.z);
    let scale = p.size.w / 540.0;
    let radius = (1.0 + mask_rough(surface) * 22.0) * scale;
    var sum = centre.rgb * centre.a;
    var conf = centre.a;
    var weights = 1.0;
    for (var i = 0; i < 12; i = i + 1) {
        let fi = f32(i) + 0.5;
        let a = fi * 2.39996;
        let rr = sqrt(fi / 12.0);
        let o = vec2<f32>(cos(a) * rr * radius, sin(a) * rr * radius * 3.0);
        let q = clamp(c + vec2<i32>(round(o)), vec2<i32>(0), lim);
        let s = textureLoad(ssr_tex, q, 0);
        // The mask's kind and depth under tap q (mask_texel(q + 0.5)).
        let qs = textureLoad(kinds_tex, q, 0).xy;
        let same = is_car(qs.x) == car;
        let dz = abs(qs.y - surface.w) / max(surface.w, 0.1);
        let w = exp(-1.5 * rr * rr) * select(0.0, 1.0, dz < 0.05 && qs.y > 0.0 && same);
        sum += s.rgb * s.a * w;
        conf += s.a * w;
        weights += w;
    }
    return vec4<f32>(sum / max(conf, 1e-4), conf / weights);
}

// Back into the scene (its size): where the mask's surface is in front,
// the reflection term the world (or car) pass added is swapped for the
// traced one by its confidence; the planar capture's (or the cube's)
// bright authored highlights (lamp flares, headlight pools) stay. Alpha
// (the bleach mask) is kept. -> the change to the scene's colour at c,
// w 1 where there is one (0: the scene stays as it is).
fn composite_change(c: vec2<i32>, frag: vec2<f32>) -> vec4<f32> {
    let term = textureLoad(term_tex, c, 0);
    let tint = textureLoad(tint_tex, c, 0);
    let car = is_car(textureLoad(surface_tex, c, 0).z);
    if term.a <= 0.002 {
        return vec4<f32>(0.0);
    }
    let surface = textureLoad(surface_tex, c, 0);
    let d = raw_depth(c);
    if is_sky(d) {
        return vec4<f32>(0.0);
    }
    // The road: its mask's depth is the scene's (anything nearer was
    // drawn over it). A car: its mask is not behind the scene (its glass
    // writes no depth: the scene's is the cabin behind it).
    let gap = surface.w - linear(d);
    let tol = 0.02 * surface.w + 0.15;
    if (!car && abs(gap) > tol) || (car && gap > tol) {
        return vec4<f32>(0.0);
    }
    let s = textureSampleLevel(ssr_tex, samp, frag / p.size.xy, 0.0);
    var conf = clamp(s.a, 0.0, 1.0);
    if !car && p.forward.w < 0.5 {
        // The native capture flips each prop about its own origin (no true
        // mirror): where the traced image (a true one) fades into it, the
        // two do not line up. The fade is kept short so the doubled image
        // shows in a narrow band only.
        conf = smoothstep(0.3, 0.7, conf);
    }
    let planar = term.rgb;
    var refl = mix(planar, s.rgb, conf);
    let keep = smoothstep(0.55, 1.0, max(planar.r, max(planar.g, planar.b)));
    refl = mix(refl, max(refl, planar), keep);
    return vec4<f32>(term.a * tint.rgb * (refl - planar), 1.0);
}

@fragment
fn fs_composite(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let c = vec2<i32>(frag.xy);
    let base = textureLoad(color_tex, c, 0);
    if p.right.w > 1.5 {
        return textureSampleLevel(ssr_tex, samp, frag.xy / p.size.xy, 0.0);
    }
    if p.right.w > 0.5 {
        let term = textureLoad(term_tex, c, 0);
        let car = is_car(textureLoad(surface_tex, c, 0).z);
        let sd = textureSampleLevel(ssr_tex, samp, frag.xy / p.size.xy, 0.0);
        let dd = raw_depth(c);
        let surf = textureLoad(surface_tex, c, 0);
        let ok = select(0.0, 1.0, abs(linear(dd) - surf.w) <= 0.02 * surf.w + 0.15);
        return vec4<f32>(term.a, sd.a, ok * 0.5 + select(0.0, 0.5, car), 1.0);
    }
    let change = composite_change(c, frag.xy);
    if change.w == 0.0 {
        return base;
    }
    return vec4<f32>(base.rgb + change.rgb, base.a);
}

// The same change added onto the scene by the blend (ONE / ONE, the
// alpha kept): no copy of the scene to read. A half-float scene only (the
// change is signed); its sum is rounded once by the blend instead of by
// the target's write, which may differ in the last bit of a half.
@fragment
fn fs_composite_add(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(composite_change(vec2<i32>(frag.xy), frag.xy).rgb, 0.0);
}

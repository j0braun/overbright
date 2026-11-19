// Motion blur (Camera / Per-Object) and depth of field (Cinematic): see
// docs/notes/graphics/motionblur-dof.md; clone deviations, opt-in. The native speed blur
// (0x5CC470, postfx.wgsl) is not this file.
//
// Motion blur: a velocity pass (camera motion from the depth, rigid car
// motion inside the drawn cars' boxes, the player's car masked), tile max,
// neighbour max, then a depth-aware gather along the neighbourhood's
// dominant motion (McGuire et al. 2012, "A reconstruction filter for
// plausible motion blur"), written over the scene before the additive
// layers (particles, glow sprites, horizon fog, streaks, HUD). Wheels spin:
// their pixels are blurred along an arc around the axle.
// Depth of field: a gather on a golden-angle disc, circle of confusion
// from the depth against a focus band (the focused car's depth range).

struct Car {
    // camera-relative world -> car space (this frame)
    inv: mat4x4<f32>,
    // car space -> camera-relative world (this frame)
    fwd: mat4x4<f32>,
    // camera-relative point now -> where it was a frame ago (relative to this eye)
    motion: mat4x4<f32>,
    // car-space box; lo.w: 1 the player's car (sharp), hi.w: this car's
    // motion scale (shutter / frame time, 0 = camera motion only)
    lo: vec4<f32>,
    hi: vec4<f32>,
    // wheel centres (xyz) and radii (w), car space
    wheel: array<vec4<f32>, 4>,
    wheel_hw: vec4<f32>,
    // each wheel's spin over the shutter (radians, 0 = no arc)
    spin: vec4<f32>,
    // camera-relative bounding sphere (xyz centre, w radius²)
    sphere: vec4<f32>,
};

struct Uniforms {
    // clip (this frame, as drawn) -> camera-relative world
    clip_to_rel: mat4x4<f32>,
    // camera-relative world -> last frame's clip (the same jitter)
    prev_rel_to_clip: mat4x4<f32>,
    // camera-relative world -> this frame's clip
    rel_to_clip: mat4x4<f32>,
    // size, 1 / size
    size: vec4<f32>,
    // camera motion scale (shutter / frame time × speed ramp), max blur (px),
    // car count, mode (1 camera, 2 per-object)
    blur: vec4<f32>,
    // tile size (px), tiles x, tiles y, taps
    tiles: vec4<f32>,
    // focus near, focus far (m), max circle of confusion (px), taps
    dof: vec4<f32>,
    // reversed depth (0/1), noise frame, debug view, 0
    misc: vec4<f32>,
};

@group(0) @binding(0) var color_tex: texture_2d<f32>;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var aux_a: texture_2d<f32>;
@group(0) @binding(3) var aux_b: texture_2d<f32>;
@group(0) @binding(4) var linear_clamp: sampler;
@group(0) @binding(5) var<uniform> u: Uniforms;
@group(0) @binding(6) var<uniform> cars: array<Car, 32>;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

fn size_i() -> vec2<i32> {
    return vec2<i32>(u.size.xy);
}

fn load_depth(p: vec2<i32>) -> f32 {
    return textureLoad(depth_tex, clamp(p, vec2<i32>(0), size_i() - 1), 0);
}

// The camera-relative point drawn at pixel centre `p` with depth `d`.
fn rel_of(p: vec2<f32>, d: f32) -> vec3<f32> {
    let uv = p * u.size.zw;
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    let r = u.clip_to_rel * ndc;
    return r.xyz / r.w;
}

fn to_px(clip: vec4<f32>) -> vec2<f32> {
    let ndc = clip.xy / max(clip.w, 1e-6);
    return vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * u.size.xy;
}

// Interleaved gradient noise (Jimenez 2014), 0..1.
fn noise(p: vec2<f32>) -> f32 {
    let q = p + u.misc.y * vec2<f32>(5.588238, 5.588238);
    return fract(52.9829189 * fract(dot(q, vec2<f32>(0.06711056, 0.00583715))));
}

// ---- motion blur ----

// Pixel `p`'s blur (px, x right y down, the whole streak), its distance (m)
// and its kind: 0 world / moving car, 1 masked (sharp), 2 + 4·car + wheel
// a spinning wheel.
@fragment
fn fs_velocity(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(frag.xy);
    let d = load_depth(p);
    let rel = rel_of(frag.xy, d);
    var v = vec2<f32>(0.0);
    var kind = 0.0;
    var found = false;
    let count = i32(u.blur.z);
    for (var i = 0; i < count; i++) {
        let sphere = cars[i].sphere;
        let to = rel - sphere.xyz;
        if dot(to, to) > sphere.w {
            continue;
        }
        let local = (cars[i].inv * vec4<f32>(rel, 1.0)).xyz;
        let lo = cars[i].lo;
        let hi = cars[i].hi;
        if !(all(local >= lo.xyz) && all(local <= hi.xyz)) {
            continue;
        }
        found = true;
        let player = lo.w > 0.5;
        let scale = hi.w;
        // the wheel this point is in, if any
        var wheel = -1;
        let hw = cars[i].wheel_hw;
        for (var w = 0; w < 4; w++) {
            let c = cars[i].wheel[w];
            let o = local - c.xyz;
            if abs(o.y) < hw[w] && dot(o.xz, o.xz) < c.w * c.w {
                wheel = w;
            }
        }
        if player {
            kind = 1.0;
        }
        if scale > 0.0 && !player {
            let prev = to_px(u.prev_rel_to_clip * (cars[i].motion * vec4<f32>(rel, 1.0)));
            v = (frag.xy - prev) * scale;
        } else if !player {
            let prev = to_px(u.prev_rel_to_clip * vec4<f32>(rel, 1.0));
            v = (frag.xy - prev) * u.blur.x;
        }
        if wheel >= 0 && abs(cars[i].spin[wheel]) > 0.02 {
            kind = 2.0 + f32(i * 4 + wheel);
        }
        break;
    }
    if !found {
        let prev = to_px(u.prev_rel_to_clip * vec4<f32>(rel, 1.0));
        v = (frag.xy - prev) * u.blur.x;
    }
    let len = length(v);
    if len > u.blur.y {
        v *= u.blur.y / len;
    }
    if len < 0.25 || kind == 1.0 {
        v = vec2<f32>(0.0);
    }
    return vec4<f32>(v, length(rel), kind);
}

// The longest blur of each tile, from every other pixel each way (the
// motion field is smooth; a quarter of the loads). For speed, in two steps, so
// many more fragments share the loads: fs_tile_rows takes each sampled
// row of a tile (its first longest, scan order), fs_tile the first
// longest of those rows, top down: the same pick as one scan of the tile
// (a later equal length never replaces the first).
@fragment
fn fs_tile_rows(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let t = i32(u.tiles.x);
    let h = (t + 1) / 2;
    let r = vec2<i32>(frag.xy);
    let q0 = vec2<i32>(r.x * t, (r.y / h) * t + (r.y % h) * 2);
    let size = size_i();
    var best = vec2<f32>(0.0);
    var best_len = 0.0;
    if q0.y < size.y {
        for (var x = 0; x < t; x += 2) {
            let q = q0 + vec2<i32>(x, 0);
            if q.x >= size.x {
                continue;
            }
            let v = textureLoad(aux_a, q, 0).xy;
            let l = dot(v, v);
            if l > best_len {
                best_len = l;
                best = v;
            }
        }
    }
    return vec4<f32>(best, 0.0, 0.0);
}

@fragment
fn fs_tile(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let t = i32(u.tiles.x);
    let h = (t + 1) / 2;
    let c = vec2<i32>(frag.xy);
    var best = vec2<f32>(0.0);
    var best_len = 0.0;
    for (var y = 0; y < h; y++) {
        let v = textureLoad(aux_a, vec2<i32>(c.x, c.y * h + y), 0).xy;
        let l = dot(v, v);
        if l > best_len {
            best_len = l;
            best = v;
        }
    }
    return vec4<f32>(best, 0.0, 0.0);
}

// The longest blur among a tile and its eight neighbours.
@fragment
fn fs_neighbour(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let c = vec2<i32>(frag.xy);
    let n = vec2<i32>(u.tiles.yz);
    var best = vec2<f32>(0.0);
    var best_len = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let q = clamp(c + vec2<i32>(x, y), vec2<i32>(0), n - 1);
            let v = textureLoad(aux_a, q, 0).xy;
            let l = dot(v, v);
            if l > best_len {
                best_len = l;
                best = v;
            }
        }
    }
    return vec4<f32>(best, 0.0, 0.0);
}

fn cone(dist: f32, len: f32) -> f32 {
    return clamp(1.0 - dist / max(len, 1e-3), 0.0, 1.0);
}

fn cylinder(dist: f32, len: f32) -> f32 {
    return 1.0 - smoothstep(0.95 * len, 1.05 * len + 1e-3, dist);
}

// 1 when distance `a` is in front of `b` (soft over a few per cent).
fn in_front(a: f32, b: f32) -> f32 {
    return clamp(1.0 - (a - b) / (0.03 * b + 0.05), 0.0, 1.0);
}

fn load_color(p: vec2<i32>) -> vec4<f32> {
    return textureLoad(color_tex, clamp(p, vec2<i32>(0), size_i() - 1), 0);
}

// A wheel's pixel blurred along the arc of its spin about the axle (and
// by its car's own motion): taps that leave this wheel are dropped.
fn wheel_blur(frag: vec2<f32>, centre: vec4<f32>, kind: f32) -> vec4<f32> {
    let id = i32(kind) - 2;
    let i = id / 4;
    let w = id % 4;
    let d = load_depth(vec2<i32>(frag));
    let rel = rel_of(frag, d);
    let local = (cars[i].inv * vec4<f32>(rel, 1.0)).xyz;
    let c = cars[i].wheel[w].xyz;
    let o = local - c;
    let span = cars[i].spin[w];
    let n = 12;
    let j = noise(frag);
    var sum = centre;
    var weight = 1.0;
    let v = textureLoad(aux_a, vec2<i32>(frag), 0).xy;
    for (var k = 0; k < n; k++) {
        let t = (f32(k) + j) / f32(n) - 0.5;
        let a = span * t;
        let ca = cos(a);
        let sa = sin(a);
        let r = vec3<f32>(o.x * ca - o.z * sa, o.y, o.x * sa + o.z * ca);
        let q = to_px(u.rel_to_clip * (cars[i].fwd * vec4<f32>(c + r, 1.0))) + v * t;
        let qi = vec2<i32>(q);
        if any(qi < vec2<i32>(0)) || any(qi >= size_i()) {
            continue;
        }
        if abs(textureLoad(aux_a, qi, 0).w - kind) > 0.5 {
            continue;
        }
        sum += textureSampleLevel(color_tex, linear_clamp, q * u.size.zw, 0.0);
        weight += 1.0;
    }
    return sum / weight;
}

// For speed, the gather over a single-sampled scene leaves the pixels it would
// return unchanged as they are (the scene holds what its copy holds there)
// rather than writing them again. A multisampled scene takes the resolved
// colour into every sample, so it keeps the write.
override SKIP_UNCHANGED: bool = false;

@fragment
fn fs_gather(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(frag.xy);
    let centre = load_color(p);
    let x = textureLoad(aux_a, p, 0);
    let debug = u.misc.z;
    if debug > 0.5 {
        if x.w >= 1.5 {
            return vec4<f32>(0.0, 1.0, 0.0, centre.a);
        }
        if x.w > 0.5 {
            return vec4<f32>(1.0, 0.0, 0.0, centre.a);
        }
        return vec4<f32>(abs(x.xy) / 32.0, 0.0, centre.a);
    }
    if x.w > 0.5 && x.w < 1.5 {
        if SKIP_UNCHANGED {
            discard;
        }
        return centre;
    }
    if x.w >= 1.5 {
        return wheel_blur(frag.xy, centre, x.w);
    }
    let t = i32(u.tiles.x);
    let vn = textureLoad(aux_b, p / t, 0).xy;
    let ln = length(vn);
    if ln < 0.5 {
        if SKIP_UNCHANGED {
            discard;
        }
        return centre;
    }
    let lx = length(x.xy);
    let zx = x.z;
    let n = i32(u.tiles.w);
    let j = noise(frag.xy) - 0.5;
    // McGuire: the centre weighs 1 / its own blur.
    var wsum = 1.0 / max(lx, 1.0);
    var sum = centre * wsum;
    for (var k = 0; k < n; k++) {
        if k == n / 2 {
            continue;
        }
        // taps across the whole streak, centred on the pixel
        let s = mix(-0.5, 0.5, (f32(k) + j + 0.5) / f32(n));
        let q = frag.xy + vn * s;
        let qi = vec2<i32>(q);
        if any(qi < vec2<i32>(0)) || any(qi >= size_i()) {
            continue;
        }
        let y = textureLoad(aux_a, qi, 0);
        let dist = length(vn * s);
        let ly = length(y.xy);
        let zy = y.z;
        let fore = in_front(zy, zx);
        let back = in_front(zx, zy);
        // the half-lengths: a pixel's streak spreads half its blur each way
        let w = fore * cone(dist, ly * 0.5) + back * cone(dist, lx * 0.5) + cylinder(dist, ly * 0.5) * cylinder(dist, lx * 0.5) * 2.0;
        sum += load_color(qi) * w;
        wsum += w;
    }
    let r = sum / wsum;
    // the scene's alpha (the bleach mask) stays the pixel's own
    return vec4<f32>(r.rgb, centre.a);
}

// ---- depth of field ----

fn coc(z: f32) -> f32 {
    let near = u.dof.x;
    let far = u.dof.y;
    let m = u.dof.z;
    if z > far {
        return m * clamp((z - far) / max(z, 1e-3) * 1.6, 0.0, 1.0);
    }
    if z < near {
        return m * clamp((near - z) / max(z, 0.05) * 1.2, 0.0, 1.0);
    }
    return 0.0;
}

// Each pixel's distance (m) and circle of confusion (px), once: the
// composite and the half-size pass read these instead of the
// (multisampled) depth.
@fragment
fn fs_coc(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let d = load_depth(vec2<i32>(frag.xy));
    let z = length(rel_of(frag.xy, d));
    return vec4<f32>(z, coc(z), 0.0, 0.0);
}

// The scene at half size (each texel a 2 x 2 block's mean): the gather's
// taps read it bilinearly, a 4 x 4 footprint each.
@fragment
fn fs_down(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(frag.xy) * 2;
    return (load_color(p) + load_color(p + vec2<i32>(1, 0)) + load_color(p + vec2<i32>(0, 1)) + load_color(p + vec2<i32>(1, 1))) * 0.25;
}

// The distance (m) and circle of confusion (px) at half size, once for the
// gather: the block's nearest pixel (a near edge wins the block).
@fragment
fn fs_coc_half(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(frag.xy) * 2;
    let m = size_i() - 1;
    var best = textureLoad(aux_a, min(p, m), 0);
    for (var i = 1; i < 4; i++) {
        let y = textureLoad(aux_a, min(p + vec2<i32>(i & 1, i >> 1), m), 0);
        if y.x < best.x {
            best = y;
        }
    }
    return best;
}

// The gather at half size (each texel the centre of a 2 x 2 block; bound:
// the half-size scene and the half-size distance / circle of confusion):
// rgb the blurred colour, a how much blurred foreground covers it.
@fragment
fn fs_dof_half(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let full = frag.xy * 2.0;
    let half_size = vec2<i32>((size_i() + 1) / 2);
    let x = textureLoad(aux_a, vec2<i32>(frag.xy), 0);
    let zx = x.x;
    let cx = x.y;
    let n = i32(u.dof.w);
    let radius = u.dof.z;
    // a fixed golden-angle spiral (a per-texel turn made the fetches
    // incoherent and the result grainy), each tap's direction stepped by
    // the golden angle's rotation
    var dir = vec2<f32>(1.0, 0.0);
    let step = vec2<f32>(cos(2.3999632), sin(2.3999632));
    let c0 = textureSampleLevel(color_tex, linear_clamp, full * u.size.zw, 0.0).rgb;
    // the centre weighs little: a block on a sharp edge takes its taps
    var sum = vec4<f32>(c0 * 0.05, 0.05);
    var near = 0.0;
    for (var k = 0; k < n; k++) {
        let f = (f32(k) + 0.5) / f32(n);
        let r = sqrt(f) * radius;
        let q = full + dir * r;
        dir = vec2<f32>(dir.x * step.x - dir.y * step.y, dir.x * step.y + dir.y * step.x);
        let qi = clamp(vec2<i32>(q * 0.5), vec2<i32>(0), half_size - 1);
        let y = textureLoad(aux_a, qi, 0);
        // behind the pixel: no more than the pixel's own blur (a sharp car
        // keeps its edges); in front: its own blur spreads over the pixel
        let front = y.x < zx * 0.97 - 0.05;
        let reach = select(min(y.y, cx), y.y, front);
        let w = clamp(reach - r + 1.0, 0.0, 1.0);
        if front {
            near = max(near, w * clamp(y.y * 0.5, 0.0, 1.0));
        }
        let c = textureSampleLevel(color_tex, linear_clamp, q * u.size.zw, 0.0).rgb;
        // bokeh-lite: bright taps weigh a little more within the display
        // range (to 1.3 times at white); over white (an HDR scene's lamps)
        // they weigh less, as 1 / (1 + luma) (Karis' average). Before,
        // the weight rose without bound over white, and even a plain average
        // spreads a lamp at 10 over its circle of confusion above white: a
        // flat white disc with a hard rim over the blurred background, the
        // lamp looking sharp.
        let l = dot(c, vec3<f32>(0.299, 0.587, 0.114));
        let b = w * (1.0 + 2.0 * clamp(l - 0.85, 0.0, 0.15)) / (1.0 + max(l - 1.0, 0.0));
        sum += vec4<f32>(c * b, b);
    }
    return vec4<f32>(sum.rgb / sum.w, near);
}

// The half-size gather over the scene: where the pixel's own circle of
// confusion or a blurred foreground covers it; in focus the pixel itself.
@fragment
fn fs_dof(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(frag.xy);
    let centre = load_color(p);
    let x = textureLoad(aux_a, p, 0);
    let z = x.x;
    let cx = x.y;
    if u.misc.z > 0.5 {
        return vec4<f32>(cx / u.dof.z, select(0.0, 1.0, z < u.dof.x), 0.0, centre.a);
    }
    let h = textureSampleLevel(aux_b, linear_clamp, frag.xy * u.size.zw, 0.0);
    let t = max(clamp((cx - 0.5) / 1.5, 0.0, 1.0), h.a);
    return vec4<f32>(mix(centre.rgb, h.rgb, t), centre.a);
}

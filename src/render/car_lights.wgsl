// Next-Gen car lights in the air and on the lens (docs/notes/graphics/car-lights.md; a
// clone deviation, opt-in; every constant inferred, by eye).
//
// fs_march (reduced size): per pixel the light the night air scatters toward
// the eye along the view ray up to the scene's depth:
//  * every nearby car's headlight cone: the ray clipped to the projector's
//    pyramid (view 9's axes and scales, `headlight_uniform`), marched with an
//    interleaved-gradient offset; per step both lamps' HEADLIGHTS cookie ×
//    HeadlightClipTex (the native beam's own pattern and reach) ×
//    Henyey–Greenstein forward scattering × a drifting density noise;
//  * every lamp (head, tail, brake) as a point light: the closed-form
//    in-scattering integral of 1 / (h² + s²) along the ray, shaped by the
//    lamp's lobe;
//  faded by the native fog and the air's extinction. Alpha = the distance
//  marched (for the upsample).
// fs_up: depth-aware upsample, added to the scene.
// vs_streak / fs_streak: a lens streak and soft core over each visible lamp
// (occlusion: scene depth taps around it), added to the scene.

struct Spot {
    // camera-relative lamp midpoint, w half the lamps' separation
    position: vec4<f32>,
    // right (w: x scale), up (w: y scale), forward
    right: vec4<f32>,
    up: vec4<f32>,
    dir: vec4<f32>,
    colour: vec4<f32>,
};

struct Point {
    // camera-relative position, w kind (0 head, 1 tail, 2 brake)
    position: vec4<f32>,
    // rgb colour, a the air's density at the lamp (`air`, made on the CPU: car_lights::air_at)
    colour: vec4<f32>,
    facing: vec4<f32>,
};

struct Uniforms {
    clip_to_rel: mat4x4<f32>,
    rel_to_clip: mat4x4<f32>,
    size: vec4<f32>,
    map: vec4<f32>,
    // density, spots, points, reversed
    params: vec4<f32>,
    fog: vec4<f32>,
    // spot gain, point gain, g, time
    gains: vec4<f32>,
    // the eye (world), streak gain
    eye: vec4<f32>,
    // The point lights' tiles across, tile size (reduced pixels)
    tiles: vec4<f32>,
    // The aureole's cosines (`Look::aureole`), 0, 0
    shape: vec4<f32>,
};

@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var<uniform> u: Uniforms;
@group(0) @binding(2) var<storage, read> spots: array<Spot>;
@group(0) @binding(3) var<storage, read> points: array<Point>;
@group(0) @binding(4) var low_tex: texture_2d<f32>;
@group(0) @binding(5) var cookie_tex: texture_2d<f32>;
@group(0) @binding(6) var cookie_samp: sampler;
// Per tile the points whose halo can reach it (car_lights::point_tiles).
@group(0) @binding(7) var<storage, read> tiles: array<vec2<u32>>;
@group(0) @binding(8) var noise_tex: texture_3d<f32>;
@group(0) @binding(9) var noise_samp: sampler;
const NOISE: f32 = 32.0;

const PI: f32 = 3.14159265;
// The air's extinction length (m), as Volumetric Fog's.
const EXTINCTION: f32 = 60.0;
// Steps through one cone.
const STEPS: i32 = 12;
// The cone's useful reach (HeadlightClipTex is 0 by ~48 m).
const REACH: f32 = 46.0;
// The cookie's lit part across / down (uv 0.12..0.88, 0.1..0.9).
const COOKIE_X: f32 = 0.38;
const COOKIE_Y: f32 = 0.40;
// A lamp's halo core (m): the closed form's softening.
const CORE: f32 = 0.18;
// The halo's reach (m).
const HALO: f32 = 6.0;

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

fn rel_at(px: vec2<f32>, d: f32) -> vec3<f32> {
    let uv = px * u.size.zw;
    let h = u.clip_to_rel * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    return h.xyz / h.w;
}

fn is_far(d: f32) -> bool {
    return select(d >= 0.99999, d <= 0.00001, u.params.w > 0.5);
}

fn depth_at(px: vec2<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(depth_tex));
    return textureLoad(depth_tex, clamp(vec2<i32>(px), vec2<i32>(0), dims - 1), 0);
}

fn eye_distance(px: vec2<f32>) -> f32 {
    let d = depth_at(px);
    return select(length(rel_at(px, d)), 400.0, is_far(d));
}

fn fog_keep(z: f32) -> f32 {
    if u.fog.w < 0.5 {
        return 1.0;
    }
    return clamp((u.fog.x - z) * u.fog.y, u.fog.z, 1.0);
}

// HeadlightClipTex (0x5BC5E0) as world.wgsl samples it.
fn clip_texel(i: f32) -> f32 {
    if i < 33.0 || i > 47.0 {
        return 0.0;
    }
    return trunc(255.0 * sqrt(1.0 - (i - 33.0) * (1.0 / 15.0))) / 255.0;
}

fn headlight_clip(uc: f32) -> f32 {
    let x = clamp(uc, 0.0, 1.0) * 64.0 - 0.5;
    let i = floor(x);
    return mix(clip_texel(clamp(i, 0.0, 63.0)), clip_texel(clamp(i + 1.0, 0.0, 63.0)), x - i);
}

// Henyey–Greenstein, × 4π (1 = isotropic).
fn phase(c: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / pow(max(1.0 + g2 - 2.0 * g * c, 1e-4), 1.5);
}

// Value noise (trilinear, smooth). The lattice's hashes are a periodic
// texture (car_lights::noise_texels, NOISE³, repeat): one filtered sample at
// the smooth weights (texel i's centre at (i + 0.5) / NOISE) in place of
// eight hashes and seven mixes.
fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let w = f * f * (3.0 - 2.0 * f);
    return textureSampleLevel(noise_tex, noise_samp, (i + w + 0.5) * (1.0 / NOISE), 0.0).r;
}

// The air's density at a world point: two octaves drifting with the wind
// and sinking with the rain's mist (0.55..1.45 around the mean).
fn air(p: vec3<f32>) -> f32 {
    let t = u.gains.w;
    let q = p * 0.22 + vec3<f32>(t * 0.05, t * 0.03, t * 0.08);
    let n = noise3(q) * 0.65 + noise3(q * 2.7 + vec3<f32>(5.2, 1.3, t * 0.2)) * 0.35;
    return 0.55 + 0.9 * n;
}

fn ign(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

fn cookie(uv: vec2<f32>) -> f32 {
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return 0.0;
    }
    return textureSampleLevel(cookie_tex, cookie_samp, uv, 0.0).r;
}

// Clip the ray o + d·t, t in [range.x, range.y], to n·q ≤ c.
fn clip_plane(range: vec2<f32>, o: vec3<f32>, d: vec3<f32>, n: vec3<f32>, c: f32) -> vec2<f32> {
    let den = dot(n, d);
    let num = c - dot(n, o);
    if abs(den) < 1e-6 {
        return select(range, vec2<f32>(1.0, 0.0), num < 0.0);
    }
    let t = num / den;
    if den > 0.0 {
        return vec2<f32>(range.x, min(range.y, t));
    }
    return vec2<f32>(max(range.x, t), range.y);
}

// One point light's halo along the ray (`dir`, length `len`): zero where it
// cannot reach (a function, so fs_march runs it for its tile's points).
fn point_halo(i: u32, dir: vec3<f32>, len: f32, g: f32) -> vec3<f32> {
    let p = points[i];
    let c = p.position.xyz;
    let t0 = dot(c, dir);
    let h2 = max(dot(c, c) - t0 * t0, 0.0);
    if h2 > HALO * HALO {
        return vec3<f32>(0.0);
    }
    let a = max(t0 - HALO, 0.0);
    let b = min(t0 + HALO, len);
    if b <= a {
        return vec3<f32>(0.0);
    }
    let hs = sqrt(h2 + CORE * CORE);
    let integral = (atan((b - t0) / hs) - atan((a - t0) / hs)) / hs;
    // The lamp's lobe toward the ray's nearest point, and the phase.
    let q = dir * clamp(t0, a, b) - c;
    let qn = q * inverseSqrt(max(dot(q, q), 1e-6));
    let f = dot(p.facing.xyz, qn);
    let lobe = select(0.25 + 0.75 * smoothstep(-0.3, 0.7, f), 0.05 + 0.95 * smoothstep(0.0, 0.85, f), p.position.w < 0.5);
    let ph = mix(1.0, phase(dot(qn, -dir), g), 0.6);
    let window = (1.0 - h2 / (HALO * HALO)) * (1.0 - h2 / (HALO * HALO));
    // Tail lights (kind 1) glow in the air at half (dim bulbs behind red
    // glass; a street of them would veil the view red).
    let kind_gain = select(1.0, 0.5, abs(p.position.w - 1.0) < 0.5);
    return p.colour.rgb * (kind_gain * integral * lobe * ph * window * u.gains.y * fog_keep(t0) * exp(-max(t0, 0.0) / EXTINCTION) * p.colour.a);
}

@fragment
fn fs_march(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let px = pos.xy * u.map.xy + u.map.zw;
    let d = depth_at(px);
    let end = rel_at(px, d);
    let len = select(length(end), 400.0, is_far(d));
    let dir = normalize(end);
    let jitter = ign(pos.xy);
    let g = u.gains.z;
    var sum = vec3<f32>(0.0);
    let n_spots = u32(u.params.y);
    for (var i = 0u; i < n_spots; i++) {
        let s = spots[i];
        let right = s.right.xyz;
        let up = s.up.xyz;
        let fwd = s.dir.xyz;
        let sep = s.position.w;
        // The ray in the cone's axes (the eye at -position).
        let o = vec3<f32>(dot(-s.position.xyz, right), dot(-s.position.xyz, up), dot(-s.position.xyz, fwd));
        let dl = vec3<f32>(dot(dir, right), dot(dir, up), dot(dir, fwd));
        let ax = COOKIE_X / s.right.w;
        let ay = COOKIE_Y / s.up.w;
        var r = vec2<f32>(0.0, len);
        r = clip_plane(r, o, dl, vec3<f32>(0.0, 0.0, -1.0), -0.3);
        r = clip_plane(r, o, dl, vec3<f32>(0.0, 0.0, 1.0), REACH);
        r = clip_plane(r, o, dl, vec3<f32>(1.0, 0.0, -ax), sep);
        r = clip_plane(r, o, dl, vec3<f32>(-1.0, 0.0, -ax), sep);
        r = clip_plane(r, o, dl, vec3<f32>(0.0, 1.0, -ay), 0.0);
        r = clip_plane(r, o, dl, vec3<f32>(0.0, -1.0, -ay), 0.0);
        if r.y <= r.x {
            continue;
        }
        let dt = (r.y - r.x) / f32(STEPS);
        var acc = 0.0;
        for (var k = 0; k < STEPS; k++) {
            let t = r.x + (f32(k) + jitter) * dt;
            let q = o + dl * t;
            let z = max(q.z, 0.05);
            let v = q.y * s.up.w / z + 0.5;
            let ca = cookie(vec2<f32>((q.x - sep) * s.right.w / z + 0.5, v));
            let cb = cookie(vec2<f32>((q.x + sep) * s.right.w / z + 0.5, v));
            // Squared: the cookie's bright centre is the beam the air shows
            // (its wide soft fan lights the road, not the haze); the light
            // thins with the distance from the lamps.
            let c = ca * ca + cb * cb;
            if c <= 0.0 {
                continue;
            }
            let lit = c * 0.5 * headlight_clip(z * 0.005 + 0.5) / (1.0 + z * z * (1.0 / 100.0));
            let cos_t = dot(q, -dl) * inverseSqrt(max(dot(q, q), 1e-4));
            // The aureole: where the eye stands in the beam and looks back
            // along it toward the lamps, the forward-scattered light is the
            // lamps' own glare, which the native glow sprite and the lens core
            // draw; the march leaves it to them (it once added it again,
            // ×2 by the phase, over the beam's whole length: oncoming cars'
            // fronts went under a white disc). Across the beam (the shafts) and
            // from behind it (the player's own beams) nothing changes.
            let ph = mix(1.0, phase(cos_t, g), 0.6) * (1.0 - smoothstep(u.shape.x, u.shape.y, cos_t));
            let world = dir * t + u.eye.xyz;
            acc += lit * ph * air(world) * exp(-t / EXTINCTION) * fog_keep(t);
        }
        sum += s.colour.rgb * (acc * dt * u.gains.x);
    }
    // The points binned to this pixel's tile only (the others add
    // nothing here: the same sum in the same order). The loop stays over
    // every point (a uniform index: the lamps' loads stay scalar), the tile's
    // mask skips the rest.
    let tile = vec2<u32>(pos.xy) / u32(u.tiles.y);
    let mask = tiles[tile.y * u32(u.tiles.x) + tile.x];
    let n_points = u32(u.params.z);
    for (var i = 0u; i < n_points; i++) {
        if (mask[i >> 5u] & (1u << (i & 31u))) == 0u {
            continue;
        }
        sum += point_halo(i, dir, len, g);
    }
    // A soft knee: looking into several cones in rain the sum is kept
    // under white (hue kept), the faint shafts stay linear.
    let light = sum * u.params.x;
    return vec4<f32>(light / (1.0 + 0.8 * max(light.r, max(light.g, light.b))), len);
}

@fragment
fn fs_up(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let hp = (pos.xy - u.map.zw) / u.map.xy - 0.5;
    let base = floor(hp);
    let fr = hp - base;
    let last = vec2<i32>(textureDimensions(low_tex)) - 1;
    var lit = false;
    var tex: array<vec4<f32>, 4>;
    for (var k = 0; k < 4; k++) {
        let t = clamp(vec2<i32>(base + vec2<f32>(f32(k & 1), f32(k >> 1))), vec2<i32>(0), last);
        tex[k] = textureLoad(low_tex, t, 0);
        lit = lit || any(tex[k].rgb != vec3<f32>(0.0));
    }
    if !lit {
        discard;
    }
    let z = eye_distance(pos.xy);
    var sum = vec3<f32>(0.0);
    var total = 0.0;
    var plain = vec3<f32>(0.0);
    for (var k = 0; k < 4; k++) {
        let o = vec2<f32>(f32(k & 1), f32(k >> 1));
        let wb = select(1.0 - fr.x, fr.x, o.x > 0.5) * select(1.0 - fr.y, fr.y, o.y > 0.5);
        let gap = abs(tex[k].a - z) / max(z, 1e-3) * 100.0;
        let w = wb / (1.0 + gap * gap);
        sum += tex[k].rgb * w;
        total += w;
        plain += tex[k].rgb * wb;
    }
    return vec4<f32>(select(sum / max(total, 1e-6), plain, total < 1e-4), 0.0);
}

// ---------------------------------------------------------------------
// Lens streaks.

struct StreakOut {
    @builtin(position) pos: vec4<f32>,
    // pixels from the lamp (x along the streak, y across)
    @location(0) local: vec2<f32>,
    // streak half length, core radius (px), kind
    @location(1) shape: vec3<f32>,
    @location(2) colour: vec3<f32>,
};

// Streak length and core size as fractions of the screen height at full
// strength (inferred).
const STREAK_LEN: f32 = 0.16;
const STREAK_THICK: f32 = 0.0016;
const CORE_SIZE: f32 = 0.012;

@vertex
fn vs_streak(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> StreakOut {
    var out: StreakOut;
    out.pos = vec4<f32>(2.0, 2.0, 0.0, 1.0);
    out.local = vec2<f32>(0.0);
    out.shape = vec3<f32>(1.0, 1.0, 0.0);
    out.colour = vec3<f32>(0.0);
    let p = points[ii];
    let clip = u.rel_to_clip * vec4<f32>(p.position.xyz, 1.0);
    if clip.w < 0.5 {
        return out;
    }
    let ndc = clip.xy / clip.w;
    if any(abs(ndc) > vec2<f32>(1.1)) {
        return out;
    }
    let centre = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * u.size.xy;
    // Occlusion: 5 depth taps around the lamp; the lamp sits on its own
    // car's surface (the markers sit up to ~0.5 m inside the body), so a tap at most 1 m in front of it still sees it.
    let dist = length(p.position.xyz);
    var vis = 0.0;
    let r = max(1.5, u.size.y * 0.0015);
    for (var k = 0; k < 5; k++) {
        let o = select(vec2<f32>(0.0), vec2<f32>(cos(f32(k) * 1.5708), sin(f32(k) * 1.5708)) * r, k > 0);
        let z = eye_distance(centre + o);
        vis += select(0.0, 1.0, z > dist - 1.0);
    }
    vis *= 0.2;
    let to_eye = -p.position.xyz / max(dist, 1e-3);
    let f = dot(p.facing.xyz, to_eye);
    let head = p.position.w < 0.5;
    let facing = select(smoothstep(-0.1, 0.8, f), smoothstep(0.25, 0.95, f), head);
    let lum = dot(p.colour.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    // The light reaching the lens: full within 10 m, then as 1 / d² (it was
    // 1 / (1 + d² / 30²), nearly whole out to 30 m, so a car 30 m off kept 10 m's streak
    // and core, which covered its front between the lamps).
    let reach = min(1.0, 200.0 / (100.0 + dist * dist));
    let strength = vis * facing * min(lum * 3.0, 1.0) * u.eye.w * reach * fog_keep(dist);
    if strength < 0.004 {
        return out;
    }
    let s = sqrt(strength);
    let len = u.size.y * STREAK_LEN * s * select(0.6, 1.0, head);
    let core = max(u.size.y * CORE_SIZE * s, 2.0);
    let ext = vec2<f32>(max(len, core), core);
    var corners = array<vec2<f32>, 6>(vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
    let corner = corners[vi];
    let at = centre + corner * ext;
    out.pos = vec4<f32>(at.x * u.size.z * 2.0 - 1.0, 1.0 - at.y * u.size.w * 2.0, 0.0, 1.0);
    out.local = corner * ext;
    out.shape = vec3<f32>(len, core, select(1.0, 0.0, head));
    // The streak cooler than the lamp for headlights (an anamorphic
    // lens's blue), the lamp's own red for tail / brake lights.
    let c = p.colour.rgb / max(max(p.colour.r, max(p.colour.g, p.colour.b)), 1e-4);
    out.colour = c * strength;
    return out;
}

@fragment
fn fs_streak(in: StreakOut) -> @location(0) vec4<f32> {
    let x = abs(in.local.x) / max(in.shape.x, 1.0);
    let thick = max(u.size.y * STREAK_THICK, 1.0);
    let streak = exp(-abs(in.local.y) / thick) * pow(max(1.0 - x, 0.0), 2.5);
    let r = length(in.local) / in.shape.y;
    let core = exp(-r * r * 4.0);
    let cool = select(mix(in.colour, vec3<f32>(0.55, 0.72, 1.0) * max(in.colour.r, max(in.colour.g, in.colour.b)), 0.55), in.colour, in.shape.z > 0.5);
    // The core whiter than the lamp (an over-exposed point on the lens).
    let hot = mix(in.colour, vec3<f32>(max(in.colour.r, max(in.colour.g, in.colour.b))), 0.45);
    return vec4<f32>(cool * streak * 0.6 + hot * core * 0.35, 0.0);
}

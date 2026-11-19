// Weather effects (clone deviation, docs/notes/graphics/weather.md): fine rain
// streaks, ground splashes, the wet-air glow around lamps (Rain Detail),
// wheel spray and mist (Spray & Splashes). Drawn after the native
// particles into the scene (or after the temporal pass), premultiplied
// over; depth-aware by reading the scene's depth (no depth attachment).
// Everything is camera-relative (f32-safe across Bayview).

struct Frame {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    // xyz eye (world, for hashing only: large), w time (world clock, s)
    eye: vec4<f32>,
    // xyz camera velocity (m/s), w rain amount 0..1
    cam_vel: vec4<f32>,
    // x near, y far, z reversed (1/0), w lamp count
    depth: vec4<f32>,
    // xyz ambient (the fog colour, 0..1), w road wetness
    ambient: vec4<f32>,
    // xy wind (m/s), z spot count, w tier (1 / 2)
    wind: vec4<f32>,
    // viewport size (scene pixels), counts: rain, splashes
    size: vec4<f32>,
    // lamps: (rel position, radius), (colour, 0)
    lamps: array<vec4<f32>, 64>,
    // car lights: (rel position, range), (direction, cos outer), (colour, cos inner); tail: (rel, 0)
    spots: array<vec4<f32>, 32>,
    // Next-Gen rain: xy the wind now (m/s), zw the wind's displacement
    // integrated over the clock (m; gusts without jumps)
    ng_wind: vec4<f32>,
    // xy the splash region's centre (rel, ahead of the eye), z the ground
    // under the eye (rel z), w the projection's y scale (m11)
    ng_view: vec4<f32>,
    // x road wetness level (wet_road::level), y haze scattering (1/m),
    // z haze extinction (1/m), w 1 under a cover zone
    ng_air: vec4<f32>,
    // x the share of the mist's extinction veiling the scene, y the
    // mist's light (share of the fog colour), z the in-scatter's share
    // over the sky
    ng_haze: vec4<f32>,
};

struct Puff {
    // rel position, size
    p: vec4<f32>,
    // age 0..1, kind (0 droplet, 1 mist), seed, alpha
    q: vec4<f32>,
};

@group(0) @binding(0) var<uniform> f: Frame;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var<storage, read> puffs: array<Puff>;
// The droplets' velocities follow the puffs (two a Puff).
const SPRAY_MAX: u32 = 6000u;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) colour: vec4<f32>,
    // x view depth, y softness (m), z kind (0 streak, 1 splash, 2 halo, 3 droplet, 4 mist), w seed
    @location(2) misc: vec4<f32>,
};

fn hash3(p: vec3<u32>) -> vec3<f32> {
    var q = p * vec3<u32>(1664525u, 1013904223u, 2654435761u);
    q.x += q.y * q.z;
    q.y += q.z * q.x;
    q.z += q.x * q.y;
    q = q ^ (q >> vec3<u32>(16u));
    q.x += q.y * q.z;
    q.y += q.z * q.x;
    q.z += q.x * q.y;
    return vec3<f32>(q & vec3<u32>(0xFFFFu)) / 65535.0;
}

fn linear_depth(d: f32) -> f32 {
    let n = f.depth.x;
    let fa = f.depth.y;
    if (f.depth.z > 0.5) {
        return n * fa / (n + d * (fa - n));
    }
    return n * fa / (fa - d * (fa - n));
}

fn scene_depth_px(px: vec2<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(depth_tex));
    let c = clamp(vec2<i32>(px), vec2(0), dims - 1);
    return textureLoad(depth_tex, c, 0);
}

fn view_depth(rel: vec3<f32>) -> f32 {
    return (f.view_proj * vec4<f32>(rel, 1.0)).w;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

// The light at `rel`: the street lamps (the dynamic lamps' weight², a pool rather than a
// flat lift), the cars' headlight cones and tail lights; `fwd` scatter
// toward the eye boosts what is lit from behind (water drops scatter
// forward).
fn light_at(rel: vec3<f32>, scatter: f32) -> vec3<f32> {
    var c = f.ambient.xyz * 0.8;
    let to_eye = normalize(-rel);
    let nl = u32(f.depth.w);
    for (var i = 0u; i < nl; i++) {
        let l = f.lamps[i * 2u];
        let d = l.xyz - rel;
        let d2 = dot(d, d);
        let o2 = l.w * l.w;
        if d2 >= o2 { continue; }
        var w = 1.0 - d2 / o2;
        w *= w;
        let back = max(dot(normalize(-d), to_eye), 0.0);
        c += f.lamps[i * 2u + 1u].xyz * w * (1.0 + scatter * pow(back, 6.0) * 3.0);
    }
    let ns = u32(f.wind.z);
    for (var i = 0u; i < ns; i++) {
        let a = f.spots[i * 4u];
        let b = f.spots[i * 4u + 1u];
        let col = f.spots[i * 4u + 2u];
        let tail = f.spots[i * 4u + 3u];
        let d = rel - a.xyz;
        let dist = length(d);
        if dist < a.w {
            let cosang = dot(d / max(dist, 1e-3), b.xyz);
            let cone = smoothstep(b.w, col.w, cosang);
            let fall = 1.0 - dist / a.w;
            // Lit from behind (the beam toward the eye) scatters most.
            let back = max(dot(d / max(dist, 1e-3), to_eye), 0.0);
            c += col.xyz * cone * fall * fall * (1.0 + scatter * pow(back, 4.0) * 2.0);
        }
        let t = rel - tail.xyz;
        let td = dot(t, t);
        c += vec3<f32>(0.35, 0.025, 0.012) * 1.5 / (1.0 + td * 1.6);
    }
    return c;
}


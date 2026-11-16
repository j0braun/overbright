// Particles High / Ultra (docs/particles-smoke.md; clone deviation,
// opt-in). The native smoke particles' births expanded into smaller puffs
// (cs_spawn), moved on the GPU (cs_update) and drawn as soft, lit,
// noise-detailed billboards (vs_main / fs_main). gpu_particles.rs mirrors
// `linear_depth` and the soft fade for its tests.

struct Puff {
    // position, age (s)
    p0: vec4<f32>,
    // velocity, life (s; 0 dead)
    p1: vec4<f32>,
    // description, colour entry, seed, owner car (0xFFFFFFFF none)
    ids: vec4<u32>,
    // size scale, angle (rad), spin (rad/s), carried share
    p3: vec4<f32>,
}

struct Birth {
    pos_life: vec4<f32>,
    vel_share: vec4<f32>,
    ids: vec4<u32>,
}

struct Desc {
    // the size cubic's rows (x size, y spin)
    size: array<vec4<f32>, 4>,
    // drag, gravity, on-screen bound (px), texture layer
    params: vec4<f32>,
    // aspect w, h, flip-book columns, rows
    params2: vec4<f32>,
}

struct Tick {
    // dt, time
    dt: vec4<f32>,
    // birth offset, birth count, ring head, pool size
    counts: vec4<u32>,
    // children, size scale, linger, alpha scale
    tier: vec4<f32>,
    // wind (m/s), turbulence
    wind: vec4<f32>,
    ncars: vec4<u32>,
    // per car: position (w 1 when present), velocity
    cars: array<vec4<f32>, 16>,
}


fn pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

fn rnd(s: ptr<function, u32>) -> f32 {
    *s = pcg(*s);
    return f32(*s >> 8u) / 16777216.0;
}

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn vnoise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash3(i), hash3(i + vec3(1.0, 0.0, 0.0)), u.x);
    let b = mix(hash3(i + vec3(0.0, 1.0, 0.0)), hash3(i + vec3(1.0, 1.0, 0.0)), u.x);
    let c = mix(hash3(i + vec3(0.0, 0.0, 1.0)), hash3(i + vec3(1.0, 0.0, 1.0)), u.x);
    let d = mix(hash3(i + vec3(0.0, 1.0, 1.0)), hash3(i + vec3(1.0, 1.0, 1.0)), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}

fn cubic(m: array<vec4<f32>, 4>, t: f32) -> vec4<f32> {
    return ((m[0] * t + m[1]) * t + m[2]) * t + m[3];
}


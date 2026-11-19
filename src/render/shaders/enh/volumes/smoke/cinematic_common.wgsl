// Particles Cinematic (docs/notes/graphics/smoke.md "Particles Cinematic: the simulation and the billboard drawing"; clone deviation,
// opt-in). The native smoke births (the same feeder as High / Ultra)
// become fewer, larger billows plus a long-lived haze puff, moved by a
// divergence-free noise flow, buoyancy, the wind and the cars' wakes
// (cs_spawn / cs_update), and drawn at a reduced resolution into a
// weighted-blended OIT pair (vs_puff / fs_puff) that is composited over
// the scene with a depth-aware (bilateral) upsample (fs_composite).
// smoke_cinematic.rs mirrors `wake`, `bilateral` and `lowres_div`.

struct Puff {
    // position, age (s)
    p0: vec4<f32>,
    // velocity, life (s; 0 dead)
    p1: vec4<f32>,
    // description, colour entry, seed, owner car (0xFFFFFFFF none)
    ids: vec4<u32>,
    // size scale, angle (rad), spin (rad/s), carried share
    p3: vec4<f32>,
    // steam share (wet road), opacity scale, growth, kind (0 billow, 1 haze)
    p4: vec4<f32>,
}

struct Birth {
    pos_life: vec4<f32>,
    vel_share: vec4<f32>,
    ids: vec4<u32>,
}

struct Desc {
    size: array<vec4<f32>, 4>,
    // drag, gravity, on-screen bound (px), texture layer
    params: vec4<f32>,
    // aspect w, h, flip-book columns, rows
    params2: vec4<f32>,
    // the sprite's mean opacity (its mass), 0, 0, 0
    params3: vec4<f32>,
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

fn cubic(m: array<vec4<f32>, 4>, t: f32) -> vec4<f32> {
    return ((m[0] * t + m[1]) * t + m[2]) * t + m[3];
}

// The air a car drags along at `r` (puff − car) when moving at `cv`:
// x the weight, yzw the air's velocity. Behind the car a tube that widens
// with distance moves with part of its speed, rolling up and out (two
// counter-rotating trailing vortices); ahead and beside it the body
// pushes the air aside. Inferred (no native counterpart).
fn wake(r: vec3<f32>, cv: vec3<f32>) -> vec4<f32> {
    let speed = length(cv);
    if (speed < 0.5) {
        let w = exp(-dot(r, r) / 3.0);
        return vec4(w, 0.35 * cv);
    }
    let dir = cv / speed;
    let along = dot(r, dir);
    let lat = r - dir * along;
    let length_ = 2.5 + 0.35 * speed;
    let radius = 1.3 + 0.12 * max(-along, 0.0);
    let behind = smoothstep(2.4, 0.0, along) * exp(min(along, 0.0) / length_);
    let w = behind * exp(-dot(lat, lat) / (radius * radius));
    let side = lat - vec3(0.0, 0.0, lat.z);
    let out = side / max(length(side), 0.3);
    let roll = (out * 0.06 + vec3(0.0, 0.0, 0.07)) * min(speed, 30.0);
    return vec4(w, cv * 0.35 + roll);
}


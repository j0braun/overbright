// ---- pool ----

// An emitted particle (64 bytes; volume_fx.rs `Emit`).
struct Emit {
    // position, life (s)
    pos_life: vec4<f32>,
    // velocity, start radius (m)
    vel_r: vec4<f32>,
    // albedo, start extinction (1/m)
    albedo_sigma: vec4<f32>,
    // temperature 0..1 (lift, glow; cools), growth (m/√s), lumpiness, thinness
    heat: vec4<f32>,
}

// A simulated particle (96 bytes).
struct Sim {
    pos_age: vec4<f32>,
    vel_life: vec4<f32>,
    emit: Emit,
}

struct Fields {
    // dt, time, emits, pool size
    step: vec4<f32>,
    // ring head, emit count, cars, 0
    counts: vec4<u32>,
    // wind (m/s), turbulence (m/s²)
    wind: vec4<f32>,
    // drag (1/s), buoyancy per unit temperature (m/s²), cooling (1/s), glow gain
    phys: vec4<f32>,
    // car position, 1 (or 0); car velocity
    cars: array<vec4<f32>, 16>,
}

@group(0) @binding(0) var<uniform> fld: Fields;
@group(0) @binding(1) var<storage, read_write> sims: array<Sim>;
@group(0) @binding(2) var<storage, read> emits: array<Emit>;
@group(0) @binding(3) var<storage, read_write> out_particles: array<Particle>;
@group(0) @binding(4) var pnoise: texture_3d<f32>;
@group(0) @binding(5) var pnoise_samp: sampler;
// the first particle slot this pool exports to
@group(0) @binding(6) var<uniform> export_at: vec4<u32>;

// The air a car moving at `cv` drags along at `r` (particle − car): x the
// weight, yzw the air's velocity. Behind the car a widening tube moves
// with part of its speed, rolling up and out (two trailing vortices);
// parked: a local pull. Inferred. volume_fx.rs `wake`.
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

@compute @workgroup_size(64)
fn cs_pool_spawn(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= fld.counts.y) {
        return;
    }
    var s: Sim;
    let e = emits[id.x];
    s.pos_age = vec4(e.pos_life.xyz, 0.0);
    s.vel_life = vec4(e.vel_r.xyz, e.pos_life.w);
    s.emit = e;
    sims[(fld.counts.x + id.x) % u32(fld.step.w)] = s;
}

// Drag, buoyancy from the temperature (which cools), a divergence-free
// turbulence (the cross product of two noise gradients), the wind (the
// older, the more), the cars' wakes.
@compute @workgroup_size(64)
fn cs_pool_update(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= u32(fld.step.w)) {
        return;
    }
    var s = sims[id.x];
    if (s.vel_life.w <= 0.0) {
        return;
    }
    let dt = fld.step.x;
    let age = s.pos_age.w + dt;
    if (age >= s.vel_life.w) {
        s.vel_life.w = 0.0;
        sims[id.x] = s;
        return;
    }
    let t = age / s.vel_life.w;
    var v = s.vel_life.xyz;
    v -= v * min(fld.phys.x * dt, 1.0);
    let temp = s.emit.heat.x * exp(-fld.phys.z * age);
    v.z += fld.phys.y * temp * dt;
    let q = s.pos_age.xyz * 0.11 + vec3(0.0, 0.0, fld.step.y * 0.025);
    let g1 = textureSampleLevel(pnoise, pnoise_samp, q, 0.0).gba * 2.0 - 1.0;
    let g2 = textureSampleLevel(pnoise, pnoise_samp, q * 1.7 + vec3(0.37, 0.71, 0.13), 0.0).gba * 2.0 - 1.0;
    v += cross(g1, g2) * fld.wind.w * (0.5 + 1.5 * t) * dt;
    let follow = 1.0 - exp(-dt * 0.8 * t);
    v.x += (fld.wind.x - v.x) * follow;
    v.y += (fld.wind.y - v.y) * follow;
    for (var c = 0u; c < min(fld.counts.z, 8u); c++) {
        let at = fld.cars[c * 2u];
        if (at.w < 0.5) {
            continue;
        }
        let wk = wake(s.pos_age.xyz - at.xyz, fld.cars[c * 2u + 1u].xyz);
        v += (wk.yzw - v) * wk.x * min(1.0, 2.0 * dt);
    }
    s.pos_age = vec4(s.pos_age.xyz + v * dt, age);
    s.vel_life = vec4(v, s.vel_life.w);
    sims[id.x] = s;
}

// The black-body-like glow of a temperature 0..1 (dark red → orange →
// yellow-white), radiance per unit extinction.
fn glow(temp: f32) -> vec3<f32> {
    let t = clamp(temp, 0.0, 1.0);
    return vec3(1.0, 0.35 + 0.55 * t, 0.08 + 0.5 * t * t) * t * t * t * 6.0;
}

// The pool's particles as renderer particles: growing (√age), thinning
// with the column kept (σ ∝ 1/r²), fading in and out.
@compute @workgroup_size(64)
fn cs_pool_export(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= u32(fld.step.w)) {
        return;
    }
    let s = sims[id.x];
    var p: Particle;
    p.pos_r = vec4(0.0);
    if (s.vel_life.w > 0.0) {
        let e = s.emit;
        let age = s.pos_age.w;
        let t = clamp(age / s.vel_life.w, 0.0, 1.0);
        let r = e.vel_r.w + e.heat.y * sqrt(age);
        let fade = smoothstep(0.0, 0.08, age) * (1.0 - smoothstep(0.55, 1.0, t));
        let sigma = e.albedo_sigma.w * pow(e.vel_r.w / max(r, 1e-3), 2.0) * fade;
        let temp = e.heat.x * exp(-fld.phys.z * age);
        let seed = bitcast<u32>(e.pos_life.x * 7.31 + e.pos_life.y * 3.17 + e.pos_life.z);
        p.pos_r = vec4(s.pos_age.xyz, r);
        p.albedo_sigma = vec4(e.albedo_sigma.rgb, sigma);
        p.emit_lump = vec4(glow(temp) * fld.phys.w, e.heat.z);
        p.noise = vec4(f32(seed & 63u) / 64.0, f32((seed >> 6u) & 63u) / 64.0, f32((seed >> 12u) & 63u) / 64.0 + age * 0.04, f32(seed >> 20u) * 0.006);
        p.misc = vec4(t, e.heat.w, 0.0, 0.0);
    }
    out_particles[export_at.x + id.x] = p;
}


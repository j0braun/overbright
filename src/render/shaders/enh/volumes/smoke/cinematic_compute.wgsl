// ---- compute ----

struct Tick {
    // dt, time, wet (0..1)
    dt: vec4<f32>,
    // birth offset, birth count, ring head, pool size
    counts: vec4<u32>,
    // billows per birth, size scale, linger, alpha scale
    tier: vec4<f32>,
    // wind (m/s), turbulence
    wind: vec4<f32>,
    // haze: size scale, life scale, alpha scale, growth
    haze: vec4<f32>,
    // cars, billow growth
    ncars: vec4<u32>,
    cars: array<vec4<f32>, 16>,
}

@group(0) @binding(0) var<storage, read_write> pool: array<Puff>;
@group(0) @binding(1) var<storage, read> births: array<Birth>;
@group(0) @binding(2) var<storage, read> descs: array<Desc>;
@group(0) @binding(3) var<uniform> tick: Tick;
@group(0) @binding(4) var noise_tex: texture_3d<f32>;
@group(0) @binding(5) var noise_samp: sampler;

// One puff per (birth, slot): `children` billows near the parent, and a
// haze puff in the last slot (larger, faint, long-lived).
@compute @workgroup_size(64)
fn cs_spawn(@builtin(global_invocation_id) id: vec3<u32>) {
    let children = u32(tick.tier.x) + 1u;
    let total = tick.counts.y * children;
    if (id.x >= total) {
        return;
    }
    let b = births[tick.counts.x + id.x / children];
    let k = id.x % children;
    let haze = k == children - 1u;
    var s = pcg(b.ids.z ^ (k * 0x9E3779B9u));
    let d = descs[b.ids.x];
    let size0 = max(cubic(d.size, 0.0).x, 0.05);
    let wet = tick.dt.z;
    let dir = normalize(vec3(rnd(&s), rnd(&s), rnd(&s)) * 2.0 - 1.0 + vec3(0.0, 0.0, 1e-3));
    let radius = (0.3 * size0 + 0.05) * pow(rnd(&s), 0.5);
    var puff: Puff;
    puff.p0 = vec4(b.pos_life.xyz + dir * radius, 0.0);
    let v = b.vel_share.xyz;
    let jitter = normalize(vec3(rnd(&s), rnd(&s), rnd(&s)) * 2.0 - 1.0 + vec3(0.0, 0.0, 1e-3));
    // Wet road: steam, lighter and shorter-lived, rising faster.
    // Steam evaporates sooner (it rose into a ceiling over the view).
    let life_wet = 1.0 - 0.6 * wet;
    if (haze) {
        puff.p1 = vec4(v * 0.45 + jitter * 0.2, b.pos_life.w * tick.haze.y * life_wet * (1.0 - 0.5 * wet) * (0.85 + 0.3 * rnd(&s)));
        puff.p3 = vec4(tick.tier.y * tick.haze.x * (0.8 + 0.4 * rnd(&s)), rnd(&s) * 6.2831853, (rnd(&s) - 0.5) * 0.3, b.vel_share.w * 0.5);
        puff.p4 = vec4(wet, tick.haze.z * (1.0 - 0.55 * wet), tick.haze.w, 1.0);
    } else {
        puff.p1 = vec4(v * (0.85 + 0.3 * rnd(&s)) + jitter * (0.3 + 0.12 * length(v)), b.pos_life.w * tick.tier.z * life_wet * (0.8 + 0.4 * rnd(&s)));
        puff.p3 = vec4(tick.tier.y * (0.75 + 0.5 * rnd(&s)), rnd(&s) * 6.2831853, (rnd(&s) - 0.5) * 1.6, b.vel_share.w);
        puff.p4 = vec4(wet, tick.tier.w * (1.0 - 0.4 * wet), f32(tick.ncars.y) / 1000.0, 0.0);
    }
    puff.ids = vec4(b.ids.x, b.ids.y, s, b.ids.w);
    pool[(tick.counts.z + id.x) % tick.counts.w] = puff;
}

// The noise volume's gradient at `p` (gba, stored as 0.5 + 0.5 g).
fn grad_at(p: vec3<f32>) -> vec3<f32> {
    return textureSampleLevel(noise_tex, noise_samp, p, 0.0).gba * 2.0 - 1.0;
}

// Age and move: the description's drag and gravity as 0x492E10, then a
// divergence-free flow (the cross product of two noise gradients: curls
// and rolls, no sinks), buoyancy while young, the wind for older smoke,
// and the cars' wakes.
@compute @workgroup_size(64)
fn cs_update(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= tick.counts.w) {
        return;
    }
    var puff = pool[id.x];
    if (puff.p1.w <= 0.0) {
        return;
    }
    let dt = tick.dt.x;
    let age = puff.p0.w + dt;
    if (age >= puff.p1.w) {
        puff.p1.w = 0.0;
        pool[id.x] = puff;
        return;
    }
    let t = age / puff.p1.w;
    let d = descs[puff.ids.x];
    let haze = puff.p4.w > 0.5;
    var v = puff.p1.xyz;
    v += -sign(v) * v * v * d.params.x * dt;
    v.z -= d.params.y * dt * select(1.0, 0.3, haze);
    let q = puff.p0.xyz * 0.11 + vec3(0.0, 0.0, tick.dt.y * 0.025) + f32(puff.ids.z & 255u) * 0.013;
    let flow = cross(grad_at(q), grad_at(q * 1.7 + vec3(0.37, 0.71, 0.13)));
    v += flow * tick.wind.w * (0.5 + 1.5 * t) * dt;
    // Buoyancy: warm smoke rises while young, steam more.
    v.z += (0.2 * (1.0 - t) + 0.7 * puff.p4.x) * dt;
    let follow = 1.0 - exp(-dt * select(0.6 * t, 0.9, haze));
    v.x += (tick.wind.x - v.x) * follow;
    v.y += (tick.wind.y - v.y) * follow;
    var pos = puff.p0.xyz;
    for (var c = 0u; c < min(tick.ncars.x, 8u); c++) {
        let at = tick.cars[c * 2u];
        if (at.w < 0.5) {
            continue;
        }
        let cv = tick.cars[c * 2u + 1u].xyz;
        let wk = wake(pos - at.xyz, cv);
        v += (wk.yzw - v) * wk.x * min(1.0, 2.0 * dt);
        if (c == puff.ids.w) {
            pos += cv * puff.p3.w * dt;
        }
    }
    // Not below the road it was born on (wakes push down at the sides).
    pos += v * dt;
    puff.p0 = vec4(pos, age);
    puff.p1 = vec4(v, puff.p1.w);
    pool[id.x] = puff;
}


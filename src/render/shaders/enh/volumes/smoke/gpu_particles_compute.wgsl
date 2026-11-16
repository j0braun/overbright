// ---- compute ----
@group(0) @binding(0) var<storage, read_write> pool: array<Puff>;
@group(0) @binding(1) var<storage, read> births: array<Birth>;
@group(0) @binding(2) var<storage, read> descs: array<Desc>;
@group(0) @binding(3) var<uniform> tick: Tick;

// One puff per (birth, child): near its parent, a little spread in
// velocity, smaller, living longer than the native particle.
@compute @workgroup_size(64)
fn cs_spawn(@builtin(global_invocation_id) id: vec3<u32>) {
    let children = u32(tick.tier.x);
    let total = tick.counts.y * children;
    if (id.x >= total) {
        return;
    }
    let b = births[tick.counts.x + id.x / children];
    let k = id.x % children;
    var s = pcg(b.ids.z ^ (k * 0x9E3779B9u));
    let d = descs[b.ids.x];
    let size0 = max(cubic(d.size, 0.0).x, 0.05);
    let dir = normalize(vec3(rnd(&s), rnd(&s), rnd(&s)) * 2.0 - 1.0 + vec3(0.0, 0.0, 1e-3));
    let radius = (0.35 * size0 + 0.05) * pow(rnd(&s), 0.5);
    var puff: Puff;
    puff.p0 = vec4(b.pos_life.xyz + dir * radius, 0.0);
    let v = b.vel_share.xyz;
    let jitter = normalize(vec3(rnd(&s), rnd(&s), rnd(&s)) * 2.0 - 1.0 + vec3(0.0, 0.0, 1e-3));
    puff.p1 = vec4(v * (0.85 + 0.3 * rnd(&s)) + jitter * (0.25 + 0.12 * length(v)), b.pos_life.w * tick.tier.z * (0.8 + 0.4 * rnd(&s)));
    puff.ids = vec4(b.ids.x, b.ids.y, s, b.ids.w);
    puff.p3 = vec4(tick.tier.y * (0.75 + 0.5 * rnd(&s)), rnd(&s) * 6.2831853, (rnd(&s) - 0.5) * 1.2, b.vel_share.w);
    pool[(tick.counts.z + id.x) % tick.counts.w] = puff;
}

// Age and move: the description's own drag (per axis, against the motion)
// and gravity as 0x492E10 moves the native particle, plus noise
// turbulence and a drift toward the wind that grow with age, and the drag
// of cars passing close by (their wake).
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
    var v = puff.p1.xyz;
    v += -sign(v) * v * v * d.params.x * dt;
    v.z -= d.params.y * dt;
    // Turbulence: three offset noises of the position and time.
    let q = puff.p0.xyz * 0.45 + vec3(0.0, 0.0, tick.dt.y * 0.35) + f32(puff.ids.z & 255u);
    let n = vec3(vnoise(q), vnoise(q + vec3(17.1, 3.3, 9.7)), vnoise(q + vec3(5.9, 23.4, 1.3))) * 2.0 - 1.0;
    v += n * tick.wind.w * (0.25 + t) * dt;
    // Older smoke follows the air.
    let follow = 1.0 - exp(-dt * 0.6 * t);
    v.x += (tick.wind.x - v.x) * follow;
    v.y += (tick.wind.y - v.y) * follow;
    var pos = puff.p0.xyz;
    for (var c = 0u; c < min(tick.ncars.x, 8u); c++) {
        let at = tick.cars[c * 2u];
        if (at.w < 0.5) {
            continue;
        }
        let cv = tick.cars[c * 2u + 1u].xyz;
        let r = pos - at.xyz;
        // The air behind a car moves at a part of its speed (inferred).
        let w = exp(-dot(r, r) / 3.0);
        v += (0.35 * cv - v) * w * min(1.0, 1.2 * dt);
        if (c == puff.ids.w) {
            pos += cv * puff.p3.w * dt;
        }
    }
    pos += v * dt;
    puff.p0 = vec4(pos, age);
    puff.p1 = vec4(v, puff.p1.w);
    pool[id.x] = puff;
}


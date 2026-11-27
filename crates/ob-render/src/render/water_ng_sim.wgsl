// Animated Water, Next-Gen (clone deviation, opt-in; docs/notes/graphics/water.md
// "Next-Gen"): the compute half. Standalone module (render/water_ng.rs).
//
//   cs_particles: the jets' particles at the frame's time (analytic on
//                 the world clock: each particle runs its chart's path
//                 from the source to the landing, cycle after cycle),
//                 plus the drops thrown up where the water lands;
//   cs_impulse:   the particles that land in a basin during one 1/60 s
//                 step push its water down (fixed-point atomics: the
//                 sum is the same in any order, shots stay deterministic);
//   cs_step:      one step of a damped wave equation per basin slot
//                 (heightfield, walls where there is no water, steps
//                 between levels as walls), rain drops, foam;
//   cs_export:    height, slope and foam into the texture the surfaces
//                 are drawn from.

const CU: u32 = 16u;
const CV: u32 = 64u;
const TABLE: u32 = 32u;
const SIM: u32 = 128u;
const WET_CELLS: f32 = 96.0;
const GRAVITY: f32 = 9.81;
const STEP_DT: f32 = 0.0166666667;

struct NgU {
    // x ents entries, y particles, z slots, w the flip-book's frames
    counts: vec4<u32>,
    // x clock (s)
    time: vec4<f32>,
    // world eye
    eye: vec4<f32>,
    // x impulse gain, y wave speed (m/s), z damping a step, w foam gain
    look: vec4<f32>,
    // per entry: chart (−1: a landing's splash), first particle, count, fade
    ents: array<vec4<f32>, 128>,
    // per entry: particle radius (m), splash: position xyz (world) + strength in yzw… see water_ng.rs
    ents2: array<vec4<f32>, 128>,
    // per slot: (field origin xy (world), field cell (m), field layer), (base z, uv scale, kind, seed)
    slots: array<vec4<f32>, 16>,
    slot_col: array<vec4<f32>, 8>,
    rect: vec4<f32>,
}

struct StepU {
    // x step index, y slots stepped (bits), z slots cleared first (bits), w 0
    n: vec4<u32>,
    // x t0, y t1, z rain, w 0
    t: vec4<f32>,
}

struct Particle {
    // world position, radius (0: none)
    pos_r: vec4<f32>,
    // velocity (m/s), aeration 0..1
    vel_a: vec4<f32>,
    // x fade (far band), y the art's brightness, z the share that has left the
    // sheet (a landing's drops: 1), w seed
    misc: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: NgU;
@group(0) @binding(1) var<storage, read> charts: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> info: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(4) var water_tex: texture_2d_array<f32>;
@group(0) @binding(5) var samp: sampler;
@group(0) @binding(6) var wet_tex: texture_2d_array<f32>;
@group(0) @binding(7) var<storage, read_write> sim: array<f32>;
@group(0) @binding(8) var<storage, read_write> force: array<atomic<i32>>;
@group(0) @binding(9) var sim_out: texture_storage_2d_array<rgba16float, write>;
@group(0) @binding(10) var<uniform> st: StepU;

fn hash_u(x: u32) -> u32 {
    var h = x * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    return (h >> 22u) ^ h;
}

fn rnd(a: u32, b: u32) -> f32 {
    return f32(hash_u(a ^ hash_u(b + 0x9E3779B9u)) & 0xFFFFFFu) / 16777216.0;
}

fn texel(c: u32, x: u32, y: u32) -> vec4<f32> {
    return charts[c * CU * CV + min(y, CV - 1u) * CU + min(x, CU - 1u)];
}

// The chart at (a, s), bilinear (w: covered).
fn chart_at(c: u32, a: f32, s: f32) -> vec4<f32> {
    let x = clamp(a, 0.0, 1.0) * f32(CU - 1u);
    let y = clamp(s, 0.0, 1.0) * f32(CV - 1u);
    let x0 = min(u32(x), CU - 2u);
    let y0 = min(u32(y), CV - 2u);
    let fx = x - f32(x0);
    let fy = y - f32(y0);
    let a0 = mix(texel(c, x0, y0), texel(c, x0 + 1u, y0), fx);
    let a1 = mix(texel(c, x0, y0 + 1u), texel(c, x0 + 1u, y0 + 1u), fx);
    return mix(a0, a1, fy);
}

fn info_f(c: u32, k: u32) -> f32 {
    return info[c * 12u + k / 4u][k % 4u];
}

// The along-coordinate at a fraction tn of the fall's time.
fn table_at(c: u32, tn: f32) -> f32 {
    let x = clamp(tn, 0.0, 1.0) * f32(TABLE - 1u);
    let k = min(u32(x), TABLE - 2u);
    return mix(info_f(c, k), info_f(c, k + 1u), x - f32(k));
}

struct Eval {
    p: Particle,
    // the particle's time in its cycle (s)
    tau: f32,
}

// Particle j of entry e in cycle `cycle` at the fraction tn of its life.
fn eval(e: u32, j: u32, cycle: f32, tn: f32) -> Eval {
    var o: Eval;
    let ent = u.ents[e];
    let ent2 = u.ents2[e];
    let seed = hash_u(u32(ent.y) + j * 7919u);
    let cyc = u32(i32(cycle) & 0x7FFFFFFF);
    o.p.misc = vec4<f32>(ent.w, 0.0, 0.0, rnd(seed, 1u));
    if ent.x < 0.0 {
        // Drops thrown up where the water lands (ballistic, as On's).
        let strength = ent2.w;
        let h1 = rnd(seed, cyc * 3u + 11u);
        let up = 0.8 + 1.7 * rnd(seed, 2u);
        let life = 2.0 * up / GRAVITY;
        let tau = tn * life;
        let ang = h1 * 6.2831853;
        let out = (0.3 + 1.0 * rnd(seed, cyc * 5u + 3u)) * (0.6 + 0.4 * strength);
        let r0 = sqrt(rnd(seed, cyc * 7u + 5u)) * (0.2 + 0.3 * strength);
        let hz = vec3<f32>(cos(ang), sin(ang), 0.0);
        let pos = ent2.xyz + hz * (r0 + out * tau) + vec3<f32>(0.0, 0.0, up * tau - 0.5 * GRAVITY * tau * tau + 0.02);
        o.p.pos_r = vec4<f32>(pos, 0.008 + 0.014 * rnd(seed, 9u));
        o.p.vel_a = vec4<f32>(hz * out + vec3<f32>(0.0, 0.0, up - GRAVITY * tau), 0.6);
        o.p.misc.z = 1.0;
        o.p.misc.y = 0.85;
        o.tau = tau;
        return o;
    }
    let c = u32(ent.x);
    let life = info_f(c, 32u);
    let arc = info_f(c, 35u);
    let uvr = info[c * 12u + 9u];
    let col = info[c * 12u + 10u];
    let layer = info_f(c, 44u);
    // Across: fixed per particle (streams keep their place), a little
    // wander per cycle.
    // The sheet runs in strands (ropes of water, as a real curtain breaks
    // up): as many as make their spacing across half the spacing along.
    let width = info_f(c, 33u) / max(info_f(c, 34u), 0.1);
    let strands = clamp(floor(sqrt(2.0 * width * ent.z / max(info_f(c, 34u), 0.1))), 1.0, 256.0);
    let k = floor(rnd(seed, 13u) * strands);
    let a = clamp((k + 0.5 + (rnd(seed, 14u) - 0.5) * 0.35 + (rnd(seed, cyc * 17u + 1u) - 0.5) * 0.1) / strands, 0.0, 1.0);
    // The art's opacity across the sheet (its streams and ragged edges),
    // at a v of its own each cycle (the art's breaks move with the water).
    let tv = fract(rnd(seed, 3u) + f32(cyc % 13u) * 0.37);
    let art = textureSampleLevel(water_tex, samp, vec2<f32>(mix(uvr.x, uvr.y, a), tv), i32(layer), 1.0);
    // An arc is a round jet (the art's ribbon is its picture from the
    // side): its water lies around the ribbon's middle line.
    let arcj = arc > 0.5;
    let present = select(max(art.a, 0.5), max(art.a, 0.85), arcj) * col.a;
    // The native shading's brightness there (2 · tex · v0: the art's bake).
    // Its streaks run along the flow: brightness from the art at one v
    // (a streak stays a streak down the whole fall), mip 2 (no texel noise).
    let streak = textureSampleLevel(water_tex, samp, vec2<f32>(mix(uvr.x, uvr.y, a), 0.5), i32(layer), 2.0);
    o.p.misc.y = dot(clamp(2.0 * streak.rgb * col.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(0.3, 0.59, 0.11));
    if rnd(seed, cyc * 23u + 4u) > present * 1.15 {
        return o;
    }
    let s = table_at(c, tn);
    let ap = select(a, 0.5, arcj);
    let q = chart_at(c, ap, s);
    if q.w < 0.5 {
        return o;
    }
    let ds = 1.0 / f32(CV - 1u);
    let da = 1.0 / f32(CU - 1u);
    let along = chart_at(c, ap, s + ds).xyz - chart_at(c, ap, s - ds).xyz;
    let across = chart_at(c, ap + da, s).xyz - chart_at(c, ap - da, s).xyz;
    var n = cross(along, across);
    n = n * inverseSqrt(max(dot(n, n), 1e-10));
    // Speed along the path: ds/dτ from the timing table.
    let ds_dt = (table_at(c, tn + 0.02) - table_at(c, tn - 0.02)) / (0.04 * life);
    let vel = along / (2.0 * ds) * ds_dt;
    let tau = tn * life;
    // Breakup: past 55 % of the path most of the water leaves the sheet
    // as drops with a velocity of their own (spread, ballistic offset);
    // the rest stays a stream. Thickness: a jitter across the sheet.
    let tb = 0.55 * life;
    let free = max(tau - tb, 0.0);
    let breaks = select(0.0, 1.0, rnd(seed, 29u) < 0.7);
    let dv = (vec3<f32>(rnd(seed, cyc * 31u + 6u), rnd(seed, cyc * 37u + 7u), rnd(seed, cyc * 41u + 8u)) - 0.5) * vec3<f32>(1.0, 1.0, 0.6) * 1.1;
    let thick = (rnd(seed, 43u) - 0.5) * 0.05 * (1.0 + 2.0 * free / max(life - tb, 0.05));
    var pos = q.xyz + n * thick + dv * free * breaks;
    if arcj {
        // A disc across the jet (about an eighth of the ribbon's width).
        let au = across * inverseSqrt(max(dot(across, across), 1e-10));
        let th = rnd(seed, 61u) * 6.2831853;
        let rho = clamp(width * 0.03, 0.03, 0.08) * sqrt(rnd(seed, 67u)) * (1.0 + 1.5 * free / max(life - tb, 0.05));
        pos = q.xyz + (au * cos(th) + n * sin(th)) * rho + dv * free * breaks;
    }
    // The drops fall a little faster than the sheet holds them.
    pos.z -= 0.5 * GRAVITY * free * free * 0.15 * breaks;
    let b = clamp(free / max(life - tb, 0.05), 0.0, 1.0) * breaks;
    let r = ent2.x * (1.0 - 0.45 * b * rnd(seed, 47u)) * (0.75 + 0.5 * rnd(seed, 53u));
    // Aerated: the broken water, the ragged edges, the art's bright parts.
    let aer = clamp(0.06 + 0.6 * b * b + (1.0 - present) * 0.25 + smoothstep(0.5, 0.95, o.p.misc.y) * 0.6, 0.0, 1.0);
    o.p.pos_r = vec4<f32>(pos, r);
    o.p.vel_a = vec4<f32>(vel + dv * breaks * smoothstep(0.0, 0.1, free), aer);
    // How far it has left the sheet (the drops' pass draws only that share).
    o.p.misc.z = b;
    o.tau = tau;
    return o;
}

// The entry particle i belongs to (0xFFFFFFFF: none).
fn entry_of(i: u32) -> u32 {
    for (var e = 0u; e < u.counts.x; e++) {
        let a = u.ents[e];
        let first = u32(a.y);
        if i >= first && i < first + u32(a.z) {
            return e;
        }
    }
    return 0xFFFFFFFFu;
}

fn splash_life(e: u32, j: u32) -> f32 {
    let seed = hash_u(u32(u.ents[e].y) + j * 7919u);
    let up = 0.8 + 1.7 * rnd(seed, 2u);
    return 2.0 * up / GRAVITY;
}

fn life_j(e: u32, j: u32) -> f32 {
    if u.ents[e].x < 0.0 {
        return splash_life(e, j);
    }
    return info_f(u32(u.ents[e].x), 32u);
}

fn phase_j(e: u32, j: u32) -> f32 {
    return rnd(hash_u(u32(u.ents[e].y) + j * 7919u), 3u);
}

@compute @workgroup_size(64)
fn cs_particles(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= u.counts.y {
        return;
    }
    var p: Particle;
    let e = entry_of(i);
    if e != 0xFFFFFFFFu {
        let j = i - u32(u.ents[e].y);
        let life = life_j(e, j);
        let x = u.time.x / life + phase_j(e, j);
        let ev = eval(e, j, floor(x), fract(x));
        p = ev.p;
    }
    particles[i] = p;
}

// The slot whose water a world point lies in (its level within 0.5 m):
// (slot, sim cell x, y); slot 0xFFFFFFFF: none.
fn slot_at(p: vec3<f32>) -> vec3<u32> {
    for (var s = 0u; s < u.counts.z; s++) {
        let a = u.slots[s * 2u];
        let b = u.slots[s * 2u + 1u];
        let uv = (p.xy - a.xy) / (a.z * WET_CELLS);
        if any(uv < vec2<f32>(0.0)) || any(uv >= vec2<f32>(1.0)) {
            continue;
        }
        let w = textureSampleLevel(wet_tex, samp, uv, i32(a.w), 0.0);
        if w.r > 0.15 || abs(p.z - (w.g + b.x)) > 0.6 {
            continue;
        }
        let c = vec2<u32>(uv * f32(SIM));
        return vec3<u32>(s, c);
    }
    return vec3<u32>(0xFFFFFFFFu, 0u, 0u);
}

@compute @workgroup_size(64)
fn cs_impulse(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= u.counts.y {
        return;
    }
    let e = entry_of(i);
    if e == 0xFFFFFFFFu {
        return;
    }
    let j = i - u32(u.ents[e].y);
    let life = life_j(e, j);
    let ph = phase_j(e, j);
    let x0 = st.t.x / life + ph;
    let x1 = st.t.y / life + ph;
    if floor(x1) <= floor(x0) {
        return;
    }
    // It lands at the end of cycle floor(x1) − 1.
    let ev = eval(e, j, floor(x1) - 1.0, 1.0);
    if ev.p.pos_r.w <= 0.0 {
        return;
    }
    let hit = slot_at(ev.p.pos_r.xyz);
    if hit.x == 0xFFFFFFFFu || ((st.n.y >> hit.x) & 1u) == 0u {
        return;
    }
    // Its momentum (∝ r³ · speed) pushes the water down, spread over 3 × 3
    // cells.
    let r = ev.p.pos_r.w;
    let speed = length(ev.p.vel_a.xyz);
    let amount = -u.look.x * r * r * r * min(speed, 12.0) * 1.0e6;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let cx = i32(hit.y) + dx;
            let cy = i32(hit.z) + dy;
            if cx < 0 || cy < 0 || cx >= i32(SIM) || cy >= i32(SIM) {
                continue;
            }
            let w = select(select(0.5, 1.0, dx == 0 && dy == 0), 0.25, dx != 0 && dy != 0);
            atomicAdd(&force[(hit.x * SIM + u32(cy)) * SIM + u32(cx)], i32(amount * w));
        }
    }
}

fn sidx(s: u32, a: u32, x: u32, y: u32) -> u32 {
    return ((s * 4u + a) * SIM + y) * SIM + x;
}

// The field at a slot's sim cell: (distance to the water (m), level (m above base)).
fn field_at(s: u32, x: u32, y: u32) -> vec2<f32> {
    let a = u.slots[s * 2u];
    let uv = (vec2<f32>(f32(x), f32(y)) + 0.5) / f32(SIM);
    return textureSampleLevel(wet_tex, samp, uv, i32(a.w), 0.0).rg;
}

@compute @workgroup_size(8, 8, 1)
fn cs_step(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    let s = gid.z;
    if x >= SIM || y >= SIM || s >= u.counts.z || ((st.n.y >> s) & 1u) == 0u {
        return;
    }
    let cur_a = st.n.x & 1u;
    let prev_a = 1u - cur_a;
    let fidx = (s * SIM + y) * SIM + x;
    let f = f32(atomicExchange(&force[fidx], 0)) * 1.0e-6;
    if ((st.n.z >> s) & 1u) != 0u {
        // First step of a slot: still water.
        for (var a = 0u; a < 4u; a++) {
            sim[sidx(s, a, x, y)] = 0.0;
        }
        return;
    }
    let me = field_at(s, x, y);
    let cell = u.slots[s * 2u].z * WET_CELLS / f32(SIM);
    if me.x > cell * 0.6 {
        sim[sidx(s, prev_a, x, y)] = 0.0;
        sim[sidx(s, 2u + prev_a, x, y)] = 0.0;
        return;
    }
    let h = sim[sidx(s, cur_a, x, y)];
    let hp = sim[sidx(s, prev_a, x, y)];
    // Neighbours: walls (mirror) where there is no water or another level.
    var lap = 0.0;
    var foam_n = 0.0;
    let offs = array<vec2<i32>, 4>(vec2(-1, 0), vec2(1, 0), vec2(0, -1), vec2(0, 1));
    for (var k = 0; k < 4; k++) {
        let nx = i32(x) + offs[k].x;
        let ny = i32(y) + offs[k].y;
        var hn = h;
        var fnb = sim[sidx(s, 2u + cur_a, x, y)];
        if nx >= 0 && ny >= 0 && nx < i32(SIM) && ny < i32(SIM) {
            let fo = field_at(s, u32(nx), u32(ny));
            if fo.x <= cell * 0.6 && abs(fo.y - me.y) < 0.05 {
                hn = sim[sidx(s, cur_a, u32(nx), u32(ny))];
                fnb = sim[sidx(s, 2u + cur_a, u32(nx), u32(ny))];
            }
        }
        lap += hn - h;
        foam_n += fnb;
    }
    let k = min(pow(u.look.y * STEP_DT / cell, 2.0), 0.45);
    // Rain: drops at hashed places in the slot (the same in every run).
    var rain = 0.0;
    if st.t.z > 0.001 {
        let seed = u32(u.slots[s * 2u + 1u].w);
        let p = (vec2<f32>(f32(x), f32(y)) + 0.5) / f32(SIM);
        for (var d = 0u; d < 12u; d++) {
            let hs = hash_u(st.n.x * 131u + d * 7u + seed * 977u);
            if f32(hs & 0xFFFFu) / 65536.0 > st.t.z {
                continue;
            }
            let c = vec2<f32>(rnd(hs, 1u), rnd(hs, 2u));
            let dd = (p - c) * f32(SIM);
            rain -= exp(-dot(dd, dd) * 0.8) * 0.004;
        }
    }
    var hn = h + (h - hp) * (1.0 - u.look.z) + k * lap + f + rain;
    hn = clamp(hn * 0.9995, -0.25, 0.25);
    sim[sidx(s, prev_a, x, y)] = hn;
    // Foam: made where the water is hit and churned, spread a little,
    // fading.
    // (Ping-ponged as the heights: every cell reads the last step's.)
    let fo = sim[sidx(s, 2u + cur_a, x, y)];
    let churn = abs(f) * u.look.w + max(-lap, 0.0) * 2.0;
    sim[sidx(s, 2u + prev_a, x, y)] = clamp(mix(fo, foam_n * 0.25, 0.12) * 0.985 + churn, 0.0, 1.0);
}

@compute @workgroup_size(8, 8, 1)
fn cs_export(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    let s = gid.z;
    if x >= SIM || y >= SIM || s >= u.counts.z {
        return;
    }
    // The newest heights: the buffer the last step wrote.
    let a = st.n.x & 1u;
    let cell = u.slots[s * 2u].z * WET_CELLS / f32(SIM);
    let h = sim[sidx(s, a, x, y)];
    let hl = sim[sidx(s, a, max(x, 1u) - 1u, y)];
    let hr = sim[sidx(s, a, min(x + 1u, SIM - 1u), y)];
    let hd = sim[sidx(s, a, x, max(y, 1u) - 1u)];
    let hu = sim[sidx(s, a, x, min(y + 1u, SIM - 1u))];
    let g = vec2<f32>(hr - hl, hu - hd) / (2.0 * cell);
    textureStore(sim_out, vec2<u32>(x, y), s, vec4<f32>(h, g, sim[sidx(s, 2u + a, x, y)]));
}

// Animated Water, Next-Gen (clone deviation, opt-in; docs/notes/graphics/water.md
// "Next-Gen"): the drawing half. Appended to water.wgsl and water_hq.wgsl
// (their Frame, lamps, cube, fog, scene copy, wet fields and the Realistic
// surface and sheet shading are shared); the On and Realistic modules are
// unchanged.
//
// Passes (render/water_ng.rs), into the scene before the temporal pass:
//   grid:      the simulated basins, a generated grid per basin displaced
//              by the wave simulation, shaded as Realistic's surfaces with
//              the simulation's slope and foam;
//   lips:      the native surface triangles of a simulated basin that do
//              not lie flat (overflow lips), as Realistic;
//   sheets:    every falling sheet and jet of the art, as Realistic, thinned
//              where its water breaks into drops;
//   drops:     the particles that have left the jets and those thrown up
//              where the water lands, lit by the lamps (energy-bounded).

struct NgU {
    counts: vec4<u32>,
    time: vec4<f32>,
    eye: vec4<f32>,
    look: vec4<f32>,
    ents: array<vec4<f32>, 128>,
    ents2: array<vec4<f32>, 128>,
    slots: array<vec4<f32>, 16>,
    slot_col: array<vec4<f32>, 8>,
    // x, y, z unused, w the debug view (NFSU2_WATER_NG_VIEW)
    rect: vec4<f32>,
}

struct NgParticle {
    pos_r: vec4<f32>,
    vel_a: vec4<f32>,
    misc: vec4<f32>,
}

@group(3) @binding(1) var<uniform> ng: NgU;
@group(3) @binding(2) var<storage, read> ng_particles: array<NgParticle>;
@group(3) @binding(5) var sim_tex: texture_2d_array<f32>;
// per field cell: the native uv, the vertex colour (water_ng.rs `field_attrs`)
@group(3) @binding(6) var field_uv: texture_2d_array<f32>;
@group(3) @binding(7) var field_col: texture_2d_array<f32>;

const GRID: u32 = 96u;
const QUAD = array<vec2<u32>, 6>(vec2(0u, 0u), vec2(1u, 0u), vec2(1u, 1u), vec2(0u, 0u), vec2(1u, 1u), vec2(0u, 1u));

// ---- the simulated basins ----

struct GOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) rel: vec3<f32>,
    @location(1) @interpolate(flat) kind: u32,
    @location(2) uv: vec2<f32>,
    @location(3) aux: vec2<f32>,
    @location(4) color: vec4<f32>,
    @location(5) world_xy: vec2<f32>,
    @location(6) view_z: f32,
    @location(7) @interpolate(flat) slot: u32,
    @location(8) fuv: vec2<f32>,
}

fn field(slot: u32, uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(wet_tex, lin_samp, uv, i32(ng.slots[slot * 2u].w), 0.0);
}

@vertex
fn vs_grid(@builtin(vertex_index) vi: u32, @builtin(instance_index) slot: u32) -> GOut {
    var o: GOut;
    o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    let q = vi / 6u;
    let qx = q % GRID;
    let qy = q / GRID;
    let a = ng.slots[slot * 2u];
    let b = ng.slots[slot * 2u + 1u];
    let span = a.z * WET_CELLS;
    let cell = span / f32(GRID);
    // The quad: drawn where its water is (some corner inside, the levels
    // of its corners together: a step between pools is a wall).
    var inside = false;
    var lo = 1e9;
    var hi = -1e9;
    for (var k = 0u; k < 4u; k++) {
        let c = vec2<f32>(f32(qx + (k & 1u)), f32(qy + (k >> 1u))) / f32(GRID);
        let w = field(slot, c);
        inside = inside || w.r < cell * 0.75;
        lo = min(lo, w.g);
        hi = max(hi, w.g);
    }
    if !inside || hi - lo > 0.3 {
        return o;
    }
    let g = vec2<u32>(qx, qy) + QUAD[vi % 6u];
    let fuv = vec2<f32>(g) / f32(GRID);
    let w = field(slot, fuv);
    let h = textureSampleLevel(sim_tex, lin_samp, fuv, i32(slot), 0.0).r;
    let xy = a.xy + fuv * span;
    let pos = vec3<f32>(xy, b.x + w.g + h);
    let rel = pos - f.eye.xyz;
    o.clip = f.view_proj * vec4<f32>(rel, 1.0);
    o.rel = rel;
    o.kind = u32(b.z + 0.5);
    // The native uv (bilinear over the field's cells) and vertex colour.
    let layer = i32(a.w);
    let tq = fuv * WET_CELLS - 0.5;
    let ti = vec2<i32>(floor(tq));
    let tf = tq - floor(tq);
    let lim = vec2<i32>(i32(WET_CELLS) - 1);
    let u00 = textureLoad(field_uv, clamp(ti, vec2<i32>(0), lim), layer, 0).rg;
    let u10 = textureLoad(field_uv, clamp(ti + vec2<i32>(1, 0), vec2<i32>(0), lim), layer, 0).rg;
    let u01 = textureLoad(field_uv, clamp(ti + vec2<i32>(0, 1), vec2<i32>(0), lim), layer, 0).rg;
    let u11 = textureLoad(field_uv, clamp(ti + vec2<i32>(1, 1), vec2<i32>(0), lim), layer, 0).rg;
    o.uv = mix(mix(u00, u10, tf.x), mix(u01, u11, tf.x), tf.y);
    // A basin flows out from its middle (as On's basins).
    let d = xy - (a.xy + vec2<f32>(span * 0.5));
    let r = length(d);
    o.aux = select(vec2<f32>(0.0), d / max(r, 1e-3) * 0.25 * min(r / 2.0, 1.0), o.kind == KIND_BASIN);
    o.color = textureSampleLevel(field_col, lin_samp, fuv, layer, 0.0);
    o.world_xy = xy;
    o.view_z = dot(rel, f.forward.xyz);
    o.slot = slot;
    o.fuv = fuv;
    return o;
}

@fragment
fn fs_grid(in: GOut) -> FOut {
    // The water's own outline (the field's distance), not the quads'.
    let w = field(in.slot, in.fuv);
    if w.r > 0.03 {
        discard;
    }
    let s = textureSampleLevel(sim_tex, lin_samp, in.fuv, i32(in.slot), 0.0);
    let si = SurfIn(in.clip, in.rel, in.kind, in.uv, in.aux, in.color, in.world_xy, in.view_z, vec4<f32>(s.r, s.g, s.b, clamp(s.a, 0.0, 1.0)));
    return surface_hq(si);
}

// The native triangles with a flag and a fade in the instance index:
// bits 0–7 the sheets' fade (255: full), bit 8 the lips of a simulated
// basin (its flat water is the grid's).
@vertex
fn vs_water_ng(v: VIn, @builtin(instance_index) ii: u32) -> VOut {
    var o: VOut;
    let rel = v.position - f.eye.xyz;
    o.clip = f.view_proj * vec4<f32>(rel, 1.0);
    o.rel = rel;
    o.kind = u32(v.kind + 0.5);
    o.uv = v.uv;
    o.aux = v.aux;
    o.color = v.color;
    o.color.a *= f32(ii & 255u) / 255.0;
    o.world_xy = v.position.xy;
    o.view_z = dot(rel, f.forward.xyz);
    return o;
}

@fragment
fn fs_surface_lips(in: VOut) -> FOut {
    let n = normalize(cross(dpdx(in.rel), dpdy(in.rel)));
    if abs(n.z) > 0.7 {
        discard;
    }
    return surface_hq(surf_in(in));
}

// ---- the jets ----

// The art's sheets with Realistic's shading; past the breakup the water
// that leaves as drops (the drops' pass) leaves the sheet too. The clear
// water between the white streaks keeps more of the art's own light and
// tint (the original's lit, tinted sheets; Realistic's is nearly clear glass,
// which leaves the streaks standing alone as stripes).
@fragment
fn fs_sheet_ng(in: VOut) -> FOut {
    return sheet_hq(in, SheetLook(0.55, 0.42, 0.55, 0.05));
}

// ---- the jets' drops ----
//
// The jets themselves are the native sheets (Realistic's shading, the art's
// streaks and tint); the particles draw only the water that has left them:
// drops breaking off past the breakup point and the drops thrown up where
// the water lands. Near the eye a particle is one drop of a few millimetres;
// farther out it stands for a small cluster of them (DROP_LOD: the particles
// are far fewer than the drops of a real jet). It is drawn at its own size
// when that is over a pixel, otherwise at a pixel with its cover scaled down
// (a far drop fades, it never grows), and smeared along its motion over a
// fixed exposure (the same streak at any frame rate) with its light spread
// over the streak. Lit as a small sphere of water: the art's light,
// the lamps' light on it and through it (forward scattering), never more
// than DROP_MAX of the light that reaches it.

struct DOut {
    @builtin(position) clip: vec4<f32>,
    // across the streak (−1..1), along it (−1..1)
    @location(0) uv: vec2<f32>,
    // the light without the sky's, cover
    @location(1) color: vec4<f32>,
    // the light's bound without the sky's
    @location(2) bound: vec3<f32>,
    // view depth, the streak's length against its width, fog factor
    @location(3) misc: vec3<f32>,
}

// The drops' exposure (s) and their radius near the eye (m): a range per
// drop; the cluster a particle stands for grows to DROP_LOD times that from
// 4 m to 30 m.
const DROP_EXPOSURE: f32 = 0.0083333;
const DROP_R_MIN: f32 = 0.002;
const DROP_R_MAX: f32 = 0.006;
const DROP_LOD: f32 = 3.5;
// The share of the light reaching a drop it sends toward the eye at most.
const DROP_MAX: f32 = 0.85;

fn drop_hash(x: f32) -> f32 {
    return fract(sin(x * 91.3458) * 47453.5453);
}

@vertex
fn vs_drop(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> DOut {
    var o: DOut;
    o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    let p = ng_particles[ii];
    // misc.z: the share of the particle that has left the sheet (1 for the
    // landings' drops).
    let free = p.misc.z;
    if p.pos_r.w <= 0.0 || free < 0.02 {
        return o;
    }
    let rel = p.pos_r.xyz - f.eye.xyz;
    let z = dot(rel, f.forward.xyz);
    if z < f.right.w * 2.0 + 0.05 {
        return o;
    }
    let px_m = 2.0 * hq.tanv.y * z / hq.size.y;
    let r = mix(DROP_R_MIN, DROP_R_MAX, drop_hash(p.misc.w * 17.0 + 3.0)) * mix(1.0, DROP_LOD, smoothstep(4.0, 30.0, z));
    let rv = max(r, 0.7 * px_m);
    let dir = rel * inverseSqrt(max(dot(rel, rel), 1e-6));
    let vp = p.vel_a.xyz - dir * dot(p.vel_a.xyz, dir);
    let sp = length(vp);
    var ax = f.right.xyz;
    var len = rv;
    if sp > 1e-3 {
        ax = vp / sp;
        len = rv + 0.5 * sp * DROP_EXPOSURE;
    }
    let ay = normalize(cross(dir, ax));
    let c = CORNERS[vi];
    // One pixel of margin around the core (its soft edge).
    let corner = rel + ax * c.x * (len + px_m) + ay * c.y * (rv + px_m);
    o.clip = f.view_proj * vec4<f32>(corner, 1.0);
    o.uv = vec2<f32>(c.y * (rv + px_m) / rv, c.x * (len + px_m) / len);
    // Cover: the drop's area against what is drawn (its own disc at a
    // pixel, smeared over its streak), the far band's fade, how far it has
    // left the sheet.
    let cover = (r / rv) * (r / rv) * (rv / len) * p.misc.x * smoothstep(0.02, 0.4, free);
    // Its light: the art's (how lit the original shows this water: its
    // brightness, in the nearest jet's hue), the lamps on it, and the lamps
    // behind it seen through it.
    let tint = mix(vec3<f32>(1.0), normalize(ng.slot_col[7].rgb + vec3<f32>(1e-3)) * 1.7, 0.35);
    var reach = vec3<f32>(0.0);
    var through = vec3<f32>(0.0);
    for (var i = 0u; i < f.counts.x; i++) {
        let lp = f.lamps[i * 2u];
        let col = f.lamps[i * 2u + 1u];
        let to = lp.xyz - rel;
        let d2 = dot(to, to);
        let w = lamp_weight(d2, lp.w, col.w);
        if w <= 0.0 {
            continue;
        }
        reach += col.rgb * w;
        let ld = to * inverseSqrt(max(d2, 1e-4));
        through += col.rgb * w * pow(max(dot(ld, dir), 0.0), 6.0);
    }
    let base = vec3<f32>(p.misc.y) * tint;
    // (The sky's share is added per fragment: the cube is not bound to the
    // vertex stage.)
    o.color = vec4<f32>(base * 0.75 + reach * 0.35 + through * 0.5, cover);
    // Energy bound: never more than DROP_MAX of what reaches it (the art's
    // light standing in for the light baked into it).
    o.bound = (base + reach) * DROP_MAX;
    o.misc = vec3<f32>(z, len / rv, fog_factor(z));
    return o;
}

@fragment
fn fs_drop(in: DOut) -> @location(0) vec4<f32> {
    // A capsule: round across, round at its ends.
    let along = max(abs(in.uv.y) * in.misc.y - (in.misc.y - 1.0), 0.0);
    let d2 = in.uv.x * in.uv.x + along * along;
    let shape = clamp(1.5 - d2 * 1.25, 0.0, 1.0);
    if shape <= 0.0 {
        discard;
    }
    let dims = vec2<i32>(textureDimensions(depth_tex));
    let px = clamp(vec2<i32>(in.clip.xy), vec2<i32>(0), dims - 1);
    let scene = linear_depth(textureLoad(depth_tex, px, 0));
    let soft = clamp((scene - in.misc.x) / 0.05, 0.0, 1.0);
    if soft <= 0.0 {
        discard;
    }
    if ng.rect.w > 0.5 {
        // NFSU2_WATER_NG_VIEW (a check aid): the drops in opaque red.
        return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    }
    let sky = sky_light();
    let light = min(min(in.color.rgb + sky * 0.2, in.bound + sky * DROP_MAX), vec3<f32>(DROP_MAX));
    let a = in.color.a * shape * soft;
    return vec4<f32>(apply_fog(light, in.misc.z) * a, a);
}

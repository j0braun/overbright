// Animated Water (clone deviation, opt-in; docs/water.md).
//
// The water textures' draws the main view leaves out while the row is on
// (render/water.rs): the fountain basins, pools and streams (surfaces)
// and the falling-water sheets, drawn from world-space copies of the same
// triangles with the native shading (2 · tex · v0, fogged) under
// flow-mapped ripples, splash rings, refraction-like distortion, the
// car cube's reflection and the light-pack lamps' glints; then spray and
// mist billboards where the sheets land. Everything moves on the world
// clock (0x7FB718), so shots are deterministic.

struct Frame {
    // camera-relative view-projection (with the temporal jitter)
    view_proj: mat4x4<f32>,
    // the camera's view matrix (the native cube's camera-relative lookup)
    view: mat4x4<f32>,
    // xyz eye, w = clock (s)
    eye: vec4<f32>,
    // xyz view direction, w = 1 reversed depth
    forward: vec4<f32>,
    // xyz camera right, w = near
    right: vec4<f32>,
    // xyz camera up, w = far
    up: vec4<f32>,
    // world.efx FogValue (far, 1 / (far - start), min), w = 1 fogged
    fog_value: vec4<f32>,
    // rgb, w = the cube: 0 none (black), 1 the native camera-relative
    // lookup, 2 world axes (Enhanced Reflections)
    fog_color: vec4<f32>,
    // x = flip-book fps, y = its frames, z = rain amount, w = pixels per metre
    anim: vec4<f32>,
    // x lamps, y landings, z spray emitters, w particles per emitter
    counts: vec4<u32>,
    // per lamp: camera-relative position + outer radius, colour + inner radius
    lamps: array<vec4<f32>, 16>,
    // per texture layer: scroll kind, speed u, speed v, step
    scroll: array<vec4<f32>, 24>,
    // per landing: camera-relative position + strength
    landings: array<vec4<f32>, 32>,
    // per landing: the water's colour there + a stable id
    landing_color: array<vec4<f32>, 32>,
    // x = arcs' apexes drawn
    counts2: vec4<u32>,
    // per apex: camera-relative position + 1, the jet's velocity + id, colour
    apexes: array<vec4<f32>, 72>,
}

@group(0) @binding(0) var<uniform> f: Frame;

const PI: f32 = 3.14159265;

// Layers of the texture array (water.rs `Layers`): the flip-book first,
// then one each.
const KIND_BASIN: u32 = 0u;
const KIND_POOL: u32 = 1u;
const KIND_STREAM: u32 = 2u;
const KIND_SHEET: u32 = 3u;
const KIND_SHEET_FAR: u32 = 4u;

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn hash1(n: f32) -> f32 {
    return fract(sin(n * 12.9898) * 43758.5453);
}

// Value noise with its gradient (quintic fade): (value, d/dx, d/dy).
fn noised(p: vec2<f32>) -> vec3<f32> {
    let i = floor(p);
    let w = fract(p);
    let u = w * w * w * (w * (w * 6.0 - 15.0) + 10.0);
    let du = 30.0 * w * w * (w * (w - 2.0) + 1.0);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    let k1 = b - a;
    let k2 = c - a;
    let k4 = a - b - c + d;
    let v = a + k1 * u.x + k2 * u.y + k4 * u.x * u.y;
    let g = du * vec2<f32>(k1 + k4 * u.y, k2 + k4 * u.x);
    return vec3<f32>(v, g);
}

// TextureOffset as 0x48B450 computes it (world.wgsl `texture_offset`):
// kind 1 frac(t · speed), kind 2 frac(trunc(t / step) · speed).
fn native_offset(layer: u32, t: f32) -> vec2<f32> {
    let s = f.scroll[layer];
    var x = vec2<f32>(0.0);
    if s.x > 0.5 && s.x < 1.5 {
        x = t * s.yz;
    } else if s.x > 1.5 && s.x < 2.5 {
        x = trunc(t / max(s.w, 1e-3)) * s.yz;
    }
    return x - trunc(x);
}

fn layer_of(kind: u32, t: f32) -> u32 {
    let frames = u32(f.anim.y);
    if kind == KIND_BASIN {
        return u32(floor(t * f.anim.x)) % max(frames, 1u);
    }
    return frames + kind - 1u;
}

// The native fog (world.efx): view depth → factor, 1 = none.
fn fog_factor(view_z: f32) -> f32 {
    if f.fog_value.w < 0.5 {
        return 1.0;
    }
    return clamp((f.fog_value.x - view_z) * f.fog_value.y, f.fog_value.z, 1.0);
}

fn apply_fog(rgb: vec3<f32>, fog: f32) -> vec3<f32> {
    return rgb * fog + f.fog_color.rgb * (1.0 - fog);
}

// The lamps' weight as lamps.rs has it (1 inside the inner radius, falling
// linearly in d² to 0 at the outer one), squared.
fn lamp_weight(d2: f32, outer: f32, inner: f32) -> f32 {
    let o2 = outer * outer;
    let i2 = min(inner * inner, o2 * 0.999);
    let w = clamp((o2 - d2) / (o2 - i2), 0.0, 1.0);
    return w * w;
}

// Glints of the lamps on a surface (camera-relative p, normal n, view v).
fn lamp_glints(p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, power: f32) -> vec3<f32> {
    var c = vec3<f32>(0.0);
    for (var i = 0u; i < f.counts.x; i++) {
        let l = f.lamps[i * 2u];
        let col = f.lamps[i * 2u + 1u];
        let to = l.xyz - p;
        let d2 = dot(to, to);
        let w = lamp_weight(d2, l.w, col.w);
        // Glints reach a little past the pool (the lamp is seen in the
        // water), at a quarter of the weight.
        let far = lamp_weight(d2, l.w * 2.0, col.w * 2.0) * 0.25;
        // A lamp out of reach adds exactly nothing (skipped).
        if max(w, far) <= 0.0 {
            continue;
        }
        let ld = to * inverseSqrt(max(d2, 1e-4));
        let h = normalize(ld + v);
        let s = pow(max(dot(n, h), 0.0), power) * (power + 8.0) / 64.0;
        c += col.rgb * s * max(w, far);
    }
    return c;
}

// The car cube (render/mod.rs `environment`) in direction r (world).
@group(0) @binding(3) var env_tex: texture_cube<f32>;
@group(0) @binding(4) var env_samp: sampler;

fn environment(r: vec3<f32>) -> vec3<f32> {
    let mode = f.fog_color.w;
    if mode < 0.5 {
        // No cube (Car Reflection Update Rate Off): the fog's colour, dimmed.
        return f.fog_color.rgb * 0.6;
    }
    var dir = r;
    if mode < 1.5 {
        // As car.wgsl looks the native cube up: the view-space direction
        // with y and z negated (the faces follow the camera's axes).
        let rv = (f.view * vec4<f32>(r, 0.0)).xyz;
        dir = vec3<f32>(rv.x, -rv.y, -rv.z);
    }
    return textureSampleLevel(env_tex, env_samp, dir, 1.0).rgb;
}

// ---- surfaces and sheets ----

@group(0) @binding(1) var water_tex: texture_2d_array<f32>;
@group(0) @binding(2) var water_samp: sampler;

struct VIn {
    @location(0) position: vec3<f32>,
    @location(1) kind: f32,
    @location(2) uv: vec2<f32>,
    // surfaces: the flow (m/s, world xy); sheets: x = height in the fall
    // (0 where it lands, 1 at the top), y = the sheet's fall length (m)
    @location(3) aux: vec2<f32>,
    @location(4) color: vec4<f32>,
}

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) rel: vec3<f32>,
    @location(1) @interpolate(flat) kind: u32,
    @location(2) uv: vec2<f32>,
    @location(3) aux: vec2<f32>,
    @location(4) color: vec4<f32>,
    @location(5) world_xy: vec2<f32>,
    @location(6) view_z: f32,
}

@vertex
fn vs_water(v: VIn) -> VOut {
    var o: VOut;
    let rel = v.position - f.eye.xyz;
    o.clip = f.view_proj * vec4<f32>(rel, 1.0);
    o.rel = rel;
    o.kind = u32(v.kind + 0.5);
    o.uv = v.uv;
    o.aux = v.aux;
    o.color = v.color;
    o.world_xy = v.position.xy;
    o.view_z = dot(rel, f.forward.xyz);
    return o;
}

// Layered noise gradient of the water's height at p (m), drifting.
fn waves(p: vec2<f32>, t: f32) -> vec2<f32> {
    let a = noised(p * 1.1 + vec2<f32>(t * 0.21, t * 0.13));
    let b = noised(p * 2.7 + vec2<f32>(-t * 0.37, t * 0.29));
    let c = noised(p * 6.3 + vec2<f32>(t * 0.61, -t * 0.47));
    return a.yz * 1.1 * 0.06 + b.yz * 2.7 * 0.03 + c.yz * 6.3 * 0.012;
}

// Flow-mapped waves: two phases half a period apart, each advected along
// the flow and reset at its weight's zero (no stretching).
fn flow_waves(p: vec2<f32>, flow: vec2<f32>, t: f32) -> vec2<f32> {
    let period = 2.0;
    let ph0 = fract(t / period);
    let ph1 = fract(t / period + 0.5);
    let w0 = 1.0 - abs(1.0 - 2.0 * ph0);
    let w1 = 1.0 - abs(1.0 - 2.0 * ph1);
    let g0 = waves(p - flow * ph0 * period + vec2<f32>(0.0, 0.0), t * 0.5);
    let g1 = waves(p - flow * ph1 * period + vec2<f32>(3.7, 1.9), t * 0.5);
    return g0 * w0 + g1 * w1;
}

// Splash rings where the sheets land (the landings), and their churn.
fn splashes(p: vec3<f32>, t: f32, churn: ptr<function, f32>) -> vec2<f32> {
    var g = vec2<f32>(0.0);
    for (var i = 0u; i < f.counts.y; i++) {
        let l = f.landings[i];
        let d3 = p - l.xyz;
        if abs(d3.z) > 1.5 {
            continue;
        }
        let dv = d3.xy;
        let d = length(dv);
        let reach = 1.4 * l.w + 0.4;
        if d > reach {
            continue;
        }
        let dir = dv / max(d, 1e-3);
        let id = f.landing_color[i].w;
        for (var k = 0u; k < 3u; k++) {
            let age = fract(t * 0.9 + f32(k) / 3.0 + hash1(id + f32(k)));
            let r = age * reach;
            let x = (d - r) / 0.09;
            let env = exp(-x * x) * (1.0 - age) * l.w;
            // d/dd of sin(2π (d − r) / λ) · env, λ = 0.22 m
            g += dir * cos((d - r) * (2.0 * PI / 0.22)) * env * 0.9;
        }
        *churn += smoothstep(0.55 * l.w + 0.15, 0.0, d) * l.w;
    }
    return g;
}

// Rain on the water: ring cells (as Wet Roads' ripples), by the rain.
fn rain_rings(p: vec2<f32>, t: f32) -> vec2<f32> {
    let rain = f.anim.z;
    if rain <= 0.001 {
        return vec2<f32>(0.0);
    }
    let cell = 0.45;
    let c = floor(p / cell);
    var g = vec2<f32>(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let id = c + vec2<f32>(f32(x), f32(y));
            let h = hash21(id);
            let centre = (id + vec2<f32>(hash21(id + 7.1), hash21(id + 3.3))) * cell;
            let age = fract(t * 1.3 + h);
            let dv = p - centre;
            let d = length(dv);
            let r = age * cell;
            let s = (d - r) / 0.03;
            g += dv / max(d, 1e-3) * cos(s * 2.0) * exp(-s * s * 0.25) * (1.0 - age);
        }
    }
    return g * 0.35 * rain;
}

struct FOut {
    @location(0) color: vec4<f32>,
}

@fragment
fn fs_surface(in: VOut) -> FOut {
    let t = f.eye.w;
    let p = in.rel;
    // The surface's own normal (streams slope), toward the eye.
    var ng = normalize(cross(dpdx(p), dpdy(p)));
    if dot(ng, -p) < 0.0 {
        ng = -ng;
    }
    var churn = 0.0;
    var g = flow_waves(in.world_xy, in.aux, t);
    g += splashes(p, t, &churn);
    g += rain_rings(in.world_xy, t);
    churn = clamp(churn, 0.0, 1.0);
    // A pool is still: half the waves.
    if in.kind == KIND_POOL {
        g *= 0.5;
    }
    let n = normalize(ng + vec3<f32>(-g.x, -g.y, 0.0));
    let v = normalize(-p);
    // The native texture, its flip-book frame and scroll, shifted by the
    // slope (refraction-like distortion of what lies under the water).
    let layer = layer_of(in.kind, t);
    let uv = in.uv + native_offset(layer, t) + vec2<f32>(g.x, g.y) * 0.045;
    let tex = textureSample(water_tex, water_samp, uv, i32(layer));
    // The native shading.
    var base = clamp(2.0 * tex.rgb * in.color.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    if f.anim.w > 0.5 {
        // NFSU2_WATER_PLAIN (a check aid): the native shading alone.
        var plain: FOut;
        let t0 = textureSample(water_tex, water_samp, in.uv + native_offset(layer, t), i32(layer));
        plain.color = vec4<f32>(apply_fog(clamp(2.0 * t0.rgb * in.color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), fog_factor(in.view_z)), clamp(t0.a * in.color.a, 0.0, 1.0));
        return plain;
    }
    // Light caught in the crests: the slope's length brightens the water
    // a little (no light is added where the art left it dark).
    let crest = clamp(length(g) * 1.5, 0.0, 1.0);
    base *= 0.92 + 0.22 * crest;
    // White water where the sheets land.
    let lum = dot(in.color.rgb, vec3<f32>(0.3, 0.59, 0.11));
    let foam = churn * (0.55 + 0.45 * noised(in.world_xy * 7.0 + vec2<f32>(t * 1.7, -t * 1.3)).x);
    base = mix(base, vec3<f32>(min(lum * 2.2, 1.0)), foam * 0.6);
    // Reflection (Fresnel, F0 = 0.02) of the car cube and the lamps.
    let ndv = clamp(dot(n, v), 0.0, 1.0);
    let fresnel = 0.02 + 0.98 * pow(1.0 - ndv, 5.0);
    let r = reflect(-v, n);
    let env = environment(r);
    var rgb = base * (1.0 - fresnel * 0.6) + env * fresnel * 0.9;
    rgb += lamp_glints(p, n, v, 180.0) * (1.0 - foam * 0.7);
    var o: FOut;
    o.color = vec4<f32>(apply_fog(rgb, fog_factor(in.view_z)), clamp(tex.a * in.color.a, 0.0, 1.0));
    return o;
}

@fragment
fn fs_sheet(in: VOut) -> FOut {
    let t = f.eye.w;
    let p = in.rel;
    let h = in.aux.x;
    let layer = layer_of(in.kind, t);
    let s = f.scroll[layer];
    // The native layer as the original scrolls it, and a second one
    // falling faster and shifted across: the sheet no longer repeats.
    let t1 = textureSample(water_tex, water_samp, in.uv + native_offset(layer, t), i32(layer));
    let off2 = s.yz * t * 1.43;
    // Arcs (aux.y < 0): the second layer three times finer along the jet,
    // as detail on the native one (their v spans 1.7 repeats over 28 m).
    let arc = in.aux.y < 0.0;
    let scale2 = select(vec2<f32>(1.0, 0.71), vec2<f32>(1.3, 3.0), arc);
    let t2 = textureSample(water_tex, water_samp, in.uv * scale2 + vec2<f32>(0.37, 0.0) + (off2 - trunc(off2)), i32(layer));
    // Streaks: noise stretched along the fall, moving with it.
    let fall = vec2<f32>(in.uv.x * 11.0, in.uv.y * 2.2) + s.yz * vec2<f32>(0.0, 2.2) * t * 1.2;
    let streak = noised(fall).x;
    if f.anim.w > 0.5 {
        var plain: FOut;
        plain.color = vec4<f32>(apply_fog(clamp(2.0 * t1.rgb * in.color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), fog_factor(in.view_z)), clamp(t1.a * in.color.a, 0.0, 1.0));
        return plain;
    }
    var a = (0.62 * t1.a + 0.38 * t2.a) * (0.84 + 0.32 * streak);
    var rgb = 2.0 * mix(t1.rgb, t2.rgb, 0.3) * in.color.rgb;
    if arc {
        let d2 = dot(t2.rgb, vec3<f32>(0.3, 0.59, 0.11));
        a = t1.a * (0.7 + 0.55 * t2.a) * (0.8 + 0.4 * streak);
        rgb = 2.0 * t1.rgb * in.color.rgb * (0.72 + 0.56 * d2);
    }
    // Where it lands: brighter, broken into drops.
    let bottom = smoothstep(0.22, 0.0, h);
    let drops = noised(vec2<f32>(in.uv.x * 23.0, in.uv.y * 9.0 + t * 6.0)).x;
    let lum = dot(in.color.rgb, vec3<f32>(0.3, 0.59, 0.11));
    rgb = mix(rgb, vec3<f32>(min(lum * 2.2, 1.0)), bottom * 0.2);
    a *= mix(1.0, smoothstep(0.25, 0.65, drops) * 1.25, bottom * 0.6);
    // Arcs (aux.y < 0, the casino's and the park's jets): the water comes
    // in pulses along the jet (thicker and thinner, moving with the flow
    // toward the landing, h falling), and its edges shimmer and break up.
    if in.aux.y < 0.0 {
        let pulse = noised(vec2<f32>(in.uv.x * 1.7, h * 5.0 + t * 3.1)).x;
        a *= 0.62 + 0.62 * pulse;
        let e = clamp(min(in.uv.x, 1.0 - in.uv.x), 0.0, 0.5);
        let edge = smoothstep(0.0, 0.2, e);
        let shimmer = noised(vec2<f32>(in.uv.x * 31.0, h * 47.0 + t * 11.0)).x;
        a *= mix(smoothstep(0.3, 0.75, shimmer), 1.0, edge);
        rgb += vec3<f32>(pow(shimmer, 6.0) * (1.0 - edge) * 0.5 * (0.4 + lum));
        // Thinning where it breaks up toward the landing.
        a *= 1.0 - 0.35 * smoothstep(0.35, 0.0, h) * (1.0 - drops);
    }
    // A wet sheen: glints of the lamps on the sheet's ripples.
    var ng = normalize(cross(dpdx(p), dpdy(p)));
    if dot(ng, -p) < 0.0 {
        ng = -ng;
    }
    let gn = noised(vec2<f32>(in.uv.x * 6.0, in.uv.y * 3.0 + t * 2.5));
    let n = normalize(ng + vec3<f32>(gn.y, gn.z, 0.0) * 0.12);
    let v = normalize(-p);
    rgb += lamp_glints(p, n, v, 60.0) * a * 0.6;
    let fresnel = 0.02 + 0.98 * pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 5.0);
    rgb += environment(reflect(-v, n)) * fresnel * 0.25 * a;
    var o: FOut;
    o.color = vec4<f32>(apply_fog(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), fog_factor(in.view_z)), clamp(a * in.color.a, 0.0, 1.0));
    return o;
}

// ---- spray ----

@group(1) @binding(0) var depth_tex: texture_depth_2d;

struct SOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    // view depth, size (m), kind (0 mist, 1 drop), seed
    @location(2) misc: vec4<f32>,
}

const CORNERS = array<vec2<f32>, 6>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));
const MIST: u32 = 24u;
const GRAVITY: f32 = 9.81;

// The lamps' light at a camera-relative point (per corner, as the GPU smoke's puffs).
fn lamp_light(p: vec3<f32>) -> vec3<f32> {
    var c = vec3<f32>(0.0);
    for (var i = 0u; i < f.counts.x; i++) {
        let l = f.lamps[i * 2u];
        let col = f.lamps[i * 2u + 1u];
        let to = l.xyz - p;
        c += col.rgb * lamp_weight(dot(to, to), l.w, col.w);
    }
    return c;
}

@vertex
fn vs_spray(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> SOut {
    var o: SOut;
    o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    let per = max(f.counts.w, 1u);
    let landing_count = f.counts.z * per;
    if ii >= landing_count {
        return apex_drop(vi, ii - landing_count);
    }
    let e = ii / per;
    let k = ii % per;
    let l = f.landings[e];
    let lc = f.landing_color[e];
    let strength = l.w;
    let t = f.eye.w;
    let id = lc.w * 131.0 + f32(k) * 7.31;
    var pos = l.xyz;
    var size = 0.0;
    var alpha = 0.0;
    var ax = f.right.xyz;
    var ay = f.up.xyz;
    var kind = 0.0;
    if k < MIST {
        // Mist: rising, spreading and fading over 2–3.4 s.
        let life = 2.0 + 1.4 * hash1(id);
        let x = t / life + hash1(id + 1.7);
        let age = fract(x);
        let cycle = floor(x);
        let a = (hash1(id + cycle * 3.1) * 2.0 - 1.0) * PI;
        let rr = sqrt(hash1(id + cycle * 5.7)) * (0.35 + 0.45 * strength);
        pos += vec3<f32>(cos(a) * rr, sin(a) * rr, 0.1) + vec3<f32>(0.35, 0.2, 0.9) * age * (0.6 + 0.4 * strength);
        size = (0.35 + 1.0 * age) * (0.7 + 0.6 * hash1(id + cycle)) * (0.6 + 0.5 * strength);
        alpha = 0.22 * sin(PI * age) * sin(PI * age) * min(strength, 1.0);
    } else {
        // Drops thrown up where the water lands, falling back (ballistic).
        let up = 1.2 + 1.6 * hash1(id + 0.3);
        let life = 2.0 * up / GRAVITY;
        let x = t / life + hash1(id + 2.9);
        let tau = fract(x) * life;
        let cycle = floor(x);
        let a = hash1(id + cycle * 1.3) * 2.0 * PI;
        let out = (0.5 + 1.1 * hash1(id + cycle * 2.1)) * (0.6 + 0.4 * strength);
        let r0 = sqrt(hash1(id + cycle * 4.7)) * (0.25 + 0.3 * strength);
        let horiz = vec3<f32>(cos(a), sin(a), 0.0);
        pos += horiz * (r0 + out * tau) + vec3<f32>(0.0, 0.0, up * tau - 0.5 * GRAVITY * tau * tau + 0.03);
        let vel = horiz * out + vec3<f32>(0.0, 0.0, up - GRAVITY * tau);
        size = 0.03 + 0.03 * hash1(id + 9.1);
        alpha = 0.75 * smoothstep(0.0, 0.05, tau) * min(strength + 0.3, 1.0);
        // Stretched along the velocity on screen (motion streak).
        let rel0 = pos - f.eye.xyz;
        let vdir = normalize(vel);
        let side = cross(vdir, normalize(rel0));
        if length(side) > 1e-3 {
            ax = normalize(side);
            ay = vdir * (1.0 + 0.04 * length(vel) / size * 0.25);
        }
        kind = 1.0;
    }
    let rel = pos - f.eye.xyz;
    let depth = dot(rel, f.forward.xyz);
    if depth < 0.3 || alpha < 0.002 {
        return o;
    }
    let c = CORNERS[vi];
    let corner = rel + ax * c.x * size + ay * c.y * size;
    o.clip = f.view_proj * vec4<f32>(corner, 1.0);
    o.uv = c;
    // The water's colour lit by the lamps (the bake holds the rest).
    let light = lc.rgb * 0.55 + lamp_light(corner) * 0.6;
    o.color = vec4<f32>(light, alpha * fog_factor(depth));
    o.misc = vec4<f32>(dot(corner, f.forward.xyz), size, kind, hash1(id));
    return o;
}

const PER_APEX: u32 = 40u;

// Drops breaking off an arc's top: thrown on with the jet's velocity,
// spread a little, falling (ballistic) and fading; streaked along their
// velocity like the landings' drops.
fn apex_drop(vi: u32, j: u32) -> SOut {
    var o: SOut;
    o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    let a = j / PER_APEX;
    let k = j % PER_APEX;
    if a >= f.counts2.x {
        return o;
    }
    let p0 = f.apexes[a * 3u];
    let jet = f.apexes[a * 3u + 1u];
    let col = f.apexes[a * 3u + 2u];
    let t = f.eye.w;
    let id = jet.w * 977.0 + f32(k) * 3.17 + 0.5;
    let life = 0.7 + 0.6 * hash1(id);
    let x = t / life + hash1(id + 4.1);
    let tau = fract(x) * life;
    let cycle = floor(x);
    let dir = normalize(vec3<f32>(jet.xy, 0.0) + vec3<f32>(1e-4, 0.0, 0.0));
    let side = vec3<f32>(-dir.y, dir.x, 0.0);
    // Where along the top it breaks off (±1.2 m) and how it scatters.
    let along = (hash1(id + cycle * 1.9) * 2.0 - 1.0) * 1.2;
    let lat = (hash1(id + cycle * 2.7) * 2.0 - 1.0) * 0.25;
    let start = p0.xyz + dir * along + side * lat;
    let v0 = jet.xyz * (0.75 + 0.35 * hash1(id + cycle * 3.3)) + side * (hash1(id + cycle * 5.1) * 2.0 - 1.0) * 0.6 + vec3<f32>(0.0, 0.0, 0.3 + 0.8 * hash1(id + cycle * 6.7));
    let pos = start + v0 * tau + vec3<f32>(0.0, 0.0, -0.5 * GRAVITY * tau * tau);
    let vel = v0 + vec3<f32>(0.0, 0.0, -GRAVITY * tau);
    let size = 0.05 + 0.05 * hash1(id + 8.3);
    let alpha = 0.85 * smoothstep(0.0, 0.06, tau) * (1.0 - smoothstep(0.6, 1.0, tau / life));
    let rel = pos;
    let depth = dot(rel, f.forward.xyz);
    if depth < 0.3 {
        return o;
    }
    var ax = f.right.xyz;
    var ay = f.up.xyz;
    let vdir = normalize(vel);
    let sd = cross(vdir, normalize(rel));
    if length(sd) > 1e-3 {
        ax = normalize(sd);
        ay = vdir * (1.0 + 0.012 * length(vel) / size);
    }
    let c = CORNERS[vi];
    let corner = rel + ax * c.x * size + ay * c.y * size;
    o.clip = f.view_proj * vec4<f32>(corner, 1.0);
    o.uv = c;
    let light = col.rgb * 0.6 + lamp_light(corner) * 0.6;
    o.color = vec4<f32>(light, alpha * fog_factor(depth));
    o.misc = vec4<f32>(dot(corner, f.forward.xyz), size, 1.0, hash1(id));
    return o;
}

fn linear_depth(d: f32) -> f32 {
    let n = f.right.w;
    let fa = f.up.w;
    if f.forward.w > 0.5 {
        return n * fa / (n + d * (fa - n));
    }
    return n * fa / (fa - d * (fa - n));
}

@fragment
fn fs_spray(in: SOut) -> @location(0) vec4<f32> {
    let dims = vec2<i32>(textureDimensions(depth_tex));
    let px = clamp(vec2<i32>(in.clip.xy), vec2<i32>(0), dims - 1);
    let scene = linear_depth(textureLoad(depth_tex, px, 0));
    // Soft against the scene (as the GPU smoke's soft particles): faded over a part of
    // the billboard's size.
    let soft = clamp((scene - in.misc.x) / max(in.misc.y * 0.6, 0.05), 0.0, 1.0);
    if soft <= 0.0 {
        discard;
    }
    let r2 = dot(in.uv, in.uv);
    var shape = 0.0;
    if in.misc.z < 0.5 {
        let n = noised(in.uv * 2.3 + vec2<f32>(in.misc.w * 17.0, f.eye.w * 0.4)).x;
        shape = clamp(1.0 - r2, 0.0, 1.0);
        shape = shape * shape * (0.55 + 0.6 * n);
    } else {
        shape = clamp(1.0 - r2, 0.0, 1.0);
    }
    let a = in.color.a * shape * soft;
    let fogged = in.color.rgb;
    // Premultiplied over; the alpha as the native blend writes it.
    return vec4<f32>(fogged * a, a);
}

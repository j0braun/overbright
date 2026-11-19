// Sparks & Fire, Next-Gen (docs/notes/graphics/sparks-fire.md): the lights' splat,
// the flames' heat haze, the sparks' streaks and the flames.

struct Frame {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    eye: vec4<f32>,       // xyz eye, w time
    depth: vec4<f32>,     // near, far, reversed, hdr
    viewport: vec4<f32>,  // x, y, w, h
    counts: vec4<f32>,    // lights, haze
};

struct Instance {
    a: vec4<f32>,
    b: vec4<f32>,
    c: vec4<f32>,
};

struct Light {
    position: vec4<f32>,
    colour: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> instances: array<Instance>;
@group(0) @binding(2) var<storage, read> lights: array<Light>;
@group(0) @binding(3) var depth_tex: texture_depth_2d;
@group(1) @binding(0) var scene_tex: texture_2d<f32>;
@group(1) @binding(1) var scene_samp: sampler;

const CORNERS = array<vec2<f32>, 6>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));

fn scene_depth(px: vec2<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(depth_tex));
    let c = clamp(vec2<i32>(px), vec2(0), dims - 1);
    return textureLoad(depth_tex, c, 0);
}

fn linear_depth(d: f32) -> f32 {
    let n = frame.depth.x;
    let f = frame.depth.y;
    if (frame.depth.z > 0.5) {
        return n * f / (n + d * (f - n));
    }
    return n * f / (f - d * (f - n));
}

// The view distance along the eye's forward of a world point.
fn view_z(p: vec3<f32>) -> f32 {
    let c = frame.view_proj * vec4(p, 1.0);
    return c.w;
}

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yxz + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(hash(i), hash(i + vec3(1.0, 0.0, 0.0)), u.x), mix(hash(i + vec3(0.0, 1.0, 0.0)), hash(i + vec3(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(hash(i + vec3(0.0, 0.0, 1.0)), hash(i + vec3(1.0, 0.0, 1.0)), u.x), mix(hash(i + vec3(0.0, 1.0, 1.0)), hash(i + vec3(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

fn fbm(p: vec3<f32>) -> f32 {
    return noise(p) * 0.6 + noise(p * 2.03 + 7.1) * 0.28 + noise(p * 4.1 + 3.7) * 0.12;
}

// Black body, 0 dull red … 1 white (mirrors sparks_fire.rs `heat_colour`).
fn heat_colour(t0: f32) -> vec3<f32> {
    let t = clamp(t0, 0.0, 1.0);
    let r = min(t * 3.0, 1.0);
    let g = clamp((t - 0.25) * 1.6, 0.0, 1.0);
    let b = clamp((t - 0.65) * 2.8, 0.0, 1.0);
    return vec3(r, g * g, b * b) * (0.25 + 1.75 * t * t);
}

// The emission's scale: bright enough to bloom in a float scene; within
// 0..1 in an 8-bit one (the native glow passes still pick it up).
fn gain() -> f32 {
    return select(1.0, 1.6, frame.depth.w > 0.5);
}

// Pixel → NDC (the viewport's).
fn ndc_of(px: vec2<f32>) -> vec2<f32> {
    let v = frame.viewport;
    return vec2((px.x - v.x) / v.z * 2.0 - 1.0, 1.0 - (px.y - v.y) / v.w * 2.0);
}

// The world point (eye-relative) at a pixel and its depth.
fn world_at(px: vec2<f32>, d: f32) -> vec3<f32> {
    let h = frame.inv_view_proj * vec4(ndc_of(px), d, 1.0);
    return h.xyz / h.w;
}

// ---------------------------------------------------------------------
// Lights: a screen rectangle round each light's sphere; per pixel the
// surface's position from the depth, its normal from the derivatives,
// Lambert × falloff; the blend multiplies by the surface's colour.

struct LightOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) index: u32,
};

@vertex
fn vs_light(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> LightOut {
    var o: LightOut;
    o.index = ii;
    let l = lights[ii];
    let c = l.position.xyz;
    let r = l.position.w;
    // The sphere's screen bound: project its box's corners.
    var lo = vec2(1e9);
    var hi = vec2(-1e9);
    var behind = false;
    for (var k = 0u; k < 8u; k++) {
        let s = vec3(select(-r, r, (k & 1u) != 0u), select(-r, r, (k & 2u) != 0u), select(-r, r, (k & 4u) != 0u));
        let p = frame.view_proj * vec4(c + s, 1.0);
        if (p.w <= frame.depth.x) {
            behind = true;
        } else {
            lo = min(lo, p.xy / p.w);
            hi = max(hi, p.xy / p.w);
        }
    }
    if (behind) {
        lo = vec2(-1.0);
        hi = vec2(1.0);
    }
    lo = clamp(lo, vec2(-1.0), vec2(1.0));
    hi = clamp(hi, vec2(-1.0), vec2(1.0));
    let k = CORNERS[vi] * 0.5 + 0.5;
    o.pos = vec4(mix(lo, hi, k), 0.0, 1.0);
    return o;
}

@fragment
fn fs_light(in: LightOut) -> @location(0) vec4<f32> {
    let l = lights[in.index];
    let d = scene_depth(in.pos.xy);
    let far = select(1.0, 0.0, frame.depth.z > 0.5);
    let p = world_at(in.pos.xy, d);
    // The normal from the neighbours on the nearer side in x and in y
    // (The screen derivatives across a silhouette — a car's edge in
    // front of the road — gave wild normals and a fringe of lit pixels).
    let pxl = world_at(in.pos.xy - vec2(1.0, 0.0), scene_depth(in.pos.xy - vec2(1.0, 0.0)));
    let pxr = world_at(in.pos.xy + vec2(1.0, 0.0), scene_depth(in.pos.xy + vec2(1.0, 0.0)));
    let pyu = world_at(in.pos.xy - vec2(0.0, 1.0), scene_depth(in.pos.xy - vec2(0.0, 1.0)));
    let pyd = world_at(in.pos.xy + vec2(0.0, 1.0), scene_depth(in.pos.xy + vec2(0.0, 1.0)));
    let dx = select(p - pxl, pxr - p, length(pxr - p) < length(p - pxl));
    let dy = select(p - pyu, pyd - p, length(pyd - p) < length(p - pyu));
    let n0 = cross(dx, dy);
    // A pixel on a depth edge (its nearer neighbours still far apart)
    // takes no light.
    let edge = max(length(dx), length(dy)) > 0.05 * max(length(p - frame.eye.xyz), 1.0);
    var n = vec3(0.0, 0.0, 1.0);
    if (dot(n0, n0) > 1e-12) {
        n = normalize(n0);
    }
    // Face the eye.
    if (dot(n, frame.eye.xyz - p) < 0.0) {
        n = -n;
    }
    let to = l.position.xyz - p;
    let dist = length(to);
    let x = max(1.0 - dist * dist / (l.position.w * l.position.w), 0.0);
    let fall = x * x;
    // Wrapped Lambert: surfaces nearly edge-on still catch a little.
    let ndl = clamp((dot(n, to / max(dist, 1e-3)) + 0.25) / 1.25, 0.0, 1.0);
    let sky = abs(d - far) < 1e-7;
    let k = select(fall * ndl, 0.0, sky || edge);
    return vec4(l.colour.rgb * k, 0.0);
}

// ---------------------------------------------------------------------
// Sparks and flames: a quad per instance.

struct FxOut {
    @builtin(position) pos: vec4<f32>,
    // Spark: x along (px from the tail), y across (px), z length (px), w half width (px).
    // Flame: xy the quad's uv (−1..1: x along the axis, head at −1), z age, w seed.
    @location(0) q: vec4<f32>,
    // Spark: x temperature, y view z, z seed, w fade. Flame: x intensity, y view z, z size (m), w blue.
    @location(1) r: vec4<f32>,
    // Flame: x how end-on it is seen (0 side, 1 along the axis), y its
    // drawn half length over its half width.
    @location(2) e: vec4<f32>,
};

fn to_clip_px(c: vec4<f32>) -> vec2<f32> {
    return (c.xy / c.w * vec2(0.5, -0.5) + 0.5) * frame.viewport.zw;
}

@vertex
fn vs_fx(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> FxOut {
    var o: FxOut;
    let inst = instances[ii];
    let corner = CORNERS[vi];
    if (inst.b.w < 0.5) {
        // A spark: the streak from tail to head in screen space.
        let head = frame.view_proj * vec4(inst.a.xyz, 1.0);
        let tail = frame.view_proj * vec4(inst.b.xyz, 1.0);
        if (head.w <= frame.depth.x || tail.w <= frame.depth.x) {
            o.pos = vec4(2.0, 2.0, 0.0, 1.0);
            return o;
        }
        let ph = to_clip_px(head);
        let pt = to_clip_px(tail);
        var dir = ph - pt;
        var len = length(dir);
        if (len < 1e-3) {
            dir = vec2(1.0, 0.0);
        } else {
            dir = dir / len;
        }
        let side = vec2(-dir.y, dir.x);
        // Its width: the fragment's size on screen, at least a pixel.
        let px_per_m = frame.viewport.w * 0.5 * frame.view_proj[1][1] / head.w;
        let half = max(inst.a.w * px_per_m, 0.75);
        let margin = half + 1.5;
        let along = select(-margin, len + margin, corner.x > 0.0);
        let across = corner.y * margin;
        let px = pt + dir * along + side * across;
        let w = mix(tail.w, head.w, clamp(along / max(len, 1e-3), 0.0, 1.0));
        let z = mix(tail.z / tail.w, head.z / head.w, clamp(along / max(len, 1e-3), 0.0, 1.0));
        let ndc = (px / frame.viewport.zw - 0.5) * vec2(2.0, -2.0);
        o.pos = vec4(ndc * w, z * w, w);
        o.q = vec4(along, across, len, half);
        // Energy spread over a long streak: dimmer per pixel.
        let fade = clamp(16.0 / max(len, 1.0), 0.45, 1.0);
        o.r = vec4(inst.c.y, w, inst.c.z, fade);
        return o;
    }
    if (inst.b.w > 1.5) {
        // A contact glow: a camera-facing capsule round the short slide
        // segment (centre ± axis), its halo a few radii wide.
        let centre = inst.a.xyz;
        let r = inst.a.w;
        let to_eye = normalize(frame.eye.xyz - centre);
        var along = inst.b.xyz - to_eye * dot(inst.b.xyz, to_eye);
        var len = length(along);
        if (len < 1e-4) {
            along = normalize(cross(to_eye, vec3(0.0, 0.0, 1.0) + vec3(1e-3, 0.0, 0.0)));
            len = 0.0;
        } else {
            along = along / len;
        }
        let side = normalize(cross(along, to_eye));
        let ext = 2.6;
        let p = centre + along * (corner.x * (len + r * ext)) + side * (corner.y * r * ext);
        let c = frame.view_proj * vec4(p, 1.0);
        o.pos = c;
        o.q = vec4(corner.x * (len / r + ext), corner.y * ext, inst.c.x, inst.c.z);
        o.r = vec4(inst.c.y, c.w, r, 0.0);
        o.e = vec4(len / r, 0.0, 0.0, 0.0);
        return o;
    }
    // A flame: a quad along its axis, facing the eye as it can.
    let centre = inst.a.xyz;
    let axis = inst.b.xyz;
    let len = length(axis);
    let ax = axis / max(len, 1e-4);
    let to_eye = normalize(frame.eye.xyz - centre);
    // The axis as seen: its part across the view (a flame seen end-on
    // keeps a round quad of its width, never a sliver).
    let f = abs(dot(ax, to_eye));
    var along = ax - to_eye * dot(ax, to_eye);
    if (dot(along, along) < 1e-6) {
        along = cross(to_eye, vec3(0.0, 0.0, 1.0));
    }
    along = normalize(along);
    let side = normalize(cross(along, to_eye)) * inst.a.w;
    let half = max(len * sqrt(max(1.0 - f * f, 0.0)), inst.a.w);
    // A little longer and wider than the shape (the noise and the haze);
    // end-on the quad is centred on the exhaust's end.
    let p = centre + along * half * corner.x * 1.15 + side * corner.y * 1.6;
    let c = frame.view_proj * vec4(p, 1.0);
    o.pos = c;
    o.q = vec4(corner.x * 1.15, corner.y * 1.6, inst.c.x, inst.c.z);
    o.r = vec4(inst.c.y, c.w, inst.a.w, inst.c.w);
    o.e = vec4(f, half / max(inst.a.w, 1e-4), 0.0, 0.0);
    return o;
}

// The scene's view distance at this pixel; the soft fade in front of it.
fn soft(px: vec2<f32>, z: f32, width: f32) -> f32 {
    let s = linear_depth(scene_depth(px));
    return clamp((s - z) / width + 0.5, 0.0, 1.0);
}

@fragment
fn fs_spark(in: FxOut) -> @location(0) vec4<f32> {
    let along = in.q.x;
    let across = in.q.y;
    let len = in.q.z;
    let half = in.q.w;
    // Distance to the core segment, in pixels.
    let t = clamp(along, 0.0, len);
    let d = length(vec2(along - t, across));
    let cover = clamp(half + 0.6 - d, 0.0, 1.0) * clamp(half / max(d, half), 0.0, 1.0);
    if (cover <= 0.001) {
        discard;
    }
    // Hotter at the head (now) than the tail (a shutter ago).
    let k = clamp(t / max(len, 1e-3), 0.0, 1.0);
    let temp = 0.3 + 0.7 * in.r.x * mix(0.7, 1.0, k);
    let vis = soft(in.pos.xy, in.r.y, 0.08);
    let flicker = 0.85 + 0.3 * hash(vec3(in.r.z * 97.0, floor(frame.eye.w * 60.0), 1.0));
    let c = heat_colour(temp) * (cover * vis * in.r.w * flicker * gain() * 2.6);
    return vec4(c, 0.0);
}

// A contact glow: the hot spot where metal grinds — a white-yellow
// core along the slide, a short orange halo, cooling with the native
// particle's age; soft against the wall and road it sits on (the native
// sprite was cut by them along a hard line).
@fragment
fn fs_glow(in: FxOut) -> @location(0) vec4<f32> {
    // Distance to the slide segment, in radii.
    let l = in.e.x;
    let x = in.q.x - clamp(in.q.x, -l, l);
    let d2 = x * x + in.q.y * in.q.y;
    let core = exp(-d2 * 3.0);
    let halo = exp(-d2 * 0.8) * 0.08;
    let dens = core + halo;
    if (dens <= 0.004) {
        discard;
    }
    let age = in.q.z;
    let flicker = 0.85 + 0.3 * hash(vec3(in.q.w * 53.0, floor(frame.eye.w * 60.0), 3.0));
    let temp = clamp(1.0 - age * 0.3 - (1.0 - core) * 0.3, 0.0, 1.0);
    let vis = soft(in.pos.xy, in.r.y, max(in.r.z * 4.0, 0.1));
    let c = heat_colour(temp) * (dens * vis * in.r.x * flicker * gain() * 2.0);
    return vec4(c, 0.0);
}

// The flame's density at uv (x along the axis, −1 the exhaust, +1 the tip).
fn flame_density(uv: vec2<f32>, seed: f32, age: f32) -> f32 {
    let x = uv.x * 0.5 + 0.5;
    // A teardrop: full near the exhaust, thinning to the tip.
    let radius = 0.95 * pow(max(1.0 - x, 0.0), 0.55) * smoothstep(0.0, 0.12, x + 0.02);
    let time = frame.eye.w;
    // Licks: noise scrolling down the axis, faster than the flame.
    let n = fbm(vec3(x * 3.2 - time * 9.0, uv.y * 2.2, seed * 31.0 + time * 1.7));
    let edge = abs(uv.y) / max(radius, 1e-3) + (n - 0.5) * 0.9 * (0.3 + x);
    let body = 1.0 - smoothstep(0.55, 1.0, edge);
    return body * (1.0 - smoothstep(0.75, 1.0, x + (n - 0.5) * 0.35)) * (1.0 - age * 0.6);
}

@fragment
fn fs_flame(in: FxOut) -> @location(0) vec4<f32> {
    let uv = in.q.xy;
    // Seen from the side: the teardrop along the axis. Seen end-on: a
    // round burst, hottest in the middle (the radius in widths).
    let f = smoothstep(0.55, 0.95, in.e.x);
    let rr = length(vec2(uv.x * in.e.y, uv.y));
    let n = fbm(vec3(uv * 2.0, in.q.w * 13.0 + frame.eye.w * 7.0));
    let end_dens = (1.0 - smoothstep(0.35, 1.0, rr + (n - 0.5) * 0.6)) * (1.0 - in.q.z * 0.6);
    let dens = mix(flame_density(uv, in.q.w, in.q.z), end_dens, f);
    if (dens <= 0.002) {
        discard;
    }
    let x = mix(uv.x * 0.5 + 0.5, clamp(rr, 0.0, 1.0), f);
    // Temperature: hottest at the exhaust and the centre line.
    let core = mix(1.0 - smoothstep(0.0, 0.6, abs(uv.y) + x * 0.9), 1.0 - smoothstep(0.0, 0.55, rr), f);
    let temp = clamp((1.0 - x) * 0.85 + core * 0.4, 0.0, 1.0);
    let fire = heat_colour(temp * 0.9 + 0.1);
    // Nitrous (the native flame's blue-orange): a white-blue core at the
    // exhaust, a blue body, orange-yellow tips as it burns out.
    var nos = mix(vec3(0.8, 0.9, 1.0) * (1.0 + core), vec3(0.18, 0.32, 1.0), smoothstep(0.08, 0.5, x));
    nos = mix(nos, vec3(1.0, 0.55, 0.15), smoothstep(0.35, 0.8, x));
    let col = mix(fire, nos, in.r.w);
    let vis = soft(in.pos.xy, in.r.y, max(in.r.z * 0.6, 0.06));
    // Thin at the edges (hot gas, not a solid): the density squared.
    return vec4(col * (dens * dens * vis * gain() * 0.8 * in.r.x), 0.0);
}

// The heat haze: the scene behind refracted by the hot air's gradient,
// strongest just past the flame's tip, fading out over its life.
@fragment
fn fs_haze(in: FxOut) -> @location(0) vec4<f32> {
    let uv = in.q.xy / vec2(1.15, 1.6);
    let x = uv.x * 0.5 + 0.5;
    let r2 = dot(vec2(uv.x * 0.9, uv.y), vec2(uv.x * 0.9, uv.y));
    let mask = (1.0 - smoothstep(0.35, 1.0, r2)) * smoothstep(0.0, 0.35, x) * (1.0 - in.q.z * 0.8);
    if (mask <= 0.002) {
        discard;
    }
    let vis = soft(in.pos.xy, in.r.y, 0.3);
    let time = frame.eye.w;
    let p = vec3(uv * 2.5, in.q.w * 17.0 + time * 6.0);
    let e = 0.15;
    let g = vec2(fbm(p + vec3(e, 0.0, 0.0)) - fbm(p - vec3(e, 0.0, 0.0)), fbm(p + vec3(0.0, e, 0.0)) - fbm(p - vec3(0.0, e, 0.0)));
    // The offset in pixels: a few at the flame's size on screen.
    let px_size = clamp(in.r.z * frame.viewport.w * 0.5 * frame.view_proj[1][1] / max(in.r.y, 0.1), 0.0, 200.0);
    let offset = g * (0.06 * px_size + 1.5) * mask;
    let dims = vec2<f32>(textureDimensions(scene_tex));
    let c = textureSampleLevel(scene_tex, scene_samp, (in.pos.xy + offset) / dims, 0.0);
    return vec4(c.rgb, clamp(mask * vis * 1.5, 0.0, 1.0));
}

// Ambient Detail's props (docs/notes/graphics/ambient-detail.md; clone deviation, opt-in).
// Low-poly street props made in code (cans, bottles, cups, boxes, crumpled paper, leaves and newspaper as curled
// cards), oil stains and drain grates as decals, grit chips and moths. Their materials are made here (nothing from
// EA's art). Shaded like world.efx (colour x baked v0 x 2, fogged with the frame's FogValue) plus the scene's direct
// lights: the light packs' street lamps (or Modern Lighting's physical falloff), the cars' headlight cones and tail
// lights, with wrapped diffuse, translucency for leaves and paper and a specular lobe for metal and glass; darker
// and glossier on a wet road. Own code; pcg2d hash after Jarzynski & Olano (JCGT 9(3) 2020).

struct U {
    view_proj: mat4x4<f32>,   // eye-relative: rotation and projection only
    eye: vec4<f32>,           // xyz, w time
    right: vec4<f32>,         // xyz, w fade start (m)
    up: vec4<f32>,            // xyz, w fade end (m)
    forward: vec4<f32>,       // xyz, w wetness 0..1
    fog_value: vec4<f32>,     // FogValue xyz, w 1 = fog on
    fog_color: vec4<f32>,     // rgb, w debug
    counts: vec4<u32>,        // lamps, spots, points, modern
    lamps: array<vec4<f32>, 32>,   // 16 x (position, radius; colour)
    spots: array<vec4<f32>, 24>,   // 8 x (position, reach; direction, cos outer; colour, cos inner)
    points: array<vec4<f32>, 16>,  // 8 x (position, radius; colour)
};
@group(0) @binding(0) var<uniform> u: U;

struct In {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) pos_s: vec4<f32>,
    @location(4) rot: vec4<f32>,
    @location(5) light_kind: vec4<f32>,
    @location(6) extra: vec4<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) light_kind: vec4<f32>,
    @location(2) @interpolate(flat) extra: vec4<f32>,
    @location(3) rel: vec3<f32>,      // world position - eye
    @location(4) normal: vec3<f32>,
    @location(5) local: vec3<f32>,
};

fn qrot(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

const MOTH: i32 = 10;

@vertex
fn vs_main(p: In) -> VOut {
    var o: VOut;
    let kind = i32(p.light_kind.w + 0.5);
    let centre = p.pos_s.xyz - u.eye.xyz;
    let dist = length(centre);
    // Shrunk to nothing between the fade's start and end (no pop).
    let fade = 1.0 - smoothstep(u.right.w, u.up.w, dist);
    var rel: vec3<f32>;
    var n: vec3<f32>;
    let s = p.pos_s.w * fade;
    if (kind == MOTH) {
        // A camera-facing pair of wings, flapping.
        let flap = 0.55 + 0.45 * sin(u.eye.w * 38.0 + p.extra.x * 60.0);
        rel = centre + (u.right.xyz * p.pos.x * flap + u.up.xyz * p.pos.y * 0.6) * s;
        n = -u.forward.xyz;
    } else {
        let l = p.pos * vec3<f32>(1.0, p.extra.y, p.extra.z);
        rel = centre + qrot(p.rot, l * s);
        n = normalize(qrot(p.rot, p.normal / vec3<f32>(1.0, max(p.extra.y, 0.05), max(p.extra.z, 0.05))));
        if (kind == 7 || kind == 8) {
            // Decals: lifted 1 cm (+ 0.03 % of the distance) off the surface.
            rel += qrot(p.rot, vec3<f32>(0.0, 0.0, 1.0)) * (0.01 + dist * 0.0003);
        }
    }
    o.clip = u.view_proj * vec4<f32>(rel, 1.0);
    o.uv = p.uv;
    o.light_kind = p.light_kind;
    o.extra = vec4<f32>(p.extra.x, p.extra.y, p.extra.z, p.extra.w * fade);
    o.rel = rel;
    o.normal = n;
    o.local = p.pos;
    return o;
}

fn pcg2d(v0: vec2<u32>) -> vec2<u32> {
    var v = v0 * 1664525u + 1013904223u;
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v = v ^ (v >> vec2<u32>(16u));
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v = v ^ (v >> vec2<u32>(16u));
    return v;
}
fn h1(c: vec2<i32>) -> f32 { return f32(pcg2d(bitcast<vec2<u32>>(c)).x) * (1.0 / 4294967296.0); }
fn vnoise(p: vec2<f32>) -> f32 {
    let i = vec2<i32>(floor(p));
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    return mix(mix(h1(i), h1(i + vec2<i32>(1, 0)), w.x), mix(h1(i + vec2<i32>(0, 1)), h1(i + vec2<i32>(1, 1)), w.x), w.y);
}

// Coverage of a signed distance (negative inside), antialiased over one pixel.
fn cover(d: f32) -> f32 {
    let w = max(fwidth(d), 1e-4);
    return clamp(0.5 - d / w, 0.0, 1.0);
}

fn rounded_box(q: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let d = abs(q) - half + r;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

fn palette(seed: f32) -> vec3<f32> {
    let k = fract(seed * 7.31);
    if (k < 0.22) { return vec3<f32>(0.62, 0.08, 0.07); }
    if (k < 0.38) { return vec3<f32>(0.08, 0.16, 0.50); }
    if (k < 0.52) { return vec3<f32>(0.10, 0.36, 0.12); }
    if (k < 0.64) { return vec3<f32>(0.75, 0.55, 0.08); }
    if (k < 0.78) { return vec3<f32>(0.08, 0.08, 0.09); }
    return vec3<f32>(0.70, 0.70, 0.72);
}

struct Mat {
    albedo: vec3<f32>,
    alpha: f32,
    spec: f32,      // specular strength
    gloss: f32,     // Blinn-Phong exponent
    trans: f32,     // light through (leaves, paper)
    emit: vec3<f32>,
};

fn material(kind: i32, uv: vec2<f32>, local: vec3<f32>, seed: f32, sy: f32) -> Mat {
    var m: Mat;
    m.alpha = 1.0;
    m.spec = 0.04;
    m.gloss = 16.0;
    m.trans = 0.0;
    m.emit = vec3<f32>(0.0);
    let sk = vec2<f32>(seed * 97.0, seed * 31.0);
    if (kind == 0) {
        // Can: printed band, bare aluminium rims and ends, dents and scuffs.
        let x = local.x / 0.061;
        let band = palette(seed);
        let rim = smoothstep(0.80, 0.86, abs(x));
        let logo = step(0.55, vnoise(vec2<f32>(x * 3.0, uv.y * 12.0) + sk)) * 0.5;
        m.albedo = mix(mix(band, vec3<f32>(0.85, 0.83, 0.78), logo * step(rim, 0.5)), vec3<f32>(0.62, 0.62, 0.64), rim);
        m.albedo *= 0.75 + 0.35 * vnoise(uv * vec2<f32>(30.0, 18.0) + sk);
        m.spec = 0.65;
        m.gloss = 48.0;
    } else if (kind == 1) {
        // Bottle: green, brown or clear glass, a paper label on the body.
        let k = fract(seed * 3.3);
        var glass = vec3<f32>(0.06, 0.20, 0.07);
        if (k < 0.4) { glass = vec3<f32>(0.20, 0.09, 0.03); } else if (k > 0.8) { glass = vec3<f32>(0.32, 0.36, 0.34); }
        let label = step(abs(local.x + 0.035), 0.045) * step(0.3, fract(seed * 11.0));
        m.albedo = mix(glass, vec3<f32>(0.72, 0.66, 0.52) * (0.8 + 0.3 * vnoise(uv * 20.0 + sk)), label);
        m.spec = mix(1.0, 0.1, label);
        m.gloss = 90.0;
    } else if (kind == 2) {
        // Paper cup: white or kraft, a coloured sleeve, a plastic lid.
        let x = local.x;
        let base = select(vec3<f32>(0.80, 0.78, 0.74), vec3<f32>(0.55, 0.40, 0.26), fract(seed * 5.0) < 0.35);
        let sleeve = step(abs(x + 0.005), 0.03);
        let lid = step(0.05, x);
        m.albedo = mix(mix(base, palette(seed * 1.7) * 0.9 + 0.05, sleeve * 0.85), vec3<f32>(0.12, 0.12, 0.13), lid);
        m.albedo *= 0.8 + 0.25 * vnoise(uv * vec2<f32>(14.0, 22.0) + sk);
        m.spec = mix(0.05, 0.4, lid);
        m.gloss = mix(12.0, 60.0, lid);
    } else if (kind == 3) {
        // Cardboard: kraft brown, corrugation, a tape stripe, darker edges, water stains.
        let q = uv * 2.0 - 1.0;
        let corr = 0.92 + 0.08 * sin(uv.x * 120.0);
        let tape = step(abs(q.x), 0.12) * step(0.45, fract(seed * 9.0));
        m.albedo = vec3<f32>(0.46, 0.33, 0.19) * corr * (0.75 + 0.35 * vnoise(uv * 9.0 + sk));
        m.albedo = mix(m.albedo, vec3<f32>(0.62, 0.52, 0.34), tape);
        m.albedo *= 1.0 - 0.35 * smoothstep(0.82, 1.0, max(abs(q.x), abs(q.y)));
        m.spec = 0.03;
        m.trans = 0.0;
    } else if (kind == 4) {
        // Crumpled paper: white, grey or kraft facets.
        let k = fract(seed * 4.1);
        var c = vec3<f32>(0.78, 0.77, 0.73);
        if (k < 0.3) { c = vec3<f32>(0.52, 0.40, 0.27); } else if (k < 0.5) { c = vec3<f32>(0.60, 0.60, 0.58); }
        m.albedo = c * (0.85 + 0.2 * vnoise(local.xy * 80.0 + sk));
        m.trans = 0.25;
    } else if (kind == 5) {
        // Leaf: a pointed blade with a midrib and veins, autumn colours, translucent.
        let q = uv * 2.0 - 1.0;
        let lobe = vnoise(vec2<f32>(q.x * 4.0, 0.0) + sk) * 0.12;
        let d = length(vec2<f32>(q.x, q.y * (1.0 + 0.9 * abs(q.x)) / 0.95)) - 0.92 + lobe * (1.0 - abs(q.x));
        m.alpha = cover(d);
        let hue = fract(seed * 5.3);
        var c = mix(vec3<f32>(0.42, 0.22, 0.05), vec3<f32>(0.55, 0.38, 0.06), smoothstep(0.2, 0.7, hue));
        c = mix(c, vec3<f32>(0.22, 0.26, 0.07), step(0.85, hue));
        c = mix(c, vec3<f32>(0.28, 0.14, 0.05), step(hue, 0.18));
        let vein = 1.0 - smoothstep(0.0, 0.05, abs(q.y)) * 0.35 - (1.0 - smoothstep(0.0, 0.03, abs(fract(q.x * 3.0 + abs(q.y) * 1.5) - 0.5))) * 0.15;
        m.albedo = c * vein * (0.8 + 0.35 * vnoise(q * 7.0 + sk));
        m.trans = 0.55;
        m.spec = 0.08;
        m.gloss = 24.0;
    } else if (kind == 6) {
        // Newspaper / flyer: a ragged sheet, columns of print, a photo block, creases.
        let q = uv * 2.0 - 1.0;
        let edge = (vnoise(q * 7.0 + sk) - 0.5) * 0.08;
        m.alpha = cover(rounded_box(q, vec2<f32>(0.96, 0.96), 0.03) + edge);
        let base = mix(vec3<f32>(0.70, 0.69, 0.64), vec3<f32>(0.78, 0.74, 0.62), fract(seed * 7.0));
        var c = base * (0.82 + 0.25 * vnoise(q * 3.0 + sk.yx));
        if (fract(seed * 13.0) < 0.7) {
            let col = step(0.08, fract(q.x * 2.0 + 0.04));
            let lines = step(0.5, fract(q.y * 18.0)) * col * step(abs(q.x), 0.85) * step(abs(q.y), 0.85);
            c *= 1.0 - 0.38 * lines * step(0.3, vnoise(q * vec2<f32>(30.0, 4.0) + sk));
            let photo = step(abs(q.x - 0.35), 0.3) * step(abs(q.y + 0.3), 0.25) * step(0.5, fract(seed * 17.0));
            c = mix(c, palette(seed * 2.3) * 0.6 + 0.1, photo * 0.8);
        } else {
            c = mix(c, palette(seed * 2.9), 0.55);
        }
        m.albedo = c;
        m.trans = 0.35;
    } else if (kind == 7) {
        // Oil / grime stain: a soft dark blot, mottled.
        let q = uv * 2.0 - 1.0;
        let r = length(q);
        let n = vnoise(q * 2.5 + sk) * 0.6 + vnoise(q * 7.0 + sk.yx) * 0.4;
        m.alpha = (1.0 - smoothstep(0.35, 1.0, r + (n - 0.5) * 0.5)) * (0.32 + 0.12 * u.forward.w) * (0.6 + 0.4 * n);
        m.albedo = vec3<f32>(0.03, 0.03, 0.035);
        m.spec = 0.5 * u.forward.w + 0.2;
        m.gloss = 60.0;
    } else if (kind == 8) {
        // Drain grate: a cast-iron frame with slots, rust in the corners.
        let q = (uv * 2.0 - 1.0) * vec2<f32>(1.0, 1.0);
        let frame = rounded_box(q, vec2<f32>(0.98, 0.96), 0.04);
        m.alpha = cover(frame);
        let slot = rounded_box(vec2<f32>(fract(q.x * 4.0 + 0.5) - 0.5, q.y), vec2<f32>(0.28, 0.72), 0.12);
        let open = 1.0 - smoothstep(-0.02, 0.02, slot);
        let iron = vec3<f32>(0.16, 0.15, 0.14) * (0.8 + 0.3 * vnoise(q * 12.0 + sk));
        let rust = vec3<f32>(0.24, 0.12, 0.05) * smoothstep(0.55, 0.9, vnoise(q * 4.0 + sk.yx));
        m.albedo = mix(iron + rust, vec3<f32>(0.005), open * step(frame, -0.08));
        m.spec = 0.35 + 0.4 * u.forward.w;
        m.gloss = 40.0;
    } else if (kind == 9) {
        // Grit: dark grey chips.
        m.albedo = vec3<f32>(0.20, 0.19, 0.17) * (0.7 + 0.5 * fract(seed * 9.7));
    } else {
        // Moth: pale dusty wings (two lobes), lit through by the lamp.
        let q = uv * 2.0 - 1.0;
        let wing = length(vec2<f32>(abs(q.x) - 0.5, q.y * 1.2)) - 0.5;
        m.alpha = cover(wing) * 0.9;
        m.albedo = vec3<f32>(0.72, 0.66, 0.55);
        m.trans = 1.0;
    }
    return m;
}

fn lamp_fall(d2: f32, r: f32) -> f32 {
    var fall = clamp(1.0 - d2 / (r * r), 0.0, 1.0);
    fall *= fall;
    if (u.counts.w == 1u) {
        let y = d2 / (r * r);
        let window = max(1.0 - y * y, 0.0);
        fall = min((81.0 + 150.0) / (d2 + 150.0), 1.6) * window * window;
    }
    return fall;
}

struct Lit { diffuse: vec3<f32>, spec: vec3<f32>, };

fn light_one(c: vec3<f32>, l: vec3<f32>, n: vec3<f32>, v: vec3<f32>, m: Mat, acc: ptr<function, Lit>) {
    let ndl = dot(n, l);
    let front = max((ndl + 0.25) / 1.25, 0.0);
    let back = max(-ndl, 0.0) * m.trans;
    (*acc).diffuse += c * (front + back);
    let h = normalize(l + v);
    (*acc).spec += c * pow(max(dot(n, h), 0.0), m.gloss) * step(0.0, ndl);
}

struct FOut {
    @location(0) colour: vec4<f32>,
};

fn shade(in: VOut, front: bool) -> vec4<f32> {
    let kind = i32(in.light_kind.w + 0.5);
    var m = material(kind, in.uv, in.local, in.extra.x, in.extra.y);
    m.alpha *= select(1.0, in.extra.w, kind >= 7);
    let wet = u.forward.w;
    // Soaked paper and leaves darker, everything glossier when wet.
    if (kind >= 3 && kind <= 6) {
        m.albedo *= mix(1.0, 0.62, wet);
    } else {
        m.albedo *= mix(1.0, 0.85, wet);
    }
    m.spec += 0.25 * wet;
    m.gloss = mix(m.gloss, max(m.gloss, 64.0), wet);
    var n = normalize(in.normal);
    if (!front && kind != MOTH) {
        n = -n;
    }
    let p = in.rel;
    let v = -normalize(p);
    var acc: Lit;
    acc.diffuse = vec3<f32>(0.0);
    acc.spec = vec3<f32>(0.0);
    for (var i = 0u; i < u.counts.x; i++) {
        let l = u.lamps[i * 2u];
        let to = l.xyz - p;
        let d2 = dot(to, to);
        let f = lamp_fall(d2, l.w);
        if (f > 1e-4) {
            light_one(u.lamps[i * 2u + 1u].rgb * f, to * inverseSqrt(d2), n, v, m, &acc);
        }
    }
    for (var i = 0u; i < u.counts.y; i++) {
        let e = u.spots[i * 3u];
        let dir = u.spots[i * 3u + 1u];
        let col = u.spots[i * 3u + 2u];
        let to = p - e.xyz;
        let dist = length(to);
        let cone = smoothstep(dir.w, col.w, dot(to / max(dist, 1e-3), dir.xyz));
        let att = clamp(1.0 - dist / e.w, 0.0, 1.0);
        if (cone * att > 1e-4) {
            light_one(col.rgb * cone * att * att, -to / max(dist, 1e-3), n, v, m, &acc);
        }
    }
    for (var i = 0u; i < u.counts.z; i++) {
        let l = u.points[i * 2u];
        let to = l.xyz - p;
        let w = clamp(1.0 - dot(to, to) / (l.w * l.w), 0.0, 1.0);
        if (w > 1e-4) {
            light_one(u.points[i * 2u + 1u].rgb * w * w, normalize(to), n, v, m, &acc);
        }
    }
    // world.efx: colour x v0 x 2 (the baked light the road under it has),
    // a little less on faces turned down; plus the direct lights.
    let baked = in.light_kind.rgb * 2.0 * (0.72 + 0.28 * clamp(n.z, 0.0, 1.0));
    var lit = m.albedo * (baked + acc.diffuse * 0.9) + acc.spec * m.spec * 1.2 + m.emit;
    if (kind == MOTH) {
        // Lit from the lamp through the wings.
        lit = m.albedo * (in.light_kind.rgb * 1.6 + acc.diffuse * 1.6);
    }
    if (u.fog_color.w > 0.5) {
        lit = select(vec3<f32>(1.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), kind >= 7);
    }
    if (u.fog_value.w > 0.5) {
        let view_z = dot(p, u.forward.xyz);
        let f = clamp((u.fog_value.x - view_z) * u.fog_value.y, u.fog_value.z, 1.0);
        lit = lit * f + u.fog_color.rgb * (1.0 - f);
    }
    return vec4<f32>(lit, m.alpha);
}

// Solid props (depth-written). Cards' outlines: alpha to coverage with
// MSAA, else a hard test (smoothed by the temporal pass if on).
@fragment
fn fs_solid(in: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let c = shade(in, front);
    if (c.a < 0.5) {
        discard;
    }
    return vec4<f32>(c.rgb, 1.0);
}

@fragment
fn fs_solid_a2c(in: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let c = shade(in, front);
    if (c.a < 0.02) {
        discard;
    }
    return c;
}

// Decals and moths: blended, no depth write.
@fragment
fn fs_decal(in: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let c = shade(in, front);
    if (c.a < 0.004) {
        discard;
    }
    return c;
}

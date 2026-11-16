
// ---------------------------------------------------------------------
// Wet Road Interaction (clone deviation, docs/wet-road-car.md),
// appended to world.wgsl (both modules). Read only inside the Wet Roads
// branch (frame.wet.x > 0.5, the main view on a wet road) and only with
// wet_fx.window.w = 1: every other view binds zeros.

struct WetFx {
    // centre x, y (world), size (m), on
    window: vec4<f32>,
    // track rate, wake rate (1/s), wake half width (m), 1 = no car boxes (SSR)
    params: vec4<f32>,
    // lights, boxes, tier, clock
    counts: vec4<f32>,
    // the cars' body in the mirror (rgb)
    ambient: vec4<f32>,
    // per light: (position, range), (direction, cos outer), (colour, kind 0 head 1 tail)
    lights: array<vec4<f32>, 96>,
    // per car: (centre, half length), (forward xy, half width, half height), (0)
    boxes: array<vec4<f32>, 24>,
};

@group(0) @binding(12) var<uniform> wet_fx: WetFx;
@group(0) @binding(13) var wet_trail: texture_2d<f32>;
@group(0) @binding(14) var wet_trail_samp: sampler;

// The trail map at `p` (world xy): (track, wake freshness, closeness ×
// freshness, height × freshness), faded out at the window's edge.
fn wet_trail_at(p: vec2<f32>) -> vec4<f32> {
    let d = abs(p - wet_fx.window.xy);
    let half_size = wet_fx.window.z * 0.5;
    let edge = 1.0 - smoothstep(half_size - 8.0, half_size - 2.0, max(d.x, d.y));
    if edge <= 0.0 {
        return vec4<f32>(0.0);
    }
    return textureSampleLevel(wet_trail, wet_trail_samp, p / wet_fx.window.z, 0.0) * edge;
}

// The wake's slope (world xy): rings moving out from the track as it
// ages, stronger in the puddles, and a churned surface on the fresh track.
fn wet_wake_slope(p: vec2<f32>, tr: vec4<f32>, puddle: f32) -> vec2<f32> {
    if tr.g < 0.01 {
        return vec2<f32>(0.0);
    }
    let h = wet_fx.window.z / f32(textureDimensions(wet_trail).x) * 1.5;
    let gx = wet_trail_at(p + vec2<f32>(h, 0.0)).b - wet_trail_at(p - vec2<f32>(h, 0.0)).b;
    let gy = wet_trail_at(p + vec2<f32>(0.0, h)).b - wet_trail_at(p - vec2<f32>(0.0, h)).b;
    let g = vec2<f32>(gx, gy);
    let gl = length(g);
    let away = select(vec2<f32>(0.0), -g / max(gl, 1e-5), gl > 1e-4);
    let fresh = tr.g;
    let age = -log(max(fresh, 1e-3)) / wet_fx.params.y;
    let close = clamp(tr.b / max(fresh, 1e-3), 0.0, 1.0);
    let d = (1.0 - close) * wet_fx.params.z;
    let ring = 0.12 + 0.95 * age;
    let x = (d - ring) / 0.14;
    let height = clamp(tr.a / max(fresh, 1e-3), 0.0, 1.0);
    let wave = sin(x * 3.14159) * exp(-x * x * 0.5) * height * fresh;
    let churn = wet_noise(p * 6.0 + vec2<f32>(age * 4.0, 0.0)).yz * (fresh * fresh * height * (1.0 - smoothstep(0.1, 0.6, d)));
    return away * wave * (0.07 + 0.16 * puddle) + churn * 0.04;
}

// A ray (o, r) against an oriented box (local axes fw, rt, z): the chord's
// length inside it (0: a miss), and the entry distance.
fn wet_box(o: vec3<f32>, r: vec3<f32>, fw: vec2<f32>, half_size: vec3<f32>) -> vec2<f32> {
    let rt = vec2<f32>(-fw.y, fw.x);
    let lo = vec3<f32>(dot(o.xy, fw), dot(o.xy, rt), o.z);
    let lr = vec3<f32>(dot(r.xy, fw), dot(r.xy, rt), r.z);
    let inv = 1.0 / select(lr, vec3<f32>(1e-6), abs(lr) < vec3<f32>(1e-6));
    let t0 = (-half_size - lo) * inv;
    let t1 = (half_size - lo) * inv;
    let tn = min(t0, t1);
    let tf = max(t0, t1);
    let near = max(max(tn.x, tn.y), max(tn.z, 0.0));
    let far = min(min(tf.x, tf.y), tf.z);
    return vec2<f32>(max(far - near, 0.0), near);
}

// The cars' bodies in the wet road's mirror (without SSR, which traces the
// cars themselves): (colour, coverage).
fn wet_car_mirror(pos: vec3<f32>, r: vec3<f32>, rough: f32) -> vec4<f32> {
    let n = u32(wet_fx.counts.y);
    var cover = 0.0;
    for (var i = 0u; i < n; i++) {
        let c = wet_fx.boxes[i * 3u];
        let f = wet_fx.boxes[i * 3u + 1u];
        let o = pos - c.xyz;
        if dot(o.xy, o.xy) > 400.0 {
            continue;
        }
        let body = wet_box(o, r, f.xy, vec3<f32>(c.w, f.z, f.w));
        let cabin = wet_box(o - vec3<f32>(f.xy * (-0.15 * c.w), f.w * 1.55), r, f.xy, vec3<f32>(c.w * 0.5, f.z * 0.85, f.w * 0.6));
        // Soft edges: a thin chord (grazing the box's edge) or a long way
        // to it on a rough surface covers less.
        let soft = 0.05 + rough * 0.6 * (1.0 + body.y);
        let a = smoothstep(0.0, soft, body.x);
        let b = smoothstep(0.0, soft, cabin.x);
        cover = max(cover, max(a, b));
    }
    return vec4<f32>(wet_fx.ambient.rgb, cover);
}

// The cars' head and tail lights mirrored in the wet road (an anisotropic
// lobe stretched along the view: the long streaks at grazing angles) and
// the tail lights' red glow on the road beneath them.
fn wet_car_lights(pos: vec3<f32>, n: vec3<f32>, rough: f32) -> vec3<f32> {
    let nl = u32(wet_fx.counts.x);
    if nl == 0u {
        return vec3<f32>(0.0);
    }
    let v = normalize(frame.camera.xyz - pos);
    let fwd = frame.forward.xyz;
    let t = normalize(vec3<f32>(fwd.y, -fwd.x, 0.0) + vec3<f32>(1e-5, 0.0, 0.0));
    let b = normalize(cross(n, t));
    let grazing = 1.0 - clamp(dot(n, v), 0.0, 1.0);
    let ax = 0.025 + 0.35 * rough;
    let ay = ax * (2.0 + 7.0 * grazing * grazing);
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < nl; i++) {
        let a = wet_fx.lights[i * 3u];
        let dir = wet_fx.lights[i * 3u + 1u];
        let col = wet_fx.lights[i * 3u + 2u];
        let ld = a.xyz - pos;
        let d2 = dot(ld, ld);
        if d2 > a.w * a.w {
            continue;
        }
        let l = ld * inverseSqrt(max(d2, 1e-6));
        let cone = smoothstep(dir.w, dir.w + 0.35, dot(dir.xyz, -l));
        if cone <= 0.0 {
            continue;
        }
        if col.w > 0.5 {
            // The glow on the road behind and beneath the tail.
            sum += col.rgb * cone * max(dot(n, l), 0.0) * 0.07 / (1.0 + d2 * 1.2);
        }
        let h = normalize(v + l);
        // For cost, the streak is narrow across the view; a pixel off
        // its line (the lobe under e^-12: below an 8-bit step after the
        // post chain even for the brightest light) skips the rest. The
        // same image to the eye; this loop was 0.7 ms of the main view at
        // 4K (SSAA 2x).
        let ht = dot(h, t) / ax;
        if ht * ht > 12.0 {
            continue;
        }
        let hn = max(dot(h, n), 0.05);
        let hb = dot(h, b) / ay;
        let lobe = exp(-(ht * ht + hb * hb) / (hn * hn));
        let fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(h, v), 0.0), 5.0);
        let fall = 1.0 / (1.0 + d2 * (6.0 / (a.w * a.w)));
        sum += col.rgb * cone * lobe * fresnel * fall * 3.0;
    }
    return sum;
}

// The interaction over Wet Roads' surface (`wet_surface`): the track takes
// the reflection away (the drier, darker trail), the cars block the
// puddles' mirror, their lights are mirrored.
fn wet_interact(out: ptr<function, WetSurface>, pos: vec3<f32>, tr: vec4<f32>, puddle: f32, k: f32) {
    let track = clamp(tr.r, 0.0, 1.0) * k * (1.0 - 0.6 * puddle);
    // The dry line: the tyre squeezed the film out, so the mirror is
    // nearly gone and rough there, and a puddle's darkening is undone (the
    // asphalt shows through the water it pushed aside).
    (*out).weight *= 1.0 - 0.92 * track;
    (*out).darken = mix((*out).darken, 1.0, 0.8 * track);
    (*out).rough = mix((*out).rough, max((*out).rough, 0.35), track);
    let v = normalize(frame.camera.xyz - pos);
    let n = (*out).normal;
    if wet_fx.params.w < 0.5 {
        let r = reflect(-v, n);
        let m = wet_car_mirror(pos, r, (*out).rough);
        // The body seen in the mirror: what the road reflected there,
        // dimmed (a dark car body against the bright capture), plus the
        // ambient body colour; fully only in the puddles' mirror.
        let share = m.a * k * (0.3 + 0.6 * puddle);
        (*out).reflected = mix((*out).reflected, (*out).reflected * 0.35 + m.rgb, share);
    }
    (*out).extra = wet_car_lights(pos, n, (*out).rough) * k * (1.0 - 0.5 * track);
    if wet_fx.ambient.w > 0.5 {
        // NFSU2_WET_ROAD_DEBUG=1: the trail map (R track, G wake, B closeness).
        (*out).extra = tr.rgb;
    }
}

// --- Rain streaks -----------------------------------------------------
// A box around the eye, the drops anchored in the world (they stream past
// when the camera moves), falling at 7–9 m/s with the wind; each drawn as
// the path it covers in a 1/40 s exposure relative to the camera.
const BOX: vec3<f32> = vec3<f32>(26.0, 26.0, 14.0);

@vertex
fn vs_rain(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    var o: Out;
    o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    let h = hash3(vec3<u32>(ii, 17u, 3u));
    let h2 = hash3(vec3<u32>(ii, 91u, 7u));
    let fall = 7.0 + 2.0 * h2.x;
    let t = f.eye.w;
    let vel = vec3<f32>(f.wind.xy * (0.8 + 0.4 * h2.y), -fall);
    // World anchored: wrap (seed·box + vel·t − eye) into the box.
    let q = h * BOX + vel * t - f.eye.xyz;
    let rel = (fract(q / BOX) - vec3<f32>(0.5, 0.5, 0.35)) * BOX;
    let dist = length(rel);
    if dist < 0.6 || dist > 13.0 {
        return o;
    }
    // The fall over 1/30 s plus the camera's own motion over 1/60 s, the
    // latter at most 0.35 m (long streaks at speed read as warp lines).
    var cam = f.cam_vel.xyz / 60.0;
    let cl = length(cam);
    if cl > 0.35 {
        cam *= 0.35 / cl;
    }
    let a = rel;
    let b = rel - vel / 30.0 + cam - normalize(vel) * 0.1;
    let ca = f.view_proj * vec4<f32>(a, 1.0);
    let cb = f.view_proj * vec4<f32>(b, 1.0);
    if ca.w < 0.3 || cb.w < 0.3 {
        return o;
    }
    let sa = ca.xy / ca.w;
    let sb = cb.xy / cb.w;
    let px = 2.0 / vec2<f32>(textureDimensions(depth_tex));
    var dir = sb - sa;
    if length(dir / px) < 0.5 {
        dir = vec2<f32>(0.0, px.y);
    }
    let side = normalize(vec2<f32>(-dir.y, dir.x) / px) * px;
    // ~1.2 px wide, thicker close.
    let width = clamp(1.2 + 3.0 / dist, 1.0, 3.0) * 0.5;
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let s = mix(sa, sb, corner.y) + side * width * (corner.x * 2.0 - 1.0);
    let w = mix(ca.w, cb.w, corner.y);
    let z = mix(ca.z / ca.w, cb.z / cb.w, corner.y);
    o.clip = vec4<f32>(s * w, z * w, w);
    o.uv = corner;
    let lit = light_at(rel, 1.0);
    // Faint in the distance, and a drop's own small area.
    let alpha = 0.6 * (1.0 - smoothstep(6.0, 13.0, dist)) * smoothstep(0.6, 3.0, dist) * (0.6 + 0.4 * h2.z);
    o.colour = vec4<f32>(lit * alpha, alpha * 0.2);
    o.misc = vec4<f32>(ca.w, 0.3, 0.0, h2.z);
    return o;
}

// --- Splashes ---------------------------------------------------------
// Each instance takes a random spot of the screen for one splash's life,
// finds the surface there in the scene's depth, keeps it when the surface
// faces up (road, roofs, puddles) and draws a small crown on it.
// (Was 0.3 s; now 0.2 s: a raindrop's crown is gone in a blink.)
const SPLASH_LIFE: f32 = 0.2;

fn rel_at(px: vec2<f32>) -> vec4<f32> {
    let d = scene_depth_px(px);
    let dims = vec2<f32>(textureDimensions(depth_tex));
    let ndc = vec2<f32>(px.x / dims.x * 2.0 - 1.0, 1.0 - px.y / dims.y * 2.0);
    let p = f.inv_view_proj * vec4<f32>(ndc, d, 1.0);
    return vec4<f32>(p.xyz / p.w, d);
}

// Splashes were seen drawn over objects the rain cannot land on
// (a car's or a wall's edge): a splash only where the
// rain lands on open ground.
// * Not at a depth edge: the point and its neighbours on both sides lie on
//   one plane (a pixel on a car's or a wall's silhouette took the normal
//   of the road beside it and floated over the object).
// * Not on or under a car (the nearest eight cars' footprints, from their
//   head and tail light points; a car driving over a splash's pixel moved
//   it onto its roof).
// * Not under a cover: something drawn in front of the points 3 m and 7 m
//   above it (bridges, tunnels, overhangs, trees).
fn splash_ground(px: vec2<f32>, c: vec3<f32>, cx: vec3<f32>, cy: vec3<f32>) -> bool {
    let cxm = rel_at(px - vec2<f32>(4.0, 0.0)).xyz;
    let cym = rel_at(px - vec2<f32>(0.0, 4.0)).xyz;
    let ex = length((cx - c) + (cxm - c));
    let ey = length((cy - c) + (cym - c));
    let span = length(cx - c) + length(cy - c) + 0.02;
    if ex + ey > 0.25 * span {
        return false;
    }
    let ns = u32(f.wind.z);
    for (var i = 0u; i < ns; i++) {
        let head = f.spots[i * 4u].xyz;
        let tail = f.spots[i * 4u + 3u].xyz;
        let fwd2 = head.xy - tail.xy;
        let len = length(fwd2);
        if len < 0.5 {
            continue;
        }
        let fw = fwd2 / len;
        let mid = (head.xy + tail.xy) * 0.5;
        let d = c.xy - mid;
        if abs(dot(d, fw)) < len * 0.5 + 0.45 && abs(dot(d, vec2<f32>(-fw.y, fw.x))) < 1.15 {
            return false;
        }
    }
    let dims = vec2<f32>(textureDimensions(depth_tex));
    for (var k = 0; k < 2; k = k + 1) {
        let up = c + vec3<f32>(0.0, 0.0, select(3.0, 7.0, k == 1));
        let clip = f.view_proj * vec4<f32>(up, 1.0);
        if clip.w <= 0.1 {
            continue;
        }
        let ndc = clip.xy / clip.w;
        if any(abs(ndc) > vec2<f32>(1.0)) {
            continue;
        }
        let upx = vec2<f32>((ndc.x * 0.5 + 0.5) * dims.x, (0.5 - ndc.y * 0.5) * dims.y);
        if linear_depth(scene_depth_px(upx)) < clip.w - 0.5 {
            return false;
        }
    }
    return true;
}

@vertex
fn vs_splash(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    var o: Out;
    o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    let h0 = hash3(vec3<u32>(ii, 5u, 11u));
    let cycle = f.eye.w / SPLASH_LIFE + h0.x;
    let slot = u32(floor(cycle));
    let age = fract(cycle);
    let h = hash3(vec3<u32>(ii, slot, 23u));
    // The lower part of the screen holds the ground.
    let px = vec2<f32>(h.x, 0.3 + 0.7 * h.y) * vec2<f32>(textureDimensions(depth_tex));
    let c = rel_at(px);
    let dist = length(c.xyz);
    if dist < 1.5 || dist > 40.0 || linear_depth(c.w) > 0.9 * f.depth.y {
        return o;
    }
    // Uniform in the world, not on the screen: the near ones are fewer.
    if h.z > clamp(dist * dist / 150.0, 0.12, 1.0) {
        return o;
    }
    let cx = rel_at(px + vec2<f32>(4.0, 0.0)).xyz;
    let cy = rel_at(px + vec2<f32>(0.0, 4.0)).xyz;
    let n = normalize(cross(cx - c.xyz, cy - c.xyz));
    if abs(n.z) < 0.85 {
        return o;
    }
    if !splash_ground(px, c.xyz, cx, cy) {
        return o;
    }
    // A crown 5–9 cm across, rising and spreading over its life. (It
    // was 8–16 cm and doubled at 30 m: white domes all over the road, the
    // "popcorn"; the far ones now stay small and fade as they get finer
    // than a pixel.)
    let size = (0.045 + 0.035 * h0.y) * (1.0 + dist / 60.0);
    let right = normalize(cross(normalize(c.xyz), vec3<f32>(0.0, 0.0, 1.0)));
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let rel = c.xyz + right * (corner.x * 2.0 - 1.0) * size + vec3<f32>(0.0, 0.0, corner.y * size * 0.9);
    o.clip = f.view_proj * vec4<f32>(rel, 1.0);
    o.uv = corner;
    // A splash is seen by the light it catches (lamps, headlights,
    // tail lights): most of the flat ambient taken off, so a dark stretch
    // of road keeps dark and the lit pools sparkle.
    let lit = max(light_at(c.xyz + vec3<f32>(0.0, 0.0, 0.05), 0.5) - f.ambient.xyz * 0.6, vec3<f32>(0.0));
    let alpha = 0.7 * (1.0 - age) * (1.0 - smoothstep(15.0, 35.0, dist));
    o.colour = vec4<f32>(lit * alpha * 1.1, alpha * 0.2);
    o.misc = vec4<f32>(view_depth(c.xyz) - 0.12, 0.06, 1.0, age);
    return o;
}

// --- Lamp glow in the wet air -----------------------------------------
@vertex
fn vs_halo(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    var o: Out;
    o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    if ii >= u32(f.depth.w) {
        return o;
    }
    let l = f.lamps[ii * 2u];
    let col = f.lamps[ii * 2u + 1u].xyz;
    // The lamp's head (the light packs' point light).
    let rel = l.xyz;
    let dist = length(rel);
    if dist < 2.0 {
        return o;
    }
    let size = 2.2 + 0.08 * dist;
    let to_eye = -rel / dist;
    let right = normalize(cross(vec3<f32>(0.0, 0.0, 1.0), to_eye));
    let up = cross(to_eye, right);
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u)) * 2.0 - 1.0;
    let p = rel + (right * corner.x + up * corner.y) * size;
    o.clip = f.view_proj * vec4<f32>(p, 1.0);
    o.uv = corner;
    let wet = f.cam_vel.w * 0.75 + f.ambient.w * 0.25;
    let s = 0.22 * wet * (1.0 - smoothstep(150.0, 300.0, dist));
    o.colour = vec4<f32>(col * s, 0.0);
    o.misc = vec4<f32>(view_depth(rel), size, 2.0, 0.0);
    return o;
}

// --- Spray and mist ----------------------------------------------------
@vertex
fn vs_spray(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    var o: Out;
    o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    let pf = puffs[ii];
    let rel = pf.p.xyz;
    let size = pf.p.w;
    let dist = length(rel);
    if dist < 0.5 || pf.q.w <= 0.0 {
        return o;
    }
    let to_eye = -rel / dist;
    let right = normalize(cross(vec3<f32>(0.0, 0.0, 1.0), to_eye));
    let up = cross(to_eye, right);
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u)) * 2.0 - 1.0;
    let ang = pf.q.z * 6.283 + pf.q.x * 1.5;
    let rc = vec2<f32>(cos(ang) * corner.x - sin(ang) * corner.y, sin(ang) * corner.x + cos(ang) * corner.y);
    var p = rel + (right * rc.x + up * rc.y) * size;
    var a = pf.q.w;
    if f.size.w > 0.5 && pf.q.y < 0.5 {
        // Wet Road Interaction: a droplet streaked by its motion
        // over a 1/45 s exposure, relative to the camera (the velocities
        // follow the puffs, wet_spray), its light spread over the streak.
        let vp = puffs[SPRAY_MAX + ii / 2u];
        let vel = select(vp.p.xyz, vp.q.xyz, (ii & 1u) == 1u);
        let m = vel * (1.0 / 45.0);
        let mp = m - to_eye * dot(m, to_eye);
        let l = length(mp);
        if l > size * 0.5 {
            let ax = mp / l;
            let bx = normalize(cross(to_eye, ax));
            p = rel - mp * 0.5 + ax * corner.x * (size + l * 0.5) + bx * corner.y * size;
            a *= size / (size + l * 0.5);
        }
    }
    o.clip = f.view_proj * vec4<f32>(p, 1.0);
    o.uv = corner;
    let lit = light_at(rel, 0.3) * 0.7 + vec3<f32>(0.02);
    o.colour = vec4<f32>(lit * a, a);
    o.misc = vec4<f32>(view_depth(rel), max(size * 0.5, 0.1), 3.0 + pf.q.y, pf.q.z);
    return o;
}

fn soft(clip: vec4<f32>, z: f32, softness: f32) -> f32 {
    let sz = linear_depth(scene_depth_px(clip.xy));
    return clamp((sz - z) / softness, 0.0, 1.0);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let fr = p - i;
    let u = fr * fr * (3.0 - 2.0 * fr);
    let o = vec2<u32>(vec2<i32>(i) + vec2<i32>(4096));
    let a = hash3(vec3<u32>(o, 1u)).x;
    let b = hash3(vec3<u32>(o + vec2<u32>(1u, 0u), 1u)).x;
    let c = hash3(vec3<u32>(o + vec2<u32>(0u, 1u), 1u)).x;
    let d = hash3(vec3<u32>(o + vec2<u32>(1u, 1u), 1u)).x;
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

@fragment
fn fs_main(in: Out) -> @location(0) vec4<f32> {
    let kind = u32(in.misc.z + 0.5);
    var shape = 1.0;
    switch kind {
        case 0u: {
            // Streak: soft across, tapered ends.
            let x = abs(in.uv.x * 2.0 - 1.0);
            shape = (1.0 - x * x) * smoothstep(0.0, 0.25, in.uv.y) * smoothstep(1.0, 0.6, in.uv.y);
        }
        case 1u: {
            // Splash: a flat flash on the surface and a few droplets
            // thrown up and out on short arcs.
            let p = vec2<f32>(in.uv.x * 2.0 - 1.0, in.uv.y);
            let age = in.misc.w;
            // (The flash a thin ring on the surface, not a dome.)
            let ring = abs(length(vec2<f32>(p.x, p.y * 4.0)) - 0.35 * min(age * 2.0, 1.0));
            var v = exp(-ring * ring * 60.0) * (1.0 - age) * 0.5 * (1.0 - smoothstep(0.0, 0.12, p.y));
            for (var k = 0; k < 5; k = k + 1) {
                let a = (f32(k) - 2.0) * 0.38 + (fract(f32(k) * 0.618) - 0.5) * 0.2;
                let t = min(age * 1.6, 1.0);
                let q = vec2<f32>(sin(a) * 0.85 * t, cos(a) * 2.2 * t * (1.0 - t) + 0.03);
                let d = p - q;
                v += exp(-dot(d, d) / 0.004) * (1.0 - t * 0.7);
            }
            shape = min(v, 1.0);
        }
        case 2u: {
            let r2 = dot(in.uv, in.uv);
            shape = exp(-r2 * 5.0) + 0.35 * exp(-r2 * 1.6);
            shape *= 1.0 - smoothstep(0.85, 1.0, r2);
        }
        case 3u: {
            // Droplets: a tight, bright dot.
            let r2 = dot(in.uv, in.uv);
            shape = max(1.0 - r2 * 1.6, 0.0);
            shape *= shape;
        }
        default: {
            // Mist: soft noisy puff (one octave; the puffs are faint and
            // large, so this is the fill-rate cost of the row).
            let r2 = dot(in.uv, in.uv);
            if r2 >= 1.0 {
                discard;
            }
            let n = noise2(in.uv * 3.0 + vec2<f32>(in.misc.w * 37.0));
            shape = (1.0 - r2) * smoothstep(0.1, 0.8, n);
        }
    }
    var a = shape * soft(in.clip, in.misc.x, in.misc.y);
    if kind == 2u {
        // The glow is behind its lamp's own geometry only when a wall is
        // well in front of the lamp.
        a = shape * clamp((linear_depth(scene_depth_px(in.clip.xy)) - in.misc.x + 3.0) / 3.0, 0.0, 1.0);
    }
    if a <= 0.002 {
        discard;
    }
    return in.colour * a;
}



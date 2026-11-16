// =====================================================================
// Next-Gen rain (Rain Detail "Next-Gen", docs/rain-nextgen.md; a
// clone deviation, nothing here is native). Only the Next-Gen entry
// points below read the ng_* fields; tiers On / High run the code above
// unchanged.

// Henyey–Greenstein phase × 4π (1 for isotropic scattering).
fn hg4(g: f32, c: f32) -> f32 {
    let d = 1.0 + g * g - 2.0 * g * c;
    return (1.0 - g * g) / (d * sqrt(d));
}

// The light a drop at `rel` scatters toward the eye: lamps (the light
// packs' (1 − d²/r²)² falloff) and the cars' headlight cones and tail
// lights, with water's strong forward scattering (HG g 0.75, plus a weak
// back lobe: the drop's retro glint). The ambient is what a drop refracts
// of its wide surroundings (the fog colour × 0.8, the overcast city sky).
fn light_ng(rel: vec3<f32>) -> vec3<f32> {
    var c = f.ambient.xyz * 0.8;
    let view = normalize(rel);
    let nl = u32(f.depth.w);
    for (var i = 0u; i < nl; i++) {
        let l = f.lamps[i * 2u];
        let d = rel - l.xyz;
        let d2 = dot(d, d);
        let o2 = l.w * l.w;
        if d2 >= o2 { continue; }
        var w = 1.0 - d2 / o2;
        w *= w;
        // Light travelling lamp → drop, scattered back to the eye (−view):
        // cos of the scattering angle (1: looking into the lamp).
        let cs = -dot(d * inverseSqrt(max(d2, 1e-4)), view);
        let ph = 0.85 * hg4(0.75, cs) + 0.15 * hg4(-0.3, cs);
        c += f.lamps[i * 2u + 1u].xyz * w * min(ph, 12.0) * 0.35;
    }
    let ns = u32(f.wind.z);
    for (var i = 0u; i < ns; i++) {
        let a = f.spots[i * 4u];
        let b = f.spots[i * 4u + 1u];
        let col = f.spots[i * 4u + 2u];
        let tail = f.spots[i * 4u + 3u];
        let d = rel - a.xyz;
        let dist = length(d);
        if dist < a.w {
            let dn = d / max(dist, 1e-3);
            let cone = smoothstep(b.w, col.w, dot(dn, b.xyz));
            let fall = 1.0 - dist / a.w;
            let ph = 0.85 * hg4(0.75, -dot(dn, view)) + 0.15 * hg4(-0.3, -dot(dn, view));
            c += col.xyz * cone * fall * fall * min(ph, 12.0) * 0.5;
        }
        let t = rel - tail.xyz;
        let td = dot(t, t);
        let tn = t * inverseSqrt(max(td, 1e-4));
        c += vec3<f32>(0.35, 0.025, 0.012) * 1.2 * min(hg4(0.75, -dot(tn, view)), 8.0) / (1.0 + td * 2.5);
    }
    return c;
}

// Under geometry (screen-space, outside the native cover zones, where the
// whole rain stops): something drawn in front of the points 2.5, 6 and
// 12 m above `rel` whose surface lies over it (within a cone around the
// vertical) and above it: a bridge, a canopy, an overhang, a tree seen in
// the frame. Covers off screen are not found (open; the native cover
// zones handle the player's own).
fn covered_ng(rel: vec3<f32>) -> bool {
    let dims = vec2<f32>(textureDimensions(depth_tex));
    for (var k = 0; k < 3; k = k + 1) {
        let hgt = select(select(2.5, 6.0, k == 1), 12.0, k == 2);
        let up = rel + vec3<f32>(0.0, 0.0, hgt);
        let clip = f.view_proj * vec4<f32>(up, 1.0);
        if clip.w <= 0.2 {
            continue;
        }
        let ndc = clip.xy / clip.w;
        if any(abs(ndc) > vec2<f32>(1.0)) {
            continue;
        }
        let px = vec2<f32>((ndc.x * 0.5 + 0.5) * dims.x, (0.5 - ndc.y * 0.5) * dims.y);
        let s = rel_at(px);
        if linear_depth(s.w) < clip.w - 0.4 && s.z > rel.z + 0.4 && length(s.xy - rel.xy) < 0.35 * hgt + 0.8 {
            return true;
        }
    }
    return false;
}

// Three nested layers of drops around the eye, each world-anchored in its
// own box (they stream past as the camera moves; the boxes of the outer
// two sit ahead of the eye): near (0.35–7 m), mid (7–18 m), far (18–42 m;
// one streak stands for a bundle of drops). Mass: 40 / 35 / 25 %.
fn layer_ng(h: f32) -> u32 {
    if h < 0.4 { return 0u; }
    if h < 0.75 { return 1u; }
    return 2u;
}

// Exposure (s) of the streaks: a drop's path relative to the camera over
// it, so a moving camera blurs the rain along its motion.
const NG_SHUTTER: f32 = 1.0 / 45.0;
// Drop diameter (m).
const NG_DROP: f32 = 0.0022;

@vertex
fn vs_rain_ng(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    var o: Out;
    o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    let h = hash3(vec3<u32>(ii, 41u, 3u));
    let h2 = hash3(vec3<u32>(ii, 77u, 9u));
    let layer = layer_ng(h2.x);
    var box = vec3<f32>(14.0, 14.0, 10.0);
    var r0 = 0.35;
    var r1 = 7.0;
    var bundle = 1.0;
    if layer == 1u {
        box = vec3<f32>(36.0, 36.0, 18.0);
        r0 = 7.0;
        r1 = 18.0;
        bundle = 3.0;
    } else if layer == 2u {
        box = vec3<f32>(80.0, 80.0, 30.0);
        r0 = 18.0;
        r1 = 42.0;
        bundle = 9.0;
    }
    // Terminal velocity of 1.5–3 mm drops (6.5–9 m/s); the wind with its
    // gusts, carried more by the smaller drops; a slight per-drop sway.
    let fall = 6.5 + 2.5 * h2.y;
    let carry = 1.15 - 0.3 * h2.y;
    let t = f.eye.w;
    let sway = vec2<f32>(sin(t * (1.3 + h2.z) + h.x * 40.0), cos(t * (1.1 + h2.z) + h.y * 40.0)) * 0.06;
    let vel = vec3<f32>(f.ng_wind.xy * carry, -fall);
    let disp = vec3<f32>(f.ng_wind.zw * carry + sway, -fall * t);
    let centre = vec3<f32>(f.ng_view.xy * (0.5 * f32(layer)), 0.0);
    let q = h * box + disp - f.eye.xyz - centre;
    let rel = (fract(q / box) - vec3<f32>(0.5, 0.5, 0.4)) * box + centre;
    let dist = length(rel);
    if dist < r0 * 0.9 || dist > r1 * 1.1 {
        return o;
    }
    // The layers overlap a little and cross-fade (no seam).
    let lw = smoothstep(r0 * 0.9, r0 * 1.1, dist) * (1.0 - smoothstep(r1 * 0.9, r1 * 1.1, dist));
    // Relative motion over the exposure, at most 1.6 m.
    var path = (vel - f.cam_vel.xyz) * NG_SHUTTER;
    let pl = length(path);
    if pl > 1.6 {
        path *= 1.6 / pl;
    }
    let a = rel;
    let b = rel - path;
    let ca = f.view_proj * vec4<f32>(a, 1.0);
    let cb = f.view_proj * vec4<f32>(b, 1.0);
    if ca.w < 0.25 || cb.w < 0.25 {
        return o;
    }
    if covered_ng(rel) {
        return o;
    }
    let dims = vec2<f32>(textureDimensions(depth_tex));
    let sa = ca.xy / ca.w;
    let sb = cb.xy / cb.w;
    let px = 2.0 / dims;
    var dir = sb - sa;
    if length(dir / px) < 0.5 {
        dir = vec2<f32>(0.0, px.y);
    }
    let side = normalize(vec2<f32>(-dir.y, dir.x) / px) * px;
    // The drop's own width in pixels at its depth: never thinner than a
    // pixel; a thinner drop is fainter instead (no aliasing, the same
    // look at every resolution).
    let ppm = f.ng_view.w * dims.y * 0.5;
    let drop_px = NG_DROP * ppm / ca.w;
    let width = max(drop_px, 1.0) * 0.5 + 0.35;
    let coverage = min(drop_px * bundle * 2.5, 1.0);
    // The same light spread over a longer streak: each pixel sees the drop
    // for a shorter part of the exposure (softened, √: the eye integrates
    // a moving drop over more than one exposure, so rain at speed does not
    // vanish).
    let energy = clamp(sqrt(fall * NG_SHUTTER / max(pl, 1e-3)), 0.4, 1.0);
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let s = mix(sa, sb, corner.y) + side * width * (corner.x * 2.0 - 1.0);
    let w = mix(ca.w, cb.w, corner.y);
    let z = mix(ca.z / ca.w, cb.z / cb.w, corner.y);
    o.clip = vec4<f32>(s * w, z * w, w);
    o.uv = corner;
    let lit = light_ng(rel);
    let alpha = 0.9 * coverage * energy * lw * smoothstep(0.35, 1.2, dist) * (0.6 + 0.4 * h2.z);
    o.colour = vec4<f32>(lit * alpha, alpha * 0.25);
    o.misc = vec4<f32>(ca.w, 0.25, 0.0, h2.z);
    return o;
}

// --- Next-Gen splashes: world-anchored impacts on every up-facing
// surface ------------------------------------------------------------
// Each instance takes a world spot in a 44 × 44 m region ahead of the eye
// for one splash's life (anchored: it stays put as the camera moves) and
// finds the surface over it in the scene's depth: from the ground under
// the eye, the depth at that point's pixel gives a height, and the point
// at that height is tried again (three times); kept when it lands back on
// itself (the spot is seen, not hidden behind something), faces up and is
// not at a depth edge or under a cover. So it also lands on car roofs and
// bonnets, rails and kerbs, never on walls.
const NG_SPLASH_LIFE: f32 = 0.32;
const NG_SPLASH_BOX: f32 = 44.0;

struct Spot {
    ok: bool,
    rel: vec3<f32>,
    n: vec3<f32>,
    age: f32,
    seed: vec3<f32>,
    // 0 road film, 1 puddle, 2 raised (car bodies, rails, kerbs)
    surface: u32,
    puddle: f32,
};

// Wet Roads' puddle field: the one wetness (wetness.wgsl; the
// same field and √film share world.wgsl `wet_surface` and
// `puddle::puddle_at` on the CPU use) at world `p`, without the
// texture's reflectivity (not readable here).
fn puddle_ng(p: vec2<f32>, level: f32) -> f32 {
    return wetness_puddle(p, level);
}

fn spot_ng(ii: u32) -> Spot {
    var sp: Spot;
    sp.ok = false;
    let h0 = hash3(vec3<u32>(ii, 5u, 13u));
    let cycle = f.eye.w / NG_SPLASH_LIFE + h0.x;
    let slot = u32(floor(cycle));
    sp.age = fract(cycle);
    let h = hash3(vec3<u32>(ii, slot, 29u));
    sp.seed = hash3(vec3<u32>(ii, slot, 31u));
    let c = f.ng_view.xy;
    let q = h.xy * NG_SPLASH_BOX - f.eye.xy - c;
    let xy = (fract(q / NG_SPLASH_BOX) - 0.5) * NG_SPLASH_BOX + c;
    let dims = vec2<f32>(textureDimensions(depth_tex));
    var z = f.ng_view.z;
    var s = vec4<f32>(0.0);
    var px = vec2<f32>(0.0);
    for (var k = 0; k < 3; k = k + 1) {
        let clip = f.view_proj * vec4<f32>(xy, z, 1.0);
        if clip.w < 1.0 {
            return sp;
        }
        let ndc = clip.xy / clip.w;
        if any(abs(ndc) > vec2<f32>(0.995)) {
            return sp;
        }
        px = vec2<f32>((ndc.x * 0.5 + 0.5) * dims.x, (0.5 - ndc.y * 0.5) * dims.y);
        s = rel_at(px);
        if linear_depth(s.w) > 0.9 * f.depth.y {
            return sp;
        }
        z = s.z;
    }
    let dist = length(s.xyz);
    if dist < 1.2 || dist > 38.0 || length(s.xy - xy) > 0.04 + 0.004 * dist {
        return sp;
    }
    // The surface's normal from its neighbours (2 px), a plane across.
    let cx = rel_at(px + vec2<f32>(2.0, 0.0)).xyz;
    let cy = rel_at(px + vec2<f32>(0.0, 2.0)).xyz;
    let cxm = rel_at(px - vec2<f32>(2.0, 0.0)).xyz;
    let cym = rel_at(px - vec2<f32>(0.0, 2.0)).xyz;
    let n = normalize(cross(cx - cxm, cy - cym));
    if abs(n.z) < 0.7 {
        return sp;
    }
    let ex = length((cx - s.xyz) + (cxm - s.xyz));
    let ey = length((cy - s.xyz) + (cym - s.xyz));
    let span = length(cx - cxm) + length(cy - cym) + 0.01;
    if ex + ey > 0.2 * span {
        return sp;
    }
    if f.ng_air.w > 0.5 || covered_ng(s.xyz + vec3<f32>(0.0, 0.0, 0.05)) {
        return sp;
    }
    sp.ok = true;
    sp.rel = s.xyz;
    sp.n = select(-n, n, n.z > 0.0);
    // Above the ground under the eye (an 8 % grade allowed).
    let raised = s.z - f.ng_view.z > 0.3 + 0.08 * length(s.xy);
    sp.puddle = select(puddle_ng(s.xy + f.eye.xy, f.ng_air.x), 0.0, raised);
    sp.surface = select(select(0u, 1u, sp.puddle > 0.35), 2u, raised);
    return sp;
}

@vertex
fn vs_splash_ng(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    var o: Out;
    o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    let sp = spot_ng(ii);
    if !sp.ok {
        return o;
    }
    let dist = length(sp.rel);
    // A crown 2.5–4 cm across on a film (a 2–3 mm drop's), 5–8 cm in a
    // puddle (with a jet), 2–3 cm on a car or a rail (mostly droplets); a
    // couple of pixels at least (a fleck in the distance).
    var size = 0.025 + 0.015 * sp.seed.x;
    if sp.surface == 1u {
        size = 0.05 + 0.03 * sp.seed.x;
    } else if sp.surface == 2u {
        size = 0.02 + 0.01 * sp.seed.x;
    }
    let ppm = f.ng_view.w * f32(textureDimensions(depth_tex).y) * 0.5 / dist;
    size = max(size, 2.5 / ppm);
    let right = normalize(cross(sp.rel / dist, vec3<f32>(0.0, 0.0, 1.0)));
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let rel = sp.rel + right * (corner.x * 2.0 - 1.0) * size + vec3<f32>(0.0, 0.0, corner.y * size * 1.2 - 0.01);
    o.clip = f.view_proj * vec4<f32>(rel, 1.0);
    o.uv = corner;
    let lit = light_ng(sp.rel + vec3<f32>(0.0, 0.0, 0.05)) * 1.4 + f.ambient.xyz * 0.15;
    let alpha = 0.85 * (1.0 - smoothstep(26.0, 38.0, dist));
    o.colour = vec4<f32>(lit * alpha, alpha * 0.3);
    o.misc = vec4<f32>(view_depth(sp.rel) - 0.1, 0.05, 5.0 + f32(sp.surface) * 0.1, sp.age);
    return o;
}

// Rain rings on puddles: a flat ring widening on the surface (the
// impact's capillary wave) where Wet Roads' puddle field holds water,
// within 14 m; none on the film or raised surfaces. Lit as a glint of the lamps on the water.
@vertex
fn vs_ring_ng(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
    var o: Out;
    o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    let sp = spot_ng(ii);
    let wet = sp.puddle;
    if !sp.ok || sp.surface == 2u || wet < 0.08 || sp.puddle < 0.15 {
        return o;
    }
    let dist = length(sp.rel);
    if dist > 14.0 {
        return o;
    }
    let size = 0.07 + 0.11 * sp.puddle;
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u)) * 2.0 - 1.0;
    let rel = sp.rel + vec3<f32>(corner * size, 0.012);
    o.clip = f.view_proj * vec4<f32>(rel, 1.0);
    o.uv = corner;
    let lit = light_ng(sp.rel + vec3<f32>(0.0, 0.0, 0.3)) * 0.9 + f.ambient.xyz * 0.2;
    // (colour, the raw opacity: the fragment makes the crest light and the
    // trough a shade)
    let alpha = 0.25 * wet * (1.0 - smoothstep(8.0, 14.0, dist));
    o.colour = vec4<f32>(lit, alpha);
    o.misc = vec4<f32>(view_depth(sp.rel) - 0.05, 0.03, 6.0, sp.age);
    return o;
}

@fragment
fn fs_ng(in: Out) -> @location(0) vec4<f32> {
    let kind = u32(in.misc.z + 0.05);
    let age = in.misc.w;
    var shape = 0.0;
    if kind == 5u {
        // Crown: the sheet's thin walls rising and spreading, its rim
        // breaking into droplets thrown out on arcs, the base ring; in a
        // puddle a central jet (Worthington) after the crown falls.
        let surface = u32(round(fract(in.misc.z) * 10.0));
        let p = vec2<f32>(in.uv.x * 2.0 - 1.0, in.uv.y * 1.2);
        let t = age;
        let r = 0.22 + 0.55 * sqrt(t);
        let hc = 0.5 * sin(3.14159 * min(t * 1.8, 1.0)) * select(1.0, 0.4, surface == 2u);
        // The impact's flash on the surface, the thin crown sheet (faint,
        // only early) and its rim breaking into droplets thrown out on
        // arcs, a few in front of the crown, a few behind (dimmer).
        let flash = exp(-(p.x * p.x / 0.03 + (p.y - 0.03) * (p.y - 0.03) / 0.002)) * (1.0 - smoothstep(0.0, 0.35, t));
        let wall = exp(-pow((abs(p.x) - r * (0.8 + 0.2 * p.y / max(hc, 0.05))) / 0.035, 2.0)) * step(p.y, hc) * (1.0 - smoothstep(0.1, 0.45, t)) * 0.35;
        var v = flash * 0.9 + wall;
        for (var k = 0; k < 8; k = k + 1) {
            let a = (f32(k) + 0.5) * 0.785 + age * 0.0;
            let tt = min(t * 1.3, 1.0);
            let front = cos(a);
            let q = vec2<f32>(sin(a) * (r + 0.5 * tt), hc * (0.9 + 0.1 * front) + 1.3 * tt * (1.0 - tt) * select(1.0, 0.5, surface == 2u) - 0.02 * front);
            let d = p - q;
            v += exp(-dot(d, d) / 0.0016) * (1.0 - tt * 0.7) * (0.65 + 0.35 * front);
        }
        if surface == 1u && t > 0.35 {
            let tj = (t - 0.35) / 0.65;
            let top = 0.45 * sin(3.14159 * tj);
            let jet = exp(-pow(p.x / 0.07, 2.0)) * smoothstep(top, top - 0.08, p.y) * (1.0 - tj);
            v += jet * 0.6;
        }
        shape = min(v, 1.0);
    } else {
        // Ring: a capillary wave widening and fading, its crest catching
        // the light, the trough just behind it a shade.
        let r = length(in.uv);
        if r >= 1.0 {
            discard;
        }
        let rr = 0.12 + 0.8 * sqrt(age);
        let fade = (1.0 - age) * (1.0 - age) * smoothstep(1.0, 0.8, r);
        let crest = exp(-pow((r - rr) / 0.05, 2.0)) * fade;
        let trough = exp(-pow((r - rr + 0.1) / 0.05, 2.0)) * fade;
        let a = in.colour.a * soft(in.clip, in.misc.x, in.misc.y);
        let o = vec4<f32>(in.colour.rgb * crest * a, (crest * 0.2 + trough * 0.3) * a);
        if max(o.a, o.g) <= 0.002 {
            discard;
        }
        return o;
    }
    let a = shape * soft(in.clip, in.misc.x, in.misc.y);
    if a <= 0.002 {
        discard;
    }
    return in.colour * a;
}


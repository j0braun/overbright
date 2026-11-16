// --- Rain haze: the wet air's in-scatter and the distance mist --------
// Per pixel along the view ray to the scene's depth (150 m at most): each
// lamp's light scattered by the rain, integrated in closed form over the
// part of the ray inside its reach (the (1 − d²/r²)² falloff is a
// polynomial along the ray), in two halves (before and after the closest
// point) each with its own HG phase: so a lamp seen through rain gets a
// halo, brightest looking into it, and the air under it glows. The same
// for the cars' headlight cones (their cone at each half). Then a thin
// extinction toward the fog colour (rain mist), 400 m on the sky.
@vertex
fn vs_haze(@builtin(vertex_index) vi: u32) -> Out {
    var o: Out;
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    o.clip = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    o.uv = uv;
    o.colour = vec4<f32>(0.0);
    o.misc = vec4<f32>(0.0, 0.0, 7.0, 0.0);
    return o;
}

// ∫ (A − s²/r²)² ds from s0 to s1 (A = 1 − h²/r²).
fn fall_integral(a: f32, r2: f32, s0: f32, s1: f32) -> f32 {
    let f1 = a * a * s1 - 2.0 * a * s1 * s1 * s1 / (3.0 * r2) + s1 * s1 * s1 * s1 * s1 / (5.0 * r2 * r2);
    let f0 = a * a * s0 - 2.0 * a * s0 * s0 * s0 / (3.0 * r2) + s0 * s0 * s0 * s0 * s0 / (5.0 * r2 * r2);
    return max(f1 - f0, 0.0);
}

// The in-scatter of a light at `l` (rel) of reach `r` along the ray
// (`dir`, length `len`), as (before, after) the closest point, with the
// light's direction toward the two halves' centres for the phase.
fn scatter_light(l: vec3<f32>, r: f32, dir: vec3<f32>, len: f32, g: f32) -> vec4<f32> {
    let b = dot(l, dir);
    let h2 = max(dot(l, l) - b * b, 0.0);
    let r2 = r * r;
    if h2 >= r2 {
        return vec4<f32>(0.0);
    }
    let a = 1.0 - h2 / r2;
    let half = sqrt(a) * r;
    // s along the ray relative to the closest point.
    let s0 = max(-half, -b);
    let s1 = min(half, len - b);
    if s1 <= s0 {
        return vec4<f32>(0.0);
    }
    let m = min(max(0.0, s0), s1);
    let i0 = fall_integral(a, r2, s0, m);
    let i1 = fall_integral(a, r2, m, s1);
    let c0 = dir * (b + 0.5 * (s0 + m)) - l;
    let c1 = dir * (b + 0.5 * (m + s1)) - l;
    // Scattered back toward the eye (−dir).
    let p0 = hg4(g, -dot(normalize(c0 + dir * 1e-3), dir));
    let p1 = hg4(g, -dot(normalize(c1 + dir * 1e-3), dir));
    return vec4<f32>(i0 * p0 + i1 * p1, b + 0.5 * (s0 + m), b + 0.5 * (m + s1), i0 + i1);
}

// The length of the view ray through the scene pixel `px` (400 m on the
// sky) and its direction.
fn view_ray(px: vec2<f32>) -> vec4<f32> {
    let d = scene_depth_px(px);
    let dims = vec2<f32>(textureDimensions(depth_tex));
    let ndc = vec2<f32>(px.x / dims.x * 2.0 - 1.0, 1.0 - px.y / dims.y * 2.0);
    let far_d = select(1.0, 0.0, f.depth.z > 0.5);
    let pf = f.inv_view_proj * vec4<f32>(ndc, far_d * 0.5 + 0.25, 1.0);
    let dir = normalize(pf.xyz / pf.w);
    let sky = linear_depth(d) > 0.9 * f.depth.y;
    let p = f.inv_view_proj * vec4<f32>(ndc, d, 1.0);
    return vec4<f32>(dir, select(length(p.xyz / p.w), 400.0, sky));
}

// The lamps' and headlights' in-scatter along the ray through `px` (to
// 150 m at most), attenuated by the mist halfway.
fn haze_scatter(px: vec2<f32>) -> vec3<f32> {
    let ray = view_ray(px);
    let dir = ray.xyz;
    let len = ray.w;
    let reach = min(len, 150.0);
    var c = vec3<f32>(0.0);
    let nl = u32(f.depth.w);
    for (var i = 0u; i < nl; i++) {
        let l = f.lamps[i * 2u];
        let s = scatter_light(l.xyz, l.w * abs(f.ng_haze.w), dir, reach, 0.75);
        c += f.lamps[i * 2u + 1u].xyz * s.x;
    }
    // A negative reach scale: the cars' cones are the car lights' air (rain_fx.rs, one volume).
    let ns = select(u32(f.wind.z), 0u, f.ng_haze.w < 0.0);
    for (var i = 0u; i < ns; i++) {
        let a = f.spots[i * 4u];
        let b = f.spots[i * 4u + 1u];
        let col = f.spots[i * 4u + 2u];
        let s = scatter_light(a.xyz, a.w, dir, reach, 0.7);
        if s.w > 0.0 {
            // The cone at the two halves' centres.
            let q0 = dir * s.y - a.xyz;
            let q1 = dir * s.z - a.xyz;
            let k = 0.5 * (smoothstep(b.w, col.w, dot(normalize(q0 + b.xyz * 1e-3), b.xyz)) + smoothstep(b.w, col.w, dot(normalize(q1 + b.xyz * 1e-3), b.xyz)));
            c += col.xyz * s.x * k * 0.6;
        }
    }
    let t = exp(-f.ng_air.z * min(len, 400.0));
    let sky = select(1.0, f.ng_haze.z, len >= 400.0);
    return c * f.ng_air.y * (0.5 + 0.5 * t) * sky;
}

// The in-scatter is smooth on the screen (a glow): it is worked out at a
// quarter of the resolution each way (each texel at the centre of its
// 4 × 4 pixels) and filtered up; the mist (the extinction, sharp at silhouettes) is worked
// out per pixel in the upsample.
@fragment
fn fs_haze_half(in: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(haze_scatter(floor(in.clip.xy) * 4.0 + 2.0), 0.0);
}

@group(1) @binding(0) var haze_tex: texture_2d<f32>;
@group(1) @binding(1) var haze_samp: sampler;

@fragment
fn fs_haze_up(in: Out) -> @location(0) vec4<f32> {
    let p = in.clip.xy;
    let hd = vec2<f32>(textureDimensions(haze_tex));
    // Texel i's centre is full pixel 4i + 2.
    let uv = p / (hd * 4.0);
    let scatter = textureSampleLevel(haze_tex, haze_samp, uv, 0.0).rgb;
    // The mist by the view depth (within 15 % of the ray's length over the
    // view; cheaper than the ray per pixel).
    let lin = linear_depth(scene_depth_px(p));
    let len = select(lin, 400.0, lin > 0.9 * f.depth.y);
    // Look rule "no double fog" (docs/look-direction.md): only `veil` of the mist's extinction is
    // applied again (the native fog owns it), and the mist's own light is a
    // small share of the fog colour.
    let alpha = select(f.ng_haze.x, f.ng_haze.x * f.ng_haze.z, len >= 400.0) * (1.0 - exp(-f.ng_air.z * min(len, 400.0)));
    let out = scatter + f.ambient.xyz * f.ng_haze.y * alpha;
    if max(max(out.r, out.g), max(out.b, alpha)) <= 0.0005 {
        discard;
    }
    return vec4<f32>(out, alpha);
}

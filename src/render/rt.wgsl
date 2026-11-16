
// ---- Ray-traced reflections (rt.rs, docs/rt-reflections.md) ----
// Appended to ssr.wgsl for the trace pass of the "Ray Traced
// (experimental)" tier only; the SSR modules never hold it.

struct RtParams {
    eye: vec4<f32>,   // the eye in the world; w the reach (m)
    misc: vec4<f32>,  // x: instances, y: textured hits, z: layers, w: the glow debug view
    cars: vec4<f32>,  // x: the cars' glows
    // The cars' glows' records (rt/geometry.rs `Glow::surface`, three each):
    // in the uniform, so every lane reads the same words (scalar loads).
    glows: array<vec4<u32>, 48>,
}

@group(1) @binding(0) var rt_tlas: acceleration_structure;
// One colour per triangle (RGB8 of the colour / 2), by the instance's
// first triangle + the primitive.
@group(1) @binding(1) var<storage, read> rt_colors: array<u32>;
@group(1) @binding(2) var<uniform> rt: RtParams;
// The textured hit (world triangles, rt.misc.y > 0.5): per triangle three
// vec4<u32> — [u0 v0 u1 v1] [u2 v2 c0 c1] [c2 layer lod 0] (UVs and the
// LOD term as f32 bits, c the baked vertex colours RGBA8) — and the hit
// texture array (every world texture box-filtered to one size with mips).
@group(1) @binding(3) var<storage, read> rt_surfaces: array<vec4<u32>>;
@group(1) @binding(4) var rt_tex: texture_2d_array<f32>;
@group(1) @binding(5) var rt_samp: sampler;

// How far a ray that meets nothing looks for glows (m; the sprites fade out by
// 200 m, docs/formats.md "Light glow sprites").
const GLOW_REACH: f32 = 120.0;
// A streaked glow on a wet road at this share of the sprite's light: the
// streak spreads it over ten times the area; at 1 the night-rain frames'
// p95 luma rose by 4 levels (the look rule allows 3).
const GLOW_ROAD: f32 = 0.6;

// The light the neon and lit signs and the light glow sprites (instance
// mask 4, not opaque; rt/geometry.rs `emissive_geometry`, `Glow`) add along
// a ray from o along d up to tmax: every triangle it passes, as the world
// draws them additively (texture × alpha × the vertex colour × 2), and every
// glow whose octahedron it enters once (the sprite's soft disc at the ray's
// nearest point to its centre, sized by the reflected path `ed` + t as the
// sprite is by depth), summed (additive blending does not depend on the
// order). The query commits nothing: it visits them all. `from_car`: the
// ray leaves a car (the glows within 1.5 m are its own lights).
fn rt_glow(o: vec3<f32>, d: vec3<f32>, tmin: f32, tmax: f32, ed: f32, from_car: bool) -> vec3<f32> {
    var gq: ray_query;
    // GLOW BODY (rt/trace.rs inlines it on the list's one query)
    rayQueryInitialize(&gq, rt_tlas, RayDesc(RAY_FLAG_NONE, 0x04u, tmin, tmax, o, d));
    var sum = vec3<f32>(0.0);
    // The last glows taken (an octahedron is entered and left: two candidates).
    var seen = vec4<u32>(0xFFFFFFFFu);
    while rayQueryProceed(&gq) {
        let h = rayQueryGetCandidateIntersection(&gq);
        let tri = h.instance_custom_data + h.primitive_index;
        let a = rt_surfaces[tri * 3u];
        let b = rt_surfaces[tri * 3u + 1u];
        let c = rt_surfaces[tri * 3u + 2u];
        if c.y == 0xFFFFFFFEu {
            // (A glow's eight triangles share its record: its centre names it.)
            let id = a.x ^ (a.y * 0x9E3779B1u) ^ (a.z * 0x85EBCA77u);
            if any(seen == vec4<u32>(id)) {
                continue;
            }
            seen = vec4<u32>(id, seen.xyz);
            sum += rt_sprite(a, b, c, o, d, tmin, tmax, ed, from_car);
            continue;
        }
        if f32(c.y) >= rt.misc.z {
            continue;
        }
        let w = vec3<f32>(1.0 - h.barycentrics.x - h.barycentrics.y, h.barycentrics.x, h.barycentrics.y);
        let uv = bitcast<vec2<f32>>(a.xy) * w.x + bitcast<vec2<f32>>(a.zw) * w.y + bitcast<vec2<f32>>(b.xy) * w.z;
        let v = unpack4x8unorm(b.z).rgb * w.x + unpack4x8unorm(b.w).rgb * w.y + unpack4x8unorm(c.x).rgb * w.z;
        let t = textureSampleLevel(rt_tex, rt_samp, uv, i32(c.y), 1.0);
        sum += t.rgb * t.a * v * 2.0;
    }
    // GLOW END
    return sum;
}

// One glow sprite's light on the ray from o along d (its record a, b, c,
// rt/geometry.rs `Glow::surface`): the sprite's soft disc at the ray's
// nearest point to its centre, its radius as the sprite's at the reflected
// path's length `ed` + t, the facing term, the streak buffer's steady state
// where the glows go through it (as the march sees a glow on screen,
// ssr.wgsl GLOW_STEADY). Nothing behind the hit (tmax) or, for a ray
// leaving a car (`from_car`), within 1.5 m (its own lights).
fn rt_sprite(a: vec4<u32>, b: vec4<u32>, c: vec4<u32>, o: vec3<f32>, d: vec3<f32>, tmin: f32, tmax: f32, ed: f32, from_car: bool) -> vec3<f32> {
    let centre = bitcast<vec3<f32>>(a.xyz);
    let tc = dot(centre - o, d);
    if tc < tmin || tc > tmax + 1.0 || (from_car && tc < 1.5) {
        return vec3<f32>(0.0);
    }
    let radius = bitcast<f32>(a.w) * clamp((ed + tc) / (256.0 * p.proj.y), bitcast<f32>(b.x), bitcast<f32>(b.y));
    var off = o + d * tc - centre;
    var gain = 1.0;
    if !from_car {
        gain = GLOW_ROAD;
        // A wet road streaks a lamp's reflection along the vertical as the
        // native mirror sprite does (0x490920: its length × 1.2 … 10 by the
        // wetness, glow/reflection.rs `streak_factor`).
        off.z = off.z / (1.2 + 8.8 * clamp(p.fog_color.w, 0.0, 1.0));
    }
    let q = max(1.0 - dot(off, off) / max(radius * radius, 1e-6), 0.0);
    if q <= 0.0 {
        return vec3<f32>(0.0);
    }
    var facing = 1.0;
    let e = bitcast<f32>(c.x);
    if e > 0.0 {
        let dir = unpack4x8snorm(b.w).xyz;
        facing = pow(max(dot(dir, -d), 0.0), e);
    }
    let steady = select(vec3<f32>(1.0), GLOW_STEADY, p.up.w > 0.5);
    return unpack4x8unorm(b.z).rgb * steady * (q * q * facing * gain);
}

// The cars' head and brake light glows (the nearest cars', rt.cars.x of
// them, rt.glows): tested one by one, no acceleration structure (they move
// every frame and are few).
fn rt_car_glows(o: vec3<f32>, d: vec3<f32>, tmin: f32, tmax: f32, ed: f32, from_car: bool) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < u32(rt.cars.x); i = i + 1u) {
        sum += rt_sprite(rt.glows[i * 3u], rt.glows[i * 3u + 1u], rt.glows[i * 3u + 2u], o, d, tmin, tmax, ed, from_car);
    }
    return sum;
}

// A world hit's colour as world.efx draws it (tex × v0 × 2): the texture at
// the hit's UV, the baked colour interpolated, the mip by the ray cone's
// width `width` (m) at the hit (`lod`: log2 texels per metre, rt/geometry.rs
// `lod_term`). `flat`: the triangle's colour where it has no layer.
fn rt_surface(tri: u32, bary: vec2<f32>, width: f32, flat: vec3<f32>) -> vec3<f32> {
    let a = rt_surfaces[tri * 3u];
    let b = rt_surfaces[tri * 3u + 1u];
    let c = rt_surfaces[tri * 3u + 2u];
    let layer = c.y;
    if f32(layer) >= rt.misc.z {
        return flat;
    }
    let w = vec3<f32>(1.0 - bary.x - bary.y, bary.x, bary.y);
    let uv = bitcast<vec2<f32>>(a.xy) * w.x + bitcast<vec2<f32>>(a.zw) * w.y + bitcast<vec2<f32>>(b.xy) * w.z;
    let v = unpack4x8unorm(b.z).rgb * w.x + unpack4x8unorm(b.w).rgb * w.y + unpack4x8unorm(c.x).rgb * w.z;
    let lod = bitcast<f32>(c.z) + log2(max(width, 1e-4));
    let t = textureSampleLevel(rt_tex, rt_samp, uv, i32(layer), max(lod, 0.0)).rgb;
    return t * v * 2.0;
}

// The screen march's result `c` (rgb, confidence) completed by a ray
// query where it is unsure: from the view-space `origin` along the
// view-space reflection `r` (surface normal `n`, view space); `car`: the
// ray leaves a car; `hit_car`: the march's hit is a car. The hit's colour
// is its triangle's baked colour (world.efx tex × v0 × 2), fogged over the
// path for a car's ray (as the march's), held to the planar term's
// luminance on a dry road (docs/chrome-flicker.md) unless it is a car.
fn rt_cast(o: vec3<f32>, d: vec3<f32>, tmin: f32, tmax: f32, mask: u32) -> RayIntersection {
    var rq: ray_query;
    rayQueryInitialize(&rq, rt_tlas, RayDesc(RAY_FLAG_FORCE_OPAQUE, mask, tmin, tmax, o, d));
    while rayQueryProceed(&rq) {}
    return rayQueryGetCommittedIntersection(&rq);
}

fn rt_fill(c: vec4<f32>, origin: vec3<f32>, r: vec3<f32>, n: vec3<f32>, car: bool, term: vec4<f32>, hit_car: bool) -> vec4<f32> {
    if c.a >= 0.98 {
        return c;
    }
    let o = rt.eye.xyz + p.right.xyz * origin.x + p.up.xyz * origin.y + p.forward.xyz * origin.z;
    let nw = p.right.xyz * n.x + p.up.xyz * n.y + p.forward.xyz * n.z;
    let d = normalize(p.right.xyz * r.x + p.up.xyz * r.y + p.forward.xyz * r.z);
    let start = o + nw * 0.05;
    var h = rt_cast(start, d, select(0.25, 0.12, car), rt.eye.w, 0x03u);
    var path = h.t;
    if car && h.kind != RAY_QUERY_INTERSECTION_NONE && h.instance_index > 0u && h.t < 1.5 {
        // From a car, a car within 1.5 m is (almost always) its own traced
        // model: a coarser LOD than the one drawn, so the ray leaves the
        // drawn body inside it. On through it, the world only.
        h = rt_cast(start + d * h.t, d, 0.01, rt.eye.w - h.t, 0x01u);
        path += h.t;
    }
    // The neon, signs and light glows the ray passes on its way to the hit
    // (or to its reach: a lamp against the sky).
    let missed = h.kind == RAY_QUERY_INTERSECTION_NONE;
    var glow = vec3<f32>(0.0);
    // Not from a dry road: its reflection is held to the native term's
    // luminance below, which the glow could not pass.
    if rt.misc.y > 0.5 && (car || p.fog_color.w >= 0.5) {
        let glow_reach = select(path, min(rt.eye.w, GLOW_REACH), missed);
        glow = rt_glow(start, d, select(0.25, 0.12, car), glow_reach, length(origin), car);
        glow += rt_car_glows(start, d, select(0.25, 0.12, car), glow_reach, length(origin), car);
    }
    if rt.misc.w > 0.5 {
        // NFSU2_RT_DEBUG=glow (a check aid): the glow alone, strong.
        return vec4<f32>(glow * 20.0, 1.0);
    }
    if missed {
        if max(glow.r, max(glow.g, glow.b)) <= 0.0 {
            return c;
        }
        // The reflection's own term (the planar capture or the cube) with the glow on it.
        var sky = term.rgb + glow;
        if !car && p.fog_color.w < 0.5 {
            let lp = dot(term.rgb, vec3<f32>(0.299, 0.587, 0.114));
            sky = sky * min(1.0, lp / max(dot(sky, vec3<f32>(0.299, 0.587, 0.114)), 1e-5));
        }
        return vec4<f32>(c.rgb * c.a + sky * (1.0 - c.a), 1.0);
    }
    let packed = rt_colors[h.instance_custom_data + h.primitive_index];
    var col = unpack4x8unorm(packed).rgb * 2.0;
    let on_car = h.instance_index > 0u;
    if !on_car && rt.misc.y > 0.5 {
        // The ray cone (a mirror): one trace pixel's angle, spread over the
        // eye's distance to the surface and the reflected path.
        let angle = 2.0 / (p.proj.y * p.size.w);
        col = rt_surface(h.primitive_index, h.barycentrics, (length(origin) + path) * angle, col);
    }
    col += glow;
    if car && p.fog.w > 0.5 {
        let f = fog_at(path);
        col = col * f + p.fog_color.rgb * (1.0 - f);
    }
    if !car && !on_car && p.fog_color.w < 0.5 {
        let lp = dot(term.rgb, vec3<f32>(0.299, 0.587, 0.114));
        let lc = dot(col, vec3<f32>(0.299, 0.587, 0.114));
        col = col * min(1.0, lp / max(lc, 1e-5));
    }
    let conf = 1.0 - smoothstep(0.85, 1.0, path / rt.eye.w);
    let a = c.a + (1.0 - c.a) * conf;
    return vec4<f32>((c.rgb * c.a + col * (1.0 - c.a) * conf) / max(a, 1e-4), a);
}

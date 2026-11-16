
// ---- Ray-traced reflections (rt.rs, docs/rt-reflections.md) ----
// Appended to ssr.wgsl for the trace pass of the "Ray Traced
// (experimental)" tier only; the SSR modules never hold it.

struct RtParams {
    eye: vec4<f32>,   // the eye in the world; w the reach (m)
    misc: vec4<f32>,  // x: instances
}

@group(1) @binding(0) var rt_tlas: acceleration_structure;
// One colour per triangle (RGB8 of the colour / 2), by the instance's
// first triangle + the primitive.
@group(1) @binding(1) var<storage, read> rt_colors: array<u32>;
@group(1) @binding(2) var<uniform> rt: RtParams;

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
    var h = rt_cast(start, d, select(0.25, 0.12, car), rt.eye.w, 0xFFu);
    var path = h.t;
    if car && h.kind != RAY_QUERY_INTERSECTION_NONE && h.instance_index > 0u && h.t < 1.5 {
        // From a car, a car within 1.5 m is (almost always) its own traced
        // model: a coarser LOD than the one drawn, so the ray leaves the
        // drawn body inside it. On through it, the world only.
        h = rt_cast(start + d * h.t, d, 0.01, rt.eye.w - h.t, 0x01u);
        path += h.t;
    }
    if h.kind == RAY_QUERY_INTERSECTION_NONE {
        return c;
    }
    let packed = rt_colors[h.instance_custom_data + h.primitive_index];
    var col = unpack4x8unorm(packed).rgb * 2.0;
    let on_car = h.instance_index > 0u;
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

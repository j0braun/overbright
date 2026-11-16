// Modern + RT: the frame's traced scene (render/rt.rs: the world's low LOD
// and the cars; instance mask 1 the world, 2 the cars). For speed, traced by the
// half-resolution pass before the colour pass (rt_shadow.wgsl, after the
// depth pre-pass); kept bound here.
@group(0) @binding(11) var ml_tlas: acceleration_structure;
// That pass's blocks (2 × 2 pixels each): x, y (light + 1) << 16 |
// visibility, z the block's view depth (bits), w its face normal
// (octahedral 2 × 16 bits).
@group(0) @binding(15) var ml_vis_blocks: texture_2d<u32>;
// The NIS characters' pipelines (`LF_ML_CHAR`) trace their rays here
// in the colour shader (they are not in the depth pre-pass); every other world draw
// reads the blocks (the ray code is folded out of its pipelines).
override D9_ML_CHAR: bool = false;
// BEGIN RTVIS C
// The fraction of a light's disc (centre l, radius `radius`, facing p)
// that nothing of the scene hides from p (face normal ng); `mask` 1 for a
// car's own light (the world only). Soft shadows (no harsh, hard
// edges): an area light, not a point — `samples` rays per pixel (block) to
// points of a fixed Vogel disc set; the 2×2 pixels (blocks) take interleaved
// quarters of it (samples × 4 points, `g_ml_pix`) and each light its own
// turn of the set (`key`); nothing depends on time: a still view is still.
fn ml_ray(p: vec3<f32>, ng: vec3<f32>, l: vec3<f32>, radius: f32, mask: u32, key: u32, samples: u32) -> f32 {
    // Off the surface by more where the ray grazes it (the traced world is
    // the low LOD: its faces sit a little off the drawn ones).
    let d0 = normalize(l - p);
    let o = p + ng * (0.08 + 0.3 * (1.0 - max(dot(ng, d0), 0.0)));
    if length(l - o) < 1.3 { return 1.0; }
    // Walls start farther out (the low LOD's facades lie up to ~0.5 m
    // off the drawn curved ones).
    let tmin = mix(0.6, 0.15, smoothstep(0.3, 0.7, ng.z));
    // The disc's axes (across the direction to its centre).
    let t = normalize(cross(d0, select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(d0.z) > 0.9)));
    let b = cross(d0, t);
    let block = vec2<u32>(g_ml_pix) & vec2<u32>(1u);
    let quarter = block.x + 2u * block.y;
    let turn = f32(key & 1023u) * ML_GOLDEN;
    // Stop short of the light's own body (a lamp's head, a sign's card).
    let stop = 0.6 + radius;
    var vis = 0.0;
    for (var j = 0u; j < samples; j++) {
        let m = quarter + 4u * j;
        let r = sqrt((f32(m) + 0.5) / f32(4u * samples)) * radius;
        let a = f32(m) * ML_GOLDEN + turn;
        let to = l + (t * cos(a) + b * sin(a)) * r - o;
        let len = length(to);
        vis += ml_ray_one(o, to / len, tmin, max(len - stop, tmin + 0.01), mask);
    }
    return vis / f32(samples);
}
// One ray: 1 when it reaches tmax.
fn ml_ray_one(o: vec3<f32>, d: vec3<f32>, tmin: f32, tmax: f32, mask: u32) -> f32 {
    var rq: ray_query;
    rayQueryInitialize(&rq, ml_tlas, RayDesc(RAY_FLAG_FORCE_OPAQUE | RAY_FLAG_TERMINATE_ON_FIRST_HIT, mask, tmin, tmax, o, d));
    while rayQueryProceed(&rq) {}
    let h = rayQueryGetCommittedIntersection(&rq);
    return select(1.0, 0.0, h.kind != RAY_QUERY_INTERSECTION_NONE);
}

// END RTVIS C
// The blocks around this pixel (`ml_vis_taps`): the lights their centre
// block and the next one diagonally toward the pixel traced (+ 1; 0 none),
// and per light the weighted visibility and weight over the 3 × 3 blocks;
// the centre block (its depth).
var<private> g_vis_ids: vec4<u32>;
var<private> g_vis_sum: vec4<f32>;
var<private> g_vis_wt: vec4<f32>;
var<private> g_vis_c: vec4<u32>;
var<private> g_vis_cb: vec2<i32>;
// The octahedral encoding of a unit normal (as rt_shadow.wgsl stores it).
fn ml_vis_oct(n: vec3<f32>) -> vec2<f32> {
    var e = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    if n.z < 0.0 {
        e = (1.0 - abs(e.yx)) * select(vec2<f32>(-1.0), vec2<f32>(1.0), e >= vec2<f32>(0.0));
    }
    return e;
}
// One block `q` of the pixel's (filter weight `fw`) into the sums: on the
// pixel's plane (`want` its view depth there, `tol`) and facing as the
// pixel (`e` its normal's octahedral code; ~0.15 apart is ~15°).
fn ml_vis_add(q: vec2<i32>, dims: vec2<i32>, fw: f32, want: f32, tol: f32, e: vec2<f32>) {
    let t = textureLoad(ml_vis_blocks, clamp(q, vec2<i32>(0), dims - 1), 0);
    let de = unpack2x16snorm(t.w) - e;
    let wd = 1.0 - smoothstep(tol, 2.0 * tol, abs(bitcast<f32>(t.z) - want));
    let wn = 1.0 - smoothstep(0.1, 0.2, abs(de.x) + abs(de.y));
    let inside = all(q >= vec2<i32>(0)) && all(q < dims);
    let w = select(0.0, fw * wd * wn, inside);
    let mx = select(vec4<f32>(0.0), vec4<f32>(w), g_vis_ids == vec4<u32>(t.x >> 16u));
    let my = select(vec4<f32>(0.0), vec4<f32>(w), g_vis_ids == vec4<u32>(t.y >> 16u));
    g_vis_sum += mx * (f32(t.x & 0xFFFFu) / 65535.0) + my * (f32(t.y & 0xFFFFu) / 65535.0);
    g_vis_wt += mx + my;
}
// The pixel's blocks: per axis the three around it weighted
// (1 − f) / 2, 1 / 2, f / 2 (f its position between block centres): as
// much weight on odd as on even blocks wherever the pixel is, so every
// pixel takes all four quarters of the blocks' disc sets in equal parts,
// and the weights move smoothly from pixel to pixel (no 2-pixel steps; a
// bilinear blend of 2 × 2 block means). Each block counts only on this
// pixel's plane (view depth z, its screen slopes dz) and facing as its
// face normal ng.
fn ml_vis_taps(pix: vec2<f32>, z: f32, dz: vec2<f32>, ng: vec3<f32>) {
    let dims = vec2<i32>(textureDimensions(ml_vis_blocks));
    let u = (pix - 0.5) * 0.5 + 0.5;
    let k = floor(u);
    let f = u - k;
    let ax = vec3<f32>(0.5 - 0.5 * f.x, 0.5, 0.5 * f.x);
    let ay = vec3<f32>(0.5 - 0.5 * f.y, 0.5, 0.5 * f.y);
    let c = vec2<i32>(k);
    g_vis_cb = c;
    g_vis_c = textureLoad(ml_vis_blocks, clamp(c, vec2<i32>(0), dims - 1), 0);
    let side = select(vec2<i32>(-1), vec2<i32>(1), f >= vec2<f32>(0.5));
    let d = textureLoad(ml_vis_blocks, clamp(c + side, vec2<i32>(0), dims - 1), 0);
    g_vis_ids = vec4<u32>(g_vis_c.x >> 16u, g_vis_c.y >> 16u, d.x >> 16u, d.y >> 16u);
    g_vis_sum = vec4<f32>(0.0);
    g_vis_wt = vec4<f32>(0.0);
    let tol = 0.004 * z + 0.02;
    let e = ml_vis_oct(ng);
    // The view depth at block (c + o) on this pixel's plane: z0 + dz·2o.
    let z0 = z + dot(dz, vec2<f32>(c * 2) + 0.5 - pix);
    let sx = dz.x * 2.0;
    let sy = dz.y * 2.0;
    ml_vis_add(c + vec2<i32>(-1, -1), dims, ax.x * ay.x, z0 - sx - sy, tol, e);
    ml_vis_add(c + vec2<i32>(0, -1), dims, ax.y * ay.x, z0 - sy, tol, e);
    ml_vis_add(c + vec2<i32>(1, -1), dims, ax.z * ay.x, z0 + sx - sy, tol, e);
    ml_vis_add(c + vec2<i32>(-1, 0), dims, ax.x * ay.y, z0 - sx, tol, e);
    ml_vis_add(c, dims, ax.y * ay.y, z0, tol, e);
    ml_vis_add(c + vec2<i32>(1, 0), dims, ax.z * ay.y, z0 + sx, tol, e);
    ml_vis_add(c + vec2<i32>(-1, 1), dims, ax.x * ay.z, z0 - sx + sy, tol, e);
    ml_vis_add(c + vec2<i32>(0, 1), dims, ax.y * ay.z, z0 + sy, tol, e);
    ml_vis_add(c + vec2<i32>(1, 1), dims, ax.z * ay.z, z0 + sx + sy, tol, e);
}
// The blocks for this pixel at p (face normal ng, view depth z, its
// screen slopes dp / dz). An overlay the pre-pass does not hold (a blended
// decal on the road: the start line) lies a little over the surface the
// blocks traced; there the blocks of the surface point under p are taken
// (their screen position from p's plane), as long as it is within 25 cm.
fn ml_vis_find(p: vec3<f32>, z: f32, dpx: vec3<f32>, dpy: vec3<f32>, dz: vec2<f32>, ng: vec3<f32>) {
    ml_vis_taps(g_ml_pix, z, dz, ng);
    if max(g_vis_wt.x, g_vis_wt.y) >= 0.025 || ng.z < 0.5 || (g_vis_c.x | g_vis_c.y) == 0u { return; }
    let b = g_vis_cb;
    let behind = bitcast<f32>(g_vis_c.z) - (z + dot(dz, vec2<f32>(b * 2) + 0.5 - g_ml_pix));
    let ray = normalize(p - frame.camera.xyz);
    let rf = dot(ray, frame.forward.xyz);
    if behind <= 0.0 || rf <= 0.0 { return; }
    let t = behind / rf;
    let h = t * max(-dot(ray, ng), 0.0);
    if h > 0.25 { return; }
    // From the surface point the pixel sees (p + ray t) to the one under
    // p (p − ng h): in the plane, in pixels along dpx / dpy.
    let w = -(ray * t + ng * h);
    let aa = dot(dpx, dpx);
    let ab = dot(dpx, dpy);
    let bb = dot(dpy, dpy);
    let det = aa * bb - ab * ab;
    if abs(det) < 1e-12 { return; }
    let ds = vec2<f32>(bb * dot(dpx, w) - ab * dot(dpy, w), aa * dot(dpy, w) - ab * dot(dpx, w)) / det;
    if dot(ds, ds) > 4096.0 { return; }
    let r = p - ng * h;
    ml_vis_taps(g_ml_pix + ds, dot(r - frame.camera.xyz, frame.forward.xyz), dz, ng);
}
// The traced visibility of lamp `light` at this pixel (its blocks'
// weighted mean); -1 when the blocks did not trace it or too few of them
// lie on this surface (the light keeps its fallback).
fn ml_vis(light: u32) -> f32 {
    let m = g_vis_ids == vec4<u32>(light + 1u);
    let w = dot(select(vec4<f32>(0.0), g_vis_wt, m), vec4<f32>(1.0));
    // (Below a tenth of the full weight: too few blocks on this surface.)
    if w < 0.025 { return -1.0; }
    return dot(select(vec4<f32>(0.0), g_vis_sum, m), vec4<f32>(1.0)) / w;
}

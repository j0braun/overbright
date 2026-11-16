// ---- temporal ----

// The temporal resolve of the low-res march (clone deviation; the
// smoke is drawn after the game's TAA, so it carries its own): this
// frame's march blended with the last frame's result, reprojected by the
// camera's motion at the medium's mean distance, the history clamped to
// this frame's 3 × 3 neighbourhood (no ghosting; Karis 2014, Salvi 2016
// variance clip). Out: the resolved colour and coverage, also the next
// frame's history.
@group(0) @binding(0) var<uniform> tv: Vol;
@group(0) @binding(1) var tcur: texture_2d<f32>;
@group(0) @binding(2) var tdepth: texture_2d<f32>;
@group(0) @binding(3) var thist: texture_2d<f32>;
@group(0) @binding(4) var tsamp: sampler;
@group(0) @binding(5) var<storage, read> tbounds: array<u32>;

@vertex
fn vs_temporal(@builtin(vertex_index) vi: u32) -> FsIn {
    var o: FsIn;
    let lo = vec3(f32(tbounds[12]), f32(tbounds[13]), f32(tbounds[14]));
    let hi = vec3(f32(tbounds[15]), f32(tbounds[16]), f32(tbounds[17]));
    o.pos = rect_vertex(vi, tv, lo, hi, tv.sizes.xy, 1.0, 3.0);
    return o;
}

@fragment
fn fs_temporal(in: FsIn) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.pos.xy);
    let hi = vec2<i32>(tv.sizes.xy) - 1;
    let cur = textureLoad(tcur, p, 0);
    if (tv.temporal.z < 0.5 || tv.prev_eye.w < 0.5) {
        return cur;
    }
    // The neighbourhood's mean and deviation (premultiplied rgba).
    var m1 = vec4(0.0);
    var m2 = vec4(0.0);
    for (var k = 0; k < 9; k++) {
        let c = textureLoad(tcur, clamp(p + vec2(k % 3 - 1, k / 3 - 1), vec2(0), hi), 0);
        m1 += c;
        m2 += c * c;
    }
    m1 /= 9.0;
    let sd = sqrt(max(m2 / 9.0 - m1 * m1, vec4(0.0)));
    let lo_c = m1 - 1.25 * sd;
    let hi_c = m1 + 1.25 * sd;
    // This pixel's medium, back into the last frame.
    let px = in.pos.xy;
    let ndc = vec2((px.x - tv.viewport.x) / tv.viewport.z * 2.0 - 1.0, 1.0 - (px.y - tv.viewport.y) / tv.viewport.w * 2.0);
    let h = tv.inv_vp * vec4(ndc, 0.5, 1.0);
    let dir = normalize(h.xyz / h.w);
    let dist = textureLoad(tdepth, p, 0).b;
    let world = dir * min(dist, tv.forward.w) - tv.prev_eye.xyz;
    let c = tv.prev_vp * vec4(world, 1.0);
    if (c.w < 0.05) {
        return cur;
    }
    let pn = c.xy / c.w;
    let pp = tv.prev_view.xy + vec2(pn.x * 0.5 + 0.5, 0.5 - pn.y * 0.5) * tv.prev_view.zw;
    if (any(pp < vec2(0.0)) || any(pp > tv.sizes.xy)) {
        return cur;
    }
    let hist = clamp(textureSampleLevel(thist, tsamp, pp / tv.sizes.xy, 0.0), lo_c, hi_c);
    // (Less history where the view moved far across the pixel: a fast pan
    // keeps its detail sharp rather than smeared.)
    let moved = length(pp - px);
    let w = tv.temporal.x * (1.0 - 0.4 * smoothstep(2.0, 12.0, moved));
    return max(mix(cur, hist, w), vec4(0.0));
}


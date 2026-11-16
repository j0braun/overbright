// ---------------------------------------------------------------------
// The glow's C when the scene is not the screen's size (SSAA / Render
// Scale, docs/post-scale.md): the box of the scene under each glow texel,
// params.x × params.x bilinear taps offsets[0].xy apart.
fn box_taps(uv: vec2<f32>) -> vec4<f32> {
    let n = i32(pass_.params.x);
    let step = pass_.offsets[0].xy;
    let first = uv - step * (f32(n - 1) * 0.5);
    var sum = vec4<f32>(0.0);
    for (var y = 0; y < 8; y++) {
        if y >= n {
            break;
        }
        for (var x = 0; x < 8; x++) {
            if x >= n {
                break;
            }
            sum += tap(first + step * vec2<f32>(f32(x), f32(y)));
        }
    }
    return sum / f32(n * n);
}

@fragment
fn fs_box(in: VsOut) -> @location(0) vec4<f32> {
    return box_taps(in.uv);
}

@fragment
fn fs_box_hdr(in: VsOut) -> @location(0) vec4<f32> {
    return sat(box_taps(in.uv));
}

// A = overbright(C), saturated as its 8-bit target is (High Precision
// Post, scaled; `fs_overbright_mrt_hdr`'s second output).
@fragment
fn fs_overbright_sat(in: VsOut) -> @location(0) vec4<f32> {
    return sat(overbright_of(tap(in.uv)));
}

// ---------------------------------------------------------------------
// SSAA with High Precision Post (docs/ssaa-screen-passes.md): the scene is
// resolved to the screen's size before the passes after the bleach
// bypass, which then run at the screen's size. offsets[1].xy: the scene's
// texels per screen pixel.

// The box under the screen pixel at `uv` along one axis (`n` scene texels,
// `r` of them per pixel), as enhance.wgsl's `fs_resample` takes it: the
// first texel it touches and the area each of the next three covers (the
// box spans at most three when `r` is at most 2: SSAA 2x and 4x).
struct Span3 {
    first: i32,
    w: vec3<f32>,
}

fn span3(u: f32, n: f32, r: f32) -> Span3 {
    let lo = u * n - r * 0.5;
    let hi = lo + r;
    let first = floor(lo);
    var w = vec3<f32>(0.0);
    for (var i = 0; i < 3; i++) {
        let x = first + f32(i);
        w[i] = max(0.0, min(hi, x + 1.0) - max(lo, x));
    }
    return Span3(i32(first), w);
}

// What `fs_bleach_hdr` makes of a texel (the bleach bypass off: the texel
// saturated, as the last pass takes it).
override RESOLVE_BLEACH: bool = true;

fn graded(v: vec4<f32>) -> vec4<f32> {
    let c = sat(v);
    if !RESOLVE_BLEACH {
        return c;
    }
    let p0 = sat(bleach0_of(c));
    let p1 = sat(bleach1_of(p0));
    return sat(bleach2_of(p1));
}

fn resolve_texel(c: vec2<i32>, max_coord: vec2<i32>) -> vec4<f32> {
    return graded(textureLoad(source, clamp(c, vec2<i32>(0), max_coord), 0));
}

// The resolve: each screen pixel the box of the scene under it, every
// texel graded first (as before: the bleach bypass at the scene's size,
// then the resample) and weighed by the area it covers.
@fragment
fn fs_resolve(in: VsOut) -> @location(0) vec4<f32> {
    let ratio = pass_.offsets[1].xy;
    let dims = vec2<i32>(textureDimensions(source));
    let max_coord = dims - vec2<i32>(1);
    var sum = vec4<f32>(0.0);
    if ratio.x <= 2.0 && ratio.y <= 2.0 {
        let sx = span3(in.uv.x, f32(dims.x), ratio.x);
        let sy = span3(in.uv.y, f32(dims.y), ratio.y);
        for (var j = 0; j < 3; j++) {
            for (var i = 0; i < 3; i++) {
                let w = sx.w[i] * sy.w[j];
                if w > 0.0 {
                    sum += resolve_texel(vec2<i32>(sx.first + i, sy.first + j), max_coord) * w;
                }
            }
        }
    } else {
        let lo = in.uv * vec2<f32>(dims) - ratio * 0.5;
        let hi = lo + ratio;
        let first = vec2<i32>(floor(lo));
        let end = vec2<i32>(ceil(hi));
        for (var y = first.y; y < end.y; y++) {
            let wy = max(0.0, min(hi.y, f32(y + 1)) - max(lo.y, f32(y)));
            for (var x = first.x; x < end.x; x++) {
                let wx = max(0.0, min(hi.x, f32(x + 1)) - max(lo.x, f32(x)));
                sum += resolve_texel(vec2<i32>(x, y), max_coord) * (wx * wy);
            }
        }
    }
    return sum / (ratio.x * ratio.y);
}

// The streak buffer (the scene's size, the glow sprites drawn into it
// supersampled) taken to the screen pixel at `uv`: the same box. At most
// three texels a side: two bilinear reads a side, each the pair of texels
// it lies between in the share of the box they cover (the middle texel's
// split between them); larger boxes texel by texel.
fn streak_box(uv: vec2<f32>) -> vec4<f32> {
    let ratio = pass_.offsets[1].xy;
    let dims = vec2<f32>(textureDimensions(gainmap));
    if ratio.x <= 2.0 && ratio.y <= 2.0 {
        let sx = span3(uv.x, dims.x, ratio.x);
        let sy = span3(uv.y, dims.y, ratio.y);
        // Per axis: read A over texels 0 and 1 (weights w0, w1 / 2), read B
        // over texels 1 and 2 (w1 / 2, w2); w1 > 0 always (the middle texel
        // lies inside a box at least one texel wide).
        let ax = vec2<f32>(sx.w.x + 0.5 * sx.w.y, 0.5 * sx.w.y + sx.w.z);
        let ay = vec2<f32>(sy.w.x + 0.5 * sy.w.y, 0.5 * sy.w.y + sy.w.z);
        let px = vec2<f32>(f32(sx.first) + 0.5 + 0.5 * sx.w.y / ax.x, f32(sx.first) + 1.5 + sx.w.z / ax.y) / dims.x;
        let py = vec2<f32>(f32(sy.first) + 0.5 + 0.5 * sy.w.y / ay.x, f32(sy.first) + 1.5 + sy.w.z / ay.y) / dims.y;
        var sum = vec4<f32>(0.0);
        for (var j = 0; j < 2; j++) {
            for (var i = 0; i < 2; i++) {
                sum += textureSampleLevel(gainmap, samp, vec2<f32>(px[i], py[j]), 0.0) * (ax[i] * ay[j]);
            }
        }
        return sum / (ratio.x * ratio.y);
    }
    return resolve_box(gainmap, uv);
}

// `t`'s box under the screen pixel at `uv`, texel by texel.
fn resolve_box(t: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    let ratio = pass_.offsets[1].xy;
    let dims = vec2<i32>(textureDimensions(t));
    let lo = uv * vec2<f32>(dims) - ratio * 0.5;
    let hi = lo + ratio;
    let first = vec2<i32>(floor(lo));
    let end = vec2<i32>(ceil(hi));
    let max_coord = dims - vec2<i32>(1);
    var sum = vec4<f32>(0.0);
    for (var y = first.y; y < end.y; y++) {
        let wy = max(0.0, min(hi.y, f32(y + 1)) - max(lo.y, f32(y)));
        for (var x = first.x; x < end.x; x++) {
            let wx = max(0.0, min(hi.x, f32(x + 1)) - max(lo.x, f32(x)));
            sum += textureLoad(t, clamp(vec2<i32>(x, y), vec2<i32>(0), max_coord), 0) * (wx * wy);
        }
    }
    return sum / (ratio.x * ratio.y);
}

// The resolved image plus the streak buffer's box, as `fs_out_add` adds
// them.
@fragment
fn fs_out_add_resolved(in: VsOut) -> @location(0) vec4<f32> {
    return to_output(tap(in.uv) + streak_box(in.uv), in.position);
}

@fragment
fn fs_out_add_resolved_a0(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(to_output(tap(in.uv) + streak_box(in.uv), in.position).rgb, 0.0);
}

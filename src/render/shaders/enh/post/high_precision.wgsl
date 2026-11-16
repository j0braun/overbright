// ---------------------------------------------------------------------
// High Precision Post (clone deviation, off by default; docs/hdr-post.md):
// the same passes on half-float targets. Where the native chain's 8-bit
// writes saturate, these saturate too (`sat`); where they round, these
// keep the precision. The last pass dithers back to the 8-bit output.

fn sat(v: vec4<f32>) -> vec4<f32> {
    return clamp(v, vec4<f32>(0.0), vec4<f32>(1.0));
}

// For the streak buffer's saturation (BLENDOP_MIN with the target).
@fragment
fn fs_one(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}

// screenfilter.efx's three passes in one, each hand-off saturated as
// its A8R8G8B8 target is.
@fragment
fn fs_bleach_hdr(in: VsOut) -> @location(0) vec4<f32> {
    let p0 = sat(bleach0_of(sat(tap(in.uv))));
    let p1 = sat(bleach1_of(p0));
    return sat(bleach2_of(p1));
}

// overbright.efx's C (the prefiltered image, as its 8-bit target holds
// it: saturated) and A = overbright(C).
@fragment
fn fs_overbright_mrt_hdr(in: VsOut) -> OverbrightOut {
    let c = sat(filter_taps(in.uv));
    var out: OverbrightOut;
    out.c = c;
    // (x · g)² reaches 3.1 at white: saturated as A's 8-bit target is.
    out.a = sat(overbright_of(c));
    return out;
}

// A Gaussian of σ = params.w texels along offsets[0].xy (one texel), two
// texels per bilinear read.
fn gauss(uv: vec2<f32>) -> vec4<f32> {
    let step = pass_.offsets[0].xy;
    let sigma = pass_.params.w;
    let k = -0.5 / (sigma * sigma);
    let radius = i32(ceil(sigma * 3.0));
    var sum = tap(uv);
    var total = 1.0;
    for (var i = 1; i <= radius; i += 2) {
        let a = f32(i);
        let w0 = exp(k * a * a);
        let w1 = exp(k * (a + 1.0) * (a + 1.0));
        let w = w0 + w1;
        let o = (a * w0 + (a + 1.0) * w1) / w;
        sum += (tap(uv + step * o) + tap(uv - step * o)) * w;
        total += 2.0 * w;
    }
    return sum / total;
}

@fragment
fn fs_blur_hdr(in: VsOut) -> @location(0) vec4<f32> {
    return gauss(in.uv);
}

// The blur down, then A − C · factor (C bound in `gainmap`'s place;
// factor in tint.x), held at 0 as BLENDOP_REVSUBTRACT into an 8-bit
// target leaves it.
@fragment
fn fs_blur_sub_hdr(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(gainmap, samp, in.uv, 0.0);
    return max(gauss(in.uv) - c * pass_.tint.x, vec4<f32>(0.0));
}

// A triangular dither of ±1 step of the 8-bit output, fixed per pixel
// (shots stay deterministic), left out where the colour is saturated
// (black and white stay clean).
fn hash(p: vec2<u32>, salt: u32) -> f32 {
    var h = ((p.x * 0x8da6b343u) ^ (p.y * 0xd8163841u)) ^ (salt * 0xcb1ab31fu);
    h ^= h >> 16u;
    h *= 0x7feb352du;
    h ^= h >> 15u;
    h *= 0x846ca68bu;
    h ^= h >> 16u;
    return f32(h >> 8u) * (1.0 / 16777216.0);
}

fn to_output(c: vec4<f32>, position: vec4<f32>) -> vec4<f32> {
    let v = sat(c);
    let p = vec2<u32>(position.xy);
    let n = (hash(p, 1u) + hash(p, 2u) - 1.0) / 255.0;
    let inside = select(vec3<f32>(0.0), vec3<f32>(1.0), (v.rgb > vec3<f32>(0.5 / 255.0)) & (v.rgb < vec3<f32>(254.5 / 255.0)));
    return vec4<f32>(v.rgb + n * inside, v.a);
}

@fragment
fn fs_out(in: VsOut) -> @location(0) vec4<f32> {
    return to_output(filter_taps(in.uv), in.position);
}

@fragment
fn fs_out_a0(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(to_output(filter_taps(in.uv), in.position).rgb, 0.0);
}

// The image plus the streak buffer, as `add_streaks`.
@fragment
fn fs_out_add(in: VsOut) -> @location(0) vec4<f32> {
    return to_output(tap(in.uv) + textureSampleLevel(gainmap, samp, in.uv, 0.0), in.position);
}

@fragment
fn fs_out_add_a0(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(to_output(tap(in.uv) + textureSampleLevel(gainmap, samp, in.uv, 0.0), in.position).rgb, 0.0);
}


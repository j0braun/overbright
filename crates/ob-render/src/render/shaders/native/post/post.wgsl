// The full-screen passes of the frame's end (0x5CB800…0x5CBA29), see
// postfx.rs. Each fragment entry is one pixel shader of filter.efx,
// overbright.efx or screenfilter.efx (technique 0), transcribed from their
// ps_1_1 / ps_1_4 bytecode.

struct Pass {
    // filter.efx FILTERWEIGHTS c0..c3, one per tap.
    weights: array<vec4<f32>, 4>,
    // Tap offsets (xy, in uv units of the source).
    offsets: array<vec4<f32>, 4>,
    // x: number of taps; y: TINTCOLOR.w; z: OVERBRIGHTOFFSET.
    params: vec4<f32>,
    // OVERBRIGHTGREYSCALE.
    grey: vec4<f32>,
    // screenfilter.efx TINTCOLOR.rgb (scaled) and SATURATIONSCALE in w.
    tint: vec4<f32>,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var gainmap: texture_2d<f32>;
@group(0) @binding(2) var rampmap: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;
@group(0) @binding(4) var<uniform> pass_: Pass;
// Texel k = (k, k, k, k) / 255, read as the passes read an 8-bit texel:
// the fused passes' stand-in for an 8-bit target read back (see `q`).
@group(0) @binding(5) var unorm_lut: texture_2d<f32>;

// Where the device's float → 8-bit UNORM conversion of a render target
// write steps from k to k + 1, per k (the bits of the smallest float
// written as k + 1), measured at start-up (postfx.rs `measure_bounds`).
// The specs leave this conversion loose (Vulkan: "one of the two closest
// values, should round to nearest"; D3D: 0.6 ULP), and devices differ: one
// steps a few thousandths of a step past the exact tie.
@group(0) @binding(6) var<storage, read> unorm_bounds: array<u32, 512>;

// 1: `q` rounds by the measured steps; 0: to nearest, ties to even (for
// the self-check's report).
override MEASURED: bool = true;

// One channel written to an 8-bit UNORM target and read back.
fn q1(x: f32) -> f32 {
    let v = clamp(x, 0.0, 1.0);
    var k = clamp(round(v * 255.0), 0.0, 255.0);
    if MEASURED {
        let i = u32(k);
        if i > 0u && v < bitcast<f32>(unorm_bounds[256u + i - 1u]) {
            k -= 1.0;
        } else if i < 255u && v >= bitcast<f32>(unorm_bounds[256u + i]) {
            k += 1.0;
        }
    }
    // Read as the passes read their 8-bit sources: through the bilinear
    // sampler at a texel's centre.
    return textureSampleLevel(unorm_lut, samp, vec2<f32>((k + 0.5) / 256.0, 0.5), 0.0).r;
}

fn q(v: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(q1(v.x), q1(v.y), q1(v.z), q1(v.w));
}

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// One triangle over the screen; uv (0,0) is the top left.
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.position = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

fn tap(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(source, samp, uv, 0.0);
}

// filter.efx: r0 = Σ tᵢ · FilterWeights[i], one to four taps.
fn filter_taps(uv: vec2<f32>) -> vec4<f32> {
    let n = i32(pass_.params.x);
    var sum = vec4<f32>(0.0);
    for (var i = 0; i < 4; i++) {
        if i < n {
            sum += tap(uv + pass_.offsets[i].xy) * pass_.weights[i];
        }
    }
    return sum;
}

@fragment
fn fs_filter(in: VsOut) -> @location(0) vec4<f32> {
    return filter_taps(in.uv);
}

// The last pass with alpha 0 (the UI's destination-alpha textures then
// need no full-screen alpha-0 pass of their own; see ui.rs).
@fragment
fn fs_filter_a0(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(filter_taps(in.uv).rgb, 0.0);
}

// Nothing: for passes whose blend ignores the source (the streak
// buffer's decay, ZERO / BLENDFACTOR), so no texture is read.
@fragment
fn fs_zero(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}

// The image plus the streak buffer (bound in `gainmap`'s place): what a
// one-tap copy of the image followed by the buffer added ONE / ONE leaves,
// the sum of two 8-bit values, saturated.
fn add_streaks(uv: vec2<f32>) -> vec4<f32> {
    return clamp(tap(uv) + textureSampleLevel(gainmap, samp, uv, 0.0), vec4<f32>(0.0), vec4<f32>(1.0));
}

@fragment
fn fs_add(in: VsOut) -> @location(0) vec4<f32> {
    return add_streaks(in.uv);
}

@fragment
fn fs_add_a0(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(add_streaks(in.uv).rgb, 0.0);
}

// 0x5C5C10 with motion blur's shifts (0x5CC470): four taps of ¼, tap i
// at uv − i·shift, the shift interpolated between the per-corner values
// offsets[0] = (left u, top v, right u, bottom v).
@fragment
fn fs_motion(in: VsOut) -> @location(0) vec4<f32> {
    let c = pass_.offsets[0];
    let shift = vec2<f32>(mix(c.x, c.z, in.uv.x), mix(c.y, c.w, in.uv.y));
    var sum = vec4<f32>(0.0);
    for (var i = 0; i < 4; i++) {
        sum += tap(in.uv - f32(i) * shift) * pass_.weights[i];
    }
    return sum;
}

struct FogOut {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec4<f32>,
}

// horizon_fog.efx: FVF 0x142 under identity transforms, no shaders; the
// vertex colour (A, R, G, B) is passed through (no texture is bound).
@vertex
fn vs_fog(@location(0) position: vec3<f32>, @location(1) argb: vec4<f32>) -> FogOut {
    var out: FogOut;
    out.position = vec4<f32>(position, 1.0);
    out.colour = vec4<f32>(argb.y, argb.z, argb.w, argb.x);
    return out;
}

@fragment
fn fs_fog(in: FogOut) -> @location(0) vec4<f32> {
    return in.colour;
}

// overbright.efx: x = sat(4·(t0 − OverBrightOffset)),
// out = (x · dot(x, OverBrightGreyScale))², alpha 0.
fn overbright_of(t0: vec4<f32>) -> vec4<f32> {
    let x = clamp((t0.rgb - vec3<f32>(pass_.params.z)) * 4.0, vec3<f32>(0.0), vec3<f32>(1.0));
    let g = dot(x, pass_.grey.rgb);
    let r = x * g;
    return vec4<f32>(r * r, 0.0);
}

@fragment
fn fs_overbright(in: VsOut) -> @location(0) vec4<f32> {
    return overbright_of(tap(in.uv));
}

// overbright.efx's C and A in one pass: C (location 0) the half-size tap
// as before, A (location 1) from C as its 8-bit target holds it.
struct OverbrightOut {
    @location(0) c: vec4<f32>,
    @location(1) a: vec4<f32>,
}

@fragment
fn fs_overbright_mrt(in: VsOut) -> OverbrightOut {
    let c = filter_taps(in.uv);
    var out: OverbrightOut;
    out.c = c;
    out.a = overbright_of(q(c));
    return out;
}

// screenfilter.efx technique 0, pass p0 (ps_1_4, the blob's ps at 0x3600;
// pixel shader object of pass p0 as the game binds it, seen in an apitrace
// of SPEED2.EXE): GAINMAP read at (lum, r) and (g, b), lum = dp4(rgba,
// (0.3, 0.59, 0.11, 0)); k = sat(1 − gain(lum)) blends the curves in.
// Alpha = gain(r)·(1 − k) + gain(lum)·k, which p2 takes as its luminance.
fn bleach0_of(t0: vec4<f32>) -> vec4<f32> {
    let lum = dot(t0.rgb, vec3<f32>(0.3, 0.59, 0.11));
    let t1 = textureSampleLevel(gainmap, samp, vec2<f32>(lum, t0.r), 0.0);
    let t2 = textureSampleLevel(gainmap, samp, vec2<f32>(t0.g, t0.b), 0.0);
    let k = clamp(1.0 - t1.a, 0.0, 1.0);
    let rgb = t0.rgb * (1.0 - k) + vec3<f32>(t1.r, t2.g, t2.b) * k;
    return vec4<f32>(rgb, t1.r * (1.0 - k) + t1.a * k);
}

@fragment
fn fs_bleach0(in: VsOut) -> @location(0) vec4<f32> {
    return bleach0_of(tap(in.uv));
}

// Pass p1 (ps_1_4, the blob's ps at 0x2D70): the tint, strongest where
// blue is near max(r, g): rgb += sat(1 − (max(r, g) − b)) ·
// SATURATIONSCALE · TINTCOLOR.rgb; alpha kept.
fn bleach1_of(t0: vec4<f32>) -> vec4<f32> {
    let s = clamp(1.0 - (max(t0.r, t0.g) - t0.b), 0.0, 1.0) * pass_.tint.w;
    return vec4<f32>(t0.rgb + s * pass_.tint.rgb, t0.a);
}

@fragment
fn fs_bleach1(in: VsOut) -> @location(0) vec4<f32> {
    return bleach1_of(tap(in.uv));
}

// Pass p2 (ps_1_4): blue through RAMPMAP (read at (g, b)), then the colour
// towards p1's luminance (its alpha) by TINTCOLOR.w again.
fn bleach2_of(r0: vec4<f32>) -> vec4<f32> {
    let w = pass_.params.y;
    let ramp = textureSampleLevel(rampmap, samp, vec2<f32>(r0.g, r0.b), 0.0);
    let keep = 1.0 - w;
    let lum = r0.a * w;
    return vec4<f32>(r0.r * keep + lum, r0.g * keep + lum, ramp.b * keep + lum, keep * keep + w);
}

@fragment
fn fs_bleach2(in: VsOut) -> @location(0) vec4<f32> {
    return bleach2_of(tap(in.uv));
}

// The three passes in one, each 8-bit hand-off emulated by `q`.
// The self-check's stage diagnosis (`NFSU2_CHECK_STAGES`): the first n
// passes only.
override STAGES: u32 = 3u;

@fragment
fn fs_bleach_fused(in: VsOut) -> @location(0) vec4<f32> {
    let p0 = bleach0_of(tap(in.uv));
    if STAGES == 1u {
        return p0;
    }
    if STAGES == 4u {
        return q(p0);
    }
    let p1 = bleach1_of(q(p0));
    if STAGES == 2u {
        return p1;
    }
    return bleach2_of(q(p1));
}

// The self-check's input: tile params.x of 4096×1024 texels, every texel
// a different colour (the four tiles hold all 2^24 colours).
@fragment
fn fs_pattern(in: VsOut) -> @location(0) vec4<f32> {
    let i = u32(pass_.params.x) * 4194304u + u32(in.position.y) * 4096u + u32(in.position.x);
    return vec4<f32>(f32(i & 255u), f32((i >> 8u) & 255u), f32((i >> 16u) & 255u), 255.0) / 255.0;
}


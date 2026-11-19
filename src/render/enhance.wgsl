// The Graphics page's screen passes (clone deviations, every one opt-in;
// see enhance.rs): FXAA on the scene's image, its resample to the screen
// (Render Scale), and contrast-adaptive sharpening. None of them runs with
// the page at its native values.

// Zero retains the general fractional/upscale path. Integer variants are
// selected only from exact source/output dimensions on the CPU.
override INTEGER_SCALE: u32 = 0u;

struct Pass {
    // xy: 1 / the source's size in texels; zw: the source's size.
    texel: vec4<f32>,
    // x: the sharpness (CAS, 0..1); y: source / target size across
    // (above 1: a downscale); z: the same down.
    params: vec4<f32>,
}

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> pass_: Pass;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// One triangle over the target; uv (0, 0) is the top left.
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.position = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

fn at(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(src, samp, uv, 0.0);
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

fn luma_at(uv: vec2<f32>) -> f32 {
    return luma(at(uv).rgb);
}

// FXAA 3.11, the PC quality path (Timothy Lottes, public domain), preset
// 12: subpixel 0.75, edge threshold 0.166, minimum 0.0833. Works on the
// stored (gamma-encoded) values, as the original's image is.
const EDGE_THRESHOLD: f32 = 0.166;
const EDGE_THRESHOLD_MIN: f32 = 0.0833;
const SUBPIX: f32 = 0.75;

@fragment
fn fs_fxaa(in: VsOut) -> @location(0) vec4<f32> {
    let rcp = pass_.texel.xy;
    let uv = in.uv;
    let rgba_m = at(uv);
    let luma_m = luma(rgba_m.rgb);
    var luma_s = luma_at(uv + vec2<f32>(0.0, rcp.y));
    let luma_e = luma_at(uv + vec2<f32>(rcp.x, 0.0));
    var luma_n = luma_at(uv - vec2<f32>(0.0, rcp.y));
    let luma_w = luma_at(uv - vec2<f32>(rcp.x, 0.0));
    let max_luma = max(max(max(luma_n, luma_w), max(luma_e, luma_s)), luma_m);
    let min_luma = min(min(min(luma_n, luma_w), min(luma_e, luma_s)), luma_m);
    let range = max_luma - min_luma;
    if range < max(EDGE_THRESHOLD_MIN, max_luma * EDGE_THRESHOLD) {
        return rgba_m;
    }
    let luma_nw = luma_at(uv + vec2<f32>(-rcp.x, -rcp.y));
    let luma_se = luma_at(uv + vec2<f32>(rcp.x, rcp.y));
    let luma_ne = luma_at(uv + vec2<f32>(rcp.x, -rcp.y));
    let luma_sw = luma_at(uv + vec2<f32>(-rcp.x, rcp.y));
    let luma_ns = luma_n + luma_s;
    let luma_we = luma_w + luma_e;
    let subpix_rcp_range = 1.0 / range;
    let subpix_nswe = luma_ns + luma_we;
    let edge_horz1 = -2.0 * luma_m + luma_ns;
    let edge_vert1 = -2.0 * luma_m + luma_we;
    let luma_nese = luma_ne + luma_se;
    let luma_nwne = luma_nw + luma_ne;
    let edge_horz2 = -2.0 * luma_e + luma_nese;
    let edge_vert2 = -2.0 * luma_n + luma_nwne;
    let luma_nwsw = luma_nw + luma_sw;
    let luma_swse = luma_sw + luma_se;
    let edge_horz4 = abs(edge_horz1) * 2.0 + abs(edge_horz2);
    let edge_vert4 = abs(edge_vert1) * 2.0 + abs(edge_vert2);
    let edge_horz3 = -2.0 * luma_w + luma_nwsw;
    let edge_vert3 = -2.0 * luma_s + luma_swse;
    let edge_horz = abs(edge_horz3) + edge_horz4;
    let edge_vert = abs(edge_vert3) + edge_vert4;
    let subpix_nwswnese = luma_nwsw + luma_nese;
    var length_sign = rcp.x;
    let horz_span = edge_horz >= edge_vert;
    let subpix_a = subpix_nswe * 2.0 + subpix_nwswnese;
    if !horz_span {
        luma_n = luma_w;
        luma_s = luma_e;
    } else {
        length_sign = rcp.y;
    }
    let subpix_b = subpix_a * (1.0 / 12.0) - luma_m;
    let gradient_n = luma_n - luma_m;
    let gradient_s = luma_s - luma_m;
    var luma_nn = luma_n + luma_m;
    let luma_ss = luma_s + luma_m;
    let pair_n = abs(gradient_n) >= abs(gradient_s);
    let gradient = max(abs(gradient_n), abs(gradient_s));
    if pair_n {
        length_sign = -length_sign;
    }
    let subpix_c = clamp(abs(subpix_b) * subpix_rcp_range, 0.0, 1.0);
    var pos_b = uv;
    var off_np = vec2<f32>(0.0, 0.0);
    if horz_span {
        off_np.x = rcp.x;
        pos_b.y += length_sign * 0.5;
    } else {
        off_np.y = rcp.y;
        pos_b.x += length_sign * 0.5;
    }
    var pos_n = pos_b - off_np;
    var pos_p = pos_b + off_np;
    let subpix_d = -2.0 * subpix_c + 3.0;
    var luma_end_n = luma_at(pos_n);
    let subpix_e = subpix_c * subpix_c;
    var luma_end_p = luma_at(pos_p);
    if !pair_n {
        luma_nn = luma_ss;
    }
    let gradient_scaled = gradient * 0.25;
    let luma_mm = luma_m - luma_nn * 0.5;
    let subpix_f = subpix_d * subpix_e;
    let luma_m_lt_zero = luma_mm < 0.0;
    luma_end_n -= luma_nn * 0.5;
    luma_end_p -= luma_nn * 0.5;
    var done_n = abs(luma_end_n) >= gradient_scaled;
    var done_p = abs(luma_end_p) >= gradient_scaled;
    // Preset 12's search steps after the first (1.0).
    var steps = array<f32, 4>(1.5, 2.0, 4.0, 12.0);
    for (var i = 0; i < 4; i++) {
        if done_n && done_p {
            break;
        }
        let q = steps[i];
        if !done_n {
            pos_n -= off_np * q;
            luma_end_n = luma_at(pos_n) - luma_nn * 0.5;
            done_n = abs(luma_end_n) >= gradient_scaled;
        }
        if !done_p {
            pos_p += off_np * q;
            luma_end_p = luma_at(pos_p) - luma_nn * 0.5;
            done_p = abs(luma_end_p) >= gradient_scaled;
        }
    }
    var dst_n = uv.y - pos_n.y;
    var dst_p = pos_p.y - uv.y;
    if horz_span {
        dst_n = uv.x - pos_n.x;
        dst_p = pos_p.x - uv.x;
    }
    let good_span_n = (luma_end_n < 0.0) != luma_m_lt_zero;
    let good_span_p = (luma_end_p < 0.0) != luma_m_lt_zero;
    let span_length = dst_p + dst_n;
    let direction_n = dst_n < dst_p;
    let dst = min(dst_n, dst_p);
    let good_span = select(good_span_p, good_span_n, direction_n);
    let subpix_g = subpix_f * subpix_f;
    let pixel_offset = dst * (-1.0 / span_length) + 0.5;
    let subpix_h = subpix_g * SUBPIX;
    let pixel_offset_good = select(0.0, pixel_offset, good_span);
    let pixel_offset_subpix = max(pixel_offset_good, subpix_h);
    var pos_m = uv;
    if horz_span {
        pos_m.y += pixel_offset_subpix * length_sign;
    } else {
        pos_m.x += pixel_offset_subpix * length_sign;
    }
    return vec4<f32>(at(pos_m).rgb, rgba_m.a);
}

// The scene's image to the screen's size. Downscaling (Render Scale above
// 100 %): area-weighted box over every covered source pixel. Upscaling (below 100 %): Catmull-Rom in nine bilinear
// taps (sharper than bilinear, no ringing past the neighbours' range).
@fragment
fn fs_resample(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let ratio = pass_.params.yz;
    let alpha = at(uv).a;
    if ratio.x >= 1.0 && ratio.y >= 1.0 {
        // Integrate the complete source-pixel footprint. Fractional SSAA
        // scales weight boundary texels by their overlap; integer scales
        // are exact NxN averages (up to 4x4 for SSAA 16x).
        let lo = uv * pass_.texel.zw - ratio * 0.5;
        let hi = lo + ratio;
        let first = vec2<i32>(floor(lo));
        let end = vec2<i32>(ceil(hi));
        let max_coord = vec2<i32>(textureDimensions(src)) - vec2<i32>(1);
        var sum = vec4<f32>(0.0);
        if INTEGER_SCALE > 0u {
            // Interpolated UVs can move an otherwise integer footprint slightly
            // off-grid. Preserve those weights, the extra boundary texel and
            // row-major accumulation exactly as in the general path.
            for (var j = 0u; j <= INTEGER_SCALE; j++) {
                let y = first.y + i32(j);
                if y < end.y {
                    let wy = max(0.0, min(hi.y, f32(y + 1)) - max(lo.y, f32(y)));
                    for (var i = 0u; i <= INTEGER_SCALE; i++) {
                        let x = first.x + i32(i);
                        if x < end.x {
                            let wx = max(0.0, min(hi.x, f32(x + 1)) - max(lo.x, f32(x)));
                            sum += textureLoad(src, clamp(vec2<i32>(x, y), vec2<i32>(0), max_coord), 0) * wx * wy;
                        }
                    }
                }
            }
            return sum / (ratio.x * ratio.y);
        }
        for (var y = first.y; y < end.y; y++) {
            let wy = max(0.0, min(hi.y, f32(y + 1)) - max(lo.y, f32(y)));
            for (var x = first.x; x < end.x; x++) {
                let wx = max(0.0, min(hi.x, f32(x + 1)) - max(lo.x, f32(x)));
                sum += textureLoad(src, clamp(vec2<i32>(x, y), vec2<i32>(0), max_coord), 0) * wx * wy;
            }
        }
        return sum / (ratio.x * ratio.y);
    }
    let size = pass_.texel.zw;
    let sample_pos = uv * size;
    let tex_pos1 = floor(sample_pos - 0.5) + 0.5;
    let f = sample_pos - tex_pos1;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let offset12 = w2 / w12;
    let p0 = (tex_pos1 - 1.0) * pass_.texel.xy;
    let p3 = (tex_pos1 + 2.0) * pass_.texel.xy;
    let p12 = (tex_pos1 + offset12) * pass_.texel.xy;
    var c = vec3<f32>(0.0);
    c += at(vec2<f32>(p0.x, p0.y)).rgb * w0.x * w0.y;
    c += at(vec2<f32>(p12.x, p0.y)).rgb * w12.x * w0.y;
    c += at(vec2<f32>(p3.x, p0.y)).rgb * w3.x * w0.y;
    c += at(vec2<f32>(p0.x, p12.y)).rgb * w0.x * w12.y;
    c += at(vec2<f32>(p12.x, p12.y)).rgb * w12.x * w12.y;
    c += at(vec2<f32>(p3.x, p12.y)).rgb * w3.x * w12.y;
    c += at(vec2<f32>(p0.x, p3.y)).rgb * w0.x * w3.y;
    c += at(vec2<f32>(p12.x, p3.y)).rgb * w12.x * w3.y;
    c += at(vec2<f32>(p3.x, p3.y)).rgb * w3.x * w3.y;
    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), alpha);
}

// Contrast-adaptive sharpening (AMD FidelityFX CAS, MIT; its no-scale
// path): each pixel sharpened against its cross by a weight that falls
// where the neighbourhood already has contrast, so edges do not ring.
// CAS's undershoot limiter: the 3×3 peak luma from which it starts
// and where it is full.
const BRIGHT_LO: f32 = 0.35;
const BRIGHT_HI: f32 = 0.7;

@fragment
fn fs_cas(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = pass_.texel.xy;
    let a = at(uv + vec2<f32>(-t.x, -t.y)).rgb;
    let b = at(uv + vec2<f32>(0.0, -t.y)).rgb;
    let c = at(uv + vec2<f32>(t.x, -t.y)).rgb;
    let d = at(uv + vec2<f32>(-t.x, 0.0)).rgb;
    let e4 = at(uv);
    let e = e4.rgb;
    let f = at(uv + vec2<f32>(t.x, 0.0)).rgb;
    let g = at(uv + vec2<f32>(-t.x, t.y)).rgb;
    let h = at(uv + vec2<f32>(0.0, t.y)).rgb;
    let i = at(uv + vec2<f32>(t.x, t.y)).rgb;
    var mn = min(min(min(d, e), min(f, b)), h);
    let mn2 = min(min(mn, a), min(min(c, g), i));
    mn += mn2;
    var mx = max(max(max(d, e), max(f, b)), h);
    let mx2 = max(max(mx, a), max(max(c, g), i));
    mx += mx2;
    let amp = sqrt(clamp(min(mn, 2.0 - mx) / max(mx, vec3<f32>(1.0e-5)), vec3<f32>(0.0), vec3<f32>(1.0)));
    let peak = -1.0 / mix(8.0, 5.0, clamp(pass_.params.x, 0.0, 1.0));
    let w = amp * peak;
    let rcp_weight = 1.0 / (1.0 + 4.0 * w);
    let sharp = (b * w + d * w + f * w + h * w + e) * rcp_weight;
    // Around bright light (glow, lamps, neon) the negative lobe would
    // darken the pixels beside it below anything near them: a dark ring.
    // There the result is held at the 3×3 neighbourhood's minimum (no
    // undershoot); elsewhere CAS is unchanged (docs/notes/graphics/ssaa.md "Post effects under SSAA and Render Scale").
    let lo = min(min(min(d, e), min(f, b)), min(min(h, a), min(min(c, g), i)));
    let peak_luma = dot(mx2, vec3<f32>(0.299, 0.587, 0.114));
    let bright = smoothstep(BRIGHT_LO, BRIGHT_HI, peak_luma);
    let limited = max(sharp, mix(sharp, lo, bright));
    let out = clamp(limited, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(out, e4.a);
}

// FSR 1 (AMD FidelityFX Super Resolution 1.0, MIT licence, (c) 2021
// Advanced Micro Devices, Inc.): EASU (edge-adaptive spatial upsampling)
// and RCAS (robust contrast-adaptive sharpening), ported to WGSL from
// ffx_fsr1.h (FsrEasuF / FsrRcasF, the 32-bit paths). A clone deviation
// (docs/modern-extras.md), opt-in on the Upscaling row; nothing of
// it is in SPEED2.EXE. The approximate reciprocals of the original
// (APrxLoRcpF1 …) are exact here.

struct Uniforms {
    // input size, 1 / input size
    input: vec4<f32>,
    // output size, 1 / output size
    output: vec4<f32>,
    // RCAS sharpness (linear: exp2(-stops)), unused x3
    params: vec4<f32>,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var<uniform> u: Uniforms;

struct VsOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VsOut;
    o.position = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

fn rcp(x: f32) -> f32 {
    return 1.0 / max(x, 1e-8);
}

fn fetch(p: vec2<i32>) -> vec4<f32> {
    let size = vec2<i32>(u.input.xy);
    return textureLoad(src, clamp(p, vec2<i32>(0), size - 1), 0);
}

// The luma FSR steers by: B / 2 + R / 2 + G (ffx_fsr1.h's EASU input).
fn luma(c: vec4<f32>) -> f32 {
    return c.b * 0.5 + (c.r * 0.5 + c.g);
}

// FsrEasuSetF: one bilinear corner's share of the direction and length.
fn easu_set(dir: ptr<function, vec2<f32>>, len: ptr<function, f32>, w: f32, la: f32, lb: f32, lc: f32, ld: f32, le: f32) {
    //    a
    //  b c d
    //    e
    let dc = ld - lc;
    let cb = lc - lb;
    var len_x = max(abs(dc), abs(cb));
    len_x = rcp(len_x);
    let dir_x = ld - lb;
    (*dir).x += dir_x * w;
    len_x = clamp(abs(dir_x) * len_x, 0.0, 1.0);
    len_x *= len_x;
    *len += len_x * w;
    let ec = le - lc;
    let ca = lc - la;
    var len_y = max(abs(ec), abs(ca));
    len_y = rcp(len_y);
    let dir_y = le - la;
    (*dir).y += dir_y * w;
    len_y = clamp(abs(dir_y) * len_y, 0.0, 1.0);
    len_y *= len_y;
    *len += len_y * w;
}

// FsrEasuTapF: one tap of the lobe (a windowed approximation of Lanczos 2).
fn easu_tap(ac: ptr<function, vec4<f32>>, aw: ptr<function, f32>, off: vec2<f32>, dir: vec2<f32>, len: vec2<f32>, lob: f32, clp: f32, c: vec4<f32>) {
    var v = vec2<f32>(off.x * dir.x + off.y * dir.y, off.x * (-dir.y) + off.y * dir.x);
    v *= len;
    var d2 = v.x * v.x + v.y * v.y;
    d2 = min(d2, clp);
    var wb = (2.0 / 5.0) * d2 - 1.0;
    var wa = lob * d2 - 1.0;
    wb *= wb;
    wa *= wa;
    wb = (25.0 / 16.0) * wb - (25.0 / 16.0 - 1.0);
    let w = wb * wa;
    *ac += c * w;
    *aw += w;
}

@fragment
fn fs_easu(in: VsOut) -> @location(0) vec4<f32> {
    // The output pixel's centre in input pixels, less half a pixel.
    var pp = in.position.xy * u.input.xy * u.output.zw - vec2<f32>(0.5);
    let fp = floor(pp);
    pp -= fp;
    let f0 = vec2<i32>(fp);
    //    b c
    //  e f g h
    //  i j k l
    //    n o
    let bc = fetch(f0 + vec2<i32>(0, -1));
    let cc = fetch(f0 + vec2<i32>(1, -1));
    let ec = fetch(f0 + vec2<i32>(-1, 0));
    let fc = fetch(f0);
    let gc = fetch(f0 + vec2<i32>(1, 0));
    let hc = fetch(f0 + vec2<i32>(2, 0));
    let ic = fetch(f0 + vec2<i32>(-1, 1));
    let jc = fetch(f0 + vec2<i32>(0, 1));
    let kc = fetch(f0 + vec2<i32>(1, 1));
    let lc = fetch(f0 + vec2<i32>(2, 1));
    let nc = fetch(f0 + vec2<i32>(0, 2));
    let oc = fetch(f0 + vec2<i32>(1, 2));
    let bl = luma(bc); let cl = luma(cc); let el = luma(ec); let fl = luma(fc);
    let gl = luma(gc); let hl = luma(hc); let il = luma(ic); let jl = luma(jc);
    let kl = luma(kc); let ll = luma(lc); let nl = luma(nc); let ol = luma(oc);
    var dir = vec2<f32>(0.0);
    var len = 0.0;
    easu_set(&dir, &len, (1.0 - pp.x) * (1.0 - pp.y), bl, el, fl, gl, jl);
    easu_set(&dir, &len, pp.x * (1.0 - pp.y), cl, fl, gl, hl, kl);
    easu_set(&dir, &len, (1.0 - pp.x) * pp.y, fl, il, jl, kl, nl);
    easu_set(&dir, &len, pp.x * pp.y, gl, jl, kl, ll, ol);
    // Normalise the direction (none: along x).
    let dir2 = dir * dir;
    var dir_r = dir2.x + dir2.y;
    let zero = dir_r < (1.0 / 32768.0);
    dir_r = inverseSqrt(max(dir_r, 1e-12));
    if zero {
        dir_r = 1.0;
        dir.x = 1.0;
    }
    dir *= dir_r;
    // The edge's length: 0 none, 1 a clear edge; it stretches the kernel
    // along the edge and sharpens across it.
    len = len * 0.5;
    len *= len;
    let stretch = (dir.x * dir.x + dir.y * dir.y) * rcp(max(abs(dir.x), abs(dir.y)));
    let len2 = vec2<f32>(1.0 + (stretch - 1.0) * len, 1.0 - 0.5 * len);
    let lob = 0.5 + ((1.0 / 4.0 - 0.04) - 0.5) * len;
    let clp = rcp(lob);
    var ac = vec4<f32>(0.0);
    var aw = 0.0;
    easu_tap(&ac, &aw, vec2<f32>(0.0, -1.0) - pp, dir, len2, lob, clp, bc);
    easu_tap(&ac, &aw, vec2<f32>(1.0, -1.0) - pp, dir, len2, lob, clp, cc);
    easu_tap(&ac, &aw, vec2<f32>(-1.0, 1.0) - pp, dir, len2, lob, clp, ic);
    easu_tap(&ac, &aw, vec2<f32>(0.0, 1.0) - pp, dir, len2, lob, clp, jc);
    easu_tap(&ac, &aw, vec2<f32>(0.0, 0.0) - pp, dir, len2, lob, clp, fc);
    easu_tap(&ac, &aw, vec2<f32>(-1.0, 0.0) - pp, dir, len2, lob, clp, ec);
    easu_tap(&ac, &aw, vec2<f32>(1.0, 1.0) - pp, dir, len2, lob, clp, kc);
    easu_tap(&ac, &aw, vec2<f32>(2.0, 1.0) - pp, dir, len2, lob, clp, lc);
    easu_tap(&ac, &aw, vec2<f32>(2.0, 0.0) - pp, dir, len2, lob, clp, hc);
    easu_tap(&ac, &aw, vec2<f32>(1.0, 0.0) - pp, dir, len2, lob, clp, gc);
    easu_tap(&ac, &aw, vec2<f32>(1.0, 2.0) - pp, dir, len2, lob, clp, oc);
    easu_tap(&ac, &aw, vec2<f32>(0.0, 2.0) - pp, dir, len2, lob, clp, nc);
    // Deringing: held to the 2x2 around the sample.
    let mn = min(min(fc, gc), min(jc, kc));
    let mx = max(max(fc, gc), max(jc, kc));
    return min(mx, max(mn, ac / aw));
}

// RCAS's limit on the lobe (FSR_RCAS_LIMIT).
const RCAS_LIMIT: f32 = 0.25 - 1.0 / 16.0;

@fragment
fn fs_rcas(in: VsOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    //    b
    //  d e f
    //    h
    let b4 = textureLoad(src, clamp(p + vec2<i32>(0, -1), vec2<i32>(0), vec2<i32>(u.input.xy) - 1), 0);
    let d4 = textureLoad(src, clamp(p + vec2<i32>(-1, 0), vec2<i32>(0), vec2<i32>(u.input.xy) - 1), 0);
    let e4 = textureLoad(src, p, 0);
    let f4 = textureLoad(src, clamp(p + vec2<i32>(1, 0), vec2<i32>(0), vec2<i32>(u.input.xy) - 1), 0);
    let h4 = textureLoad(src, clamp(p + vec2<i32>(0, 1), vec2<i32>(0), vec2<i32>(u.input.xy) - 1), 0);
    // The lobe from the 0..1 image (High Precision Post's float scene
    // may go over 1: the limits are taken on the clipped colours).
    let b = clamp(b4.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let d = clamp(d4.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let e = clamp(e4.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let f = clamp(f4.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let h = clamp(h4.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let bl = b.b * 0.5 + (b.r * 0.5 + b.g);
    let dl = d.b * 0.5 + (d.r * 0.5 + d.g);
    let el = e.b * 0.5 + (e.r * 0.5 + e.g);
    let fl = f.b * 0.5 + (f.r * 0.5 + f.g);
    let hl = h.b * 0.5 + (h.r * 0.5 + h.g);
    // Noise detection: a lone bright or dark pixel is sharpened less.
    var nz = 0.25 * bl + 0.25 * dl + 0.25 * fl + 0.25 * hl - el;
    nz = clamp(abs(nz) * rcp(max(max(max(bl, dl), max(el, fl)), hl) - min(min(min(bl, dl), min(el, fl)), hl)), 0.0, 1.0);
    nz = -0.5 * nz + 1.0;
    let mn4 = min(min(b, d), min(f, h));
    let mx4 = max(max(b, d), max(f, h));
    let peak = vec2<f32>(1.0, -4.0);
    let hit_min = min(mn4, e) / max(4.0 * mx4, vec3<f32>(1e-8));
    let hit_max = (peak.x - max(mx4, e)) / min(4.0 * mn4 + peak.y, vec3<f32>(-1e-8));
    let lobe_rgb = max(-hit_min, hit_max);
    var lobe = max(-RCAS_LIMIT, min(max(lobe_rgb.r, max(lobe_rgb.g, lobe_rgb.b)), 0.0)) * u.params.x;
    lobe *= nz;
    let rcp_l = 1.0 / (4.0 * lobe + 1.0);
    let rgb = (lobe * b4.rgb + lobe * d4.rgb + lobe * h4.rgb + lobe * f4.rgb + e4.rgb) * rcp_l;
    return vec4<f32>(rgb, e4.a);
}

// The input's depth at the output's size (nearest input texel): the late
// sprites' depth test after the upscale, as the temporal upscaler's.
@group(0) @binding(2) var depth_ss: texture_depth_2d;
@group(0) @binding(3) var depth_ms: texture_depth_multisampled_2d;

@fragment
fn fs_depth(in: VsOut) -> @builtin(frag_depth) f32 {
    let p = vec2<i32>(floor(in.position.xy * u.input.xy * u.output.zw));
    return textureLoad(depth_ss, clamp(p, vec2<i32>(0), vec2<i32>(u.input.xy) - 1), 0);
}

@fragment
fn fs_depth_ms(in: VsOut) -> @builtin(frag_depth) f32 {
    let p = vec2<i32>(floor(in.position.xy * u.input.xy * u.output.zw));
    return textureLoad(depth_ms, clamp(p, vec2<i32>(0), vec2<i32>(u.input.xy) - 1), 0);
}

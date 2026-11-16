// ---------------------------------------------------------------------
// HDR Output (clone deviation, off by default; docs/hdr.md): the grade
// (screenfilter.efx's three passes) captured as a 32³ LUT, the strip
// LUT_N² × LUT_N bound in `rampmap`'s place (texel (r + LUT_N·b, g) is
// the native grade of the 8-bit colour (k(r), k(g), k(b)), k(i) =
// round(i · 255 / 31): the lattice sits on 8-bit codes, so the LUT gives
// the native value exactly at each of them).

const LUT_N: u32 = 32u;

// 0: trilinear interpolation (8 texels; measured closer to the native
// passes over all 2^24 colours, docs/hdr.md), 1: tetrahedral (4).
override TETRA: bool = false;

fn lattice_code(i: u32) -> f32 {
    return floor(f32(i) * 255.0 / f32(LUT_N - 1u) + 0.5);
}

// The cell and the position in it, along one axis, of code k (0..255,
// fractional between codes).
fn lut_axis(k: f32) -> vec2<f32> {
    var i = min(u32(floor(k * f32(LUT_N - 1u) / 255.0)), LUT_N - 2u);
    if lattice_code(i) > k && i > 0u {
        i -= 1u;
    } else if lattice_code(i + 1u) <= k && i < LUT_N - 2u {
        i += 1u;
    }
    let a = lattice_code(i);
    let b = lattice_code(i + 1u);
    return vec2<f32>(f32(i), clamp((k - a) / (b - a), 0.0, 1.0));
}

fn lut_at(r: u32, g: u32, b: u32) -> vec4<f32> {
    return textureLoad(rampmap, vec2<i32>(i32(r + LUT_N * b), i32(g)), 0);
}

fn grade_lut(c: vec3<f32>) -> vec4<f32> {
    let k = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)) * 255.0;
    let ar = lut_axis(k.r);
    let ag = lut_axis(k.g);
    let ab = lut_axis(k.b);
    let r = u32(ar.x);
    let g = u32(ag.x);
    let b = u32(ab.x);
    let f = vec3<f32>(ar.y, ag.y, ab.y);
    let c000 = lut_at(r, g, b);
    let c111 = lut_at(r + 1u, g + 1u, b + 1u);
    if TETRA {
        if f.x >= f.y {
            if f.y >= f.z {
                return c000 + f.x * (lut_at(r + 1u, g, b) - c000) + f.y * (lut_at(r + 1u, g + 1u, b) - lut_at(r + 1u, g, b)) + f.z * (c111 - lut_at(r + 1u, g + 1u, b));
            } else if f.x >= f.z {
                return c000 + f.x * (lut_at(r + 1u, g, b) - c000) + f.z * (lut_at(r + 1u, g, b + 1u) - lut_at(r + 1u, g, b)) + f.y * (c111 - lut_at(r + 1u, g, b + 1u));
            } else {
                return c000 + f.z * (lut_at(r, g, b + 1u) - c000) + f.x * (lut_at(r + 1u, g, b + 1u) - lut_at(r, g, b + 1u)) + f.y * (c111 - lut_at(r + 1u, g, b + 1u));
            }
        } else {
            if f.z >= f.y {
                return c000 + f.z * (lut_at(r, g, b + 1u) - c000) + f.y * (lut_at(r, g + 1u, b + 1u) - lut_at(r, g, b + 1u)) + f.x * (c111 - lut_at(r, g + 1u, b + 1u));
            } else if f.z >= f.x {
                return c000 + f.y * (lut_at(r, g + 1u, b) - c000) + f.z * (lut_at(r, g + 1u, b + 1u) - lut_at(r, g + 1u, b)) + f.x * (c111 - lut_at(r, g + 1u, b + 1u));
            } else {
                return c000 + f.y * (lut_at(r, g + 1u, b) - c000) + f.x * (lut_at(r + 1u, g + 1u, b) - lut_at(r, g + 1u, b)) + f.z * (c111 - lut_at(r + 1u, g + 1u, b));
            }
        }
    }
    let x00 = mix(c000, lut_at(r + 1u, g, b), f.x);
    let x10 = mix(lut_at(r, g + 1u, b), lut_at(r + 1u, g + 1u, b), f.x);
    let x01 = mix(lut_at(r, g, b + 1u), lut_at(r + 1u, g, b + 1u), f.x);
    let x11 = mix(lut_at(r, g + 1u, b + 1u), c111, f.x);
    return mix(mix(x00, x10, f.y), mix(x01, x11, f.y), f.z);
}

// The grade through the LUT on an 8-bit image (the verification).
@fragment
fn fs_grade_lut_check(in: VsOut) -> @location(0) vec4<f32> {
    return grade_lut(tap(in.uv).rgb);
}

// HDR Output's grade: the scene (half float, may exceed 1) through the
// LUT; above white the colour's hue (t / m) is graded and scaled back by m,
// so highlights keep their headroom for the display transform and every
// colour up to white gets the native grade.
@fragment
fn fs_grade_lut(in: VsOut) -> @location(0) vec4<f32> {
    let t = max(tap(in.uv), vec4<f32>(0.0));
    let m = max(t.r, max(t.g, t.b));
    if m > 1.0 {
        let g = grade_lut(t.rgb / m);
        return vec4<f32>(g.rgb * m, g.a);
    }
    return grade_lut(t.rgb);
}

// The last pass with HDR Output: the SDR image as High Precision Post
// writes it (location 0) and the unclipped image (location 1) for the
// display transform.
struct HdrOut {
    @location(0) sdr: vec4<f32>,
    @location(1) unclipped: vec4<f32>,
}

fn hdr_out(t: vec4<f32>, position: vec4<f32>, a0: bool) -> HdrOut {
    var out: HdrOut;
    out.sdr = to_output(t, position);
    if a0 {
        out.sdr.a = 0.0;
    }
    out.unclipped = vec4<f32>(max(t.rgb, vec3<f32>(0.0)), 1.0);
    return out;
}

@fragment
fn fs_out_hdr(in: VsOut) -> HdrOut {
    return hdr_out(filter_taps(in.uv), in.position, false);
}

@fragment
fn fs_out_hdr_a0(in: VsOut) -> HdrOut {
    return hdr_out(filter_taps(in.uv), in.position, true);
}

@fragment
fn fs_out_add_hdr(in: VsOut) -> HdrOut {
    return hdr_out(tap(in.uv) + textureSampleLevel(gainmap, samp, in.uv, 0.0), in.position, false);
}

@fragment
fn fs_out_add_hdr_a0(in: VsOut) -> HdrOut {
    return hdr_out(tap(in.uv) + textureSampleLevel(gainmap, samp, in.uv, 0.0), in.position, true);
}

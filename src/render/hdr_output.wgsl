// HDR Output's display transform (clone deviation, docs/hdr.md; see
// hdr_output.rs). Reads the finished SDR frame (the native image with the
// UI over it) and the post passes' unclipped image, writes the HDR
// swap chain: BT.2100 PQ (HDR10) or scRGB (extended linear BT.709).

struct Params {
    // x: paper white (nits), y: peak (nits), z: signal (0 PQ, 1 scRGB),
    // w: the SDR transfer (0 gamma 2.2, 1 sRGB piecewise, 2 gamma 2.4).
    a: vec4<f32>,
    // x: the calibration pattern (1 on), y: the unclipped image is this
    // frame's (1), zw: the output's size in pixels.
    b: vec4<f32>,
    // The calibration pattern's rectangle, fractions of the output: xy its
    // top left, zw its bottom right (the calibration page's free rows,
    // placed on the CPU from the menu's layout).
    c: vec4<f32>,
}

@group(0) @binding(0) var sdr: texture_2d<f32>;
@group(0) @binding(1) var unclipped: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var<uniform> p: Params;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.position = vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(q.x, 1.0 - q.y);
    return out;
}

// The SDR signal to relative light (1 = paper white).
fn eotf1(v: f32) -> f32 {
    let x = max(v, 0.0);
    if p.a.w > 1.5 {
        return pow(x, 2.4);
    }
    if p.a.w > 0.5 {
        return select(pow((x + 0.055) / 1.055, 2.4), x / 12.92, x <= 0.04045);
    }
    return pow(x, 2.2);
}

fn eotf(v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(eotf1(v.r), eotf1(v.g), eotf1(v.b));
}

// The extended transfer's exponent above white (the power law of the
// chosen transfer; sRGB's top segment is ~2.2 near 1).
fn exponent() -> f32 {
    return select(2.2, 2.4, p.a.w > 1.5);
}

// Highlights: r ≥ 1 (light relative to paper white the unclipped value
// asks for) to [1, h): slope 1 at white, approaching the headroom h =
// peak / paper white (hdr_output.rs `shoulder`).
fn shoulder(r: f32, h: f32) -> f32 {
    if h <= 1.0 {
        return 1.0;
    }
    return 1.0 + (h - 1.0) * (1.0 - exp(-(r - 1.0) / (h - 1.0)));
}

const BT709_TO_BT2020: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>(0.6274039, 0.0690973, 0.0163914),
    vec3<f32>(0.3292830, 0.9195404, 0.0880133),
    vec3<f32>(0.0433131, 0.0113623, 0.8955953),
);

fn pq(nits: vec3<f32>) -> vec3<f32> {
    let m1 = 0.1593017578125;
    let m2 = 78.84375;
    let c1 = 0.8359375;
    let c2 = 18.8515625;
    let c3 = 18.6875;
    let y = pow(clamp(nits / 10000.0, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(m1));
    return pow((c1 + c2 * y) / (1.0 + c3 * y), vec3<f32>(m2));
}

// The calibration pattern (the Display page is the calibration screen),
// in nits; a < 0 outside it. It fills the rectangle `p.c` (the page's
// free rows under its last row, inside the menu: never over a row or the
// help text), left to right in quarters:
// * peak: a 10 000-nit box holding a square at the chosen peak; at the
//   display's real peak the inner square disappears;
// * paper white: a patch at the chosen paper white beside one at half;
// * black: steps of SDR codes 0, 1, 2, 3, 4, 6, 8, 12 (of 255) through the
//   chosen transfer (step 0 is black; 1-2 just visible at the right gamma);
// * gamma: lines alternating black and paper white beside a patch at the
//   SDR code whose light is their average under the chosen transfer
//   (equal at the right transfer, seen from a distance).
// The peak box and its square are square in pixels (sized by the band's
// height), whatever the band's shape.
fn pattern(uv: vec2<f32>, px: vec2<f32>) -> vec4<f32> {
    let lo = p.c.xy;
    let hi = p.c.zw;
    if any(uv < lo) || any(uv > hi) {
        return vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    let paper = p.a.x;
    let peak = p.a.y;
    let q = (uv - lo) / (hi - lo);
    // The band's width over its height, in pixels.
    let band = (hi - lo) * p.b.zw;
    let wide = band.x / max(band.y, 1.0);
    let black = vec4<f32>(vec3<f32>(0.0), 1.0);
    let inside_y = q.y > 0.15 && q.y < 0.85;
    // Peak (in band heights from the quarter's centre).
    if q.x < 0.25 {
        let d = abs(vec2<f32>((q.x - 0.125) * wide, q.y - 0.5));
        let outer = min(0.35, 0.1 * wide);
        if max(d.x, d.y) < outer * 0.43 {
            return vec4<f32>(vec3<f32>(peak), 1.0);
        }
        if max(d.x, d.y) < outer {
            return vec4<f32>(vec3<f32>(10000.0), 1.0);
        }
        return black;
    }
    // Paper white and half of it.
    if q.x < 0.5 {
        if inside_y && q.x > 0.27 && q.x < 0.37 {
            return vec4<f32>(vec3<f32>(paper), 1.0);
        }
        if inside_y && q.x > 0.39 && q.x < 0.48 {
            return vec4<f32>(vec3<f32>(paper * 0.5), 1.0);
        }
        return black;
    }
    // Black steps.
    if q.x < 0.75 {
        let codes = array<f32, 8>(0.0, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0);
        let t = (q.x - 0.52) / 0.21;
        if t < 0.0 || t >= 1.0 || q.y < 0.25 || q.y > 0.75 {
            return black;
        }
        let i = min(u32(t * 8.0), 7u);
        let inner = fract(t * 8.0);
        if inner < 0.12 || inner > 0.88 {
            return black;
        }
        return vec4<f32>(vec3<f32>(eotf1(codes[i] / 255.0) * paper), 1.0);
    }
    // Gamma: alternating lines, then the code of their average light.
    if inside_y && q.x > 0.77 && q.x < 0.86 {
        let on = (u32(px.y) & 1u) == 0u;
        return vec4<f32>(vec3<f32>(select(0.0, paper, on)), 1.0);
    }
    if inside_y && q.x > 0.87 && q.x < 0.96 {
        let e = 1.0 / exponent();
        let code = round(pow(0.5, e) * 255.0) / 255.0;
        return vec4<f32>(vec3<f32>(eotf1(code) * paper), 1.0);
    }
    return black;
}

fn encode(nits: vec3<f32>) -> vec4<f32> {
    if p.a.z > 0.5 {
        // scRGB: 1.0 = 80 nits, BT.709 primaries, linear.
        return vec4<f32>(nits / 80.0, 1.0);
    }
    return vec4<f32>(pq(max(BT709_TO_BT2020 * nits, vec3<f32>(0.0))), 1.0);
}

// The light (nits, BT.709) of one output pixel.
fn light(uv: vec2<f32>) -> vec3<f32> {
    let paper = p.a.x;
    let s = textureSampleLevel(sdr, samp, uv, 0.0).rgb;
    var lin = eotf(s) * paper;
    if p.b.y > 0.5 {
        let t = textureSampleLevel(unclipped, samp, uv, 0.0).rgb;
        let m = max(t.r, max(t.g, t.b));
        if m > 1.0 {
            // Only where the frame still shows the post passes' image (no
            // UI drawn over it): within the dither's step and a little.
            let d = abs(s - clamp(t, vec3<f32>(0.0), vec3<f32>(1.0)));
            let w = 1.0 - smoothstep(3.0 / 255.0, 8.0 / 255.0, max(d.r, max(d.g, d.b)));
            let o = shoulder(pow(m, exponent()), p.a.y / paper);
            lin = mix(lin, eotf(t / m) * o * paper, w);
        }
    }
    return lin;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if p.b.x > 0.5 {
        let c = pattern(in.uv, in.position.xy);
        if c.a >= 0.0 {
            return encode(c.rgb);
        }
    }
    return encode(light(in.uv));
}

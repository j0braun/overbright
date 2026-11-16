// Frozen reference from 0e7a31b; preserves UV rounding and weighted edges.
// The Graphics page's screen passes (clone deviations, every one opt-in;
// see enhance.rs): FXAA on the scene's image, its resample to the screen
// (Render Scale), and contrast-adaptive sharpening. None of them runs with
// the page at its native values.

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

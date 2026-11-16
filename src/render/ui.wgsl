// Front-end quads: canvas-space corners already in clip space, a texture
// (or a white texel for untextured images), a tint.

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) clip: vec3<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>, @location(3) clip: vec3<f32>) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    out.clip = clip;
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

// HD UI Art (clone, opt-in; `UiRenderer::record`): sharp bilinear. Per
// axis, k = the whole number of target pixels per texel (at least 1); the
// texture is magnified as if first enlarged k times by nearest neighbour
// and that then filtered bilinearly to the target: inside a texel its own
// colour, between two texels a linear step 1/k texel (1 to 2 target
// pixels) wide. Wraps like the sampler, from level 0 (only ever drawn
// magnified). Premultiplied: colour weighted by alpha, back to straight.
fn sharp(uv: vec2<f32>, ppt: vec2<f32>, premultiplied: bool) -> vec4<f32> {
    let n = vec2<i32>(textureDimensions(tex, 0));
    let x = uv * vec2<f32>(n);
    let b = floor(x + 0.5);
    let k = max(floor(ppt + 1e-3), vec2<f32>(1.0));
    let w = clamp((x - b) * k + 0.5, vec2<f32>(0.0), vec2<f32>(1.0));
    let i0 = vec2<i32>(b) - 1;
    var sum = vec4<f32>(0.0);
    for (var j = 0; j < 4; j++) {
        let o = vec2<i32>(j & 1, j >> 1u);
        let p = ((i0 + o) % n + n) % n;
        var c = textureLoad(tex, p, 0);
        if (premultiplied) {
            c = vec4<f32>(c.rgb * c.a, c.a);
        }
        let ww = select(1.0 - w, w, vec2<bool>(o == vec2<i32>(1)));
        sum += c * ww.x * ww.y;
    }
    if (premultiplied) {
        sum = vec4<f32>(select(vec3<f32>(0.0), sum.rgb / sum.a, sum.a > 0.0), sum.a);
    }
    return sum;
}

#include "enh/ui/palette.wgsl"
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // fe.efx's sampling (bilinear, wrap, mipmaps), what the game does.
    var t = textureSample(tex, samp, in.uv);
    // Target pixels per texel on each axis (for HD UI Art).
    let x = in.uv * vec2<f32>(textureDimensions(tex, 0));
    let ppt = 1.0 / max(vec2<f32>(length(vec2<f32>(dpdx(x.x), dpdy(x.x))), length(vec2<f32>(dpdx(x.y), dpdy(x.y)))), vec2<f32>(1e-6));
    var mode = in.clip.z;
    if (mode > 3.5) {
        let premultiplied = mode > 7.5;
        mode -= select(4.0, 8.0, premultiplied);
        if (min(ppt.x, ppt.y) > 1.0) {
            t = sharp(in.uv, ppt, premultiplied);
        }
    }
    // The texture's alpha test (entry byte 0x55): ALPHAFUNC GREATER,
    // ALPHAREF 95, on fe.efx's output alpha (texture × vertex).
    if (mode > 1.5) {
        let o = t * in.color;
        if (o.a * 255.0 <= 95.0) {
            discard;
        }
        return o;
    }
    if (mode > 0.5) {
        // Destination-alpha texture inside the mask's circle: opaque there.
        let r = length(in.clip.xy);
        let edge = clamp((1.0 - r) * 40.0, 0.0, 1.0);
        return vec4<f32>(t.rgb * in.color.rgb, in.color.a * edge);
    }
    return t * in.color;
}
#include "enh/ui/palette_fs.wgsl"

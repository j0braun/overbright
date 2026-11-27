// ---- full ----
// The downsample (the scene depth to a reduced-resolution nearest view
// depth) and the composite (the OIT pair, upsampled with depth-aware
// weights, over the scene). DEPTH_TEXTURE is texture_depth_2d or its
// multisampled form (sample 0).

struct Full {
    // near, far, reversed (0/1), divisor
    depth: vec4<f32>,
    // low-res size, full-res size
    sizes: vec4<f32>,
    // debug mode (NFSU2_SMOKE2_DEBUG)
    debug: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Full;
@group(0) @binding(1) var scene_depth: DEPTH_TEXTURE;
@group(0) @binding(2) var lo_depth: texture_2d<f32>;
@group(0) @binding(3) var accum_tex: texture_2d<f32>;
@group(0) @binding(4) var reveal_tex: texture_2d<f32>;
@group(0) @binding(5) var reveal_samp: sampler;

struct FsIn {
    @builtin(position) pos: vec4<f32>,
}

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> FsIn {
    var o: FsIn;
    let p = vec2(f32((vi << 1u) & 2u), f32(vi & 2u));
    o.pos = vec4(p * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

fn lin(d: f32) -> f32 {
    let n = u.depth.x;
    let fa = u.depth.y;
    if (u.depth.z > 0.5) {
        return n * fa / (n + d * (fa - n));
    }
    return n * fa / (fa - d * (fa - n));
}

fn full_depth(p: vec2<i32>) -> f32 {
    let c = clamp(p, vec2(0), vec2<i32>(u.sizes.zw) - 1);
    return lin(textureLoad(scene_depth, c, 0));
}

// The nearest view depth over the block a low-res pixel covers.
@fragment
fn fs_downsample(in: FsIn) -> @location(0) vec4<f32> {
    // The block's four corners (all of it at 2×).
    let div = i32(u.depth.w);
    let base = vec2<i32>(in.pos.xy) * div;
    let e = div - 1;
    let z = min(min(full_depth(base), full_depth(base + vec2(e, 0))), min(full_depth(base + vec2(0, e)), full_depth(base + vec2(e, e))));
    return vec4(z, 0.0, 0.0, 0.0);
}

// The bilateral weight of a low-res tap at view depth `zl` for a pixel at
// `zf` (smoke_cinematic.rs `bilateral`).
fn bilateral(zl: f32, zf: f32) -> f32 {
    let r = abs(zl - zf) / (0.02 * zf + 0.05);
    return 1.0 / (1.0 + r * r) + 1e-4;
}

@fragment
fn fs_composite(in: FsIn) -> @location(0) vec4<f32> {
    // Most of the screen has no smoke: one filtered fetch of the
    // revealage decides (its four texels all clear), before any depth.
    if (u.debug.x != 1.0 && textureSampleLevel(reveal_tex, reveal_samp, in.pos.xy / (u.depth.w * u.sizes.xy), 0.0).r >= 0.999) {
        discard;
    }
    let zf = full_depth(vec2<i32>(in.pos.xy));
    let lp = in.pos.xy / u.depth.w - 0.5;
    let i0 = vec2<i32>(floor(lp));
    let fr = lp - floor(lp);
    let hi = vec2<i32>(u.sizes.xy) - 1;
    var acc = vec4(0.0);
    var rev = 0.0;
    var wsum = 0.0;
    for (var k = 0; k < 4; k++) {
        let o = vec2(k & 1, k >> 1u);
        let p = clamp(i0 + o, vec2(0), hi);
        let bw = select(1.0 - fr.x, fr.x, o.x == 1) * select(1.0 - fr.y, fr.y, o.y == 1);
        let w = bw * bilateral(textureLoad(lo_depth, p, 0).r, zf);
        acc += textureLoad(accum_tex, p, 0) * w;
        rev += textureLoad(reveal_tex, p, 0).r * w;
        wsum += w;
    }
    acc /= max(wsum, 1e-6);
    if (u.debug.x == 1.0) {
        return vec4(fract(zf / 10.0), fract(textureLoad(lo_depth, clamp(i0, vec2(0), hi), 0).r / 10.0), 1.0 - rev / max(wsum, 1e-6), 1.0);
    }
    let alpha = clamp(1.0 - rev / max(wsum, 1e-6), 0.0, 1.0);
    if (alpha <= 0.002) {
        discard;
    }
    let rgb = acc.rgb / max(acc.a, 1e-5);
    return vec4(rgb * alpha, alpha);
}

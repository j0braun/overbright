// The nearest view depth over the block a low-res pixel covers (its four
// corners). DEPTH_TEXTURE is texture_depth_2d or its multisampled form
// (sample 0).
@group(0) @binding(0) var<uniform> cv: Vol;
@group(0) @binding(1) var scene_depth: DEPTH_TEXTURE;
@group(0) @binding(2) var clo_depth: texture_2d<f32>;
@group(0) @binding(3) var smoke_tex: texture_2d<f32>;
@group(0) @binding(4) var smoke_samp: sampler;
@group(0) @binding(5) var<storage, read> cbounds: array<u32>;
@group(0) @binding(6) var<storage, read> ctiles: array<u32>;

// The composite over the tiles the march flagged (one instance a tile,
// drawn when it or a neighbour holds smoke: the bilateral taps reach one
// low-res pixel past a tile; the others are degenerate quads). Tiles do
// not overlap (no pixel blended twice).
@vertex
fn vs_tiles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> FsIn {
    var o: FsIn;
    o.pos = vec4(-2.0, -2.0, 0.0, 1.0);
    let nx = i32((u32(cv.sizes.x) + 15u) / 16u);
    let ny = i32((u32(cv.sizes.y) + 15u) / 16u);
    let ti = vec2(i32(ii) % nx, i32(ii) / nx);
    var any_ = 0u;
    for (var k = 0; k < 9; k++) {
        let q = ti + vec2(k % 3 - 1, k / 3 - 1);
        if (all(q >= vec2(0)) && q.x < nx && q.y < ny) {
            any_ |= ctiles[q.y * nx + q.x];
        }
    }
    if (any_ == 0u) {
        return o;
    }
    let t = vec2<f32>(ti);
    let div = cv.depth.w;
    let size = cv.sizes.zw;
    let p0 = clamp(t * 16.0 * div, vec2(0.0), size);
    let p1 = clamp((t + 1.0) * 16.0 * div, vec2(0.0), size);
    let corner = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0))[vi];
    let px = mix(p0, p1, corner);
    o.pos = vec4(px.x / size.x * 2.0 - 1.0, 1.0 - px.y / size.y * 2.0, 0.0, 1.0);
    return o;
}

fn lin(d: f32) -> f32 {
    let n = cv.depth.x;
    let fa = cv.depth.y;
    if (cv.depth.z > 0.5) {
        return n * fa / (n + d * (fa - n));
    }
    return n * fa / (fa - d * (fa - n));
}

fn full_depth(p: vec2<i32>) -> f32 {
    return lin(textureLoad(scene_depth, clamp(p, vec2(0), vec2<i32>(cv.sizes.zw) - 1), 0));
}

// ---- composite ----

// The bilateral weight of a low-res tap at view depth `zl` for a pixel at
// `zf` (volume_fx.rs `bilateral`).
fn bilateral(zl: f32, zf: f32) -> f32 {
    let r = abs(zl - zf) / (0.02 * zf + 0.05);
    return 1.0 / (1.0 + r * r) + 1e-4;
}

@fragment
fn fs_composite(in: FsIn) -> @location(0) vec4<f32> {
    let div = cv.depth.w;
    let zf = full_depth(vec2<i32>(in.pos.xy));
    let lp = in.pos.xy / div - 0.5;
    let i0 = vec2<i32>(floor(lp));
    let fr = lp - floor(lp);
    let hi = vec2<i32>(cv.sizes.xy) - 1;
    var acc = vec4(0.0);
    var sh = 0.0;
    var wsum = 0.0;
    for (var k = 0; k < 4; k++) {
        let o = vec2(k & 1, k >> 1u);
        let p = clamp(i0 + o, vec2(0), hi);
        let bw = select(1.0 - fr.x, fr.x, o.x == 1) * select(1.0 - fr.y, fr.y, o.y == 1);
        let lo = textureLoad(clo_depth, p, 0);
        let w = bw * bilateral(lo.r, zf);
        acc += textureLoad(smoke_tex, p, 0) * w;
        sh += lo.g * w;
        wsum += w;
    }
    acc /= max(wsum, 1e-6);
    // The medium's shadow (the march's g: the light it blocks, 0
    // where it wrote nothing) darkens the scene under the smoke: the
    // scene × (1 − a) · s.
    sh = 1.0 - clamp(sh / max(wsum, 1e-6), 0.0, 1.0);
    let a = clamp(acc.a, 0.0, 1.0);
    if (a <= 0.002 && sh > 0.998) {
        discard;
    }
    return vec4(acc.rgb, 1.0 - (1.0 - a) * sh);
}

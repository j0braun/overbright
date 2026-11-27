// For speed (docs/notes/performance/gpu.md; Particles High / Ultra, a clone deviation): the
// detailed smoke drawn at half size (gpu_particles.wgsl `fs_half`) put
// over the scene. Per pixel the four half-size texels around it, weighted
// bilinearly and by how close the depth each texel's puffs were softened
// against is to the pixel's own (so smoke behind a car does not bleed
// onto its edge, nor the car's edge cut a hole into the smoke), give the
// colour C, the transmittance T and the alpha sum A; the blend makes
// scene · T + C (dual source: T in source 1) and alpha · T + A, the
// native "over" of all the puffs at once.
enable dual_source_blending;

struct Map {
    // the depth texel under a half-size texel centre: (t + 0.5) · xy + zw
    depth_map: vec4<f32>,
    // half-size extent (texels), near, far
    half: vec4<f32>,
    // reversed depth (0/1)
    flags: vec4<f32>,
}

@group(0) @binding(0) var colour_tex: texture_2d<f32>;
@group(0) @binding(1) var alpha_tex: texture_2d<f32>;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var<uniform> m: Map;

struct Out {
    @location(0) @blend_src(0) colour: vec4<f32>,
    @location(0) @blend_src(1) keep: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

fn linear_depth(d: f32) -> f32 {
    let n = m.half.z;
    let fa = m.half.w;
    if (m.flags.x > 0.5) {
        return n * fa / (n + d * (fa - n));
    }
    return n * fa / (fa - d * (fa - n));
}

fn depth_at(px: vec2<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(depth_tex));
    return linear_depth(textureLoad(depth_tex, clamp(vec2<i32>(px), vec2(0), dims - 1), 0));
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> Out {
    // The pixel in half-size texels (texel t's centre at t + 0.5).
    let h = (pos.xy - m.depth_map.zw) / m.depth_map.xy - 0.5;
    let base = floor(h);
    let fr = h - base;
    let last = vec2<i32>(m.half.xy) - 1;
    // No puff over any of the four texels (colour 0, transmittance 1,
    // alpha sum 0): the blend would leave the scene as it is.
    var any_smoke = false;
    for (var k = 0; k < 4; k++) {
        let t = clamp(vec2<i32>(base + vec2<f32>(f32(k & 1), f32(k >> 1))), vec2(0), last);
        let tc = textureLoad(colour_tex, t, 0);
        any_smoke = any_smoke || any(tc != vec4<f32>(0.0, 0.0, 0.0, 1.0)) || textureLoad(alpha_tex, t, 0).r != 0.0;
    }
    if !any_smoke {
        discard;
    }
    let z = depth_at(pos.xy);
    var c = vec4<f32>(0.0);
    var a = 0.0;
    var total = 0.0;
    var plain_c = vec4<f32>(0.0);
    var plain_a = 0.0;
    for (var k = 0; k < 4; k++) {
        let o = vec2<f32>(f32(k & 1), f32(k >> 1));
        let t = clamp(vec2<i32>(base + o), vec2(0), last);
        let wb = select(1.0 - fr.x, fr.x, o.x > 0.5) * select(1.0 - fr.y, fr.y, o.y > 0.5);
        let tz = depth_at((vec2<f32>(t) + 0.5) * m.depth_map.xy + m.depth_map.zw);
        // Relative depth gap; 1 within 1 %, falling off past it.
        let gap = abs(tz - z) / max(z, 1e-3);
        let wd = 1.0 / (1.0 + (gap * 100.0) * (gap * 100.0));
        let w = wb * wd;
        let tc = textureLoad(colour_tex, t, 0);
        let ta = textureLoad(alpha_tex, t, 0).r;
        c += tc * w;
        a += ta * w;
        total += w;
        plain_c += tc * wb;
        plain_a += ta * wb;
    }
    // No texel at the pixel's depth (a thin object): bilinear.
    if total < 1e-4 {
        c = plain_c;
        a = plain_a;
    } else {
        c /= total;
        a /= total;
    }
    var out: Out;
    out.colour = vec4<f32>(c.rgb, a);
    out.keep = vec4<f32>(c.a);
    return out;
}

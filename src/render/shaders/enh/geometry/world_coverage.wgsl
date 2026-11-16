// Alpha-to-coverage (the Graphics page's MSAA, a clone deviation; not
// world.efx): the alpha test's edge (ALPHA_REF) spread over the alpha's
// change across one pixel and given as that share of the pixel's SAMPLES
// samples, rotated per pixel so neighbours cover different ones. The
// colour and its alpha (the bleach bypass mask) are written as the alpha
// test writes them; only which samples take them changes.
override SAMPLES: f32 = 1.0;

struct CoverageOut {
    @location(0) colour: vec4<f32>,
    @builtin(sample_mask) mask: u32,
};

fn coverage_mask(a: f32, frag: vec2<f32>) -> u32 {
    let n = u32(SAMPLES);
    let w = max(fwidth(a), 1.0 / 255.0);
    let share = clamp((a - ALPHA_REF) / w + 0.5, 0.0, 1.0);
    let k = min(u32(share * SAMPLES + 0.5), n);
    let full = (1u << n) - 1u;
    let m = (1u << k) - 1u;
    let r = (u32(frag.x) * 3u + u32(frag.y) * 5u) % n;
    return ((m << r) | (m >> (n - r))) & full;
}

fn covered(c: vec4<f32>, a: f32, frag: vec4<f32>) -> CoverageOut {
    let mask = coverage_mask(a, frag.xy);
    if mask == 0u {
        discard;
    }
    return CoverageOut(c, mask);
}

@fragment
#if !MULTIVIEW
fn fs_world_coverage(in: VertexOut) -> CoverageOut {
#else
fn fs_world_coverage(in: VertexOut, @builtin(view_index) view_index: u32) -> CoverageOut {
    frame = frames[view_index];
#endif
    let c = world_colour(in, false, true);
    return covered(c, c.a, in.clip);
}

@fragment
#if !MULTIVIEW
fn fs_nofog_coverage(in: VertexOut) -> CoverageOut {
#else
fn fs_nofog_coverage(in: VertexOut, @builtin(view_index) view_index: u32) -> CoverageOut {
    frame = frames[view_index];
#endif
    let c = world_colour(in, false, false);
    return covered(c, c.a, in.clip);
}

@fragment
#if !MULTIVIEW
fn fs_reflect_coverage(in: VertexOut) -> CoverageOut {
#else
fn fs_reflect_coverage(in: VertexOut, @builtin(view_index) view_index: u32) -> CoverageOut {
    frame = frames[view_index];
#endif
    let r0 = sample_tex(in.uv) * in.color;
    return covered(reflect_colour(in, false), r0.a, in.clip);
}

// LOD Fade with MSAA's alpha-to-coverage (both clone deviations): the
// screen-door dither's samples ANDed with the alpha edge's.
fn covered_fade(c: vec4<f32>, a: f32, frag: vec4<f32>, fade: f32) -> CoverageOut {
    let mask = coverage_mask(a, frag.xy) & fade_mask(frag, fade);
    if mask == 0u {
        discard;
    }
    return CoverageOut(c, mask);
}

@fragment
#if !MULTIVIEW
fn fs_world_coverage_fade(in: VertexOutFade) -> CoverageOut {
#else
fn fs_world_coverage_fade(in: VertexOutFade, @builtin(view_index) view_index: u32) -> CoverageOut {
    frame = frames[view_index];
#endif
    let c = world_colour(without_fade(in), false, true);
    return covered_fade(c, c.a, in.clip, in.fade);
}

@fragment
#if !MULTIVIEW
fn fs_nofog_coverage_fade(in: VertexOutFade) -> CoverageOut {
#else
fn fs_nofog_coverage_fade(in: VertexOutFade, @builtin(view_index) view_index: u32) -> CoverageOut {
    frame = frames[view_index];
#endif
    let c = world_colour(without_fade(in), false, false);
    return covered_fade(c, c.a, in.clip, in.fade);
}

@fragment
#if !MULTIVIEW
fn fs_reflect_coverage_fade(in: VertexOutFade) -> CoverageOut {
#else
fn fs_reflect_coverage_fade(in: VertexOutFade, @builtin(view_index) view_index: u32) -> CoverageOut {
    frame = frames[view_index];
#endif
    let r0 = sample_tex(in.uv) * in.color;
    return covered_fade(reflect_colour(without_fade(in), false), r0.a, in.clip, in.fade);
}

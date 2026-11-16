// LOD Fade (clone deviation, the Graphics page; docs/formats.md
// "Graphics enhancements"): the main view's draws with a per-instance fade
// (location 7, from `WorldRenderer`'s fade buffer). In that mode the frame's
// size need (`cull.y`) is 0 and the CPU decides which instances are drawn
// (with a hysteresis band); `cull.w` is the scene's sample count. A fade f
// in (0, 1) keeps the samples whose screen-door threshold is below f, a
// fade −g keeps those at or above 1 − g (the complement of g's), 1 keeps
// all, 0 none (the vertex shader drops the instance). The fade is given as
// a sample mask, so with MSAA the dither is per sample (alpha to coverage's
// effect without touching the alpha, which the destination-alpha blends
// and the bleach bypass mask read).
struct VertexOutFade {
#if !INVARIANT_POSITION
    @builtin(position) clip: vec4<f32>,
#else
    @builtin(position) @invariant clip: vec4<f32>,
#endif
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) fog: f32,
    @location(3) screen: vec4<f32>,
    @location(4) hl_tex: vec4<f32>,
    @location(5) hl_clip: f32,
    @location(6) @interpolate(flat) lit: f32,
    @location(8) world_position: vec3<f32>,
    @location(7) @interpolate(flat) fade: f32,
};

struct FragmentFade {
    @location(0) color: vec4<f32>,
    @builtin(sample_mask) mask: u32,
};

fn with_fade(o: VertexOut, v: VertexIn, fade: f32) -> VertexOutFade {
    var clip = o.clip;
    // An instance the fade has gone out on (scenery below its size, a
    // section gone) is not drawn.
    if fade == 0.0 {
        clip = vec4<f32>(0.0);
    }
    return VertexOutFade(clip, o.color, o.uv, o.fog, o.screen, o.hl_tex, o.hl_clip, o.lit, o.world_position, fade);
}

fn without_fade(in: VertexOutFade) -> VertexOut {
    return VertexOut(in.clip, in.color, in.uv, in.fog, in.screen, in.hl_tex, in.hl_clip, in.lit, in.world_position);
}

// A 4 × 4 Bayer matrix's thresholds, (0.5 … 15.5) / 16.
fn bayer4(p: vec2<f32>) -> f32 {
    let x = u32(p.x) & 3u;
    let y = u32(p.y) & 3u;
    let m = array<u32, 16>(0u, 8u, 2u, 10u, 12u, 4u, 14u, 6u, 3u, 11u, 1u, 9u, 15u, 7u, 13u, 5u);
    return (f32(m[y * 4u + x]) + 0.5) / 16.0;
}

fn fade_mask(p: vec4<f32>, fade: f32) -> u32 {
    if fade >= 1.0 {
        return 0xFFFFFFFFu;
    }
    let n = max(u32(frame.cull.w), 1u);
    let b = bayer4(p.xy);
    var mask = 0u;
    for (var s = 0u; s < n; s = s + 1u) {
        let t = fract(b + f32(s) / f32(n));
        let keep = select((t >= 1.0 + fade), (t < fade), (fade >= 0.0));
        if keep {
            mask = mask | (1u << s);
        }
    }
    return mask;
}

// The instance's fade (a struct, so the multiview form's rewrite of the
// entry points' parameter lists sees no parenthesis inside them).
struct FadeIn {
    @location(7) fade: f32,
};

@vertex
#if !MULTIVIEW
fn vs_main_fade(v: VertexIn, f: FadeIn) -> VertexOutFade {
#else
fn vs_main_fade(v: VertexIn, f: FadeIn, @builtin(view_index) view_index: u32) -> VertexOutFade {
    frame = frames[view_index];
#endif
    var out = vs_common(v);
    out.uv = v.uv + texture_offset(frame.clip.y);
    return with_fade(out, v, f.fade);
}

@vertex
#if !MULTIVIEW
fn vs_reflect_fade(v: VertexIn, f: FadeIn) -> VertexOutFade {
#else
fn vs_reflect_fade(v: VertexIn, f: FadeIn, @builtin(view_index) view_index: u32) -> VertexOutFade {
    frame = frames[view_index];
#endif
    return with_fade(vs_common(v), v, f.fade);
}

@fragment
#if !MULTIVIEW
fn fs_world_fade(in: VertexOutFade) -> FragmentFade {
#else
fn fs_world_fade(in: VertexOutFade, @builtin(view_index) view_index: u32) -> FragmentFade {
    frame = frames[view_index];
#endif
    return FragmentFade(world_colour(without_fade(in), false, true), fade_mask(in.clip, in.fade));
}

@fragment
#if !MULTIVIEW
fn fs_world_alpha_test_fade(in: VertexOutFade) -> FragmentFade {
#else
fn fs_world_alpha_test_fade(in: VertexOutFade, @builtin(view_index) view_index: u32) -> FragmentFade {
    frame = frames[view_index];
#endif
    return FragmentFade(world_colour(without_fade(in), true, true), fade_mask(in.clip, in.fade));
}

@fragment
#if !MULTIVIEW
fn fs_nofog_fade(in: VertexOutFade) -> FragmentFade {
#else
fn fs_nofog_fade(in: VertexOutFade, @builtin(view_index) view_index: u32) -> FragmentFade {
    frame = frames[view_index];
#endif
    return FragmentFade(world_colour(without_fade(in), false, false), fade_mask(in.clip, in.fade));
}

@fragment
#if !MULTIVIEW
fn fs_nofog_alpha_test_fade(in: VertexOutFade) -> FragmentFade {
#else
fn fs_nofog_alpha_test_fade(in: VertexOutFade, @builtin(view_index) view_index: u32) -> FragmentFade {
    frame = frames[view_index];
#endif
    return FragmentFade(world_colour(without_fade(in), true, false), fade_mask(in.clip, in.fade));
}

@fragment
#if !MULTIVIEW
fn fs_reflect_fade(in: VertexOutFade) -> FragmentFade {
#else
fn fs_reflect_fade(in: VertexOutFade, @builtin(view_index) view_index: u32) -> FragmentFade {
    frame = frames[view_index];
#endif
    return FragmentFade(reflect_colour(without_fade(in), false), fade_mask(in.clip, in.fade));
}

@fragment
#if !MULTIVIEW
fn fs_reflect_alpha_test_fade(in: VertexOutFade) -> FragmentFade {
#else
fn fs_reflect_alpha_test_fade(in: VertexOutFade, @builtin(view_index) view_index: u32) -> FragmentFade {
    frame = frames[view_index];
#endif
    return FragmentFade(reflect_colour(without_fade(in), true), fade_mask(in.clip, in.fade));
}

@fragment
#if !MULTIVIEW
fn fs_skymorph_fade(in: VertexOutFade) -> FragmentFade {
#else
fn fs_skymorph_fade(in: VertexOutFade, @builtin(view_index) view_index: u32) -> FragmentFade {
    frame = frames[view_index];
#endif
    return FragmentFade(skymorph_colour(without_fade(in)), 0xFFFFFFFFu);
}


// The SSR mask pass (main view's worldreflect draws again, into the mask's
// own targets; docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections"): 0 = the reflection term as the
// main pass added it (rgb, unweighted) and its weight × fog; 1 = the
// normal's x, y, the roughness and the view depth.
struct MaskOut {
    @location(0) term: vec4<f32>,
    @location(1) surface: vec4<f32>,
    // The term's per-channel weight over its largest (white here; the
    // cars' env tint, car_ssr.wgsl) and the surface's kind (0 road).
    @location(2) tint: vec4<f32>,
};

fn wet_mask(in: VertexOut, alpha_test: bool) -> MaskOut {
    let n = cross(dpdx(in.world_position), dpdy(in.world_position));
    #if LAMPS
    // BEGIN LAMPS
    // Surface Detail (G7c2): the mask pass takes the lamps' module while
    // the row is on, so SSR sees the material and the mapped normal as the
    // planar Wet Roads term does (wet_surface).
    g_surface = surface_at(in, n);
    // END LAMPS
    #endif
    let to_eye = frame.camera.xyz - in.world_position;
    let ndotv = clamp(abs(dot(n, to_eye)) * inverseSqrt(max(dot(n, n) * dot(to_eye, to_eye), 1e-20)), 0.0, 1.0);
    let texel = sample_tex(in.uv);
    let screen = vec2<f32>(in.screen.x, -in.screen.y) / in.screen.w * 0.5 + 0.5;
    let r0 = texel * in.color;
    if alpha_test && r0.a <= ALPHA_REF {
        discard;
    }
    let wet = wet_surface(in, screen, n, ndotv, r0.a);
    let depth = dot(in.world_position - frame.camera.xyz, frame.forward.xyz);
    return MaskOut(vec4<f32>(wet.reflected, wet.weight * in.fog), vec4<f32>(wet.normal.xy, wet.rough, depth), vec4<f32>(1.0, 1.0, 1.0, 0.0));
}

@fragment
#if !MULTIVIEW
fn fs_wet_mask(in: VertexOut) -> MaskOut {
#else
fn fs_wet_mask(in: VertexOut, @builtin(view_index) view_index: u32) -> MaskOut {
    frame = frames[view_index];
#endif
    return wet_mask(in, false);
}

@fragment
#if !MULTIVIEW
fn fs_wet_mask_alpha_test(in: VertexOut) -> MaskOut {
#else
fn fs_wet_mask_alpha_test(in: VertexOut, @builtin(view_index) view_index: u32) -> MaskOut {
    frame = frames[view_index];
#endif
    return wet_mask(in, true);
}

// An overlay the main view drew (the GPS arrow, mod.rs `overlay_draws`):
// no reflection term, its view depth, and tint.a 1, the flag the SSR
// trace reads to leave it out of every traced reflection (the game draws
// it in the player's view only, never into the road reflection or the
// cube). The whole mesh: its translucent glow edges are part of it.
@fragment
#if !MULTIVIEW
fn fs_overlay_mask(in: VertexOut) -> MaskOut {
#else
fn fs_overlay_mask(in: VertexOut, @builtin(view_index) view_index: u32) -> MaskOut {
    frame = frames[view_index];
#endif
    let depth = dot(in.world_position - frame.camera.xyz, frame.forward.xyz);
    return MaskOut(vec4<f32>(0.0), vec4<f32>(0.0, 0.0, 0.0, depth), vec4<f32>(0.0, 0.0, 0.0, 1.0));
}

// docs/notes/graphics/fsr2.md "Animation mask": FSR 2's animation mask, the main
// view's draws whose texture moves on a fixed surface (flip-books, UV
// scrolls, water, the wet road's ripples): a coverage, which the pipeline's
// blend scales by the draw's (reactive, composition) values (its blend
// constant). The mask's own depth keeps the surface's place (fsr2.wgsl's
// motion pass drops a mark behind the final scene's surface).
fn anim_mask(in: VertexOut, alpha_test: bool) -> vec4<f32> {
    let a = clamp((sample_tex(in.uv) * in.color).a, 0.0, 1.0);
    if (alpha_test && a <= ALPHA_REF) || a < 1.0 / 64.0 {
        discard;
    }
    return vec4<f32>(select(a, 1.0, alpha_test));
}

@fragment
#if !MULTIVIEW
fn fs_anim_mask(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_anim_mask(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return anim_mask(in, false);
}

@fragment
#if !MULTIVIEW
fn fs_anim_mask_alpha_test(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_anim_mask_alpha_test(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return anim_mask(in, true);
}

// An additive draw between its texture's steps (the chevron barriers): the
// light it adds (its black parts add nothing and show the scene behind).
@fragment
#if !MULTIVIEW
fn fs_anim_mask_light(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_anim_mask_light(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    let t = sample_tex(in.uv) * in.color;
    let c = clamp(t.a * max(t.r, max(t.g, t.b)) * 2.0, 0.0, 1.0);
    if c < 1.0 / 64.0 {
        discard;
    }
    return vec4<f32>(c);
}


// The SSR mask's term and surface from the main view itself (docs/notes/graphics/reflections.md "The mask from the main
// view"): with the depth pre-pass and one sample, the main view's worldreflect draws (the opaque block of
// its order) are drawn in a pass of their own whose second and third targets are the mask's, and these
// entries write there what `wet_mask` writes, from the one `wet_surface` the colour takes as well. The
// colour is `reflect_colour`'s, line for line (`reflect_colour` itself, the native path's, stays as it is).
// The mask pass then draws only the road's depth and kind (`fs_wet_mask_kind`).
struct ReflectMrt {
    @location(0) colour: vec4<f32>,
    @location(1) term: vec4<f32>,
    @location(2) surface: vec4<f32>,
};

fn reflect_mrt(in: VertexOut, alpha_test: bool) -> ReflectMrt {
    let n = cross(dpdx(in.world_position), dpdy(in.world_position));
    #if LAMPS
    // BEGIN LAMPS
    g_surface = surface_at(in, n);
    // END LAMPS
    #endif
    let to_eye = frame.camera.xyz - in.world_position;
    let ndotv = clamp(abs(dot(n, to_eye)) * inverseSqrt(max(dot(n, n) * dot(to_eye, to_eye), 1e-20)), 0.0, 1.0);
    let texel = sample_tex(in.uv);
    let screen = vec2<f32>(in.screen.x, -in.screen.y) / in.screen.w * 0.5 + 0.5;
    var reflected = textureSample(reflected_tex, reflected_samp, screen).rgb;
    let r0 = texel * in.color;
    let depth = dot(in.world_position - frame.camera.xyz, frame.forward.xyz);
    // (The mask's values whatever the weather: the dry road's are `wet_surface`'s dry branch, as the mask
    // pass took them.)
    let wet = wet_surface(in, screen, n, ndotv, r0.a);
    let term = vec4<f32>(wet.reflected, wet.weight * in.fog);
    let surface = vec4<f32>(wet.normal.xy, wet.rough, depth);
    if frame.wet.x > 0.5 {
        if alpha_test && r0.a <= ALPHA_REF {
            discard;
        }
        #if LAMPS
        // BEGIN LAMPS
        if (D9_LAMPS && frame.lamps.x > 0.5) || (D9_CONTACT && frame.lamps.y > 0.0) || (D9_BEAMS && frame.cluster_near.z > 0.5) || g_surface.on > 0.5 {
            return ReflectMrt(vec4<f32>(apply_fog(lamp_shade(in, texel, headlit(in), n) * wet.darken + wet.reflected * wet.weight + wet.extra, in.fog), 1.0), term, surface);
        }
        // END LAMPS
        #endif
        let wet_rgb = texel.rgb * headlit(in) * (2.0 * wet.darken) + wet.reflected * wet.weight + wet.extra;
        return ReflectMrt(vec4<f32>(apply_fog(wet_rgb, in.fog), 1.0), term, surface);
    }
    if frame.clip.x > 0.5 {
        reflected = filtered_road_reflection(screen, reflected, r0.a, ndotv);
    }
    if alpha_test && r0.a <= ALPHA_REF {
        discard;
    }
    #if LAMPS
    // BEGIN LAMPS
    if (D9_LAMPS && frame.lamps.x > 0.5) || (D9_CONTACT && frame.lamps.y > 0.0) || (D9_BEAMS && frame.cluster_near.z > 0.5) || g_surface.on > 0.5 {
        return ReflectMrt(vec4<f32>(apply_fog(lamp_shade(in, texel, headlit(in), n) + reflected * r0.a, in.fog), 1.0), term, surface);
    }
    // END LAMPS
    #endif
    let rgb = texel.rgb * headlit(in) * 2.0 + reflected * r0.a;
    return ReflectMrt(vec4<f32>(apply_fog(rgb, in.fog), 1.0), term, surface);
}

@fragment
#if !MULTIVIEW
fn fs_reflect_mrt(in: VertexOut) -> ReflectMrt {
#else
fn fs_reflect_mrt(in: VertexOut, @builtin(view_index) view_index: u32) -> ReflectMrt {
    frame = frames[view_index];
#endif
    return reflect_mrt(in, false);
}

@fragment
#if !MULTIVIEW
fn fs_reflect_alpha_test_mrt(in: VertexOut) -> ReflectMrt {
#else
fn fs_reflect_alpha_test_mrt(in: VertexOut, @builtin(view_index) view_index: u32) -> ReflectMrt {
    frame = frames[view_index];
#endif
    return reflect_mrt(in, true);
}

// LOD Fade's dithered forms (world_fade.wgsl): the discard (1 sample) or the sample mask (MSAA, `_ms`)
// drops the mask's targets too.
struct ReflectMrtFade {
    @location(0) colour: vec4<f32>,
    @location(1) term: vec4<f32>,
    @location(2) surface: vec4<f32>,
    @builtin(sample_mask) mask: u32,
};

@fragment
#if !MULTIVIEW
fn fs_reflect_fade_mrt(in: VertexOutFade) -> ReflectMrt {
#else
fn fs_reflect_fade_mrt(in: VertexOutFade, @builtin(view_index) view_index: u32) -> ReflectMrt {
    frame = frames[view_index];
#endif
    // The colour first: a discard ends the invocation, and the texture samples' derivatives need its quad.
    let c = reflect_mrt(without_fade(in), false);
    if !fade_keep(in.clip, in.fade) {
        discard;
    }
    return c;
}

@fragment
#if !MULTIVIEW
fn fs_reflect_alpha_test_fade_mrt(in: VertexOutFade) -> ReflectMrt {
#else
fn fs_reflect_alpha_test_fade_mrt(in: VertexOutFade, @builtin(view_index) view_index: u32) -> ReflectMrt {
    frame = frames[view_index];
#endif
    // The colour first: a discard ends the invocation, and the texture samples' derivatives need its quad.
    let c = reflect_mrt(without_fade(in), true);
    if !fade_keep(in.clip, in.fade) {
        discard;
    }
    return c;
}

@fragment
#if !MULTIVIEW
fn fs_reflect_fade_mrt_ms(in: VertexOutFade) -> ReflectMrtFade {
#else
fn fs_reflect_fade_mrt_ms(in: VertexOutFade, @builtin(view_index) view_index: u32) -> ReflectMrtFade {
    frame = frames[view_index];
#endif
    let o = reflect_mrt(without_fade(in), false);
    return ReflectMrtFade(o.colour, o.term, o.surface, fade_mask(in.clip, in.fade));
}

@fragment
#if !MULTIVIEW
fn fs_reflect_alpha_test_fade_mrt_ms(in: VertexOutFade) -> ReflectMrtFade {
#else
fn fs_reflect_alpha_test_fade_mrt_ms(in: VertexOutFade, @builtin(view_index) view_index: u32) -> ReflectMrtFade {
    frame = frames[view_index];
#endif
    let o = reflect_mrt(without_fade(in), true);
    return ReflectMrtFade(o.colour, o.term, o.surface, fade_mask(in.clip, in.fade));
}

// The mask pass after them: the road's depth (the pass's own) and its kind; the term and the surface are
// the main view's (loaded). The alpha test as `wet_mask`'s.
struct MaskKind {
    @location(2) tint: vec4<f32>,
};

@fragment
#if !MULTIVIEW
fn fs_wet_mask_kind(in: VertexOut) -> MaskKind {
#else
fn fs_wet_mask_kind(in: VertexOut, @builtin(view_index) view_index: u32) -> MaskKind {
    frame = frames[view_index];
#endif
    return MaskKind(vec4<f32>(1.0, 1.0, 1.0, 0.0));
}

@fragment
#if !MULTIVIEW
fn fs_wet_mask_kind_alpha_test(in: VertexOut) -> MaskKind {
#else
fn fs_wet_mask_kind_alpha_test(in: VertexOut, @builtin(view_index) view_index: u32) -> MaskKind {
    frame = frames[view_index];
#endif
    let r0 = sample_tex(in.uv) * in.color;
    if r0.a <= ALPHA_REF {
        discard;
    }
    return MaskKind(vec4<f32>(1.0, 1.0, 1.0, 0.0));
}

// Then the road the main view hides (under the cars, behind a wall): its term and surface as `wet_mask` works
// them out, so the SSR filter's taps beside an occluder read what they read before. The pass tests against the
// main view's depth (only fragments behind it reach here) and keeps the road's frontmost fragment, the mask's
// own depth (the kind pass's, group 3 binding 2, read).
@group(3) @binding(2) var mask_road_depth: texture_depth_2d;

fn wet_mask_hidden(in: VertexOut, alpha_test: bool) -> MaskOut {
    let o = wet_mask(in, alpha_test);
    if in.clip.z != textureLoad(mask_road_depth, vec2<i32>(in.clip.xy), 0) {
        discard;
    }
    return o;
}

@fragment
#if !MULTIVIEW
fn fs_wet_mask_hidden(in: VertexOut) -> MaskOut {
#else
fn fs_wet_mask_hidden(in: VertexOut, @builtin(view_index) view_index: u32) -> MaskOut {
    frame = frames[view_index];
#endif
    return wet_mask_hidden(in, false);
}

@fragment
#if !MULTIVIEW
fn fs_wet_mask_hidden_alpha_test(in: VertexOut) -> MaskOut {
#else
fn fs_wet_mask_hidden_alpha_test(in: VertexOut, @builtin(view_index) view_index: u32) -> MaskOut {
    frame = frames[view_index];
#endif
    return wet_mask_hidden(in, true);
}

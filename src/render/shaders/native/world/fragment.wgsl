// HeadlightClipTex (0x5BC5E0): 64 x 1 A8R8G8B8, texels 33..47 grey
// trunc(255 * sqrt(1 - (i - 33) / 15)), the rest 0; sampled linearly
// (clamped: inferred).
fn clip_texel(i: f32) -> f32 {
    if i < 33.0 || i > 47.0 {
        return 0.0;
    }
    return trunc(255.0 * sqrt(1.0 - (i - 33.0) * (1.0 / 15.0))) / 255.0;
}

fn headlight_clip(u: f32) -> f32 {
    let x = clamp(u, 0.0, 1.0) * 64.0 - 0.5;
    let i = floor(x);
    return mix(clip_texel(clamp(i, 0.0, 63.0)), clip_texel(clamp(i + 1.0, 0.0, 63.0)), x - i);
}

// worldheadlight's r0.rgb: the vertex colour plus the projector's light.
// A draw view 9 did not take keeps world.efx (just v0).
fn headlit(in: VertexOut) -> vec3<f32> {
    // Behind the projector the clip texture is 0 (u below 33/64). (The
    // projector's texture is read only where its light falls.)
    if in.lit < 0.5 || in.hl_tex.w <= 0.0 {
        return in.color.rgb;
    }
    #if LAMPS
    // BEGIN LAMPS
    // Headlight Beams: the player's projector is the first beam.
    if (D9_BEAMS && frame.cluster_near.z > 0.5) {
        return in.color.rgb;
    }
    // Lamp Shadows (clone deviation): a uniform branch.
    if (D9_SHADOWS && frame.lamps.z > 0.5) {
        let shaded = textureSampleLevel(headlight_tex, headlight_samp, in.hl_tex.xy / in.hl_tex.w, 0.0).rgb * headlight_visibility(in.world_position, in.screen);
        return shaded * headlight_clip(in.hl_clip) * frame.hl_color.rgb + in.color.rgb;
    }
    // END LAMPS
    #endif
    let light = textureSampleLevel(headlight_tex, headlight_samp, in.hl_tex.xy / in.hl_tex.w, 0.0).rgb;
    return light * headlight_clip(in.hl_clip) * frame.hl_color.rgb + in.color.rgb;
}

fn shade(texel: vec4<f32>, color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(texel.rgb * color.rgb * 2.0, texel.a * color.a);
}

fn apply_fog(rgb: vec3<f32>, fog: f32) -> vec3<f32> {
    return rgb * fog + frame.fog_color.rgb * (1.0 - fog);
}

// D3DRS_ALPHATESTENABLE with ALPHAREF 95 (texture entry byte 0x55 == 1).
// The compare function is assumed to be GREATER.
const ALPHA_REF: f32 = 95.0 / 255.0;

// No other discard: native non-alpha-tested shaders retain early depth.
fn world_colour(in: VertexOut, alpha_test: bool, fogged: bool) -> vec4<f32> {
    let texel = sample_tex(in.uv);
    #if LAMPS
    // BEGIN LAMPS
    // The face normal for Dynamic Lamps, taken in uniform control flow.
    let n = cross(dpdx(in.world_position), dpdy(in.world_position));
    #if MODERN
    // BEGIN MODERN
    #if MLRT
    // BEGIN MLRT
    // Modern + RT: the traced blocks first (`ml_vis_early`).
    if fogged {
        ml_vis_early(in.world_position, n, in.clip.xy);
    }
    // END MLRT
    #endif
    // END MODERN
    #endif
    // Surface Detail: the material and the mapped normal, likewise.
    g_surface = surface_at(in, n);
    // END LAMPS
    #endif
    var c = shade(texel, in.color);
    // world → worldheadlight for the draws view 9 took (0x5C5940, table
    // 0x7FF090 / 0x7FF0B4); additive textures go to worldnofog instead.
    if fogged && in.lit > 0.5 {
        c = vec4<f32>(clamp(texel.rgb * headlit(in) * 2.0, vec3<f32>(0.0), vec3<f32>(1.0)), clamp(c.a, 0.0, 1.0));
    }
    #if LAMPS
    // BEGIN LAMPS
    // Dynamic Lamps / Contact Shadows (clone deviations): uniform branch.
    if fogged && ((D9_LAMPS && frame.lamps.x > 0.5) || (D9_CONTACT && frame.lamps.y > 0.0) || (D9_BEAMS && frame.cluster_near.z > 0.5) || g_surface.on > 0.5) {
        let base = select(in.color.rgb, headlit(in), in.lit > 0.5);
        c = vec4<f32>(lamp_shade(in, texel, base, n), c.a);
    }
    // END LAMPS
    #endif
    if alpha_test && c.a <= ALPHA_REF {
        discard;
    }
    if fogged {
        return vec4<f32>(apply_fog(c.rgb, in.fog), c.a);
    }
    return c;
}

@fragment
#if !MULTIVIEW
fn fs_world(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_world(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return world_colour(in, false, true);
}

@fragment
#if !MULTIVIEW
fn fs_world_alpha_test(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_world_alpha_test(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return world_colour(in, true, true);
}

// Dedicated entry points keep clipping unreachable from native fragments.
fn planar_colour(in: VertexOut, alpha_test: bool) -> vec4<f32> {
    // Half a centimetre prevents the receiver itself occluding its reflection.
    if in.world_position.z <= frame.clip.x + 0.005 { discard; }
    return world_colour(in, alpha_test, false);
}

@fragment
#if !MULTIVIEW
fn fs_planar_nofog(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_planar_nofog(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return planar_colour(in, false);
}

@fragment
#if !MULTIVIEW
fn fs_planar_nofog_alpha_test(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_planar_nofog_alpha_test(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return planar_colour(in, true);
}

@fragment
#if !MULTIVIEW
fn fs_nofog(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_nofog(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return world_colour(in, false, false);
}

@fragment
#if !MULTIVIEW
fn fs_nofog_alpha_test(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_nofog_alpha_test(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return world_colour(in, true, false);
}


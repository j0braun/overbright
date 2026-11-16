fn reflect_colour(in: VertexOut, alpha_test: bool) -> vec4<f32> {
    // Derivatives before branches/discard also validate in multiview,
    // where `frame` is copied from a per-view array into private storage.
    let n = cross(dpdx(in.world_position), dpdy(in.world_position));
    #if LAMPS
    // BEGIN LAMPS
    // Surface Detail: taken in uniform control flow.
    g_surface = surface_at(in, n);
    // END LAMPS
    #endif
    let to_eye = frame.camera.xyz - in.world_position;
    let ndotv = clamp(abs(dot(n, to_eye)) * inverseSqrt(max(dot(n, n) * dot(to_eye, to_eye), 1e-20)), 0.0, 1.0);
    let texel = sample_tex(in.uv);
    // oT1 = ((x + w) / 2, (w - y) / 2, z, w), divided per pixel.
    let screen = vec2<f32>(in.screen.x, -in.screen.y) / in.screen.w * 0.5 + 0.5;
    var reflected = textureSample(reflected_tex, reflected_samp, screen).rgb;
    let r0 = texel * in.color;
    // Wet Roads (clone deviation): a uniform branch; the native path below
    // is unchanged.
    if frame.wet.x > 0.5 {
        let wet = wet_surface(in, screen, n, ndotv, r0.a);
        if alpha_test && r0.a <= ALPHA_REF {
            discard;
        }
        #if LAMPS
        // BEGIN LAMPS
        if (D9_LAMPS && frame.lamps.x > 0.5) || (D9_CONTACT && frame.lamps.y > 0.0) || (D9_BEAMS && frame.cluster_near.z > 0.5) || g_surface.on > 0.5 {
            return vec4<f32>(apply_fog(lamp_shade(in, texel, headlit(in), n) * wet.darken + wet.reflected * wet.weight + wet.extra, in.fog), 1.0);
        }
        // END LAMPS
        #endif
        let wet_rgb = texel.rgb * headlit(in) * (2.0 * wet.darken) + wet.reflected * wet.weight + wet.extra;
        return vec4<f32>(apply_fog(wet_rgb, in.fog), 1.0);
    }
    if frame.clip.x > 0.5 {
        reflected = filtered_road_reflection(screen, reflected, r0.a, ndotv);
    }
    if alpha_test && r0.a <= ALPHA_REF {
        discard;
    }
    // worldreflect → worldheadlightreflect for the draws view 9 took
    // (0x7FF090).
    #if LAMPS
    // BEGIN LAMPS
    // Dynamic Lamps / Contact Shadows (clone deviations): uniform branch.
    if (D9_LAMPS && frame.lamps.x > 0.5) || (D9_CONTACT && frame.lamps.y > 0.0) || (D9_BEAMS && frame.cluster_near.z > 0.5) || g_surface.on > 0.5 {
        return vec4<f32>(apply_fog(lamp_shade(in, texel, headlit(in), n) + reflected * r0.a, in.fog), 1.0);
    }
    // END LAMPS
    #endif
    let rgb = texel.rgb * headlit(in) * 2.0 + reflected * r0.a;
    return vec4<f32>(apply_fog(rgb, in.fog), 1.0);
}


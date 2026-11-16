// world.efx / worldheadlight.efx with the lamps added to the light term
// and the contact shadow over the result (fogged draws only: the additive
// worldnofog textures take no light).
fn lamp_shade(in: VertexOut, texel: vec4<f32>, light_in_native: vec3<f32>, n_geo: vec3<f32>) -> vec3<f32> {
    var light_in = light_in_native;
    // Surface Detail: the shading normal for every light below, and the
    // native headlight projector's light (view 9: light_in − v0) given
    // the map's relief (its N·L over the face's) and a specular lobe.
    var n = n_geo;
    var lamps_sum = vec3<f32>(0.0);
    if g_surface.on > 0.5 {
        n = g_surface.n;
        // The street lamps (the clusters are built while Surface Detail is
        // on): their specular, and the baked light given the map's relief
        // toward them, by how much they light this pixel (the bake under
        // a street lamp is that lamp's light).
        #if MODERN
        // BEGIN MODERN
        if frame.lamps.x < 1.5 {
        // END MODERN
        #endif
        lamps_sum = lamp_light(in.world_position, n, in.screen);
        #if MODERN
        // BEGIN MODERN
        }
        // END MODERN
        #endif
        if g_dir.w > 0.0 {
            let l = normalize(g_dir.xyz);
            var ng = normalize(n_geo);
            if dot(ng, frame.camera.xyz - in.world_position) < 0.0 { ng = -ng; }
            let k = smoothstep(0.0, SURFACE_LAMP_FULL, g_dir.w) * SURFACE_RELIEF_BAKED;
            let relief = clamp(1.0 + (dot(n, l) - dot(ng, l)) / max(dot(ng, l), 0.25) * k, 0.6, 1.4);
            light_in = in.color.rgb * relief + (light_in_native - in.color.rgb);
        }
        let proj = light_in_native - in.color.rgb;
        if in.lit > 0.5 && frame.hl_eye.w > 0.5 && any(proj > vec3<f32>(0.0)) {
            let l = normalize(frame.hl_eye.xyz - in.world_position);
            let v = normalize(frame.camera.xyz - in.world_position);
            var ng = normalize(n_geo);
            if dot(ng, v) < 0.0 { ng = -ng; }
            // The relief: the change of N·L the map makes, relative to the
            // face's (at least 0.25: the headlights graze the road, where
            // a plain ratio would explode), bounded.
            let relief = clamp(1.0 + (dot(n, l) - dot(ng, l)) / max(dot(ng, l), 0.25) * SURFACE_RELIEF, 0.3, 1.7);
            light_in = light_in - proj + proj * relief;
            g_spec += proj * surface_spec(n, v, l, g_surface.rough, g_surface.f0, 0.05);
        }
    }
    var light = light_in;
    #if MODERN
    // BEGIN MODERN
    // Modern Lighting: the bake's lamp share relit.
    if frame.lamps.x > 1.5 {
        g_ml_pix = in.clip.xy;
        g_ml_char = in.lit < -0.5;
        light = ml_relight(in.world_position, light_in, n_geo, n, in.screen);
    } else
    // END MODERN
    #endif
    if (D9_LAMPS && frame.lamps.x > 0.5) {
        // Scaled by the baked light itself: a lamp brightens (and tints)
        // what the vertex colours already light, so a surface the art left
        // dark stays dark and the baked hue is kept (adding white on top
        // washed the grade out). And only by the bake's headroom below
        // LAMP_PEAK (so the lamp spots are not too bright): the baked
        // pools already hold each street lamp, so where the bake is at or
        // above LAMP_PEAK nothing is added and nothing ends brighter than
        // max(bake, LAMP_PEAK) (lamps::lamp_term, its test).
        let base = dot(light_in, vec3<f32>(0.2126, 0.7152, 0.0722));
        let headroom = clamp(1.0 - base / LAMP_PEAK, 0.0, 1.0);
        var sum = lamps_sum;
        // (No headroom: nothing to add, the lamps not needed.)
        if g_surface.on < 0.5 && headroom > 0.0 {
            sum = lamp_light(in.world_position, n, in.screen);
        }
        let added = min(sum, vec3<f32>(LAMP_MAX));
        light += added * light_in * (2.0 * headroom);
    }
    // Headlight Beams: added as worldheadlight adds its projector.
    if (D9_BEAMS && frame.cluster_near.z > 0.5) {
        light += beam_light(in.world_position, n, dot(in.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), in.screen);
    }
    var rgb = texel.rgb * light * 2.0;
    #if MODERN
    // BEGIN MODERN
    // Modern Lighting's specular without Surface Detail's material.
    if frame.lamps.x > 1.5 && g_surface.on < 0.5 {
        rgb += g_spec * SURFACE_SPEC;
    }
    // END MODERN
    #endif
    if g_surface.on > 0.5 {
        rgb += g_spec * SURFACE_SPEC;
        if frame.cull.z > 2.5 {
            // NFSU2_SURFACE_DEBUG: the shading normal's tilt off the face.
            var ng = normalize(n_geo);
            if dot(ng, frame.camera.xyz - in.world_position) < 0.0 { ng = -ng; }
            return vec3<f32>(0.5) + (g_surface.n - ng) * 4.0;
        }
    }
    if (D9_CONTACT && frame.lamps.y > 0.0) {
        let vc = contact_visibility(in.world_position, in.screen);
        #if MODERN
        // BEGIN MODERN
        // With Modern Lighting the contact shadow and the lights'
        // shadows (the car's own, under it) give the darker of the two,
        // not their product (stacked, the ground under a car went near
        // black); as shadow_depth.wgsl does for the car's shadow texture.
        if frame.lamps.x > 1.5 {
            rgb /= max(g_ml_ratio, vc);
        }
        // END MODERN
        #endif
        rgb *= vc;
    }
    return rgb;
}


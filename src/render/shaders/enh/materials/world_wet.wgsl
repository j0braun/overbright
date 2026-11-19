// ---------------------------------------------------------------------
// Wet Roads and the SSR mask (clone deviations, docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections").
// Nothing here runs on the native path (frame.wet.x = 0, no mask pass).
// The planar capture (view 4, or Enhanced Reflections' plane) stays the
// base; on top of it: a puddle mask from the texture's authored
// reflection alpha and the weather's wetness and rain, a world-space
// normal (fine distortion, rain ripples), and a roughness blur stretched
// vertically on screen (the long wet-road streaks).

fn wet_hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn wet_hash22(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

// Value noise and its gradient (x, d/dx, d/dy).
fn wet_noise(p: vec2<f32>) -> vec3<f32> {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let du = 6.0 * f * (1.0 - f);
    let a = wet_hash12(i);
    let b = wet_hash12(i + vec2<f32>(1.0, 0.0));
    let c = wet_hash12(i + vec2<f32>(0.0, 1.0));
    let d = wet_hash12(i + vec2<f32>(1.0, 1.0));
    let k = a - b - c + d;
    let v = a + (b - a) * u.x + (c - a) * u.y + k * u.x * u.y;
    let g = du * (vec2<f32>(b - a, c - a) + k * u.yx);
    return vec3<f32>(v, g);
}

// One layer of rain ripples: a ring per cell (`cell` m), born at a random
// place and time, widening and fading over its life. Returns the
// surface's slope (world x, y).
fn wet_ripples(p: vec2<f32>, t: f32, cell: f32, seed: f32) -> vec2<f32> {
    let g = p / cell + seed;
    let i = floor(g);
    let h = wet_hash22(i);
    let life = fract(t * 0.9 + h.x * 7.31 + seed);
    let d = (fract(g) - 0.5 - (h - 0.5) * 0.5) * cell;
    let r = length(d);
    let ring = life * 0.42 * cell;
    let x = (r - ring) / (0.035 * cell);
    let wave = sin(x * 3.14159) * exp(-x * x * 0.5);
    let fade = (1.0 - life) * (1.0 - life) * smoothstep(0.5 * cell, 0.3 * cell, r);
    return d / max(r, 1e-4) * (wave * fade);
}

struct WetSurface {
    reflected: vec3<f32>,
    // The reflection's weight (native: r0.a) and the diffuse's factor.
    weight: f32,
    darken: f32,
    // The world normal (facing the eye) and the roughness (0 mirror).
    normal: vec3<f32>,
    rough: f32,
    // Wet Road Interaction: the cars' lights mirrored, added after
    // the weighted reflection (0 without the row).
    extra: vec3<f32>,
};

// The wet road's reflection at this pixel. `n` is the triangle's
// (unnormalised) normal from the position's derivatives; every texture
// read here is explicit-level (called after branches).
#if LAMPS
// BEGIN LAMPS
const SURFACE_SSR_DRY: f32 = 0.35;
// END LAMPS
#endif
// The water film's reflectance at most (grazing views), and the
// share of the film's added reflection taken from the diffuse.
const FILM_MAX: f32 = 0.6;
const FILM_DIFFUSE: f32 = 0.35;
fn wet_surface(in: VertexOut, screen: vec2<f32>, n: vec3<f32>, ndotv: f32, alpha: f32) -> WetSurface {
    let to_eye = frame.camera.xyz - in.world_position;
    var geo = normalize(n);
    if dot(geo, to_eye) < 0.0 {
        geo = -geo;
    }
    var out: WetSurface;
    // The reflection as the native (or Enhanced Reflections') path adds it.
    var native = textureSampleLevel(reflected_tex, reflected_samp, screen, 0.0).rgb;
    if frame.clip.x > 0.5 {
        native = filtered_road_reflection(screen, native, alpha, ndotv);
    }
    if frame.wet.x < 0.5 {
        // SSR alone (mask pass): the native or Enhanced term as drawn.
        out.reflected = native;
        out.weight = alpha;
        out.darken = 1.0;
        out.normal = geo;
        out.rough = 0.03;
        #if LAMPS
        // BEGIN LAMPS
        // Surface Detail (G7c2): on the dry road SSR's rays take the map's
        // grain, at a third of its tilt (the road's mirror stays a mirror;
        // the dry-road strength hold in ssr.wgsl is untouched).
        if g_surface.on > 0.5 {
            let tilt = g_surface.n - geo * dot(g_surface.n, geo);
            out.normal = normalize(geo + tilt * SURFACE_SSR_DRY);
        }
        // END LAMPS
        #endif
        return out;
    }
    let p = in.world_position.xy;
    let rain = clamp(frame.wet.y, 0.0, 1.0);
    // One wetness (wetness.wgsl): the film and the puddles as
    // every wet effect reads them.
    let level = wetness_film(frame.wet.y, frame.wet.z);
    // How far the wet look is in: none on a road the rain has not wetted
    // (the native look, continuous with the native path the CPU picks at
    // level 0), all of it from a level of 0.15 on.
    let k = smoothstep(0.0, 0.15, level);
    // Puddles: low-frequency patches, wider the wetter, drying more
    // slowly than the film (√level), only where the texture's authored
    // alpha makes the surface reflective.
    var puddle = wetness_puddle(p, level) * smoothstep(0.04, 0.25, alpha);
    // Wet Road Interaction (wet_road_world.wgsl): the trail map's tracks
    // and wakes.
    let interact = wet_fx.window.w > 0.5;
    var trail = vec4<f32>(0.0);
    if interact {
        trail = wet_trail_at(p);
    }
    // The slope: a fine distortion (dry parts only) and the ripples.
    var slope = wet_noise(p * 1.7).yz * (0.012 * (1.0 - puddle) * k);
    let t = frame.wet.w;
    let ripples = wet_ripples(p, t, 0.55, 0.0) + wet_ripples(p, t * 1.13, 0.43, 0.37) + wet_ripples(p, t * 0.87, 0.67, 0.71);
    slope += ripples * (0.09 * rain * (0.4 + 0.6 * puddle));
    if interact {
        slope += wet_wake_slope(p, trail, puddle) * k;
    }
    #if LAMPS
    // BEGIN LAMPS
    // Surface Detail: the map's grain breaks the wet reflection up
    // (not in the puddles' mirror), the material sets its roughness.
    var rough_scale = 1.0;
    if g_surface.on > 0.5 {
        let tilt = g_surface.n - geo * dot(g_surface.n, geo);
        slope -= tilt.xy * (0.5 * k * (1.0 - puddle));
        rough_scale = clamp(g_surface.rough / 0.62, 0.6, 1.4);
    }
    // END LAMPS
    #endif
    // On screen: across follows the camera's right, a slope along the
    // view moves the reflection up or down.
    let fwd = frame.forward.xyz;
    let right = normalize(vec3<f32>(fwd.y, -fwd.x, 0.0) + vec3<f32>(1e-5, 0.0, 0.0));
    let along = normalize(vec3<f32>(fwd.x, fwd.y, 0.0) + vec3<f32>(0.0, 1e-5, 0.0));
    let shift = vec2<f32>(dot(slope, right.xy), -dot(slope, along.xy)) * 0.35;
    let centre = screen + shift;
    // Roughness: smoother the wetter, a mirror in the puddles; blurred
    // along the screen's vertical (the stretch grows toward grazing).
    var rough = mix(mix(0.28, 0.12, level), 0.015, puddle) * k;
    #if LAMPS
    // BEGIN LAMPS
    rough = mix(rough, rough * rough_scale, 1.0 - puddle);
    if g_surface.on > 0.5 {
        // The lamps' and headlights' highlights on the wet road: smoother.
        g_surface.rough = mix(g_surface.rough, max(rough * 1.6, 0.08), k);
        g_surface.f0 = mix(g_surface.f0, max(g_surface.f0, 0.02), k);
    }
    // END LAMPS
    #endif
    let grazing = 1.0 - ndotv;
    let len_y = rough * (0.012 + 0.05 * grazing * grazing);
    let len_x = rough * 0.004;
    var sum = textureSampleLevel(reflected_tex, reflected_samp, centre, 0.0).rgb;
    var total = 1.0;
    for (var i = 1; i <= 4; i = i + 1) {
        let s = f32(i) * 0.25;
        let w = exp(-2.5 * s * s);
        let jx = select(-len_x, len_x, (i & 1) == 1) * s;
        sum += (textureSampleLevel(reflected_tex, reflected_samp, centre + vec2<f32>(jx, s * len_y), 0.0).rgb
            + textureSampleLevel(reflected_tex, reflected_samp, centre + vec2<f32>(-jx, -s * len_y), 0.0).rgb) * w;
        total += 2.0 * w;
    }
    let reflected = sum / total;
    // Enhanced Reflections' angle weight, as its own filter applies it
    // (`native` holds it already): on the authored reflection and the
    // puddles' mirror, not on the water film's own Fresnel below.
    var angle = 1.0;
    if frame.clip.x > 0.5 {
        angle = mix(1.0, 0.35 + 0.65 * pow(1.0 - ndotv, 5.0), k);
    }
    out.reflected = mix(native, reflected, k);
    // A wet road reflects more everywhere (up to 1.6× its authored
    // strength), fully in the puddles; and at least as much as
    // the water film on it reflects (water's Fresnel, where the surface
    // faces the rain), whatever the texture's authored strength: before,
    // a road authored with little reflection (Coal Harbor's plaza road)
    // stayed dry-looking unless SSR was on.
    let up = smoothstep(0.55, 0.85, geo.z);
    let authored = min(alpha * (1.0 + 0.6 * level), 1.0) * angle;
    let film = min(wetness_film_fresnel(ndotv), FILM_MAX) * up * k;
    out.weight = mix(max(authored, film), max(alpha, 0.9) * angle, puddle);
    // Puddles darken what is under them a little; the film's added
    // reflection takes a share from the diffuse (FILM_DIFFUSE of it: the
    // sheen neither lifts the road nor turns the lit kerbs dark).
    out.darken = (1.0 - FILM_DIFFUSE * max(film - authored, 0.0)) * (1.0 - 0.25 * puddle);
    out.normal = normalize(geo - vec3<f32>(slope, 0.0) * 1.5);
    out.rough = max(rough, 0.03 * (1.0 - k));
    if interact {
        wet_interact(&out, in.world_position, trail, puddle, k);
    }
    return out;
}


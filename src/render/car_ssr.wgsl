// Screen-Space Reflections on the cars (clone deviation, the Graphics page's
// SSR row; docs/wet-roads-ssr.md). Appended to car.wgsl for the mask
// pipelines only (`CarShared::ssr_pipelines`); the cars' own pipelines never
// use it. After the car pass the cars draw once more into the road's mask
// (ssr.rs reads it), against the mask's own depth (the road's): per pixel
// the env term car.efx added, unweighted, and its weight, the surface and
// its roughness. The trace then swaps that term for what the reflected view
// meets on screen, by its confidence, as it does on the wet road.

// 1 for the alpha-blended draws (glass): the term reaches the screen × the
// source alpha too.
override MASK_BLENDED: f32 = 0.0;

struct SsrMaskOut {
    // The env cube's colour as car.efx's combine took it; a = the term's
    // weight (its largest channel).
    @location(0) term: vec4<f32>,
    // World normal x, y; the kind and roughness (1 + r a car facing up,
    // 2 + r facing down; the road's is r alone, ssr.wgsl); view depth.
    @location(1) surface: vec4<f32>,
    // The weight per channel over a (EnvmapMin/Range are tinted).
    @location(2) tint: vec4<f32>,
};

fn ssr_mask(in: VertexOut, alpha_test: bool) -> SsrMaskOut {
    draw = draws[in.draw_index];
    // The world normal and its spread over the pixel (uniform control
    // flow: derivatives first).
    let nw0 = (draw.world * vec4<f32>(in.local_normal, 0.0)).xyz;
    let spread = length(fwidth(nw0 * inverseSqrt(max(dot(nw0, nw0), 1e-12))));
    let texel = textureSample(diffuse_tex, diffuse_sampler, in.uv);
    let r1 = 2.0 * texel * in.diffuse;
    if alpha_test && r1.w <= ALPHA_REF {
        discard;
    }
    // The env direction and Fresnel as car_colour takes them.
    var direction = in.reflection;
    var fresnel = in.specular.w;
    let n = in.local_normal * inverseSqrt(max(dot(in.local_normal, in.local_normal), 1e-12));
    let eye = draw.local_eye.xyz - in.local_position;
    let v = eye * inverseSqrt(max(dot(eye, eye), 1e-12));
    let ndv = clamp(dot(v, n), 0.01, 1.0);
    let reflected = 2.0 * ndv * n - v;
    let rv = (frame.view * draw.world * vec4<f32>(reflected, 0.0)).xyz;
    if draw.harmonic_scale.y > 0.5 {
        direction = vec3<f32>(rv.x, -rv.y, -rv.z);
        if draw.harmonic_scale.z > 0.5 {
            direction = (draw.world * vec4<f32>(reflected, 0.0)).xyz;
        }
        fresnel = clamp(exp2(log2(ndv) * draw.powers.y), 0.0, 1.0);
    }
    var env = textureSampleLevel(envmap, envmap_sampler, direction, 0.0).rgb;
    let paint = draw.paint;
    if paint.w > 0.0 && draw.harmonic_scale.y < 0.5 {
        // Enhanced Car Paint's crisp reflection (car_paint.wgsl).
        let crisp = textureSampleLevel(envmap, envmap_sampler, vec3<f32>(rv.x, -rv.y, -rv.z), 0.0).rgb;
        env = mix(env, crisp, paint.w);
    }
    // r0 = 2·env·E + spec; colour = 2·r0·r1.a + r1: the env term is
    // 4·E·r1.a·env (× the source alpha when blended).
    let e = draw.envmap_min.xyz + fresnel * draw.envmap_range.xyz;
    let weight = max(4.0 * e * r1.w * select(1.0, r1.w, MASK_BLENDED > 0.5), vec3<f32>(0.0));
    let w = max(weight.x, max(weight.y, weight.z));
    // Roughness: the car's lacquer and glass are near mirrors; with
    // Enhanced Car Paint its type's (clearcoat smoother, metallic flake
    // rougher, chrome a mirror); widened by the normal's spread in the
    // pixel (no sparkle on far, curved panels).
    var rough = 0.04;
    if any(paint != vec4<f32>(0.0)) {
        rough = mix(0.10, 0.02, clamp(paint.x, 0.0, 1.0)) + 0.04 * paint.y;
        if paint.x < 0.5 && paint.y == 0.0 && paint.z == 0.0 {
            rough = 0.015; // chrome
        }
    }
    rough = clamp(max(rough, 0.35 * spread), 0.0, 0.35);
    // The world normal, turned to the eye (the body draws both sides).
    var nw = nw0 * inverseSqrt(max(dot(nw0, nw0), 1e-12));
    let vw = (draw.world * vec4<f32>(v, 0.0)).xyz;
    if dot(nw, vw) < 0.0 {
        nw = -nw;
    }
    let p = draw.world * vec4<f32>(in.local_position, 1.0);
    let depth = -(frame.view * p).z;
    let tint = weight / max(w, 1e-6);
    return SsrMaskOut(vec4<f32>(env, w), vec4<f32>(nw.xy, select(1.0, 2.0, nw.z < 0.0) + rough, depth), vec4<f32>(tint, 0.0));
}

@fragment
fn fs_ssr_mask(in: VertexOut) -> SsrMaskOut {
    return ssr_mask(in, false);
}

@fragment
fn fs_ssr_mask_alpha_test(in: VertexOut) -> SsrMaskOut {
    return ssr_mask(in, true);
}

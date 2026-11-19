
// ---------------------------------------------------------------------
// Enhanced Glass (clone deviation, opt-in; docs/notes/graphics/lighting2.md),
// appended to car.wgsl in its own module (car_glass.rs): the native
// pipelines never see it. The car's glass draws (the window tints,
// WINDSHIELD, WINDOWMASK, CLEARPLASTIC, HEADLIGHTGLASS) are left out of the
// car pass and drawn here over a copy of the scene taken after it, so what
// is behind the glass (the interior, the world) can be read bent:
//   native  = glass.rgb · a + behind(here) · (1 − a)       (car.efx, SRCALPHA blend)
//   enhanced = glass.rgb · a + behind(here + offset) · (1 − a) · (1 − F/2) + env(per pixel) · F · k
// glass = car.efx's own colour and alpha (the native tint stays the base),
// offset = the view-space normal's xy × the glass's bend (a screen-space
// refraction), F = Schlick (F0 0.04) on the per-pixel n·v, env = the car's
// cube along the per-pixel reflection (as Enhanced Reflections reads it).
// Written opaque (the blend done here); the far side's glass is drawn first
// (car_glass.rs sorts each car's glass far to near).

@group(3) @binding(0) var glass_scene: texture_2d<f32>;
@group(3) @binding(1) var glass_sampler: sampler;

// 1: a window, 2: a lamp cover (thicker, more bend).
override GLASS_KIND: f32 = 1.0;

const GLASS_F0: f32 = 0.04;
const GLASS_REFLECT: f32 = 0.5;

@fragment
fn fs_glass(in: VertexOut) -> @location(0) vec4<f32> {
    let c = car_colour(in);
    let a = clamp(c.a, 0.0, 1.0);
    let n_local = in.local_normal * inverseSqrt(max(dot(in.local_normal, in.local_normal), 1e-12));
    let eye = draw.local_eye.xyz - in.local_position;
    let v = eye * inverseSqrt(max(dot(eye, eye), 1e-12));
    var n = n_local;
    // The side facing the eye (the cull keeps the outer skin; thin parts).
    if dot(n, v) < 0.0 { n = -n; }
    let ndv = clamp(dot(n, v), 0.0, 1.0);
    let fresnel = GLASS_F0 + (1.0 - GLASS_F0) * pow(1.0 - ndv, 5.0);
    // Screen-space refraction: the view-space normal bends the lookup.
    let nv = (frame.view * draw.world * vec4<f32>(n, 0.0)).xyz;
    let size = vec2<f32>(textureDimensions(glass_scene));
    let bend = select(0.018, 0.035, GLASS_KIND > 1.5);
    let here = in.clip.xy / size;
    let uv = clamp(here - vec2<f32>(nv.x, -nv.y) * bend * (1.0 - ndv * 0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    let behind = textureSampleLevel(glass_scene, glass_sampler, uv, 0.0).rgb;
    // The reflection along the per-pixel direction (car_colour's enhanced
    // path: camera axes, or world XYZ for an enhanced dynamic cube).
    let reflected = 2.0 * ndv * n - v;
    let rv = (frame.view * draw.world * vec4<f32>(reflected, 0.0)).xyz;
    var direction = vec3<f32>(rv.x, -rv.y, -rv.z);
    if draw.harmonic_scale.z > 0.5 {
        direction = (draw.world * vec4<f32>(reflected, 0.0)).xyz;
    }
    let env = textureSampleLevel(envmap, envmap_sampler, direction, 0.0).rgb;
    let rgb = c.rgb * a + behind * (1.0 - a) * (1.0 - 0.5 * fresnel) + env * (fresnel * GLASS_REFLECT);
    return vec4<f32>(rgb, 1.0);
}

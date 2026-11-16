// car.efx, transcribed instruction by instruction from its vs_1_1 / ps_1_1
// bytecode (register bindings from the shaders' constant tables):
//
//   c0..c9   HarmonicCoeff   quadratic lighting: c0 + c1 x² + c2 y² + c3 z²
//                            + c4 xy + c5 xz + c6 yz + c7 x + c8 y + c9 z
//   c14..16  WorldView rows  (rotate the reflection vector for the cube map)
//   c17      LocalEyePos
//   c18/c19  DiffuseMin / DiffuseRange
//   c20/c21  SpecularMin / SpecularRange
//   c22/c23  SpecularPower / EnvmapPower
//   ps c0/c1 EnvmapMin / EnvmapRange
//
// All in the model's local space (the car's, or a wheel's own). The game's colour interpolators (oD0/oD1)
// saturate to 0..1, which is reproduced here.

struct Frame {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
};

struct Draw {
    world: mat4x4<f32>,
    local_eye: vec4<f32>,
    diffuse_min: vec4<f32>,
    diffuse_range: vec4<f32>,
    specular_min: vec4<f32>,
    specular_range: vec4<f32>,
    envmap_min: vec4<f32>,
    envmap_range: vec4<f32>,
    powers: vec4<f32>,      // x = SpecularPower, y = EnvmapPower
    harmonic_scale: vec4<f32>, // x = the part's context's factor on HarmonicCoeff (0x620AF0)
    harmonic: array<vec4<f32>, 10>, // the part's light context (wheels and brakes have their own)
    // Enhanced Car Paint (clone deviation, docs/car-paint-enhanced.md):
    // x clearcoat, y metallic flake, z pearl shift, w crisp reflection;
    // all zero = native (the layer below is skipped).
    paint: vec4<f32>,
    // Dynamic Lamps (clone deviation, docs/lamps-shadows.md): four lamps,
    // (position, outer radius) and (colour, inner radius); zeros natively.
    lamps: array<vec4<f32>, 8>,
    // Wet Car (clone deviation, docs/weather.md): read by car_wet.wgsl
    // only; zeros natively.
    wet: array<vec4<f32>, 3>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var envmap: texture_cube<f32>;
@group(0) @binding(2) var envmap_sampler: sampler;
// Every draw's values; a draw's instance index is its entry, which each
// stage copies into `draw` first.
@group(1) @binding(0) var<storage, read> draws: array<Draw>;
var<private> draw: Draw;
@group(2) @binding(0) var diffuse_tex: texture_2d<f32>;
@group(2) @binding(1) var diffuse_sampler: sampler;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) diffuse: vec4<f32>,     // oD0
    @location(1) specular: vec4<f32>,    // oD1 (w = envmap fresnel)
    @location(2) reflection: vec3<f32>,  // oT0 (view space)
    @location(3) uv: vec2<f32>,          // oT1
    @location(4) @interpolate(flat) draw_index: u32,
    @location(5) local_position: vec3<f32>,
    @location(6) local_normal: vec3<f32>,
};

fn harmonic(d: vec3<f32>) -> vec4<f32> {
    let h = draw.harmonic;
    return draw.harmonic_scale.x * (h[0] + h[1] * d.x * d.x + h[2] * d.y * d.y + h[3] * d.z * d.z
         + h[4] * d.x * d.y + h[5] * d.x * d.z + h[6] * d.z * d.y
         + h[7] * d.x + h[8] * d.y + h[9] * d.z);
}

@vertex
fn vs_main(v: VertexIn, @builtin(instance_index) draw_index: u32) -> VertexOut {
    draw = draws[draw_index];
    let n = v.normal;
    let to_eye = draw.local_eye.xyz - v.position;
    let view_dir = to_eye * inverseSqrt(dot(to_eye, to_eye));
    let n_dot_v = max(dot(view_dir, n), 0.01);
    let log_ndv = log2(n_dot_v);
    let reflected = 2.0 * n_dot_v * n - view_dir;

    // Diffuse: harmonic(normal) * (DiffuseMin + n·v * DiffuseRange) / 2.
    let irradiance = harmonic(n).xyz;
    let diffuse_scale = draw.diffuse_min + n_dot_v * draw.diffuse_range;
    let diffuse = vec4<f32>(irradiance * diffuse_scale.xyz, diffuse_scale.w) * 0.5;

    // Specular: harmonic(reflection), its w raised to SpecularPower, times
    // (SpecularMin + (n·v)^SpecularPower * SpecularRange), halved.
    let r = harmonic(reflected);
    let highlight = exp2(log2(clamp(r.w, 0.0, 1.0)) * draw.powers.x);
    let spec_scale = draw.specular_min.xyz + exp2(log_ndv * draw.powers.x) * draw.specular_range.xyz;
    let fresnel = exp2(log_ndv * draw.powers.y);

    let world_view = frame.view * draw.world;
    var out: VertexOut;
    out.clip = frame.view_proj * draw.world * vec4<f32>(v.position, 1.0);
    out.diffuse = clamp(diffuse, vec4<f32>(0.0), vec4<f32>(1.0));
    out.specular = clamp(vec4<f32>(r.xyz * highlight * spec_scale * 0.5, fresnel), vec4<f32>(0.0), vec4<f32>(1.0));
    // WorldView is the game's camera (x right, y down, z forward); the
    // clone's view looks down -z with y up, so flip y and z to sample the
    // cube in the game's camera axes.
    let rv = (world_view * vec4<f32>(reflected, 0.0)).xyz;
    out.reflection = vec3<f32>(rv.x, -rv.y, -rv.z);
    out.uv = v.uv;
    out.draw_index = draw_index;
    out.local_position = v.position;
    out.local_normal = v.normal;
    return out;
}

// Texture entry byte 0x55 == 1: D3D alpha test with reference 95 (0x5BCC40).
const ALPHA_REF: f32 = 95.0 / 255.0;

@fragment
fn fs_alpha_test(in: VertexOut) -> @location(0) vec4<f32> {
    let c = car_colour(in);
    if c.a <= ALPHA_REF {
        discard;
    }
    return c;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return car_colour(in);
}

// Alpha-to-coverage (the Graphics page's MSAA, clone deviation): as
// world.wgsl's, the alpha test's edge as a share of the SAMPLES samples.
override SAMPLES: f32 = 1.0;

struct CoverageOut {
    @location(0) colour: vec4<f32>,
    @builtin(sample_mask) mask: u32,
};

fn coverage_mask(a: f32, clip: vec4<f32>) -> u32 {
    let n = u32(SAMPLES);
    let w = max(fwidth(a), 1.0 / 255.0);
    let share = clamp((a - ALPHA_REF) / w + 0.5, 0.0, 1.0);
    let k = min(u32(share * SAMPLES + 0.5), n);
    let m = (1u << k) - 1u;
    let r = (u32(clip.x) * 3u + u32(clip.y) * 5u) % n;
    return ((m << r) | (m >> (n - r))) & ((1u << n) - 1u);
}

@fragment
fn fs_alpha_coverage(in: VertexOut) -> CoverageOut {
    let c = car_colour(in);
    let mask = coverage_mask(c.a, in.clip);
    if mask == 0u {
        discard;
    }
    return CoverageOut(c, mask);
}

fn car_colour(in: VertexOut) -> vec4<f32> {
    draw = draws[in.draw_index];
    var direction = in.reflection;
    var specular = in.specular;
    if draw.harmonic_scale.y > 0.5 {
        // Optional enhanced reflections: normalize interpolated normals and
        // evaluate direction, harmonic highlight and material Fresnel per
        // pixel. Native keeps its original per-vertex evaluation above.
        let n = in.local_normal * inverseSqrt(max(dot(in.local_normal, in.local_normal), 1e-12));
        let eye = draw.local_eye.xyz - in.local_position;
        let v = eye * inverseSqrt(max(dot(eye, eye), 1e-12));
        let ndv = clamp(dot(v, n), 0.01, 1.0);
        let log_ndv = log2(ndv);
        let reflected = 2.0 * ndv * n - v;
        let rv = (frame.view * draw.world * vec4<f32>(reflected, 0.0)).xyz;
        direction = vec3<f32>(rv.x, -rv.y, -rv.z);
        if draw.harmonic_scale.z > 0.5 {
            // Enhanced dynamic cubes use world XYZ. Frontend static cubes
            // continue using the camera-relative direction above.
            direction = (draw.world * vec4<f32>(reflected, 0.0)).xyz;
        }
        let h = harmonic(reflected);
        let highlight = exp2(log2(max(clamp(h.w, 0.0, 1.0), 1e-20)) * draw.powers.x);
        let scale = draw.specular_min.xyz + exp2(log_ndv * draw.powers.x) * draw.specular_range.xyz;
        specular = clamp(vec4<f32>(h.xyz * highlight * scale * 0.5, exp2(log_ndv * draw.powers.y)), vec4<f32>(0.0), vec4<f32>(1.0));
    }
    let env = textureSample(envmap, envmap_sampler, direction).rgb;
    let texel = textureSample(diffuse_tex, diffuse_sampler, in.uv);
    // r0 = 2 * env * (EnvmapMin + fresnel * EnvmapRange) + specular
    var r0 = 2.0 * env * (draw.envmap_min.xyz + specular.w * draw.envmap_range.xyz) + specular.xyz;
    // r1 = 2 * texture * diffuse
    var r1 = 2.0 * texel * in.diffuse;
    // Dynamic Lamps (clone deviation): per draw, zero natively.
    if draw.lamps[0].w > 0.0 {
        r1 = vec4<f32>(r1.xyz + 2.0 * texel.rgb * car_lamps(in), r1.w);
    }
    // r0 = 2 * r0 * r1.a + r1;  alpha = r1.a
    r0 = 2.0 * r0 * r1.w + r1.xyz;
    return vec4<f32>(r0, r1.w);
}

// Dynamic Lamps (clone deviation, docs/lamps-shadows.md): the draw's four
// lamps per pixel with the native weight (squared, as world.wgsl's), as
// diffuse light through the material's diffuse scale (as the harmonic's,
// halved).
fn car_lamps(in: VertexOut) -> vec3<f32> {
    let p = (draw.world * vec4<f32>(in.local_position, 1.0)).xyz;
    let nw = (draw.world * vec4<f32>(in.local_normal, 0.0)).xyz;
    let n = nw * inverseSqrt(max(dot(nw, nw), 1e-12));
    var sum = vec3<f32>(0.0);
    // For speed: read straight from the
    // storage buffer. Indexing the private copy `draw` with `i` made the
    // compiler keep the whole 560-byte record in scratch memory for every
    // pixel of every car draw; the values are the same.
    for (var i = 0u; i < 4u; i++) {
        let l = draws[in.draw_index].lamps[i * 2u];
        let c = draws[in.draw_index].lamps[i * 2u + 1u];
        if l.w <= 0.0 { break; }
        let d = l.xyz - p;
        let d2 = dot(d, d);
        let i2 = c.w * c.w;
        let o2 = l.w * l.w;
        var w = select(select(1.0 - (d2 - i2) / max(o2 - i2, 1e-6), 1.0, d2 <= i2), 0.0, d2 >= o2);
        w *= w;
        sum += c.rgb * (w * max(dot(n, d * inverseSqrt(max(d2, 1e-12))), 0.0));
    }
    let scale = (draw.diffuse_min.xyz + draw.diffuse_range.xyz) * 0.5;
    return sum * scale;
}

// LOD Fade (clone deviation, docs/formats.md "Graphics enhancements"): a
// car LOD switch cross-faded with a screen-door dither as a sample mask
// (per sample with MSAA). powers.z: the draw's fade — f in (0, 1) keeps
// the samples whose threshold is below f, −g keeps those at or above
// 1 − g (the complement of the new LOD's); powers.w: the sample count.
struct FragmentFade {
    @location(0) color: vec4<f32>,
    @builtin(sample_mask) mask: u32,
};

fn bayer4(p: vec2<f32>) -> f32 {
    let x = u32(p.x) & 3u;
    let y = u32(p.y) & 3u;
    let m = array<u32, 16>(0u, 8u, 2u, 10u, 12u, 4u, 14u, 6u, 3u, 11u, 1u, 9u, 15u, 7u, 13u, 5u);
    return (f32(m[y * 4u + x]) + 0.5) / 16.0;
}

fn fade_mask(p: vec4<f32>, fade: f32, samples: f32) -> u32 {
    let n = max(u32(samples), 1u);
    let b = bayer4(p.xy);
    var mask = 0u;
    for (var s = 0u; s < n; s = s + 1u) {
        let t = fract(b + f32(s) / f32(n));
        if select((t >= 1.0 + fade), (t < fade), (fade >= 0.0)) {
            mask = mask | (1u << s);
        }
    }
    return mask;
}

@fragment
fn fs_main_fade(in: VertexOut) -> FragmentFade {
    let c = car_colour(in);
    return FragmentFade(c, fade_mask(in.clip, draw.powers.z, draw.powers.w));
}

@fragment
fn fs_alpha_test_fade(in: VertexOut) -> FragmentFade {
    let c = car_colour(in);
    var mask = fade_mask(in.clip, draw.powers.z, draw.powers.w);
    if c.a <= ALPHA_REF {
        mask = 0u;
    }
    return FragmentFade(c, mask);
}

// LOD Fade with MSAA's alpha-to-coverage: the dither's samples ANDed with
// the alpha edge's.
@fragment
fn fs_alpha_coverage_fade(in: VertexOut) -> FragmentFade {
    let c = car_colour(in);
    let mask = coverage_mask(c.a, in.clip) & fade_mask(in.clip, draw.powers.z, draw.powers.w);
    if mask == 0u {
        discard;
    }
    return FragmentFade(c, mask);
}

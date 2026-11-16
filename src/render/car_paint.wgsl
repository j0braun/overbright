// Enhanced Car Paint's layer, appended to car.wgsl for the paint pipelines
// only (`CarShared::paint_shader`); the native pipelines use car.wgsl alone.

// Enhanced Car Paint (clone deviation, off by default). Built on car.efx's
// own terms so each paint keeps its hue and brightness: the base is the
// native result (with the env looked up per pixel when `paint.w`), and the
// layers added are a clearcoat highlight (a sharp, normalised, Fresnel-
// weighted lobe towards the harmonic's dominant light; the native Fresnel
// env term already is the coat's reflection), a filtered metallic flake (the mean glint of sub-pixel flakes, a bounded per-cell spread; fades out once a
// flake is smaller than a pixel, and a luminance-neutral pearl hue shift
// towards the material's specular tint at grazing angles.

fn hash33(p: vec3<f32>) -> vec3<f32> {
    var q = vec3<u32>(vec3<i32>(p) + vec3<i32>(1 << 20));
    q = q * vec3<u32>(1664525u, 1013904223u, 2654435761u);
    q.x += q.y * q.z;
    q.y += q.z * q.x;
    q.z += q.x * q.y;
    q = q ^ (q >> vec3<u32>(16u));
    q.x += q.y * q.z;
    q.y += q.z * q.x;
    q.z += q.x * q.y;
    return vec3<f32>(q & vec3<u32>(0xFFFFu)) / 65535.0;
}

const LUMA: vec3<f32> = vec3<f32>(0.299, 0.587, 0.114);
// Flake cell edge (model units, metres): ~1 px at the garage turntable at 1080p.
const FLAKE_CELL: f32 = 0.004;
const COAT_POWER: f32 = 96.0;
// The flake glint, filtered: the mean over a cell's tilted normals
// (uniform +-0.35 per axis) of a power-20 lobe is ~0.18 (r.l)^3 (Monte Carlo,
// docs/car-raindrops.md), and 40 % of the cells carry a flake.
const FLAKE_MEAN: f32 = 0.4 * 0.18;
const FLAKE_POWER: f32 = 3.0;
// Per-cell spread of the filtered glint around its mean (+-40 %).
const FLAKE_SPREAD: f32 = 0.8;
// Zero-mean per-cell brightness grain (+-1 %; +-3 % turned into white dots
// under the Sharpen row at 100, docs/car-raindrops.md).
const FLAKE_GRAIN: f32 = 0.02;

// car.wgsl's final combine (`r0 = 2 * r0 * r1.a + r1`) in the paint module:
// the native result for the materials without a paint type, else the layer.
// The derivatives come first, in uniform control flow.
fn paint_layer(in: VertexOut, r0: vec3<f32>, env: vec3<f32>, texel: vec3<f32>, specular: vec4<f32>, r1: vec4<f32>, direction: vec3<f32>) -> vec3<f32> {
    let footprint = max(length(dpdx(in.local_position)), length(dpdy(in.local_position)));
    let normal_spread = fwidth(in.local_normal);
    if all(draw.paint == vec4<f32>(0.0)) {
        return 2.0 * r0 * r1.w + r1.xyz;
    }
    return enhanced_paint(in, env, texel, specular, r1, direction, footprint, normal_spread);
}

fn enhanced_paint(in: VertexOut, env_native: vec3<f32>, texel: vec3<f32>, specular: vec4<f32>, r1: vec4<f32>, native_direction: vec3<f32>, footprint: f32, normal_spread: vec3<f32>) -> vec3<f32> {
    let p = draw.paint;
    let n = in.local_normal * inverseSqrt(max(dot(in.local_normal, in.local_normal), 1e-12));
    let eye = draw.local_eye.xyz - in.local_position;
    let v = eye * inverseSqrt(max(dot(eye, eye), 1e-12));
    let ndv = clamp(dot(v, n), 0.01, 1.0);
    let reflected = 2.0 * ndv * n - v;

    // Crisp reflection: the cube looked up along the per-pixel direction,
    // in the convention the native path uses for this cube.
    var direction = native_direction;
    if draw.harmonic_scale.y < 0.5 {
        let rv = (frame.view * draw.world * vec4<f32>(reflected, 0.0)).xyz;
        direction = vec3<f32>(rv.x, -rv.y, -rv.z);
    }
    let env_pixel = textureSampleLevel(envmap, envmap_sampler, direction, 0.0).rgb;
    let env = mix(env_native, env_pixel, p.w);

    // car.efx's combine with that env (as in car_colour).
    let env_term = 2.0 * env * (draw.envmap_min.xyz + specular.w * draw.envmap_range.xyz);
    var r0 = env_term + specular.xyz;
    var base = 2.0 * r0 * r1.w + r1.xyz;

    // The harmonic's dominant light: its linear terms' specular channel.
    let h = draw.harmonic;
    let lin = vec3<f32>(h[7].w, h[8].w, h[9].w);
    let light_dir = lin * inverseSqrt(max(dot(lin, lin), 1e-12));
    let light = max(harmonic(light_dir), vec4<f32>(0.0));
    let has_light = select(0.0, 1.0, dot(lin, lin) > 1e-8);
    // Toksvig-style widening of the sharp lobes by the normal's spread in
    // the pixel (stable highlights at distance).
    let spread = dot(normal_spread, normal_spread);
    let gloss = r1.w * 2.0;

    // Clearcoat: car.efx's Fresnel env term already stands for the coat's
    // reflection (EnvmapPower), so the coat adds no second env layer (that
    // would brighten every grazing panel); it adds the coat's own sharp
    // highlight towards the dominant light, Fresnel-weighted, its lobe
    // widened by the normal's spread in the pixel and dimmed in proportion
    // (normalised), so distant cars do not gain a broad sheen.
    let coat_f = 0.04 + 0.96 * pow(1.0 - ndv, 5.0);
    let coat_power = COAT_POWER / (1.0 + COAT_POWER * spread * 4.0);
    let coat_norm = (coat_power + 2.0) / (COAT_POWER + 2.0);
    let coat_lobe = pow(max(dot(reflected, light_dir), 0.0), coat_power) * has_light * coat_norm;
    let coat_highlight = p.x * coat_lobe * light.xyz * clamp(gloss, 0.0, 1.0) * (0.5 + coat_f);
    var colour = base + coat_highlight;

    // Metallic flake. Real flakes are far below a pixel, so a pixel sees
    // their average: the glint is the filtered (mean) lobe over the flakes'
    // tilts, varied per cell by a bounded +-40 % (fine, low-contrast
    // sparkle), plus a faint (+-1 %) zero-mean grain. A per-cell glint of a single
    // tilted normal gave isolated bright cells on a dark panel that read as
    // water drops (docs/car-raindrops.md).
    // Mip-like: the cell grows with the pixel's footprint in power-of-two
    // octaves (two blended, so nothing pops), never below ~1.5 px, so it
    // cannot shimmer; it weakens with each octave and is gone by the sixth.
    if p.y > 0.0 {
        let lod = max(log2(max(footprint, 1e-6) * 1.5 / FLAKE_CELL), 0.0);
        let l0 = floor(lod);
        let t = lod - l0;
        let fade = exp2(-0.6 * lod) * (1.0 - smoothstep(4.0, 6.0, lod));
        // Pearl flake carries the material's specular tint, metallic the paint's.
        let spec_tint = max(draw.specular_min.xyz + draw.specular_range.xyz, vec3<f32>(0.0));
        let tint = mix(texel * 2.0, spec_tint / max(dot(spec_tint, LUMA), 1e-3), clamp(p.z, 0.0, 1.0));
        let glint = FLAKE_MEAN * pow(max(dot(reflected, light_dir), 0.0), FLAKE_POWER) * has_light;
        var sparkle = 0.0;
        var grain = 0.0;
        for (var k = 0; k < 2; k = k + 1) {
            let octave = l0 + f32(k);
            let size = FLAKE_CELL * exp2(octave);
            let rnd = hash33(floor(in.local_position / size) + vec3<f32>(octave * 131.0));
            let w = select(1.0 - t, t, k == 1);
            sparkle += w * glint * (1.0 + FLAKE_SPREAD * (rnd.x - 0.5));
            grain += w * (rnd.y - 0.5);
        }
        colour = colour * (1.0 + p.y * fade * grain * FLAKE_GRAIN) + p.y * fade * sparkle * 0.5 * light.xyz * tint * clamp(gloss, 0.0, 1.0);
    }

    // Pearl / candy: hue towards the specular tint at grazing angles, the
    // luminance unchanged.
    if p.z > 0.0 {
        let spec_tint = max(draw.specular_min.xyz + draw.specular_range.xyz * pow(ndv, draw.powers.x), vec3<f32>(0.0));
        let chroma = spec_tint / max(dot(spec_tint, LUMA), 1e-3) - vec3<f32>(1.0);
        let edge = pow(1.0 - ndv, 3.0);
        colour = colour + p.z * 0.45 * edge * dot(colour, LUMA) * chroma;
    }
    return max(colour, vec3<f32>(0.0));
}

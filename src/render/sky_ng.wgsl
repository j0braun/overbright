// Sky "Next-Gen" (clone deviation, opt-in: `enh.sky 1`; docs/atmosphere.md). Ported from the earlier night-sky
// prototype (NFSU2_NIGHT_SKY), at the next-gen level: the clouds are rendered into a
// low-resolution sky panorama (cs_panorama: a lat-long of the upper hemisphere, a quarter of its rows a frame),
// lit from below by the city's glow; the main view's hemisphere and the reflection views (road mirror, the
// cars' cube faces) sample it, stars and the moon stay analytic at full resolution.
//
// In the main view it is drawn after the native SKYDOME (0x60F890) and the opaque world, over the dome's pixels
// (elevation >= 0; the world then covers it as it covers the dome). Everything is in the dome's "texture space":
// the colours come from the native sky pair's own textures (per-elevation statistics taken at load: the clear
// sky (10th luma percentile), the mean and the lit clouds (90th)), then the dome's native shading is applied
// (world.efx: texel x v0 x 2 with v0 = 0x50 grey, fogged with the frame's FogValue / FogColor), so the sky keeps
// the native palette, the time of day (sunrise / night / sunset and the skymorph fade) and the fog's tint.
// The colour target's alpha (the bleach bypass mask) is left as the dome wrote it (blend: alpha keeps dst).
//
// Algorithms (own code, written for this file): pcg2d integer hash (Jarzynski & Olano, "Hash Functions for GPU
// Rendering", JCGT 9(3) 2020), value-noise fBm with quintic fade, a 2.5D cloud layer (a ray / plane hit, domain-
// warped fBm, a two-tap light march toward the moon in the spirit of Schneider's "Nubis" (SIGGRAPH 2015) and
// Hillaire's Frostbite clouds (SIGGRAPH 2016), reduced to one layer), hashed star cells.

struct Sky {
    inv_view_proj: mat4x4<f32>,
    view_rot_proj: mat4x4<f32>,
    eye: vec4<f32>,
    fog_value: vec4<f32>,   // far, 1 / (far - start), minimum factor
    fog_color: vec4<f32>,
    // x time (s), y rain 0..1, z the pair drawn (0..3), w the pair faded from (0..3)
    p0: vec4<f32>,
    // x SKYMORPHALPHA (1 = the pair drawn), y cloud cover bias, z debug view, w star / moon strength
    p1: vec4<f32>,
    // x the panorama rows updated this frame (row % 4 == x; 4: all), y city glow gain, z the night weight
    // (sky_ng::night_weight: 0 sunrise / sunset, the native dome alone; 1 night), w unused
    p2: vec4<f32>,
    // Per pair, 16 elevation rows of the main texture (0..65.1 deg): clear, mean, lit (rgb, a = alpha mean).
    main_rows: array<vec4<f32>, 192>,
    // Per pair, 16 radial rows of the cap (centre .. rim, 90..31.3 deg): clear, mean, lit.
    cap_rows: array<vec4<f32>, 192>,
};

// SKYGROUP: 0 in the main view's and the panorama's module, 1 in the reflection views' (sky_ng.rs).
@group(SKYGROUP) @binding(0) var<uniform> sky: Sky;
@group(SKYGROUP) @binding(1) var pano: texture_2d<f32>;
@group(SKYGROUP) @binding(2) var pano_samp: sampler;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) dir: vec3<f32>,
};

// A hemisphere of radius DOME_R about the eye (AZ x EL quads, from 2 deg below the horizon to the zenith),
// made from the vertex index. inv_view_proj's inverse (projection x view rotation) is passed as view_rot_proj.
// Drawn after the world's opaque draws with the depth test: the native dome sits 6334..9698 m away and the
// world nearer, so only the dome's pixels (nearer than DOME_R: no) pass and the shading runs on visible sky.
const AZ: u32 = 48u;
const EL: u32 = 16u;
const DOME_R: f32 = 6000.0;

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VOut {
    let quad = vi / 6u;
    let k = vi % 6u;
    var c = vec2<u32>(0u, 0u);
    if (k == 1u || k == 4u) { c = vec2<u32>(1u, 0u); }
    if (k == 2u || k == 3u) { c = vec2<u32>(0u, 1u); }
    if (k == 5u) { c = vec2<u32>(1u, 1u); }
    let ia = quad % AZ + c.x;
    let ie = quad / AZ + c.y;
    let az = f32(ia) / f32(AZ) * 6.2831853;
    let el = mix(-2.0, 90.0, f32(ie) / f32(EL)) * 0.01745329;
    let d = vec3<f32>(cos(el) * cos(az), cos(el) * sin(az), sin(el));
    var o: VOut;
    o.clip = sky.view_rot_proj * vec4<f32>(d * DOME_R, 1.0);
    o.dir = d;
    return o;
}

const PI: f32 = 3.14159265;
const DEG: f32 = 0.01745329;

fn pcg2d(v0: vec2<u32>) -> vec2<u32> {
    var v = v0 * 1664525u + 1013904223u;
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v = v ^ (v >> vec2<u32>(16u));
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v = v ^ (v >> vec2<u32>(16u));
    return v;
}

fn hash2(c: vec2<i32>) -> vec2<f32> {
    return vec2<f32>(pcg2d(bitcast<vec2<u32>>(c))) * (1.0 / 4294967296.0);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = vec2<i32>(floor(p));
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let a = hash2(i).x;
    let b = hash2(i + vec2<i32>(1, 0)).x;
    let c = hash2(i + vec2<i32>(0, 1)).x;
    let d = hash2(i + vec2<i32>(1, 1)).x;
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// fBm with `n` octaves (0..1), rotated per octave against grid artefacts.
fn fbm(p0: vec2<f32>, n: i32) -> f32 {
    var p = p0;
    var a = 0.5;
    var s = 0.0;
    var w = 0.0;
    let r = mat2x2<f32>(0.8, 0.6, -0.6, 0.8);
    for (var k = 0; k < n; k++) {
        s += a * vnoise(p);
        w += a;
        p = r * p * 2.03 + vec2<f32>(17.1, 9.7);
        a *= 0.5;
    }
    return s / w;
}

// Main texture row (0..15) at elevation e (degrees) from the dome's v: 1 at 0 deg, 0.476 at 20.3, 0.152 at 41.7,
// 0 at 65.1 (the SKYDOME mesh); rows are top (v = 0) to bottom (v = 1).
fn main_v(e: f32) -> f32 {
    if (e < 20.3) { return mix(1.0, 0.476, e / 20.3); }
    if (e < 41.7) { return mix(0.476, 0.152, (e - 20.3) / 21.4); }
    return mix(0.152, 0.0, clamp((e - 41.7) / 23.4, 0.0, 1.0));
}

fn row(rows_main: bool, pair: i32, t: f32, k: i32) -> vec4<f32> {
    let x = clamp(t, 0.0, 1.0) * 15.0;
    let i0 = i32(floor(x));
    let i1 = min(i0 + 1, 15);
    let base = pair * 48 + k;
    if (rows_main) {
        return mix(sky.main_rows[base + i0 * 3], sky.main_rows[base + i1 * 3], fract(x));
    }
    return mix(sky.cap_rows[base + i0 * 3], sky.cap_rows[base + i1 * 3], fract(x));
}

// The native dome's colour statistic `k` (0 clear, 1 mean, 2 lit) at elevation e for one pair: the main texture,
// with the alpha-blended cap over it from 31.3 deg (its uv radius 0.5 at 31.3, 0 at 90).
fn dome_stat(pair: i32, e: f32, k: i32) -> vec3<f32> {
    var c = row(true, pair, main_v(e), k).rgb;
    if (e > 65.1) { c = row(true, pair, 0.0, k).rgb; }
    if (e > 31.3) {
        let r = clamp((90.0 - e) / (90.0 - 31.3), 0.0, 1.0);
        let cap = row(false, pair, r, k);
        c = mix(c, cap.rgb, cap.a);
    }
    return c;
}

fn stat(e: f32, k: i32) -> vec3<f32> {
    let now = dome_stat(i32(sky.p0.z), e, k);
    if (sky.p1.x >= 1.0) { return now; }
    return mix(dome_stat(i32(sky.p0.w), e, k), now, sky.p1.x);
}

// The cloud layer's far fade into the dome's mean (m along the ray; was 18..42 km).
const FAR_FADE: vec2<f32> = vec2<f32>(10000.0, 30000.0);

fn luma(c: vec3<f32>) -> f32 { return dot(c, vec3<f32>(0.299, 0.587, 0.114)); }

const MOON_DIR: vec3<f32> = vec3<f32>(0.5497, -0.5694, 0.6111); // azimuth ~ -46 deg, elevation ~ 37.7 deg

// The cloud layer's density at plane point q (metres), 0..1.
fn cloud_density(q: vec2<f32>, cover: f32, oct: i32, soft: f32) -> f32 {
    let p = q * (1.0 / 1700.0);
    let warp = vec2<f32>(fbm(p * 0.7 + vec2<f32>(3.1, 7.7), 2), fbm(p * 0.7 + vec2<f32>(11.3, 1.9), 2)) - 0.5;
    // fBm of value noise clusters near 0.5 (sd ~0.12): stretched so the cover reads as a fraction of the sky.
    let n = clamp((fbm(p + warp * 1.4, oct) - 0.5) * 2.4 + 0.5, 0.0, 1.0);
    // Streaky stratocumulus: stretch along the wind a little.
    // `soft` widens the edge far away (fewer octaves there: a hard edge on a smooth blob read as
    // a flat cut-out low over the horizon).
    return smoothstep(1.0 - cover, 1.0 - cover + 0.32 + soft, n);
}


// The city's glow: the dome's lit horizon colour, warmed (the sodium street lamps), × the gain.
fn city_glow() -> vec3<f32> {
    return stat(1.0, 2) * vec3<f32>(1.3, 1.08, 0.82) * sky.p2.y;
}

// The panorama's direction for a texel (u: azimuth 0..1, v: sqrt(elevation / 90 deg)) and back.
fn pano_dir(uv: vec2<f32>) -> vec3<f32> {
    let az = uv.x * 6.2831853;
    let el = uv.y * uv.y * 90.0 * DEG;
    return vec3<f32>(cos(el) * cos(az), cos(el) * sin(az), sin(el));
}

fn pano_uv(dir: vec3<f32>) -> vec2<f32> {
    let e = max(asin(clamp(dir.z, -1.0, 1.0)), 0.0) / (90.0 * DEG);
    let az = atan2(dir.y, dir.x) / 6.2831853;
    return vec2<f32>(fract(az), sqrt(e));
}

// The cloud layer toward dir: rgb its colour (the dome's texture space), a its cover; the clear sky's fade into
// the dome's mean far away in the rgb when a = 0 is not used (the caller mixes).
fn clouds(dir: vec3<f32>) -> vec4<f32> {
    let e = asin(clamp(dir.z, -1.0, 1.0)) / DEG;
    let time = sky.p0.x;
    let rain = sky.p0.y;
    let mean = stat(e, 1);
    let lit = stat(e, 2);
    let horizon = stat(1.0, 1);
    let zen = luma(stat(80.0, 0));
    let night = smoothstep(0.30, 0.12, zen) * (1.0 - rain) * sky.p1.w;
    let cm = dot(dir, MOON_DIR);
    // Lower and denser in rain.
    let height = mix(1500.0, 650.0, rain);
    var cloud_a = 0.0;
    var cloud_rgb = mean;
    var far_fade = 0.9;
    if (dir.z > 0.004) {
        let t = (height - sky.eye.z) / dir.z;
        let wind = vec2<f32>(6.0, 2.5) * mix(1.0, 2.2, rain) * time;
        let q = sky.eye.xy + dir.xy * t + wind;
        let dist = t;
        // Against cut-outs at speed: the racing camera looks at the layer low over the
        // horizon (elevation 2..10 deg, 9..40 km away), where the octave count fell to 2 and the edge band
        // stayed 0.32: smooth blobs with sharp rims, lit flat by the city's glow and squashed by the
        // perspective into bright slabs on the dark band. Not a temporal fault (stable from frame to
        // frame: no reprojection, the panorama's quarter-row update moves 4.5 m of eye per row phase,
        // nothing at 1.5 km). Now at least 3 octaves, the rim widening with distance, and the far fade
        // nearer (into the dome's mean, below).
        let oct = i32(clamp(6.0 - log2(max(dist / 3000.0, 1.0)) * 1.3, 3.0, 6.0));
        let soft = 0.6 * smoothstep(4000.0, 20000.0, dist);
        let cover = clamp(0.5 + sky.p1.y + rain * 0.45, 0.0, 0.97);
        let d = cloud_density(q, cover, oct, soft);
        let to_moon = normalize(MOON_DIR.xy) * 420.0;
        let d1 = cloud_density(q + to_moon, cover, 2, soft);
        let d2 = cloud_density(q + to_moon * 2.5, cover, 2, soft);
        let shadow = exp(-(d1 * 1.4 + d2 * 0.8));
        let silver = pow(max(cm, 0.0), 8.0) * night;
        let hi = mean + (lit - mean) * 3.5;
        let lit_c = mix(mean * 0.85, hi, clamp(shadow * (0.75 + 0.8 * silver) * mix(1.0, 0.55, rain), 0.0, 1.0));
        // The city's light on the undersides: dense cores take it most (their bases face the streets), low
        // clouds (far away, or in rain) more than the ones overhead.
        let under = mix(0.35, 1.0, smoothstep(40.0, 4.0, e)) * mix(1.0, 1.6, rain) * (0.4 + 0.6 * d)
            * (1.0 - 0.45 * smoothstep(8000.0, 24000.0, dist));
        cloud_rgb = mix(lit_c, horizon, 0.25) * mix(1.0, 0.82, rain) + city_glow() * under;
        let fade = 1.0 - smoothstep(FAR_FADE.x, FAR_FADE.y, dist);
        let ero = fbm(q * (1.0 / 260.0) + vec2<f32>(5.3, 2.1), clamp(oct - 3, 1, 2));
        let dd = clamp(d - (1.0 - d) * (ero - 0.35) * 0.9, 0.0, 1.0);
        cloud_a = dd * fade * mix(0.85, 0.97, rain);
        far_fade = (1.0 - fade) * 0.9;
    }
    // rgb premultiplied-free cloud colour, a its alpha; the far fade goes in via the caller's own rows.
    return vec4<f32>(cloud_rgb, cloud_a + 0.0 * far_fade);
}

@group(PANOGROUP) @binding(3) var pano_out: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn cs_panorama(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = textureDimensions(pano_out);
    // After the first frame only this frame's rows are dispatched (every 4th, from `phase`): the
    // invocations map onto them, so no lane of a workgroup idles on a skipped row (the same texels as before).
    let phase = u32(sky.p2.x);
    let xy = vec2<u32>(id.x, select(id.y, id.y * 4u + phase, phase < 4u));
    if (xy.x >= dims.x || xy.y >= dims.y) { return; }
    let uv = (vec2<f32>(xy) + 0.5) / vec2<f32>(dims);
    textureStore(pano_out, xy, clouds(pano_dir(uv)));
}

// The sky toward dir as the dome's shading would show it: `fwd` the view's forward (the fog's view depth),
// `fog_value` / `fog_color` the view's fog, `fp` the pixel's angular footprint (rad), `pix` its position.
fn shade(dir_in: vec3<f32>, fwd: vec3<f32>, fog_value: vec4<f32>, fog_color: vec4<f32>, fp: f32, pix: vec2<f32>) -> vec4<f32> {
    let dir = normalize(dir_in);
    let e = asin(clamp(dir.z, -1.0, 1.0)) / DEG;
    let time = sky.p0.x;
    let rain = sky.p0.y;
    let clear = stat(e, 0);
    let mean = stat(e, 1);
    let zen = luma(stat(80.0, 0));
    // The night weight's two halves, one after the other, so the dome's painted clouds and these never
    // show together: first this sky's clear part veils the dome (`veil`), then its clouds, stars and moon come
    // in (`ca`). At night both are 1 (the sky as before).
    let veil = smoothstep(0.0, 1.0, clamp(sky.p2.z * 2.0, 0.0, 1.0));
    let ca = smoothstep(0.0, 1.0, clamp(sky.p2.z * 2.0 - 1.0, 0.0, 1.0));
    let night = smoothstep(0.30, 0.12, zen) * (1.0 - rain) * sky.p1.w * ca;

    // The clear sky with the city's glow low over the horizon (light pollution).
    var c = clear + city_glow() * (exp(-max(e, 0.0) / 5.0) * 0.55 + exp(-max(e, 0.0) / 18.0) * 0.2) * mix(1.0, 1.4, rain);

    // Stars (hashed cells), dimmed by the glow near the horizon.
    let az = atan2(dir.y, dir.x);
    let cells = 260.0;
    let g = vec2<f32>(az * cos(e * DEG) * cells / PI, e * DEG * cells / PI);
    let gi = vec2<i32>(floor(g));
    let h = hash2(gi + vec2<i32>(7919, 104729));
    let star_on = step(0.985, h.x);
    let centre = vec2<f32>(gi) + 0.2 + 0.6 * hash2(gi + vec2<i32>(31, 57));
    let dpix = length(g - centre) / max(fp * cells / PI, 1e-4);
    let mag = 0.15 + 0.85 * pow(h.y, 4.0);
    let twinkle = 0.85 + 0.15 * sin(time * (1.3 + 3.0 * h.y) + h.x * 50.0);
    let star = star_on * mag * twinkle * exp(-dpix * dpix * 1.2) * smoothstep(12.0, 35.0, e);
    var emissive = vec3<f32>(0.80, 0.85, 1.0) * star * 0.55 * night;

    // Moon.
    let cm = dot(dir, MOON_DIR);
    let ang = atan2(length(cross(dir, MOON_DIR)), cm) / DEG;
    let disc_r = 0.6;
    let aa = max(fp / DEG, 0.02);
    let disc = 1.0 - smoothstep(disc_r - aa, disc_r + aa, ang);
    let tang = dir - MOON_DIR * cm;
    var maria = 1.0;
    if (ang < 1.0) { maria = 0.78 + 0.22 * fbm(vec2<f32>(dot(tang, vec3<f32>(1.0, 0.0, 0.0)), dot(tang, vec3<f32>(0.0, 0.0, 1.0))) * 260.0, 3); }
    let limb = sqrt(max(1.0 - (ang / disc_r) * (ang / disc_r), 0.0));
    let moon_rgb = vec3<f32>(0.93, 0.94, 1.0) * maria * (0.75 + 0.25 * limb);
    let halo = exp(-ang / 2.2) * 0.10 + exp(-ang / 9.0) * 0.05;

    // The clouds from the panorama; beyond the layer's reach the dome's mean (its painted clouds' average).
    var cl = vec4<f32>(mean, 0.0);
    if (dir.z > 0.004) {
        cl = textureSampleLevel(pano, pano_samp, pano_uv(dir), 0.0);
        cl.a *= ca;
        let t = (mix(1500.0, 650.0, rain) - sky.eye.z) / dir.z;
        c = mix(c, mean, smoothstep(FAR_FADE.x, FAR_FADE.y, t) * 0.9);
    } else {
        c = mix(c, mean, 0.9);
    }
    c = mix(c, cl.rgb, cl.a);
    emissive *= 1.0 - cl.a;
    emissive += moon_rgb * disc * night * (1.0 - cl.a * 0.92) * 0.85;
    emissive += vec3<f32>(0.6, 0.65, 0.8) * halo * night * (1.0 - cl.a * 0.5) * 0.6;

    if (sky.p1.z > 0.5) {
        return vec4<f32>(cl.a, night, star * 4.0, 1.0);
    }
    // world.efx on the dome: texel × v0 × 2 (v0 = 0x50 grey), fogged.
    let shaded = c * (2.0 * 80.0 / 255.0);
    let dome_dist = mix(9698.0, 6334.0, smoothstep(0.0, 90.0, e));
    let view_z = dome_dist * dot(dir, fwd);
    let fog = select(1.0, clamp((fog_value.x - view_z) * fog_value.y, fog_value.z, 1.0), fog_color.w > 0.5);
    let dither = (hash2(vec2<i32>(pix)).x - 0.5) / 255.0;
    return vec4<f32>(shaded * fog + fog_color.rgb * (1.0 - fog) * fog_color.w + emissive * (0.5 + 0.5 * fog) + dither, veil);
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let dir = normalize(in.dir);
    if (dir.z < 0.0) { discard; }
    let fc = sky.inv_view_proj * vec4<f32>(0.0, 0.0, 0.5, 1.0);
    let fwd = normalize(fc.xyz / fc.w);
    let fp = max(length(dpdx(dir)), length(dpdy(dir)));
    return shade(dir, fwd, sky.fog_value, vec4<f32>(sky.fog_color.rgb, 1.0), fp, in.clip.xy);
}

// REFL BEGIN
// ---- reflection views: the world's frame (group 0, world.wgsl `Frame`'s first fields), this sky as group 1 ----
struct Frame {
    view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    forward: vec4<f32>,
    fog_value: vec4<f32>,
    fog_color: vec4<f32>,
};
@group(0) @binding(0) var<uniform> frame: Frame;

@vertex
fn vs_refl(@builtin(vertex_index) vi: u32) -> VOut {
    let quad = vi / 6u;
    let k = vi % 6u;
    var c = vec2<u32>(0u, 0u);
    if (k == 1u || k == 4u) { c = vec2<u32>(1u, 0u); }
    if (k == 2u || k == 3u) { c = vec2<u32>(0u, 1u); }
    if (k == 5u) { c = vec2<u32>(1u, 1u); }
    let ia = quad % AZ + c.x;
    let ie = quad / AZ + c.y;
    let az = f32(ia) / f32(AZ) * 6.2831853;
    let el = mix(-2.0, 90.0, f32(ie) / f32(EL)) * 0.01745329;
    let d = vec3<f32>(cos(el) * cos(az), cos(el) * sin(az), sin(el));
    var o: VOut;
    let p = frame.view_proj * vec4<f32>(frame.camera.xyz + d * 50.0, 1.0);
    // On the far plane: only where the view drew nothing (its cleared depth) passes.
    o.clip = vec4<f32>(p.xy, p.w * 0.9999995, p.w);
    o.dir = d;
    return o;
}

@fragment
fn fs_refl(in: VOut) -> @location(0) vec4<f32> {
    let dir = normalize(in.dir);
    if (dir.z < 0.0) { discard; }
    let fp = max(length(dpdx(dir)), length(dpdy(dir)));
    // The reflection views' own fog (world.efx in them); alpha 1 as their clear (opaque black) had.
    let c = shade(dir, frame.forward.xyz, frame.fog_value, vec4<f32>(frame.fog_color.rgb, 1.0), fp, in.clip.xy);
    // Faded in from the views' native clear black by the night weight's veil.
    return vec4<f32>(c.rgb * c.a, 1.0);
}
// REFL END

// ---- light ----

@group(0) @binding(0) var<uniform> lvol: Vol;
@group(0) @binding(1) var<uniform> lights: Lights;
@group(0) @binding(2) var dens_tex: texture_3d<f32>;
@group(0) @binding(3) var albedo_tex: texture_3d<f32>;
@group(0) @binding(4) var lin_samp: sampler;
@group(0) @binding(5) var<storage, read> lbricks: array<u32>;
@group(0) @binding(6) var light_out: texture_storage_3d<rgba16float, write>;
@group(0) @binding(7) var emit_tex: texture_3d<f32>;
// The dominant direct light's direction (xyz, its length how
// one-sided the light is)
@group(0) @binding(8) var ldir_out: texture_storage_3d<rgba8snorm, write>;

fn lbrick(v: vec3<i32>) -> u32 {
    let nb = vec3<i32>(lvol.dims.xyz) / i32(BRICK);
    let b = clamp(v / i32(BRICK), vec3(0), nb - 1);
    return lbricks[u32((b.z * nb.y + b.y) * nb.x + b.x)];
}

fn sigma_at(rel: vec3<f32>) -> f32 {
    let uvw = (rel + lvol.eye.xyz - lvol.origin.xyz) / (lvol.origin.w * vec3<f32>(lvol.dims.xyz));
    return textureSampleLevel(dens_tex, lin_samp, uvw, 0.0).r;
}

// The optical depth from `p` toward `to` (eye-relative), at most `reach`
// metres: four samples, denser near the start.
fn shadow_tau(p: vec3<f32>, to: vec3<f32>, reach: f32) -> f32 {
    let d = length(to - p);
    let dir = (to - p) / max(d, 1e-4);
    let len = min(d, reach);
    var tau = 0.0;
    var prev = 0.0;
    for (var i = 1; i <= 4; i++) {
        let x = f32(i) / 4.0;
        let t = len * x * x;
        tau += sigma_at(p + dir * t) * (t - prev);
        prev = t;
    }
    return tau;
}

// Per light-volume voxel (2³ density voxels): the light scattered toward
// the eye, × albedo, plus the emission. Lamps (the light packs' or Modern
// Lighting's falloff), spot lights (headlights: cone, reach), point lights
// (tail / brake lights, neon, fire), each shadowed through the medium and
// scattered with the multiple-scattering phase; the sky and the city's
// bounce from above and the sides, occluded by the medium over it.
@compute @workgroup_size(4, 4, 4)
fn cs_light(@builtin(global_invocation_id) id: vec3<u32>) {
    let ld = lvol.dims.xyz / 2u;
    if (any(id >= ld)) {
        return;
    }
    if (lbrick(vec3<i32>(id * 2u)) == 0u && lbrick(vec3<i32>(id * 2u + 1u)) == 0u) {
        textureStore(light_out, id, vec4(0.0));
        textureStore(ldir_out, id, vec4(0.0));
        return;
    }
    let lvs = lvol.origin.w * 2.0;
    let p = lvol.origin.xyz + (vec3<f32>(id) + 0.5) * lvs - lvol.eye.xyz;
    let to_eye = -normalize(p);
    let g = lvol.ambient.w;
    var light = vec3(0.0);
    // The direct light's luminance-weighted direction toward its
    // sources (the billow shading's light side).
    var ldir = vec3(0.0);
    var lsum = 0.0;
    for (var i = 0u; i < lights.counts.x; i++) {
        let l = lights.lamps[i * 2u];
        let to = l.xyz - p;
        let d2 = dot(to, to);
        var fall = clamp(1.0 - d2 / (l.w * l.w), 0.0, 1.0);
        fall *= fall;
        if (lights.counts.w == 1u) {
            let y = d2 / (l.w * l.w);
            let window = max(1.0 - y * y, 0.0);
            fall = min((81.0 + 150.0) / (d2 + 150.0), 1.6) * window * window;
        }
        if (fall <= 1e-4) {
            continue;
        }
        let c = dot(-normalize(to), to_eye);
        let add = lights.lamps[i * 2u + 1u].rgb * fall * lvol.look3.y * multi_scatter(shadow_tau(p, l.xyz, 6.0), c, g);
        light += add;
        let al = dot(add, vec3(0.3, 0.59, 0.11));
        ldir += normalize(to) * al;
        lsum += al;
    }
    for (var i = 0u; i < lights.counts.y; i++) {
        let e = lights.spots[i * 3u];
        let dir = lights.spots[i * 3u + 1u];
        let col = lights.spots[i * 3u + 2u];
        let to = p - e.xyz;
        let dist = length(to);
        let cone = smoothstep(dir.w, col.w, dot(to / max(dist, 1e-3), dir.xyz));
        let att = clamp(1.0 - dist / e.w, 0.0, 1.0);
        if (cone * att <= 1e-4) {
            continue;
        }
        let c = dot(to / max(dist, 1e-3), to_eye);
        let add = col.rgb * cone * att * att * lvol.look3.z * multi_scatter(shadow_tau(p, e.xyz, 6.0), c, 0.75);
        light += add;
        let al = dot(add, vec3(0.3, 0.59, 0.11));
        ldir -= to / max(dist, 1e-3) * al;
        lsum += al;
    }
    for (var i = 0u; i < lights.counts.z; i++) {
        let l = lights.points[i * 2u];
        let to = l.xyz - p;
        let r = abs(l.w);
        // (A ground light — an underglow — reaches the smoke low
        // round the car, not over its body: faded from 0.3 to 1.2 m up.)
        let held = select(1.0, 1.0 - smoothstep(0.3, 1.2, -to.z), l.w < 0.0);
        let w = clamp(1.0 - dot(to, to) / (r * r), 0.0, 1.0) * sqrt(held);
        if (w <= 1e-4) {
            continue;
        }
        let c = dot(-normalize(to), to_eye);
        let add = lights.points[i * 2u + 1u].rgb * w * w * lvol.look3.w * multi_scatter(shadow_tau(p, l.xyz, 3.0), c, 0.4);
        light += add;
        let al = dot(add, vec3(0.3, 0.59, 0.11));
        ldir += normalize(to) * al;
        lsum += al;
    }
    let up = shadow_tau(p, p + vec3(0.0, 0.0, 8.0), 8.0);
    let side = 0.5 * (shadow_tau(p, p + vec3(4.0, 0.0, 1.0), 4.0) + shadow_tau(p, p + vec3(-4.0, 0.0, 1.0), 4.0));
    // The ambient's occlusion by the medium above and beside (dense
    // cores darker and greyer); the ambient itself is the scene's around
    // the medium, applied in the march (a: the occlusion × the albedo).
    let occl = 0.5 + 0.35 * exp(-0.35 * up) + 0.15 * exp(-0.35 * side);
    let uvw = (vec3<f32>(id) + 0.5) / vec3<f32>(ld);
    let albedo = textureSampleLevel(albedo_tex, lin_samp, uvw, 0.0).rgb;
    let thin = textureSampleLevel(dens_tex, lin_samp, uvw, 0.0).g;
    let alb = mix(albedo, max(albedo, vec3(lvol.look2.z)), thin);
    let em = textureSampleLevel(emit_tex, lin_samp, uvw, 0.0).rgb;
    textureStore(light_out, id, vec4(alb * light + em, occl * dot(alb, vec3(0.3, 0.59, 0.11))));
    textureStore(ldir_out, id, vec4(ldir / max(lsum, 1e-6), 0.0));
}


// Particles Cinematic → volumetric (docs/smoke-volumetric.md). The
// smoke-specific half: each live Cinematic puff (smoke_cinematic.wgsl's
// pool: the native smoke births, moved by its noise flow, wind and wakes)
// becomes a volume_fx `Particle`: its size and opacity as the Cinematic billboard
// had them (the native size and colour cubics, growth, fades), the opacity
// turned into an extinction, the native colour into an albedo.

struct Puff {
    p0: vec4<f32>,
    p1: vec4<f32>,
    ids: vec4<u32>,
    p3: vec4<f32>,
    p4: vec4<f32>,
}

struct Desc {
    size: array<vec4<f32>, 4>,
    params: vec4<f32>,
    params2: vec4<f32>,
    // the sprite's mean opacity, its opacity-weighted mean colour
    params3: vec4<f32>,
}

// volume_fx.wgsl `Particle`.
struct Particle {
    pos_r: vec4<f32>,
    albedo_sigma: vec4<f32>,
    emit_lump: vec4<f32>,
    noise: vec4<f32>,
    misc: vec4<f32>,
}

struct Smoke {
    // puffs, 0, 0, 0
    counts: vec4<u32>,
    // the haze puffs' opacity scale, steam evaporation, the albedo's hue kept,
    // dissipation (the optical depth × growth^−(this × age fraction))
    look2: vec4<f32>,
    // opacity scale, size scale, steam thinning, albedo floor
    look: vec4<f32>,
}

@group(0) @binding(0) var<uniform> sm: Smoke;
@group(0) @binding(1) var<storage, read> puffs: array<Puff>;
@group(0) @binding(2) var<storage, read> descs: array<Desc>;
@group(0) @binding(3) var<storage, read> colors: array<array<vec4<f32>, 4>>;
@group(0) @binding(4) var<storage, read_write> particles: array<Particle>;

fn cubic(m: array<vec4<f32>, 4>, t: f32) -> vec4<f32> {
    return ((m[0] * t + m[1]) * t + m[2]) * t + m[3];
}

@compute @workgroup_size(64)
fn cs_smoke_particles(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= sm.counts.x) {
        return;
    }
    let puff = puffs[id.x];
    var p: Particle;
    p.pos_r = vec4(0.0);
    if (puff.p1.w > 0.0) {
        let d = descs[puff.ids.x];
        let haze = puff.p4.w > 0.5;
        let t = clamp(puff.p0.w / puff.p1.w, 0.0, 1.0);
        let tn = t * select(1.9, 1.0, haze);
        let grow = 1.0 + puff.p4.z * sqrt(puff.p0.w);
        let size = cubic(d.size, min(tn, 1.0)).x * puff.p3.x * grow;
        let col = clamp(cubic(colors[puff.ids.y], select(t, min(t * 1.6, 1.0), haze)), vec4(0.0), vec4(255.0)) / 255.0;
        var a = col.a * puff.p4.y / pow(grow, 0.5) * d.params3.x * 2.5;
        a *= smoothstep(0.0, select(0.08, 0.6, haze), puff.p0.w) * (1.0 - smoothstep(select(0.6, 0.4, haze), 1.0, t));
        let r = size * 0.5 * sm.look.y;
        // (NaN-safe: a comparison with NaN is false.)
        if (r > 0.02 && r < 40.0 && a > 0.002 && all(abs(puff.p0.xyz) < vec3(1e6))) {
            // The optical depth through the centre the opacity asks for
            // (volume_fx.rs `sigma_for_opacity`); steam (wet road) thinner.
            let tau = -log(1.0 - min(a * select(sm.look.x, sm.look2.x, haze), 0.985));
            // Steam evaporates: on a wet road the density decays with age
            // (a dense plume at the wheels, no lasting wall of fog).
            let evaporate = mix(1.0, exp(-sm.look2.y * puff.p0.w), puff.p4.x);
            let sigma = tau / (0.9 * r) * (1.0 - sm.look.z * puff.p4.x) * evaporate * pow(grow, -sm.look2.w * t);
            // Albedo: the native 2 · tex · colour (the sprite's mean),
            // partly desaturated and lifted to a smoke grey (the lit volume
            // picks up the lights' colours itself; the native tint was the
            // sprite's only colour).
            let alb = clamp(2.0 * d.params3.yzw * col.rgb, vec3(0.0), vec3(1.0));
            let lum = dot(alb, vec3(0.3, 0.59, 0.11));
            let seed = puff.ids.z;
            p.pos_r = vec4(puff.p0.xyz, r);
            p.albedo_sigma = vec4(mix(vec3(lum), alb, sm.look2.z) * (max(lum, sm.look.w) / max(lum, 1e-3)), sigma);
            p.emit_lump = vec4(0.0, 0.0, 0.0, select(1.0, 0.0, haze));
            p.noise = vec4(f32(seed & 63u) / 64.0, f32((seed >> 6u) & 63u) / 64.0, f32((seed >> 12u) & 63u) / 64.0 + puff.p0.w * 0.04, puff.p3.y + puff.p3.z * puff.p0.w * 0.5);
            p.misc = vec4(t, puff.p4.x, 0.0, 0.0);
        }
    }
    particles[id.x] = p;
}

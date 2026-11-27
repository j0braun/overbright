// ---- splat ----

@group(0) @binding(0) var<uniform> vol: Vol;
@group(0) @binding(1) var<storage, read> particles: array<Particle>;
@group(0) @binding(2) var noise_tex: texture_3d<f32>;
@group(0) @binding(3) var noise_samp: sampler;
@group(0) @binding(4) var<storage, read_write> acc: array<atomic<u32>>;
// voxel bounds: this frame's particles (min xyz, max xyz), the last
// frame's, the occupied bricks' (volume_fx.rs `BOUNDS`)
@group(0) @binding(5) var<storage, read_write> sbounds: array<atomic<u32>>;

var<workgroup> wg_p: Particle;

// One workgroup a particle: a lumpy ball of density over its voxels, a
// smooth radial profile displaced by the noise volume in the particle's
// own (spinning) frame, so the voxels carry its shape and it rolls as it
// drifts. A radius under a voxel is widened with the mass kept.
@compute @workgroup_size(64)
fn cs_splat(@builtin(workgroup_id) wid: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let index = wid.x + wid.y * 65535u;
    if (li == 0u) {
        wg_p.pos_r = vec4(0.0);
        if (index < vol.dims.w) {
            var p = particles[index];
            let r = p.pos_r.w;
            // NaN-safe (comparisons with NaN are false): a broken particle
            // would otherwise span the whole grid.
            if (r > 0.01 && r < 64.0 && p.albedo_sigma.w > 1e-4 && p.albedo_sigma.w < 1e4 && all(abs(p.pos_r.xyz) < vec3(1e6))) {
                let rr = max(r, 0.75 * vol.origin.w);
                p.albedo_sigma.w *= pow(r / rr, 3.0);
                p.pos_r.w = rr;
                wg_p = p;
            }
        }
    }
    workgroupBarrier();
    let p = wg_p;
    if (p.pos_r.w <= 0.0) {
        return;
    }
    let vs = vol.origin.w;
    let c = p.pos_r.xyz;
    let rad = p.pos_r.w;
    let lo = vec3<i32>(floor((c - rad - vol.origin.xyz) / vs));
    let hi = vec3<i32>(floor((c + rad - vol.origin.xyz) / vs));
    let dims = vec3<i32>(vol.dims.xyz);
    let a = max(lo, vec3(0));
    let b = min(hi, dims - 1);
    if (any(b < a)) {
        return;
    }
    if (li == 0u) {
        for (var k = 0u; k < 3u; k++) {
            atomicMin(&sbounds[k], u32(a[k]));
            atomicMax(&sbounds[3u + k], u32(b[k]));
        }
    }
    let n = b - a + 1;
    let total = u32(n.x * n.y * n.z);
    let lump = p.emit_lump.w;
    let age = p.misc.x;
    for (var i = li; i < total; i += 64u) {
        let v = a + vec3<i32>(i32(i % u32(n.x)), i32((i / u32(n.x)) % u32(n.y)), i32(i / u32(n.x * n.y)));
        let wp = vol.origin.xyz + (vec3<f32>(v) + 0.5) * vs;
        let rel = (wp - c) / rad;
        let d2 = dot(rel, rel);
        if (!(d2 < 1.0)) {
            continue;
        }
        let q = vec3(rot2(rel.xy, p.noise.w), rel.z) * mix(0.22, 0.32, lump) + p.noise.xyz;
        let nv = textureSampleLevel(noise_tex, noise_samp, q, 0.0).r;
        let l = (1.0 - sqrt(d2)) * 1.5 + (nv - 0.5) * mix(0.5, 1.2, lump) - (0.08 + 0.4 * age * age * lump);
        let s = p.albedo_sigma.w * smoothstep(0.0, mix(0.9, 0.55, lump), l);
        if (!(s > 1e-4 && s < 1e4)) {
            continue;
        }
        let at = u32((v.z * dims.y + v.y) * dims.x + v.x) * CH;
        atomicAdd(&acc[at], u32(s * SCALE));
        atomicAdd(&acc[at + 1u], u32(s * p.misc.y * SCALE));
        atomicAdd(&acc[at + 2u], u32(s * age * SCALE));
        atomicAdd(&acc[at + 3u], u32(s * p.albedo_sigma.r * SCALE));
        atomicAdd(&acc[at + 4u], u32(s * p.albedo_sigma.g * SCALE));
        atomicAdd(&acc[at + 5u], u32(s * p.albedo_sigma.b * SCALE));
        if (any(p.emit_lump.rgb > vec3(0.0))) {
            atomicAdd(&acc[at + 6u], u32(s * p.emit_lump.r * 4096.0));
            atomicAdd(&acc[at + 7u], u32(s * p.emit_lump.g * 4096.0));
            atomicAdd(&acc[at + 8u], u32(s * p.emit_lump.b * 4096.0));
        }
    }
}


// ---- resolve ----

@group(0) @binding(0) var<uniform> rvol: Vol;
@group(0) @binding(1) var<storage, read_write> racc: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> bricks: array<atomic<u32>>;
@group(0) @binding(3) var dens_out: texture_storage_3d<rgba16float, write>;
@group(0) @binding(4) var albedo_out: texture_storage_3d<rgba8unorm, write>;
@group(0) @binding(5) var emit_out: texture_storage_3d<rgba16float, write>;
@group(0) @binding(6) var<storage, read_write> rbounds: array<atomic<u32>>;

fn brick_index(v: vec3<i32>) -> u32 {
    let nb = vec3<i32>(rvol.dims.xyz) / i32(BRICK);
    let b = clamp(v / i32(BRICK), vec3(0), nb - 1);
    return u32((b.z * nb.y + b.y) * nb.x + b.x);
}

// The bounds (this frame's and the last frame's particle voxels), read once
// a workgroup; whether any of the workgroup's voxels lies in them; the
// bricks its voxels mark (the 3 × 3 × 3 bricks round the workgroup's own:
// its 4³ voxels lie in one brick, a voxel's footprint reaches the next),
// gathered here and marked once each.
var<workgroup> wg_bounds: array<u32, 12>;
var<workgroup> wg_any: u32;
var<workgroup> wg_bricks: array<atomic<u32>, 27>;

// Per voxel: the accumulated density, thinness, age, albedo and emission
// into the volumes (the accumulators back to zero for the next frame);
// the voxel's brick and the bricks its filter footprint reaches marked
// occupied (the march's empty-space skipping). Faded at the grid's sides
// and top (no wall where the grid ends).
@compute @workgroup_size(4, 4, 4)
fn cs_resolve(@builtin(global_invocation_id) id: vec3<u32>, @builtin(workgroup_id) wid: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    if (li < 12u) {
        wg_bounds[li] = atomicLoad(&rbounds[li]);
    }
    if (li < 27u) {
        atomicStore(&wg_bricks[li], 0u);
    }
    workgroupBarrier();
    if (li == 0u) {
        // Any voxel of the workgroup's box (lo ..= lo + 3) in either bounds.
        let lo = wid * 4u;
        let cmin = vec3(wg_bounds[0], wg_bounds[1], wg_bounds[2]);
        let cmax = vec3(wg_bounds[3], wg_bounds[4], wg_bounds[5]);
        let pmin = vec3(wg_bounds[6], wg_bounds[7], wg_bounds[8]);
        let pmax = vec3(wg_bounds[9], wg_bounds[10], wg_bounds[11]);
        let any_cur = all(lo + 4u >= cmin) && all(lo <= cmax + 1u);
        let any_prev = all(lo + 4u >= pmin) && all(lo <= pmax + 1u);
        wg_any = u32(any_cur || any_prev);
    }
    if (workgroupUniformLoad(&wg_any) == 0u) {
        return;
    }
    let marked = resolve_voxel(id);
    workgroupBarrier();
    if (li < 27u && atomicLoad(&wg_bricks[li]) != 0u) {
        let gb = vec3<i32>(wid / 2u) + vec3(i32(li % 3u), i32((li / 3u) % 3u), i32(li / 9u)) - 1;
        let nb = vec3<i32>(rvol.dims.xyz) / i32(BRICK);
        // (The neighbours past the grid's edge clamp onto its edge bricks, as `brick_index` does.)
        atomicMax(&bricks[brick_index(clamp(gb, vec3(0), nb - 1) * i32(BRICK))], 1u);
    }
}

// One voxel of `cs_resolve`; true when it marked its bricks (into `wg_bricks`).
fn resolve_voxel(id: vec3<u32>) -> bool {
    if (any(id >= rvol.dims.xyz)) {
        return false;
    }
    // Only where particles were splatted this frame or the last (the rest
    // holds zeros already; the last frame's region is cleared once).
    let cmin = vec3(wg_bounds[0], wg_bounds[1], wg_bounds[2]);
    let cmax = vec3(wg_bounds[3], wg_bounds[4], wg_bounds[5]);
    let pmin = vec3(wg_bounds[6], wg_bounds[7], wg_bounds[8]);
    let pmax = vec3(wg_bounds[9], wg_bounds[10], wg_bounds[11]);
    let in_cur = all(id + 1u >= cmin) && all(id <= cmax + 1u);
    let in_prev = all(id + 1u >= pmin) && all(id <= pmax + 1u);
    if (!in_cur && !in_prev) {
        return false;
    }
    let dims = rvol.dims.xyz;
    let at = ((id.z * dims.y + id.y) * dims.x + id.x) * CH;
    // (Read, then cleared for the next frame's splat: nothing else touches
    // the accumulators during this pass. Plain: an exchange each.)
    let plain = rvol.look6.z > 0.5;
    var s0 = 0.0;
    var ch: array<f32, 8>;
    if (plain) {
        s0 = f32(atomicExchange(&racc[at], 0u)) / SCALE;
        for (var k = 0u; k < 8u; k++) {
            ch[k] = f32(atomicExchange(&racc[at + 1u + k], 0u));
        }
    } else {
        s0 = f32(atomicLoad(&racc[at])) / SCALE;
        atomicStore(&racc[at], 0u);
        for (var k = 0u; k < 8u; k++) {
            ch[k] = f32(atomicLoad(&racc[at + 1u + k]));
            atomicStore(&racc[at + 1u + k], 0u);
        }
    }
    let edge = vec3<f32>(min(id, dims - 1u - id));
    let fade = clamp(min(edge.x, edge.y) / 6.0, 0.0, 1.0) * clamp(edge.z / 3.0, 0.0, 1.0);
    // Below the brick threshold the density is faded out to zero
    // (0.005..0.01/m), so a marked brick and an unmarked one agree: no
    // step in thin haze where the march skips an empty brick.
    let sigma = s0 * fade * smoothstep(0.005, 0.01, s0 * fade);
    let inv = 1.0 / max(s0 * SCALE, 1e-3);
    textureStore(dens_out, id, vec4(sigma, ch[0] * inv, ch[1] * inv, 0.0));
    let alb = vec3(ch[2], ch[3], ch[4]) * inv;
    textureStore(albedo_out, id, vec4(select(vec3(0.7), alb, s0 > 1e-5), 1.0));
    textureStore(emit_out, id, vec4(vec3(ch[5], ch[6], ch[7]) / max(s0 * 4096.0, 1e-3), 0.0));
    // (Below 0.005/m a brick is not worth marching: 20 m of it is τ 0.1,
    // and the erosion thins it further; those voxels hold zero.)
    if (sigma > 0.0 && plain) {
        let v = vec3<i32>(id);
        for (var k = 0; k < 8; k++) {
            let o = vec3(k & 1, (k >> 1u) & 1, (k >> 2u) & 1) * 2 - 1;
            atomicMax(&bricks[brick_index(v + o)], 1u);
        }
        return false;
    }
    if (sigma > 0.0) {
        let v = vec3<i32>(id);
        let gb = vec3<i32>(id / 4u) / 2;
        for (var k = 0; k < 8; k++) {
            let o = vec3(k & 1, (k >> 1u) & 1, (k >> 2u) & 1) * 2 - 1;
            // The brick of v + o (`brick_index`'s division) relative to the workgroup's.
            let r = (v + o) / i32(BRICK) - gb + 1;
            atomicOr(&wg_bricks[u32(r.x + 3 * r.y + 9 * r.z)], 1u);
        }
        return true;
    }
    return false;
}

// After the resolve: the occupied bricks' bounds (for the march), and
// this frame's particle bounds become the last frame's.
var<workgroup> wb: array<atomic<u32>, 6>;

@compute @workgroup_size(256)
fn cs_bounds(@builtin(local_invocation_index) li: u32) {
    if (li < 3u) {
        atomicStore(&wb[li], 0xFFFFFFFFu);
        atomicStore(&wb[3u + li], 0u);
    }
    workgroupBarrier();
    let nb = rvol.dims.xyz / BRICK;
    let total = nb.x * nb.y * nb.z;
    for (var i = li; i < total; i += 256u) {
        if (atomicLoad(&bricks[i]) != 0u) {
            let b = vec3(i % nb.x, (i / nb.x) % nb.y, i / (nb.x * nb.y));
            for (var k = 0u; k < 3u; k++) {
                atomicMin(&wb[k], b[k]);
                atomicMax(&wb[3u + k], b[k]);
            }
        }
    }
    workgroupBarrier();
    if (li < 6u) {
        atomicStore(&rbounds[12u + li], atomicLoad(&wb[li]));
        let cur = atomicLoad(&rbounds[li]);
        atomicStore(&rbounds[6u + li], cur);
        atomicStore(&rbounds[li], select(0u, 0xFFFFFFFFu, li < 3u));
    }
}


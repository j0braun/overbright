//!pass lock
// ffx_fsr2_lock.h: thin features (a luma ridge no 2 x 2 of similar
// neighbours explains) get a new lock at their display pixel.
@group(0) @binding(2) var lock_luma: texture_2d<f32>;
@group(0) @binding(3) var<storage, read_write> new_locks: array<f32>;

fn thin_feature(p: vec2<i32>) -> bool {
    let size = vec2<i32>(render_size());
    let nucleus = textureLoad(lock_luma, p, 0).x;
    let similar = 1.05;
    var dmin = 3.4e38;
    var dmax = 0.0;
    var mask = 1u << 4u;
    var idx = 0u;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            if !(x == 0 && y == 0) {
                let l = textureLoad(lock_luma, clamp_load(p + vec2<i32>(x, y), size), 0).x;
                let diff = max(l, nucleus) / min(l, nucleus);
                if diff > 0.0 && diff < similar {
                    mask |= 1u << idx;
                } else {
                    dmin = min(dmin, l);
                    dmax = max(dmax, l);
                }
            }
            idx += 1u;
        }
    }
    let ridge = nucleus > dmax || nucleus < dmin;
    if !ridge {
        return false;
    }
    let rejections = array<u32, 4>(
        (1u << 0u) | (1u << 1u) | (1u << 3u) | (1u << 4u),
        (1u << 1u) | (1u << 2u) | (1u << 4u) | (1u << 5u),
        (1u << 3u) | (1u << 4u) | (1u << 6u) | (1u << 7u),
        (1u << 4u) | (1u << 5u) | (1u << 7u) | (1u << 8u));
    for (var i = 0; i < 4; i++) {
        if (mask & rejections[i]) == rejections[i] {
            return false;
        }
    }
    return true;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = vec2<i32>(id.xy);
    if !on_screen(p, vec2<i32>(render_size())) {
        return;
    }
    if thin_feature(p) {
        let hr = hr_from_lr(p);
        if on_screen(hr, vec2<i32>(display_size())) {
            new_locks[u32(hr.y) * u32(display_size().x) + u32(hr.x)] = 1.0;
        }
    }
}


// The temporal resolve as a compute pass (TAA, the input at the output's
// size): appended to temporal.wgsl, whose `resolve` it runs unchanged for
// each output pixel, and writes the history through a storage texture.
// One thing differs, and only in what it skips: the car-motion loop leaves
// out, for a whole 8x8 tile at once, every car whose screen rectangle
// (`car_rects`, output pixels, made on the CPU from the car's bounding
// sphere with a margin) the tile does not touch. A pixel the rectangle
// leaves out cannot pass that car's sphere test, so every pixel takes the
// same car, with the same values, as in the fragment pass
// (`Temporal::car_rect`; the pass's self-check compares the two).

@group(1) @binding(0) var history_out: texture_storage_2d<rgba16float, write>;
// (x0, y0, x1, y1) inclusive, output pixels, per car of `cars`
@group(1) @binding(1) var<uniform> car_rects: array<vec4<i32>, 32>;

// The first output pixel of this invocation's tile.
var<private> tile_lo: vec2<i32>;

fn tile_misses(i: i32) -> bool {
    let r = car_rects[i];
    return tile_lo.x + 7 < r.x || tile_lo.y + 7 < r.y || tile_lo.x > r.z || tile_lo.y > r.w;
}

@compute @workgroup_size(8, 8)
fn cs_resolve(@builtin(workgroup_id) wg: vec3<u32>, @builtin(global_invocation_id) id: vec3<u32>) {
    tile_lo = vec2<i32>(wg.xy * 8u);
    if any(id.xy >= vec2<u32>(u.output.xy)) {
        return;
    }
    // A fragment's position is its pixel's centre.
    textureStore(history_out, id.xy, resolve(vec2<f32>(id.xy) + 0.5));
}

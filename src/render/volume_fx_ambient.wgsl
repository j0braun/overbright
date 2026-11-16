// docs/smoke-volumetric.md "Integration": the scene around a
// volume, for its ambient. The target as drawn so far (the lit world, the
// cars, the shadows; before the medium) in 32 × 18 blocks, each the mean
// of 16 × 16 point taps over it, eased with the last frame's (half and
// half: the taps' detail does not flicker), then the blocks' mean. The
// march reads the blocks bilinearly where its pixel and the road under it
// are, plus the mean (volume_fx.wgsl `scene_ambient`). SCENE_TEXTURE is
// texture_2d<f32> or texture_multisampled_2d<f32> (sample 0).

@group(0) @binding(0) var scene: SCENE_TEXTURE;
// AW × AH blocks, then the mean (w: 1 once written)
@group(0) @binding(1) var<storage, read_write> blocks: array<vec4<f32>>;

const AW: u32 = 32u;
const AH: u32 = 18u;

var<workgroup> part: array<vec3<f32>, 64>;

@compute @workgroup_size(8, 8)
fn cs_ambient(@builtin(workgroup_id) wid: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    let size = vec2<f32>(textureDimensions(scene));
    let block = size / vec2(f32(AW), f32(AH));
    var sum = vec3(0.0);
    for (var k = 0u; k < 4u; k++) {
        let tap = vec2<f32>(lid.xy * 2u + vec2(k & 1u, k >> 1u)) + 0.5;
        let p = vec2<i32>((vec2<f32>(wid.xy) + tap / 16.0) * block);
        let c = textureLoad(scene, clamp(p, vec2(0), vec2<i32>(size) - 1), 0).rgb;
        // (NaN / inf safe: a broken texel adds nothing.)
        sum += select(vec3(0.0), min(c, vec3(16.0)), c == c);
    }
    part[li] = sum;
    workgroupBarrier();
    for (var s = 32u; s > 0u; s >>= 1u) {
        if (li < s) {
            part[li] += part[li + s];
        }
        workgroupBarrier();
    }
    if (li == 0u) {
        let i = wid.y * AW + wid.x;
        let now = part[0] / 256.0;
        let old = blocks[i];
        blocks[i] = vec4(select(now, mix(old.rgb, now, 0.5), blocks[AW * AH].w > 0.5), 1.0);
    }
}

var<workgroup> rows: array<vec3<f32>, 64>;

@compute @workgroup_size(64)
fn cs_ambient_mean(@builtin(local_invocation_index) li: u32) {
    var sum = vec3(0.0);
    for (var i = li; i < AW * AH; i += 64u) {
        sum += blocks[i].rgb;
    }
    rows[li] = sum;
    workgroupBarrier();
    for (var s = 32u; s > 0u; s >>= 1u) {
        if (li < s) {
            rows[li] += rows[li + s];
        }
        workgroupBarrier();
    }
    if (li == 0u) {
        blocks[AW * AH] = vec4(rows[0] / f32(AW * AH), 1.0);
    }
}

// The ray-traced reflections' hit textures made on the GPU (rt/scene.rs
// `hit_texture_gpu`): every level of one layer in one dispatch, from the
// world texture as the game stores it (DXT with its mip chain), each texel
// sampled trilinear at the source level whose texels are its level's size
// (a box of the source's texels).
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var level0: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var level1: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(4) var level2: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(5) var level3: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(6) var level4: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(7) var level5: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(8) var level6: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(9) var level7: texture_storage_2d<rgba8unorm, write>;

// Level m's texel at q (side `side`) from the source.
fn shrink(q: vec2<u32>, side: f32) -> vec4<f32> {
    let s = vec2<f32>(textureDimensions(src, 0));
    let lod = log2(max(max(s.x, s.y) / side, 1.0));
    return textureSampleLevel(src, src_samp, (vec2<f32>(q) + 0.5) / side, lod);
}

@compute @workgroup_size(8, 8)
fn cs_shrink(@builtin(global_invocation_id) id: vec3<u32>) {
    let q = id.xy;
    if all(q < vec2<u32>(128u)) { textureStore(level0, q, shrink(q, 128.0)); }
    if all(q < vec2<u32>(64u)) { textureStore(level1, q, shrink(q, 64.0)); }
    if all(q < vec2<u32>(32u)) { textureStore(level2, q, shrink(q, 32.0)); }
    if all(q < vec2<u32>(16u)) { textureStore(level3, q, shrink(q, 16.0)); }
    if all(q < vec2<u32>(8u)) { textureStore(level4, q, shrink(q, 8.0)); }
    if all(q < vec2<u32>(4u)) { textureStore(level5, q, shrink(q, 4.0)); }
    if all(q < vec2<u32>(2u)) { textureStore(level6, q, shrink(q, 2.0)); }
    if all(q < vec2<u32>(1u)) { textureStore(level7, q, shrink(q, 1.0)); }
}

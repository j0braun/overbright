
#if LAMPS
#if TEXTURE_BINDING_BOUND
struct TextureImmediates { scroll: Scroll, layer: u32, surface_layer: u32, surface_material: u32, array: u32 };
#else
struct TextureImmediates { scroll: Scroll, layer: u32, surface_layer: u32, surface_material: u32 };
#endif
#else
#if TEXTURE_BINDING_BOUND
struct TextureImmediates { scroll: Scroll, layer: u32, unused0: u32, unused1: u32, array: u32 };
#else
struct TextureImmediates { scroll: Scroll, layer: u32 };
#endif
#endif
var<immediate> imm: TextureImmediates;
#if TEXTURE_BINDING_BOUND
@group(1) @binding(0) var tex: binding_array<texture_2d_array<f32>, 256>;
#else
@group(1) @binding(0) var tex: texture_2d_array<f32>;
#endif
@group(1) @binding(1) var samp: sampler;
fn sample_tex(uv: vec2<f32>) -> vec4<f32> {
#if TEXTURE_BINDING_BOUND
#if !MIP_BIAS
    return textureSample(tex[imm.array], samp, uv, imm.layer);
#else
    return textureSampleBias(tex[imm.array], samp, uv, imm.layer, MIP_BIAS);
#endif
#else
#if !MIP_BIAS
    return textureSample(tex, samp, uv, imm.layer);
#else
    return textureSampleBias(tex, samp, uv, imm.layer, MIP_BIAS);
#endif
#endif
}
fn tex_scroll() -> Scroll {
    return imm.scroll;
}
#if LAMPS
fn surface_slot() -> vec2<u32> {
    return vec2<u32>(imm.surface_layer, imm.surface_material);
}
#endif


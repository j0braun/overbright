// BEGIN TEXTURE BINDING
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;
@group(1) @binding(2) var<uniform> scroll: Scroll;
fn sample_tex(uv: vec2<f32>) -> vec4<f32> {
#if !MIP_BIAS
    return textureSample(tex, samp, uv);
#else
    return textureSampleBias(tex, samp, uv, MIP_BIAS);
#endif
}
fn tex_scroll() -> Scroll {
    return scroll;
}
#if LAMPS
// BEGIN LAMPS
// Surface Detail needs texture arrays (the slot's map layer and material
// ride in the immediates): none here.
fn surface_slot() -> vec2<u32> {
    return vec2<u32>(0u);
}
// END LAMPS
#endif
// END TEXTURE BINDING

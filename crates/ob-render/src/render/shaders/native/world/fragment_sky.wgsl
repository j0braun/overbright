@fragment
#if !MULTIVIEW
fn fs_reflect(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_reflect(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return reflect_colour(in, false);
}

@fragment
#if !MULTIVIEW
fn fs_reflect_alpha_test(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_reflect_alpha_test(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return reflect_colour(in, true);
}

// The sky dome while the time of day fades (0x5C5940 → effect 20):
// lerp(SKYMORPHTEXTURE, tex, SKYMORPHALPHA) * v0 * 2, fogged. Otherwise the
// dome is drawn with world.efx like any other mesh.
@fragment
#if !MULTIVIEW
fn fs_skymorph(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_skymorph(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    let texel = mix(textureSample(morph_tex, morph_samp, in.uv), sample_tex(in.uv), frame.clip.z);
    // `mul r0_x2, r0, v0` writes all four components: the alpha is doubled
    // too (and clamped by the A8R8G8B8 target), unlike world.efx's.
    let c = shade(texel, in.color);
    return vec4<f32>(apply_fog(c.rgb, in.fog), clamp(c.a * 2.0, 0.0, 1.0));
}

fn skymorph_colour(in: VertexOut) -> vec4<f32> {
    let texel = mix(textureSample(morph_tex, morph_samp, in.uv), sample_tex(in.uv), frame.clip.z);
    let c = shade(texel, in.color);
    return vec4<f32>(apply_fog(c.rgb, in.fog), clamp(c.a * 2.0, 0.0, 1.0));
}


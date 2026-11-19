// docs/notes/graphics/car-look.md "Darkening settings against emissive light and the car's shadow": the cars' shadow texture (alpha-blended,
// its colour near black: it leaves dst · (1 − a)) over a ground Contact
// Shadows already left at vc of its light: the decal's alpha is cut so the
// two give the darker of (1 − a) and vc, not their product (the native
// texture and the contact shadow both stand for the car's occlusion).
fn car_shadow_colour(in: VertexOut, alpha_test: bool) -> vec4<f32> {
    var c = world_colour(in, alpha_test, false);
    if (D9_CONTACT && frame.lamps.y > 0.0) {
        let vc = contact_visibility(in.world_position, in.screen);
        let keep = clamp((1.0 - c.a) / max(vc, 1e-3), 0.0, 1.0);
        c.a = 1.0 - keep;
    }
    return c;
}

@fragment
#if !MULTIVIEW
fn fs_car_shadow(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_car_shadow(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return car_shadow_colour(in, false);
}

@fragment
#if !MULTIVIEW
fn fs_car_shadow_alpha_test(in: VertexOut) -> @location(0) vec4<f32> {
#else
fn fs_car_shadow_alpha_test(in: VertexOut, @builtin(view_index) view_index: u32) -> @location(0) vec4<f32> {
    frame = frames[view_index];
#endif
    return car_shadow_colour(in, true);
}

// Lamp Shadows' depth layers: the scenery from a lamp or the headlight
// (WorldRenderer::plan_lamp_shadows); alpha-tested draws keep their
// holes (fences, foliage).
@fragment
#if !MULTIVIEW
fn fs_shadow_alpha_test(in: VertexOut) {
#else
fn fs_shadow_alpha_test(in: VertexOut, @builtin(view_index) view_index: u32) {
    frame = frames[view_index];
#endif
    let t = sample_tex(in.uv);
    if t.a * in.color.a <= ALPHA_REF {
        discard;
    }
}

// Depth pre-pass (clone, enhanced lighting only; docs/notes/performance/gpu.md, depth pre-pass):
// the main view's opaque draws' depth before the colour pass, with the
// same vertex entry points (so the same depth) and the same coverage as
// their colour pipelines: the alpha test's discard on the same texel and
// vertex alpha (`world_colour`/`reflect_colour` test texel.a * color.a),
// LOD Fade's screen-door mask. Non-alpha-tested draws without a dither
// have no fragment stage.
@fragment
#if !MULTIVIEW
fn fs_depth_alpha_test(in: VertexOut) {
#else
fn fs_depth_alpha_test(in: VertexOut, @builtin(view_index) view_index: u32) {
    frame = frames[view_index];
#endif
    let t = sample_tex(in.uv);
    if t.a * in.color.a <= ALPHA_REF {
        discard;
    }
}

@fragment
#if !MULTIVIEW
fn fs_depth_fade(in: VertexOutFade) -> @builtin(sample_mask) u32 {
#else
fn fs_depth_fade(in: VertexOutFade, @builtin(view_index) view_index: u32) -> @builtin(sample_mask) u32 {
    frame = frames[view_index];
#endif
    return fade_mask(in.clip, in.fade);
}

@fragment
#if !MULTIVIEW
fn fs_depth_alpha_test_fade(in: VertexOutFade) -> @builtin(sample_mask) u32 {
#else
fn fs_depth_alpha_test_fade(in: VertexOutFade, @builtin(view_index) view_index: u32) -> @builtin(sample_mask) u32 {
    frame = frames[view_index];
#endif
    let t = sample_tex(in.uv);
    if t.a * in.color.a <= ALPHA_REF {
        discard;
    }
    return fade_mask(in.clip, in.fade);
}

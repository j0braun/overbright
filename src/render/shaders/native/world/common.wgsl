// World shaders, transcribed from the game's own effects (embedded as
// resources in SPEED2.EXE, disassembled from their vs_1_1/ps_1_1 bytecode):
//
//   world.efx        oFog = clamp((FogValue.x - viewZ) * FogValue.y, FogValue.z, 1)
//                    r0 = tex * v0; r0.rgb *= 2        (alpha = tex.a * v0.a)
//   worldnofog.efx   the same without fog
//   worldreflect.efx r0 = tex * v0; rgb = 2 * r0.rgb + reflected * r0.a; a = 1
//                    reflected = ReflectedTex at the pixel's screen position
//   skymorph.efx     lerp(tex1, tex0, SkyMorphAlpha) * v0 * 2
//   worldheadlight.efx (ps_1_1, its second technique)
//                    r0.rgb = HeadlightTex(t1) * HeadlightClipTex(t2) * HeadlightColor + v0
//                    rgb = sat(2 * tex * r0), a = sat(tex.a * v0.a), fogged as world
//                    vs: t1 = HeadlightWVP * pos (projected), t2 = (0.005 * w + 0.5, 0)
//   worldheadlightreflect.efx
//                    rgb = 2 * tex * r0 + reflected * tex.a * v0.a, a = 1
//
// D3D9 then blends   colour * fog + FogColor * (1 - fog).
// All maths happens on gamma-space values, as on the original hardware
// (textures and targets are UNORM, not sRGB).

// BEGIN RTVIS FRAME
struct Frame {
    view_proj: mat4x4<f32>,
    camera: vec4<f32>,      // xyz = eye position
    forward: vec4<f32>,     // xyz = unit view direction (view-space depth axis)
    fog_value: vec4<f32>,   // the effect's FogValue (x, y, z)
    fog_color: vec4<f32>,   // rgb
    clip: vec4<f32>,        // x = 1 for enhanced reflection shading, otherwise -inf; y = time (s), z = SKYMORPHALPHA, w = 1 native road/effects, 2 planar scenery (x = plane height)
    cull: vec4<f32>,        // x = the view's size scale (context +0x80), y = the size it needs
    // View 9, the headlight projector: eye (w = 1 when on), right (w = the
    // projection's x scale), up (w = its y scale), forward (w = view 9's
    // size scale), colour (HeadlightColor).
    hl_eye: vec4<f32>,
    hl_right: vec4<f32>,
    hl_up: vec4<f32>,
    hl_forward: vec4<f32>,
    hl_color: vec4<f32>,
    // Wet Roads (clone deviation, docs/wet-roads-ssr.md), main view only:
    // x = 1 on and the road wet (else 0, the native path), y = the rain's
    // amount (weather +0x1F0), z = the rain's road wetness (the native
    // +0x509C rule from rain only, `Weather::road_wet`), w = the world
    // clock (s, 0x7FB718: the ripples' time).
    wet: vec4<f32>,
    // Dynamic Lamps / Lamp Shadows / Contact Shadows (clone deviations,
    // docs/lamps-shadows.md), main view only: x = 1 lamps on, y = the
    // contact shadow's depth (0 off), z = Lamp Shadows (0 off, 1 Low,
    // 2 High), w = the car occluders. cluster: tiles x, y, slices,
    // slices / ln(far / near); cluster_near: x = near, y = the headlight's
    // shadow layer (-1 none).
    lamps: vec4<f32>,
    cluster: vec4<f32>,
    cluster_near: vec4<f32>,
};
// END RTVIS FRAME

// Per-texture scrolling (see formats::texture::Scroll).
struct Scroll {
    kind_speed_step: vec4<f32>,  // kind, speed u, speed v, step
    fixed: vec4<f32>,            // fixed offset u, v (kind 3)
};

#if !MULTIVIEW
@group(0) @binding(0) var<uniform> frame: Frame;
#else
@group(0) @binding(0) var<uniform> frames: array<Frame, 6>;
var<private> frame: Frame;
#endif
@group(0) @binding(1) var headlight_tex: texture_2d<f32>;
@group(0) @binding(2) var headlight_samp: sampler;
// The draw's texture and its scroll. With texture arrays (see
// `TextureArrays` in render/mod.rs) the renderer swaps this block for an
// array texture, the layer and scroll coming in immediates.

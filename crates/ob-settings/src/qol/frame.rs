//! The values in force a frame's draw reads, copied once
//! ([`RenderOptions::in_force`]; the frame's settings value is
//! `render::FrameSettings`, docs/developing/frame-settings.md).

use super::*;

/// The settings store's values in force that the draw reads, copied once
/// ([`RenderOptions::in_force`], `crate::qol`). Each field is the value
/// the getter of the same name returns.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RenderOptions {
    // Anti-aliasing, scale and the temporal pass.
    /// The Anti-Aliasing row (0 off, 1 FXAA, 2 SMAA, 3–5 MSAA, 6–10 SSAA, TAA).
    pub post_aa: u8,
    /// The MSAA combination row (0 the preset's).
    pub aa_msaa: u8,
    /// Render Scale (%).
    pub render_scale: u8,
    /// Upscaling (0 off).
    pub upscale: u8,
    /// The temporal pass is chosen (TAA or a temporal upscale).
    pub temporal: bool,
    /// The frame being drawn runs the temporal pass (the draw's own
    /// decision, `qol::set_temporal_frame`; at a load: the last frame's).
    pub temporal_frame: bool,
    pub fsr: bool,
    pub fsr2: bool,
    pub reversed_z: bool,
    /// Texture Filtering's anisotropy (0 native).
    pub anisotropy: u16,
    // The Graphics page's passes after the native chain.
    /// FXAA in force (off under the temporal pass).
    pub fxaa: bool,
    /// SMAA 1x in force (off under the temporal pass).
    pub smaa: bool,
    /// The CAS sharpness, 0 (off) to 1.
    pub sharpen: f32,
    /// Depth Of Field's tier (0 native, 1 Cinematic).
    pub depth_of_field: u8,
    // HDR.
    /// High Precision Post in force (HDR Output's float chain counted).
    pub hdr_post: u8,
    pub hdr_active: bool,
    pub hdr_output_active: bool,
    // The world's detail.
    pub lod_fade: bool,
    pub pop_free: bool,
    /// Keep All Loaded (the drawn sections widened to the look-ahead ring).
    pub keep_loaded: bool,
    /// World Detail Extreme's far sections.
    pub far_draw: bool,
    /// Detail Distance's factor (1 native).
    pub detail_distance: f32,
    /// Car Detail Distance's factor (1 native).
    pub car_detail: f32,
    /// The window's (or offscreen) output size (None: not known yet).
    pub output_size: Option<(u32, u32)>,
    // Reflections and shadows.
    /// Road Reflection Res (0 native, 1 half, 2 full).
    pub road_reflection: u8,
    /// The cars' environment cube's edge (128 native).
    pub car_reflection_size: u32,
    pub enhanced_reflections: bool,
    pub reflection_every_frame: bool,
    pub shadow_quality: u8,
    pub ssr: u8,
    /// The ambient occlusion's strength, 0 to 1.
    pub ssao: f32,
    // Lights and materials.
    pub dynamic_lamps: bool,
    pub lamp_shadows: u8,
    pub contact_shadows: bool,
    pub headlight_beams: bool,
    pub car_lights_next_gen: bool,
    pub light_probes: bool,
    pub surface_detail: u8,
    /// Modern Lighting in force (0 Original, 1 Low, 2 Modern, 3 Modern +
    /// RT only while the device traces).
    pub modern_lighting: u8,
    /// Modern Lighting's strength (%).
    pub modern_strength: u8,
    pub enhanced_glass: bool,
    pub enhanced_paint: bool,
    pub scene_car_specular: bool,
    // Weather and effects.
    pub ambient_detail: u8,
    pub wet_roads: bool,
    pub wet_car: bool,
    pub wet_interaction: u8,
    pub weather_fx: bool,
    /// Rain Detail (0 off, 1 On, 2 High, 3 Next-Gen).
    pub rain_detail: u8,
    /// Spray & Splashes.
    pub spray: bool,
    pub lens_drops: bool,
    pub particles: u8,
    pub sparks_fire: u8,
    pub animated_water: bool,
    /// Animated Water's tier (0 Off, 1 On, 2 Realistic, 3 Next-Gen).
    pub water_tier: u8,
    /// Sky (0 Original, 1 Next-Gen).
    pub sky: u8,
    pub volumetric_fog: u8,
    pub motion_blur: u8,
}

impl RenderOptions {
    /// `qol::rt_reflections`: ray-traced reflections in force.
    pub fn rt_reflections(&self) -> bool {
        self.ssr == 3
    }

    /// `qol::rain_next_gen`: Rain Detail at Next-Gen.
    pub fn rain_next_gen(&self) -> bool {
        self.rain_detail >= Self::RAIN_NEXT_GEN
    }

    /// `qol::output_size_or`: the output size, or `requested` while none
    /// is known.
    pub fn output_size_or(&self, requested: (u32, u32)) -> (u32, u32) {
        self.output_size.unwrap_or(requested)
    }
}

impl RenderOptions {
    /// [`DETAIL_FULL_PX`], for the draw.
    pub const DETAIL_FULL_PX: f32 = DETAIL_FULL_PX;
    /// [`LOD_FADE_SECONDS`], for the draw.
    pub const LOD_FADE_SECONDS: f32 = LOD_FADE_SECONDS;
    /// [`SECTION_FADE_SECONDS`], for the draw.
    pub const SECTION_FADE_SECONDS: f32 = SECTION_FADE_SECONDS;
    /// [`LOD_FADE_KEEP`], for the draw.
    pub const LOD_FADE_KEEP: f32 = LOD_FADE_KEEP;
    /// [`RAIN_NEXT_GEN`], for the draw.
    pub const RAIN_NEXT_GEN: u8 = RAIN_NEXT_GEN;
    /// [`FAR_RING`], for the draw.
    pub const FAR_RING: f32 = FAR_RING;

    /// The settings store's values in force now, each as its getter
    /// returns it. Taken once per drawn frame (and by a load that builds
    /// renderers, at that moment).
    pub fn in_force() -> RenderOptions {
        RenderOptions {
            post_aa: post_aa(),
            aa_msaa: read(|s| s.values.aa_msaa),
            render_scale: read(|s| s.values.render_scale),
            upscale: upscale(),
            temporal: temporal(),
            temporal_frame: temporal_frame(),
            fsr: fsr(),
            fsr2: fsr2(),
            reversed_z: reversed_z(),
            anisotropy: anisotropy(),
            fxaa: fxaa(),
            smaa: smaa(),
            sharpen: sharpen(),
            depth_of_field: depth_of_field(),
            hdr_post: hdr_post(),
            hdr_active: hdr_active(),
            hdr_output_active: hdr_output_active(),
            lod_fade: lod_fade(),
            pop_free: pop_free(),
            keep_loaded: keep_loaded(),
            far_draw: far_draw(),
            detail_distance: detail_distance(),
            car_detail: car_detail(),
            output_size: {
                let size = read(|s| s.output_size);
                (size != 0).then(|| ((size >> 32) as u32, size as u32))
            },
            road_reflection: read(|s| s.values.road_reflection),
            car_reflection_size: car_reflection_size(),
            enhanced_reflections: enhanced_reflections(),
            reflection_every_frame: reflection_every_frame(),
            shadow_quality: shadow_quality(),
            ssr: ssr(),
            ssao: ssao(),
            dynamic_lamps: dynamic_lamps(),
            lamp_shadows: lamp_shadows(),
            contact_shadows: contact_shadows(),
            headlight_beams: headlight_beams(),
            car_lights_next_gen: car_lights_next_gen(),
            light_probes: light_probes(),
            surface_detail: surface_detail(),
            modern_lighting: modern_lighting(),
            modern_strength: modern_strength(),
            enhanced_glass: enhanced_glass(),
            enhanced_paint: enhanced_paint(),
            scene_car_specular: scene_car_specular(),
            ambient_detail: ambient_detail(),
            wet_roads: wet_roads(),
            wet_car: wet_car(),
            wet_interaction: wet_interaction(),
            weather_fx: weather_fx(),
            rain_detail: rain_detail(),
            spray: spray(),
            lens_drops: lens_drops(),
            particles: particles(),
            sparks_fire: sparks_fire(),
            animated_water: animated_water(),
            water_tier: water_tier(),
            sky: sky(),
            volumetric_fog: volumetric_fog(),
            motion_blur: motion_blur(),
        }
    }

    /// [`msaa_override`] for these values: the scene's sample count the
    /// Anti-Aliasing row asks for instead of Full Screen Anti-Aliasing's,
    /// if any.
    pub fn msaa_override(&self) -> Option<u32> {
        msaa_in_frame(self.temporal, self.temporal_frame, self.post_aa, self.aa_msaa)
    }

    /// [`upscale_share`]: the share of each side an Upscaling tier `v`
    /// renders at (None: off).
    pub fn upscale_share(v: u8) -> Option<f64> {
        upscale_share(v)
    }

    /// [`supported_msaa`]: the largest sample count up to `wanted` the
    /// device offers (its caps, set when it was made).
    pub fn supported_msaa(&self, wanted: u32) -> u32 {
        supported_msaa(wanted)
    }

    /// [`road_reflection_size`] for these values: the road reflection
    /// image's size for a scene `scene` big (native 320 × 240 `native`,
    /// half the scene's or the scene's own).
    pub fn road_reflection_size(&self, native: (u32, u32), scene: (u32, u32)) -> (u32, u32) {
        road_reflection_size_for(self.road_reflection, native, scene)
    }

    /// `qol::scene_size` for these values: the 3D scene's size for an
    /// output `out` at `samples` (the adapter's caps as set at start-up).
    pub fn scene_size(&self, out: (u32, u32), samples: u32) -> (u32, u32) {
        scene_size_for(self.upscale, self.render_scale, self.post_aa, out, samples)
    }
}

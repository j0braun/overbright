//! The values in force a frame's draw reads, copied once
//! ([`RenderOptions::in_force`]; the frame's settings value is
//! `render::FrameSettings`, docs/frame-settings.md).

use super::*;
use crate::render::RenderOptions;

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
            aa_msaa: AA_MSAA.load(Ordering::Relaxed),
            render_scale: RENDER_SCALE.load(Ordering::Relaxed),
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
                let size = OUTPUT_SIZE.load(Ordering::Relaxed);
                (size != 0).then(|| ((size >> 32) as u32, size as u32))
            },
            road_reflection: ROAD_REFLECTION.load(Ordering::Relaxed),
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

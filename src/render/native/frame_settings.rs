//! The settings a drawn frame reads, taken once per frame.
//!
//! A frame's draw reads its options from one value, [`FrameSettings`],
//! built where the frame's settings are final (the game's `frame_targets`:
//! after the display mode, the HDR switch, the pack reload and the
//! temporal decision; a shot's `render_offscreen` at its start) and passed
//! down through the frame's inputs. The renderer, the post chain and the
//! Enhanced effects' draw halves read it from there, not from the settings
//! store's globals (`crate::qol`, the values in force), so a value cannot
//! change within a frame, also not on a render thread. The values are the
//! store's own: [`RenderOptions::in_force`] copies them with the same
//! rules the store's getters apply (a ray-traced tier only while the
//! device traces, HDR only where supported, ...). Code outside a frame
//! that builds renderers (a load, a renderer made on its first frame)
//! takes its own copy at that moment, as it read the globals then.

use super::display::Display;
use super::post::Switches;

/// The settings one drawn frame reads: the Customize Display values the
/// renderer takes ([`Display`]) and the screen passes' switches
/// ([`Switches`]), each computed once from the custom values, and the
/// enhancement rows in force ([`RenderOptions`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameSettings {
    pub display: Display,
    pub switches: Switches,
    pub opts: RenderOptions,
}

impl FrameSettings {
    /// The frame's settings from UI_PC_Customize_Options' values and the
    /// rows in force.
    pub fn new(custom: &[u8; 22], opts: RenderOptions) -> FrameSettings {
        FrameSettings { display: Display::from_custom(custom), switches: Switches::from_custom(custom), opts }
    }
}

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

//! The settings a tick reads, taken once per tick.
//!
//! [`TickSettings`] is read from the settings store (the custom values and
//! the Enhancements rows in force, `nfsu2::qol`) once per tick, in
//! `Game::update_world` right after all of the tick's input is applied
//! (the key queue, then the pads and the mouse in `tick_input`): an option
//! changed by a key, a pad button or a mouse click acts there, so the tick
//! sees its new value, as it did when every phase read the globals. The phases after the world update
//! read it from there: the particles' births for the Enhanced effects
//! (`Game::particle_effects`), the Enhanced effects' tick
//! (`Game::enh_fx_tick`), the native visuals (`Game::visuals_tick`). The
//! simulation's session gets it in its `TickInput`.

use nfsu2::render::postfx::Switches;

/// The settings one tick reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TickSettings {
    /// The screen passes' switches from the custom values (Light Glow,
    /// Horizon Fog, Motion Blur, ...), computed once.
    pub(crate) switches: Switches,
    /// Particles (0 Original, 1 High, 2 Ultra, 3 Cinematic).
    pub(crate) particles: u8,
    /// Sparks & Fire (0 Original).
    pub(crate) sparks_fire: u8,
    /// Motion Blur's clone tier (0 native).
    pub(crate) motion_blur: u8,
    /// Any weather row on.
    pub(crate) weather_fx: bool,
    /// Wet Road Interaction (0 off).
    pub(crate) wet_interaction: u8,
    /// World Detail Ultra / Extreme's pop-in fixes in force.
    pub(crate) pop_free: bool,
    /// LOD Fade.
    pub(crate) lod_fade: bool,
    /// Keep All Loaded (the sections drawn from the tick's eye).
    pub(crate) keep_loaded: bool,
    /// World Detail Extreme's far sections.
    pub(crate) far_draw: bool,
    /// Rain Detail (0 off .. 3 Next-Gen): the spray's and the lens
    /// drops' steps.
    pub(crate) rain_detail: u8,
    /// Spray & Splashes.
    pub(crate) spray: bool,
    /// Lens Drops.
    pub(crate) lens_drops: bool,
    /// Ambient Detail (0 Off, 1 On, 2 Next-Gen).
    pub(crate) ambient_detail: u8,
    /// Sky (0 Original, 1 Next-Gen): the panorama rows' ticks.
    pub(crate) sky: u8,
    /// The Enhancements page's Field Of View offset (radians, 0 native):
    /// the chase camera's angle.
    pub(crate) fov_offset: f32,
}

impl TickSettings {
    /// The store's values now, each as its getter returns it.
    pub(crate) fn take(custom: &[u8; 22]) -> TickSettings {
        TickSettings {
            switches: Switches::from_custom(custom),
            particles: nfsu2::qol::particles(),
            sparks_fire: nfsu2::qol::sparks_fire(),
            motion_blur: nfsu2::qol::motion_blur(),
            weather_fx: nfsu2::qol::weather_fx(),
            wet_interaction: nfsu2::qol::wet_interaction(),
            pop_free: nfsu2::qol::pop_free(),
            lod_fade: nfsu2::qol::lod_fade(),
            keep_loaded: nfsu2::qol::keep_loaded(),
            far_draw: nfsu2::qol::far_draw(),
            rain_detail: nfsu2::qol::rain_detail(),
            spray: nfsu2::qol::spray(),
            lens_drops: nfsu2::qol::lens_drops(),
            ambient_detail: nfsu2::qol::ambient_detail(),
            sky: nfsu2::qol::sky(),
            fov_offset: nfsu2::qol::fov_offset(),
        }
    }
}

impl Default for TickSettings {
    /// Before the first tick: the native values (every tick replaces it
    /// before a phase reads it).
    fn default() -> TickSettings {
        TickSettings {
            switches: Switches::from_detail(0),
            particles: 0,
            sparks_fire: 0,
            motion_blur: 0,
            weather_fx: false,
            wet_interaction: 0,
            pop_free: false,
            lod_fade: false,
            keep_loaded: false,
            far_draw: false,
            rain_detail: 0,
            spray: false,
            lens_drops: false,
            ambient_detail: 0,
            sky: 0,
            fov_offset: 0.0,
        }
    }
}

impl TickSettings {
    /// What the tick-stepped enhancement effects read of these
    /// (`render::enh::tick_views`).
    pub(crate) fn tick_options(&self) -> nfsu2::render::enh::tick_views::TickOptions {
        nfsu2::render::enh::tick_views::TickOptions {
            lod_fade: self.lod_fade,
            pop_free: self.pop_free,
            keep_loaded: self.keep_loaded,
            far_draw: self.far_draw,
        }
    }

    /// What the world renderer's tick hook reads of these
    /// (`WorldRenderer::enh_tick`).
    pub(crate) fn enh_options(&self) -> nfsu2::render::weather_fx::EnhTickOptions {
        nfsu2::render::weather_fx::EnhTickOptions {
            weather_fx: self.weather_fx,
            wet_interaction: self.wet_interaction,
            rain_detail: self.rain_detail,
            spray: self.spray,
            lens_drops: self.lens_drops,
            ambient_detail: self.ambient_detail,
            sky: self.sky,
        }
    }
}

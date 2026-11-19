use super::*;

/// The settings groups: where a row sits on the new page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UxGroup {
    Display,
    ImageQuality,
    LightingEffects,
    WeatherWater,
    Performance,
    GameplayAccessibility,
    /// See docs/playing/controller-support.md.
    Controller,
    Packs,
}

impl UxGroup {
    pub fn name(self) -> &'static str {
        match self {
            UxGroup::Display => "Display",
            UxGroup::ImageQuality => "Image Quality",
            UxGroup::LightingEffects => "Lighting & Effects",
            UxGroup::WeatherWater => "Weather & Water",
            UxGroup::Performance => "Performance",
            UxGroup::GameplayAccessibility => "Gameplay & Accessibility",
            UxGroup::Controller => "Controller",
            UxGroup::Packs => "Packs",
        }
    }
}

/// Whether a row is the original's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// One of the original's own settings only.
    Native,
    /// The original's Customize Display row `n` (`CUSTOM_ROWS`) as its
    /// first tiers, the clone's enhanced tiers above.
    Shared(usize),
    /// A clone enhancement (off = as the original).
    Enhanced,
    /// A clone convenience that changes no image (saves, overlay, keys).
    Utility,
}

/// When a change takes effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applies {
    Live,
    /// When the named thing loads next (the city, a car, free roam).
    NextLoad(&'static str),
    /// After a restart (the device is made with it; `render::rt::wanted`).
    Restart,
    /// Some tiers after a restart (Modern + RT, Ray Traced SSR), the rest live.
    RestartFor(&'static [usize]),
}

/// A row of the settings model.
#[derive(Clone, Copy, Debug)]
pub struct RowModel {
    pub id: RowId,
    pub group: UxGroup,
    pub origin: Origin,
    pub applies: Applies,
}

const fn m(id: RowId, group: UxGroup, origin: Origin, applies: Applies) -> RowModel {
    RowModel { id, group, origin, applies }
}

use Applies::{Live, NextLoad, Restart, RestartFor};
use Origin::{Enhanced, Shared, Utility};
use UxGroup::*;

/// Every clone row (Graphics groups and the Enhancements page), in the settings model's
/// groups. `model_covers_every_row` keeps it complete.
pub static MODEL: &[RowModel] = &[
    // Display.
    m(RowId::HdrOutput, Display, Enhanced, Live),
    m(RowId::HdrPeak, Display, Enhanced, Live),
    m(RowId::HdrPaper, Display, Enhanced, Live),
    m(RowId::HdrGamma, Display, Enhanced, Live),
    // Image Quality.
    m(RowId::PostAa, ImageQuality, Enhanced, Live),
    m(RowId::AaMsaa, ImageQuality, Enhanced, Live),
    m(RowId::AaFilter, ImageQuality, Enhanced, Live),
    m(RowId::Sharpen, ImageQuality, Enhanced, Live),
    m(RowId::TextureFiltering, ImageQuality, Shared(20), Live),
    m(RowId::HdrPost, ImageQuality, Enhanced, Live),
    m(RowId::LodFade, ImageQuality, Enhanced, Live),
    m(RowId::EnhancedReflections, ImageQuality, Enhanced, Live),
    m(RowId::RoadReflection, ImageQuality, Shared(7), Live),
    m(RowId::Ssr, ImageQuality, Enhanced, RestartFor(&[3])),
    m(RowId::EnhancedPaint, ImageQuality, Enhanced, Live),
    m(RowId::EnhancedGlass, ImageQuality, Enhanced, Live),
    m(RowId::CarReflection, ImageQuality, Shared(1), Live),
    m(RowId::ReflectionRate, ImageQuality, Shared(0), Live),
    m(RowId::CarDetail, ImageQuality, Shared(4), Live),
    m(RowId::SmoothCars, ImageQuality, Enhanced, NextLoad("cars")),
    m(RowId::FixModels, ImageQuality, Enhanced, NextLoad("cars")),
    // Lighting & Effects.
    m(RowId::ModernLighting, LightingEffects, Enhanced, RestartFor(&[3])),
    m(RowId::ModernStrength, LightingEffects, Enhanced, Live),
    m(RowId::Ssao, LightingEffects, Enhanced, Live),
    m(RowId::ShadowQuality, LightingEffects, Shared(2), Live),
    m(RowId::DynamicLamps, LightingEffects, Enhanced, Live),
    m(RowId::LampShadows, LightingEffects, Enhanced, Live),
    m(RowId::ContactShadows, LightingEffects, Enhanced, Live),
    m(RowId::HeadlightBeams, LightingEffects, Enhanced, Live),
    m(RowId::LightProbes, LightingEffects, Enhanced, Live),
    m(RowId::SurfaceDetail, LightingEffects, Enhanced, Live),
    m(RowId::VolumetricFog, LightingEffects, Enhanced, Live),
    m(RowId::Sky, LightingEffects, Enhanced, Live),
    m(RowId::Particles, LightingEffects, Enhanced, Live),
    m(RowId::SparksFire, LightingEffects, Enhanced, Live),
    m(RowId::MotionBlur, LightingEffects, Shared(11), Live),
    m(RowId::DepthOfField, LightingEffects, Shared(13), Live),
    m(RowId::AmbientDetail, LightingEffects, Enhanced, Live),
    // Weather & Water.
    m(RowId::WetRoads, WeatherWater, Enhanced, Live),
    m(RowId::WetCar, WeatherWater, Enhanced, Live),
    m(RowId::RainDetail, WeatherWater, Enhanced, Live),
    m(RowId::Spray, WeatherWater, Enhanced, Live),
    m(RowId::WetInteraction, WeatherWater, Enhanced, Live),
    m(RowId::LensDrops, WeatherWater, Enhanced, Live),
    m(RowId::AnimatedWater, WeatherWater, Enhanced, Live),
    // Performance.
    m(RowId::RenderScale, Performance, Enhanced, Live),
    m(RowId::Upscale, Performance, Enhanced, Live),
    m(RowId::DetailDistance, Performance, Shared(6), Live),
    m(RowId::ReversedZ, Performance, Enhanced, Live),
    m(RowId::FrameCap, Performance, Utility, Live),
    m(RowId::LowLatency, Performance, Utility, Live),
    m(RowId::PowerSaving, Performance, Utility, Live),
    m(RowId::PerfOverlay, Performance, Utility, Live),
    m(RowId::OverlayPosition, Performance, Utility, Live),
    m(RowId::OverlayScale, Performance, Utility, Live),
    m(RowId::OverlayLog, Performance, Utility, Live),
    // Gameplay & Accessibility.
    m(RowId::FovOffset, GameplayAccessibility, Enhanced, Live),
    m(RowId::HudScale, GameplayAccessibility, Enhanced, Live),
    m(RowId::HudLayout, GameplayAccessibility, Enhanced, Live),
    m(RowId::HudPalette, GameplayAccessibility, Enhanced, Live),
    m(RowId::HudMinimap, GameplayAccessibility, Enhanced, Live),
    m(RowId::HudSpeedo, GameplayAccessibility, Enhanced, Live),
    m(RowId::HudInfo, GameplayAccessibility, Enhanced, Live),
    m(RowId::ReducedFlashing, GameplayAccessibility, Enhanced, Live),
    // Controller.
    m(RowId::PadLayout, Controller, Enhanced, Live),
    m(RowId::PadDeadzone, Controller, Enhanced, Live),
    m(RowId::PadTriggerDeadzone, Controller, Enhanced, Live),
    m(RowId::PadSaturation, Controller, Enhanced, Live),
    m(RowId::PadSteerCurve, Controller, Enhanced, Live),
    m(RowId::PadRumble, Controller, Enhanced, Live),
    m(RowId::PadPauseOnDisconnect, Controller, Enhanced, Live),
    m(RowId::PadSafeBind, Controller, Utility, Live),
    m(RowId::ListsOpenInstalled, GameplayAccessibility, Enhanced, Live),
    m(RowId::OutrunVariety, GameplayAccessibility, Enhanced, Live),
    m(RowId::TrafficDensity, GameplayAccessibility, Enhanced, NextLoad("free roam")),
    m(RowId::ReplayCameras, GameplayAccessibility, Enhanced, Live),
    m(RowId::PhotoMode, GameplayAccessibility, Enhanced, Live),
    m(RowId::SkipMovies, GameplayAccessibility, Enhanced, Restart),
    m(RowId::SkipTitle, GameplayAccessibility, Enhanced, Restart),
    m(RowId::MarkSpot, GameplayAccessibility, Utility, Live),
    m(RowId::DurableSaves, GameplayAccessibility, Utility, Live),
    m(RowId::SaveBackups, GameplayAccessibility, Utility, Live),
    m(RowId::CrashReports, GameplayAccessibility, Utility, Live),
    // Packs.
    m(RowId::HdUi, Packs, Enhanced, Restart),
    m(RowId::ReloadPacks, Packs, Utility, Live),
    m(RowId::TexturePack, Packs, Enhanced, NextLoad("the city or a car")),
    m(RowId::GenerateTexturePack, Packs, Utility, Live),
];

/// The model of row `id`.
pub fn model(id: RowId) -> Option<&'static RowModel> {
    MODEL.iter().find(|r| r.id == id)
}

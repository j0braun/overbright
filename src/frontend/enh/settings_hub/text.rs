use super::*;

/// The groups in the page's order.
pub const GROUPS: [UxGroup; 8] = [
    UxGroup::Display,
    UxGroup::ImageQuality,
    UxGroup::LightingEffects,
    UxGroup::WeatherWater,
    UxGroup::Performance,
    UxGroup::GameplayAccessibility,
    UxGroup::Controller,
    UxGroup::Packs,
];

/// The strings the page adds (localisation: one table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Text {
    CompareOff,
    CompareOn,
    ShowAll,
    ShowChanged,
    ShowCostly,
    Changed,
    NoneChanged,
    Fps60,
    Custom,
    OriginNative,
    OriginSharedNative,
    OriginSharedEnhanced,
    OriginEnhancedOff,
    OriginEnhanced,
    OriginUtility,
    AppliesLive,
    AppliesNextLoad,
    AppliesRestart,
    AppliesRestartTier,
    CostAt,
    SavesAt,
    CostFree,
    CostNone,
    CostMeasuredOn,
    NoMatch,
    HubPreset,
    HubCompare,
    HubShow,
    HubRestore,
    ResetGroup,
    NativeRow,
    TabsHint,
    ModeOriginal,
    ModeEnhanced,
    NativeMsaa,
    On,
    Off,
    NotSupported,
    HelpModeOriginal,
    HelpModeEnhanced,
    HelpLow,
    HelpMedium,
    HelpHigh,
    HelpCustom,
    LevelCost,
    HelpLevels,
    HelpUpscale,
    HelpAa,
    HelpRayTracing,
    HelpRayTracingNone,
    HelpMotionBlur,
    HelpCalibration,
    HelpAdvanced,
}

/// The text of `t` (English; `{}` are filled in order by the caller).
pub fn text(t: Text) -> &'static str {
    match t {
        Text::CompareOff => "Current Settings",
        Text::CompareOn => "Original Look",
        Text::ShowAll => "All Settings",
        Text::ShowChanged => "Changed Only",
        Text::ShowCostly => "Costly Only",
        Text::Changed => "{} Changed",
        Text::NoneChanged => "Native",
        Text::Fps60 => "60 FPS",
        Text::Custom => "Custom",
        Text::OriginNative => "The original's setting (Customize Display).",
        Text::OriginSharedNative => {
            "The original's setting; this value is native, the tiers above it are enhancements."
        }
        Text::OriginSharedEnhanced => "Enhancement over the original's setting.",
        Text::OriginEnhancedOff => "Enhancement, off: as the original.",
        Text::OriginEnhanced => "Enhancement.",
        Text::OriginUtility => "Convenience: no change to the picture.",
        Text::AppliesLive => "Applies at once.",
        Text::AppliesNextLoad => "Applies when {} load next.",
        Text::AppliesRestart => "Applies after a restart.",
        Text::AppliesRestartTier => "This tier applies after a restart.",
        Text::CostAt => "Cost ~+{} ms at 4K (+{} ms at {}).",
        Text::SavesAt => "Saves ~{} ms at 4K ({} ms at {}).",
        Text::CostFree => "No measurable cost.",
        Text::CostNone => "",
        Text::CostMeasuredOn => "Costs measured on a high-end desktop GPU.",
        Text::NoMatch => "Nothing Matches the Filter",
        Text::HubPreset => {
            "Each preset adds enhancements and costs more GPU time. 60 FPS picks the best preset this machine holds at 60 frames a second at this resolution, from measured costs; nothing changes by itself while you play."
        }
        Text::HubCompare => {
            "Shows the original's look (every enhancement off) while you are on this page, to compare with your settings; nothing is changed or saved. C or the pad's X switches it on any settings page."
        }
        Text::HubShow => {
            "Which rows the group pages list: all of them, only those changed from the original, or only those that cost 0.5 ms or more here."
        }
        Text::HubRestore => crate::frontend::options::RESTORE_NATIVE_HELP,
        Text::ResetGroup => {
            "Every row on this page back to the original's value. The original's own settings on it stay as you set them."
        }
        Text::NativeRow => "The same setting as on Customize Display.",
        Text::TabsHint => "[ and ] (the pad's shoulders): previous / next group. C (X): Compare.",
        Text::ModeOriginal => "Original",
        Text::ModeEnhanced => "Enhanced",
        Text::NativeMsaa => "Native MSAA",
        Text::On => "On",
        Text::Off => "Off",
        Text::NotSupported => "Not Supported",
        Text::HelpModeOriginal => {
            "Original: exactly the 2004 game's picture; only the original's own settings (Customize Display) apply. Enhanced turns on a quality level. C (the pad's X) compares with the original look on any of these pages."
        }
        Text::HelpModeEnhanced => {
            "Enhanced: Overbright's improvements at the quality level below, with its own anti-aliasing, upscaling and ray tracing choices. Original goes back to exactly the 2004 game's picture. C (the pad's X) compares with the original look."
        }
        Text::HelpLow => {
            "Low, for integrated GPUs: richer car paint and reflections, a sharper road reflection, smoother car shadow, LOD fade, 1.5x draw distance, wet cars, fine rain. Keeps the original's car highlight."
        }
        Text::HelpMedium => {
            "Medium: Low + high-precision glow, a sharper car reflection, wet roads, ambient occlusion, lamp light and shadows, contact shadows, headlight beams, light probes, road spray and light volumetric fog."
        }
        Text::HelpHigh => {
            "High: Medium + full-size glow, full-resolution reflections, screen-space reflections, finer smoke, animated water, wet road interaction, glass, 2x draw distance and car detail, denser fog and rain."
        }
        Text::HelpCustom => {
            "Custom: rows were changed on Advanced. Left / Right picks a level (its values replace the rows; the choices on this page stay)."
        }
        Text::LevelCost => "~{} ms of GPU time at {} in the heaviest scene.",
        Text::HelpLevels => "Levels differ in cost, not in style; Advanced has every row.",
        Text::HelpUpscale => "Renders fewer pixels and rebuilds the screen's from several frames (turns on TAA). FSR 1 and FSR 2 are on Advanced (Performance).",
        Text::HelpAa => "Edge smoothing: the original's MSAA, TAA, or SSAA (2x to 16x, only with Upscaling Off). MSAA with alpha-to-coverage, FXAA, SMAA and sharpening are on Advanced (Image Quality).",
        Text::HelpRayTracing => {
            "Ray-traced shadows from every nearby light and ray-traced reflections of what the screen misses (Modern + RT lighting, Ray Traced reflections). Vulkan desktop GPUs; applies after a restart. Outside the quality levels."
        }
        Text::HelpRayTracingNone => "Ray tracing needs a GPU with hardware ray queries (Vulkan); this one has none.",
        Text::HelpMotionBlur => "The original's speed blur (strongest with nitrous). The camera and per-object blur are on Advanced (Lighting & Effects).",
        Text::HelpCalibration => "HDR calibration: peak brightness, paper white and gamma, with the test pattern.",
        Text::HelpAdvanced => {
            "Every graphics row by group (display, image quality, lighting and effects, weather and water, performance, packs), Compare and Show. Changing a row there makes the quality Custom."
        }
    }
}

/// [`text`] with its `{}` filled in order.
pub fn fill(t: Text, args: &[&str]) -> String {
    let mut out = String::new();
    let mut parts = text(t).split("{}");
    out.push_str(parts.next().unwrap_or(""));
    for (k, p) in parts.enumerate() {
        out.push_str(args.get(k).copied().unwrap_or(""));
        out.push_str(p);
    }
    out
}

impl UxGroup {
    /// The group's row label and page title (`options::clone_label`).
    pub fn label(self) -> u32 {
        match self {
            UxGroup::Display => options::GROUP_DISPLAY,
            UxGroup::ImageQuality => options::UX_GROUP_IMAGE,
            UxGroup::LightingEffects => options::UX_GROUP_LIGHTING,
            UxGroup::WeatherWater => options::UX_GROUP_WEATHER,
            UxGroup::Performance => options::GROUP_PERFORMANCE,
            UxGroup::GameplayAccessibility => options::UX_GROUP_GAMEPLAY,
            UxGroup::Controller => options::UX_GROUP_CONTROLLER,
            UxGroup::Packs => options::UX_GROUP_PACKS,
        }
    }

    /// What the group holds (the hub's detail line).
    pub fn help(self) -> &'static str {
        match self {
            UxGroup::Display => {
                "HDR output and its calibration (the test pattern shows on the page). Resolution, screen mode and aspect are on the Display page."
            }
            UxGroup::ImageQuality => {
                "Anti-aliasing, sharpening, texture filtering, reflections, car paint and glass, car model detail."
            }
            UxGroup::LightingEffects => {
                "Modern Lighting, ambient occlusion, shadows, lamps, headlight beams, fog, sky, smoke, sparks, motion blur and depth of field."
            }
            UxGroup::WeatherWater => {
                "Wet roads and cars, rain, spray, lens drops and the fountains' water. Where and when it rains stays the original's."
            }
            UxGroup::Performance => {
                "Render scale, upscaling, world detail (draw distance), frame cap, latency, power saving and the performance overlay."
            }
            UxGroup::GameplayAccessibility => {
                "Field of view, HUD size and colours, reduced flashing, lists, outrun variety, traffic, replay cameras, photo mode and saves."
            }
            UxGroup::Controller => {
                "Controller layout (triggers as pedals, Start pauses), dead zones, saturation, steering linearity, rumble, pause on disconnect and safe axis binding. Bindings are on the Controls page."
            }
            UxGroup::Packs => "The HD UI pack and the world texture pack (made from your own game files).",
        }
    }

    fn index(self) -> usize {
        GROUPS.iter().position(|&g| g == self).unwrap_or(0)
    }

    /// The previous / next group, wrapping (the page's shoulder keys).
    pub fn step(self, next: bool) -> UxGroup {
        let n = GROUPS.len();
        GROUPS[if next { (self.index() + 1) % n } else { (self.index() + n - 1) % n }]
    }
}

//! Labels of the clone's own rows.

pub(crate) const OFF: u32 = 0xCCFA; // "Off" (bin_hash "OFF")
pub(crate) const ON: u32 = 0x063C; // "On" (bin_hash "ON")

/// Labels of the clone's own rows (agreed deviations; no game string):
/// made-up hashes that [`clone_label`] spells out.
pub(crate) const SCREEN_MODE: u32 = 0xC1_04E0_01;
pub(crate) const FULL_SCREEN: u32 = 0xC1_04E0_02;
pub(crate) const WINDOWED: u32 = 0xC1_04E0_03;
pub(crate) const ASPECT_RATIO: u32 = 0xC1_04E0_04;
pub(crate) const ASPECT_WIDE: u32 = 0xC1_04E0_05;
pub(crate) const ASPECT_STRETCHED: u32 = 0xC1_04E0_06;
pub(crate) const VSYNC_OFF_MODE: u32 = 0xC1_04E0_07;
pub(crate) const MAILBOX: u32 = 0xC1_04E0_08;
pub(crate) const IMMEDIATE: u32 = 0xC1_04E0_09;
pub(crate) const ENHANCEMENTS: u32 = 0xC1_04E0_10;
pub(crate) const FOV_OFFSET: u32 = 0xC1_04E0_11;
pub(crate) const HUD_SCALE: u32 = 0xC1_04E0_12;
pub(crate) const FRAME_CAP: u32 = 0xC1_04E0_13;
pub(crate) const DURABLE_SAVES: u32 = 0xC1_04E0_14;
pub(crate) const SAVE_BACKUPS: u32 = 0xC1_04E0_15;
pub(crate) const CRASH_REPORTS: u32 = 0xC1_04E0_16;
pub(crate) const RESTORE_NATIVE: u32 = 0xC1_04E0_17;
/// Restore Native's help (the Enhancements page and the settings hub):
/// what [`Settings::restore_native`] does.
pub(crate) const RESTORE_NATIVE_HELP: &str = "Every enhancement back to the original's value, Aspect Ratio Stretched, Vsync Off Mode Immediate, and Customize Display to the original's values for your World Detail level.";
pub(crate) const NATIVE: u32 = 0xC1_04E0_18;
pub(crate) const GRAPHICS: u32 = 0xC1_04E0_19;
pub(crate) const RENDER_SCALE: u32 = 0xC1_04E0_1A;
pub(crate) const AA_MSAA: u32 = 0xC1_04E0_2B;
pub(crate) const AA_FILTER: u32 = 0xC1_04E0_2C;
pub(crate) const POST_AA: u32 = 0xC1_04E0_1B;
pub(crate) const SHARPEN: u32 = 0xC1_04E0_1C;
pub(crate) const ROAD_REFLECTION: u32 = 0xC1_04E0_1D;
pub(crate) const CAR_REFLECTION: u32 = 0xC1_04E0_1E;
pub(crate) const DETAIL_DISTANCE: u32 = 0xC1_04E0_1F;
pub(crate) const CAR_DETAIL: u32 = 0xC1_04E0_20;
pub(crate) const HD_UI: u32 = 0xC1_04E0_22;
pub(crate) const HD_UI_PAGE: u32 = 0xC1_04E0_2E;
pub(crate) const HD_UI_PACK_STATUS: u32 = 0xC1_04E0_2F;
pub(crate) const HD_UI_ACTIVATION_STATUS: u32 = 0xC1_04E0_30;
/// See docs/modding/pack-hot-reload.md.
pub(crate) const RELOAD_PACKS: u32 = 0xC1_04E0_D4;
pub(crate) const LOD_FADE: u32 = 0xC1_04E0_23;
pub(crate) const KEEP_LOADED: u32 = 0xC1_04E0_24;
pub(crate) const TEXTURE_FILTERING: u32 = 0xC1_04E0_25;
pub(crate) const REVERSED_Z: u32 = 0xC1_04E0_26;
pub(crate) const SHADOW_QUALITY: u32 = 0xC1_04E0_27;
pub(crate) const REFLECTION_RATE: u32 = 0xC1_04E0_28;
pub(crate) const ENHANCED_REFLECTIONS: u32 = 0xC1_04E0_2D;
pub(crate) const SSAO: u32 = 0xC1_04E0_29;
pub(crate) const HDR_POST: u32 = 0xC1_04E0_31;
pub(crate) const ENHANCED_PAINT: u32 = 0xC1_04E0_38;
// The Graphics page (0x40…; the Wet Roads / SSR labels must not reuse 0x38).
pub(crate) const GRAPHICS_PRESET: u32 = 0xC1_04E0_40;
pub(crate) const GROUP_IMAGE: u32 = 0xC1_04E0_41;
pub(crate) const GROUP_LIGHTING: u32 = 0xC1_04E0_42;
pub(crate) const GROUP_REFLECTIONS: u32 = 0xC1_04E0_43;
pub(crate) const GROUP_CARS: u32 = 0xC1_04E0_44;
pub(crate) const GROUP_EFFECTS: u32 = 0xC1_04E0_45;
pub(crate) const GROUP_PERFORMANCE: u32 = 0xC1_04E0_46;
pub(crate) const CUSTOM_PRESET: u32 = 0xC1_04E0_47;
pub(crate) const LISTS_OPEN_INSTALLED: u32 = 0xC1_04E0_60;
pub(crate) const HUD_LAYOUT: u32 = 0xC1_04E0_61;
pub(crate) const HUD_CENTERED: u32 = 0xC1_04E0_62;
pub(crate) const HUD_FULL_WIDTH: u32 = 0xC1_04E0_63;
/// Outrun Variety (docs/notes/parity/drag-drift-outrun-parity.md §0).
pub(crate) const OUTRUN_VARIETY: u32 = 0xC1_04E0_64;
/// Unit MF (docs/notes/graphics/model-fixes.md).
pub(crate) const FIX_MODELS: u32 = 0xC1_04E0_7A;
/// Photo mode (docs/playing/photo-mode.md): the Enhancements row and the pause
/// menu's item.
pub(crate) const PHOTO_MODE_ROW: u32 = 0xC1_04E0_90;
pub const PHOTO_MODE: u32 = 0xC1_04E0_91;
// Modern extras (docs/playing/modern-extras.md); 0xC0.., clear of the ids above.
pub(crate) const LOW_LATENCY: u32 = 0xC1_04E0_C0;
pub(crate) const REPLAY_CAMERAS: u32 = 0xC1_04E0_C1;
pub(crate) const HUD_PALETTE: u32 = 0xC1_04E0_C2;
pub(crate) const HUD_MINIMAP: u32 = 0xC1_04E0_C3;
pub(crate) const HUD_SPEEDO: u32 = 0xC1_04E0_C4;
pub(crate) const HUD_INFO: u32 = 0xC1_04E0_C5;
pub(crate) const REDUCED_FLASHING: u32 = 0xC1_04E0_C6;
/// Performance overlay (docs/notes/performance/overlay.md); 0xEC..0xEF, clear of
/// every copy's ids (2026-11-05).
pub(crate) const PERF_OVERLAY: u32 = 0xC1_04E0_EC;
pub(crate) const OVERLAY_POSITION: u32 = 0xC1_04E0_ED;
pub(crate) const OVERLAY_SCALE: u32 = 0xC1_04E0_EE;
pub(crate) const OVERLAY_LOG: u32 = 0xC1_04E0_EF;
/// Power Saving (the maintainer's private capture); 0xE8, clear of
/// every copy's ids (2026-11-05).
pub(crate) const POWER_SAVING: u32 = 0xC1_04E0_E8;
/// Mark Spot Key (docs/playing/mark-spot.md).
pub(crate) const MARK_SPOT: u32 = 0xC1_04E0_6D;
// Wet roads and SSR (docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections"); ids clear of the Graphics page's (0x40..0x47 taken).
pub(crate) const WET_ROADS: u32 = 0xC1_04E0_48;
pub(crate) const SSR: u32 = 0xC1_04E0_49;
// Lamps and shadows (docs/notes/graphics/lamps-shadows.md).
pub(crate) const DYNAMIC_LAMPS: u32 = 0xC1_04E0_4A;
pub(crate) const LAMP_SHADOWS: u32 = 0xC1_04E0_4B;
pub(crate) const CONTACT_SHADOWS: u32 = 0xC1_04E0_4C;
// Temporal tier (docs/notes/graphics/temporal.md); 0x55.. to stay clear of the ids the other
// G/S units of this wave may take next (0x4D..).
pub(crate) const UPSCALE: u32 = 0xC1_04E0_55;
pub(crate) const VOLUMETRIC_FOG: u32 = 0xC1_04E0_56;
/// Sky (docs/notes/graphics/atmosphere.md); 0xB6, unused in main and every wf3xx copy
/// (checked 2026-11-05).
pub(crate) const SKY: u32 = 0xC1_04E0_B6;
// GPU particles (docs/notes/graphics/smoke.md "Particles High and Ultra: GPU particles and detailed smoke"); 0x70 to stay clear of the ids the
// other effects may take next (0x5C..).
pub(crate) const PARTICLES: u32 = 0xC1_04E0_70;
/// Sparks & Fire (docs/notes/graphics/sparks-fire.md); chosen clear of every other id
/// (2026-11-05).
pub(crate) const SPARKS_FIRE: u32 = 0xC1_04E0_D8;
// Animated water (docs/notes/graphics/water.md); 0x7C, clear of the ids above.
pub(crate) const ANIMATED_WATER: u32 = 0xC1_04E0_7C;
// Ambient detail (docs/notes/graphics/ambient-detail.md); 0xB8, unused by the rows above and the other effects'
// copies (took 0xD8).
pub(crate) const AMBIENT_DETAIL: u32 = 0xC1_04E0_B8;
// Lighting II (docs/notes/graphics/lighting2.md); 0xB0.., clear of the ids above and the weather's 0xA0..
pub(crate) const HEADLIGHT_BEAMS: u32 = 0xC1_04E0_B0;
pub(crate) const LIGHT_PROBES: u32 = 0xC1_04E0_B1;
pub(crate) const ENHANCED_GLASS: u32 = 0xC1_04E0_B2;
/// See docs/notes/graphics/surface-maps.md.
pub(crate) const SURFACE_DETAIL: u32 = 0xC1_04E0_D0;
/// Modern Lighting (docs/notes/graphics/modern-lighting.md); 0x9A.., clear of the other ids
/// (checked 2026-11-05).
pub(crate) const MODERN_LIGHTING: u32 = 0xC1_04E0_9A;
pub(crate) const MODERN_STRENGTH: u32 = 0xC1_04E0_9B;
// HDR output (docs/playing/hdr.md).
pub(crate) const GROUP_DISPLAY: u32 = 0xC1_04E0_57;
pub(crate) const HDR_OUTPUT: u32 = 0xC1_04E0_58;
pub(crate) const HDR_PEAK: u32 = 0xC1_04E0_59;
pub(crate) const HDR_PAPER: u32 = 0xC1_04E0_5A;
pub(crate) const HDR_GAMMA: u32 = 0xC1_04E0_5B;
// Weather (docs/notes/graphics/weather.md); 0xA0.. clear of the other rows' ids.
pub(crate) const GROUP_WEATHER: u32 = 0xC1_04E0_A0;
pub(crate) const WET_CAR: u32 = 0xC1_04E0_A1;
pub(crate) const RAIN_DETAIL: u32 = 0xC1_04E0_A2;
pub(crate) const SPRAY: u32 = 0xC1_04E0_A3;
pub(crate) const LENS_DROPS: u32 = 0xC1_04E0_A4;
/// Wet Road Interaction (docs/notes/graphics/wet-road-car.md).
pub(crate) const WET_INTERACTION: u32 = 0xC1_04E0_AC;
// Assets and distance (docs/notes/graphics/assets-distance.md); 0xB0.. clear of the other rows' ids.
pub(crate) const SMOOTH_CARS: u32 = 0xC1_04E0_E0;
pub(crate) const TRAFFIC_DENSITY: u32 = 0xC1_04E0_E1;
/// Skip Title Screen (docs/original/start-and-menus.md).
pub(crate) const SKIP_TITLE: u32 = 0xC1_04E0_FB;
/// Skip Intro Movies (docs/original/start-and-menus.md).
pub(crate) const SKIP_MOVIES: u32 = 0xC1_04E0_FD;
/// G7b2.
pub(crate) const TEXTURE_PACK: u32 = 0xC1_04E0_F4;
pub(crate) const GENERATE_TEXTURE_PACK: u32 = 0xC1_04E0_F5;
/// The settings page (docs/developing/settings-page.md); 0x80..0x86, unused elsewhere
/// (checked 2026-11-06).
pub(crate) const UX_COMPARE: u32 = 0xC1_04E0_80;
pub(crate) const UX_FILTER: u32 = 0xC1_04E0_81;
pub(crate) const UX_GROUP_IMAGE: u32 = 0xC1_04E0_82;
pub(crate) const UX_GROUP_LIGHTING: u32 = 0xC1_04E0_83;
pub(crate) const UX_GROUP_WEATHER: u32 = 0xC1_04E0_84;
pub(crate) const UX_GROUP_GAMEPLAY: u32 = 0xC1_04E0_85;
pub(crate) const UX_GROUP_PACKS: u32 = 0xC1_04E0_86;
/// docs/playing/controller-support.md: the Controller group and its rows.
pub(crate) const UX_GROUP_CONTROLLER: u32 = 0xC1_04E0_32;
pub(crate) const PAD_LAYOUT: u32 = 0xC1_04E0_33;
pub(crate) const PAD_DEADZONE: u32 = 0xC1_04E0_34;
pub(crate) const PAD_TRIGGER_DEADZONE: u32 = 0xC1_04E0_35;
pub(crate) const PAD_SATURATION: u32 = 0xC1_04E0_36;
pub(crate) const PAD_STEER_CURVE: u32 = 0xC1_04E0_37;
pub(crate) const PAD_RUMBLE: u32 = 0xC1_04E0_39;
pub(crate) const PAD_PAUSE_ON_DISCONNECT: u32 = 0xC1_04E0_3A;
pub(crate) const PAD_SAFE_BIND: u32 = 0xC1_04E0_3B;
pub(crate) const PAD_STICKS_QUIT: u32 = 0xC1_04E0_3C;
pub(crate) const UX_RESET_GROUP: u32 = 0xC1_04E0_87;
pub(crate) const UX_NO_MATCH: u32 = 0xC1_04E0_88;
/// The streamlined graphics page (docs/developing/settings-page.md "Graphics mode and
/// quality levels"); 0x65..0x6A.
pub(crate) const GRAPHICS_MODE: u32 = 0xC1_04E0_65;
pub(crate) const ENHANCED_QUALITY: u32 = 0xC1_04E0_66;
pub(crate) const RAY_TRACING: u32 = 0xC1_04E0_67;
pub(crate) const ADVANCED: u32 = 0xC1_04E0_68;
pub(crate) const HDR_CALIBRATION: u32 = 0xC1_04E0_69;
pub(crate) const RAIN_ON_LENS: u32 = 0xC1_04E0_6A;

/// The text of a clone row's label ([`SCREEN_MODE`] …).
pub fn clone_label(hash: u32) -> Option<&'static str> {
    Some(match hash {
        SCREEN_MODE => "Screen Mode",
        FULL_SCREEN => "Full Screen",
        WINDOWED => "Windowed",
        ASPECT_RATIO => "Aspect Ratio",
        ASPECT_WIDE => "Wide",
        ASPECT_STRETCHED => "Stretched (Original)",
        VSYNC_OFF_MODE => "Vsync Off Mode",
        MAILBOX => "No Tearing",
        IMMEDIATE => "Immediate",
        ENHANCEMENTS => "Enhancements",
        FOV_OFFSET => "Field Of View",
        HUD_SCALE => "HUD Scale",
        HUD_LAYOUT => "HUD Layout",
        HUD_CENTERED => "Centered",
        HUD_FULL_WIDTH => "Full Width",
        FRAME_CAP => "Frame Rate Cap",
        DURABLE_SAVES => "Durable Saves",
        SAVE_BACKUPS => "Save Backups",
        CRASH_REPORTS => "Crash Reports",
        RESTORE_NATIVE => "Restore Native",
        NATIVE => "Native",
        GRAPHICS => "Graphics",
        RENDER_SCALE => "Render Scale",
        POST_AA => "Anti-Aliasing",
        AA_MSAA => "MSAA Override",
        AA_FILTER => "AA Post Filter",
        SHARPEN => "Sharpening",
        ROAD_REFLECTION => "Road Reflection Res",
        CAR_REFLECTION => "Car Reflection Res",
        DETAIL_DISTANCE => "Detail Distance",
        CAR_DETAIL => "Car Detail Distance",
        HD_UI => "HD UI + Maps",
        HD_UI_PAGE => "HD UI",
        RELOAD_PACKS => "Reload Packs When Changed",
        HD_UI_PACK_STATUS => "Pack",
        HD_UI_ACTIVATION_STATUS => "Activation",
        LOD_FADE => "LOD Fade",
        KEEP_LOADED => "Keep All Loaded",
        TEXTURE_FILTERING => "Texture Filtering",
        REVERSED_Z => "Reversed-Z Depth",
        SHADOW_QUALITY => "Shadow Quality",
        ENHANCED_REFLECTIONS => "Enhanced Reflections",
        REFLECTION_RATE => "Reflection Update Rate",
        SSAO => "Ambient Occlusion",
        HDR_POST => "Post-Processing",
        ENHANCED_PAINT => "Enhanced Car Paint",
        GRAPHICS_PRESET => "Preset",
        GROUP_IMAGE => "Image",
        GROUP_LIGHTING => "Lighting",
        GROUP_REFLECTIONS => "Reflections",
        GROUP_CARS => "Cars",
        GROUP_EFFECTS => "Effects",
        GROUP_PERFORMANCE => "Performance",
        CUSTOM_PRESET => "Custom",
        LISTS_OPEN_INSTALLED => "Lists Open on Installed",
        OUTRUN_VARIETY => "Outrun Variety",
        FIX_MODELS => "Fix Original Model Errors",
        PHOTO_MODE_ROW => "Photo Mode in Pause Menu",
        PHOTO_MODE => "Photo Mode",
        LOW_LATENCY => "Low Latency",
        REPLAY_CAMERAS => "Replay Cameras",
        HUD_PALETTE => "HUD Colours",
        HUD_MINIMAP => "Minimap Size",
        HUD_SPEEDO => "Speedometer Size",
        HUD_INFO => "Race Info Size",
        REDUCED_FLASHING => "Reduced Flashing",
        PERF_OVERLAY => "Performance Overlay",
        OVERLAY_POSITION => "Overlay Position",
        OVERLAY_SCALE => "Overlay Scale",
        OVERLAY_LOG => "Performance Log (CSV)",
        POWER_SAVING => "Power Saving",
        MARK_SPOT => "Mark Spot Key",
        WET_ROADS => "Wet Roads",
        DYNAMIC_LAMPS => "Dynamic Lamps",
        LAMP_SHADOWS => "Lamp Shadows",
        CONTACT_SHADOWS => "Contact Shadows",
        UPSCALE => "Upscaling",
        VOLUMETRIC_FOG => "Volumetric Fog",
        SKY => "Sky",
        PARTICLES => "Particles",
        SPARKS_FIRE => "Sparks & Fire",
        ANIMATED_WATER => "Animated Water",
        AMBIENT_DETAIL => "Ambient Detail",
        HEADLIGHT_BEAMS => "Headlight Beams",
        LIGHT_PROBES => "Light Probes",
        ENHANCED_GLASS => "Enhanced Glass",
        SURFACE_DETAIL => "Surface Detail",
        MODERN_LIGHTING => "Lighting",
        MODERN_STRENGTH => "Lighting Strength",
        GROUP_WEATHER => "Weather",
        WET_CAR => "Wet Car",
        RAIN_DETAIL => "Rain Detail",
        SPRAY => "Spray & Splashes",
        LENS_DROPS => "Lens Drops",
        WET_INTERACTION => "Wet Road Interaction",
        SMOOTH_CARS => "Smooth Car Shapes",
        TRAFFIC_DENSITY => "Traffic Density",
        SKIP_TITLE => "Skip Title Screen",
        SKIP_MOVIES => "Skip Intro Movies",
        TEXTURE_PACK => "World Texture Pack",
        GENERATE_TEXTURE_PACK => "Generate Texture Pack",
        GROUP_DISPLAY => "Display",
        HDR_OUTPUT => "HDR Output",
        HDR_PEAK => "Peak Brightness",
        HDR_PAPER => "Paper White",
        HDR_GAMMA => "SDR Gamma",
        SSR => "Screen-Space Reflections",
        UX_COMPARE => "Compare",
        UX_FILTER => "Show",
        UX_GROUP_IMAGE => "Image Quality",
        UX_GROUP_LIGHTING => "Lighting & Effects",
        UX_GROUP_WEATHER => "Weather & Water",
        UX_GROUP_GAMEPLAY => "Gameplay & Accessibility",
        UX_GROUP_PACKS => "Packs",
        UX_GROUP_CONTROLLER => "Controller",
        PAD_LAYOUT => "Controller Layout",
        PAD_DEADZONE => "Stick Dead Zone",
        PAD_TRIGGER_DEADZONE => "Trigger Dead Zone",
        PAD_SATURATION => "Stick Saturation",
        PAD_STEER_CURVE => "Steering Linearity",
        PAD_RUMBLE => "Rumble",
        PAD_PAUSE_ON_DISCONNECT => "Pause on Disconnect",
        PAD_SAFE_BIND => "Safe Axis Binding",
        PAD_STICKS_QUIT => "Quit with Both Sticks",
        UX_RESET_GROUP => "Reset Group to Native",
        UX_NO_MATCH => "Nothing Matches the Filter",
        GRAPHICS_MODE => "Graphics Mode",
        ENHANCED_QUALITY => "Enhanced Quality",
        RAY_TRACING => "Ray Tracing",
        ADVANCED => "Advanced",
        HDR_CALIBRATION => "HDR Calibration",
        RAIN_ON_LENS => "Rain on the Lens",
        _ => return None,
    })
}

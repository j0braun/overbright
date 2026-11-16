//! UI_PC_Customize_Options: rows, caps, presets.

/// How a UI_PC_Customize_Options row edits its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomKind {
    /// A slider 0 to the card's cap, step 1 (slot 17: min +0x28 = 0, max
    /// +0x24 = the cap, step +0x38 = 1; the value is the slider's, ftol).
    Slider,
    /// "Low" (0xB55E7678) / "High" (0x612AF8A6), toggled.
    LowHigh,
    /// "On" / "Off", toggled.
    OnOff,
}

/// UI_PC_Customize_Options' rows in 0x4B6510's order: label, kind, the
/// setting's address (the values, 0x870CB8…) and the card's cap that
/// gates the row (0x8764D0…; a slider's maximum). Each row's change
/// handler (slot 1) also sets 0x8709D1 ("custom"). Select toggles the
/// Low/High and On/Off rows too (they take 0x0C407210 as well as Left /
/// Right); Vsync takes Left / Right only (0x49B8A0).
pub const CUSTOM_ROWS: [(u32, CustomKind, u32, u32); 22] = [
    (0x7D39_30E7, CustomKind::Slider, 0x870CB8, 0x8764D0), // Car Reflection Update Rate
    (0x8FE9_288E, CustomKind::LowHigh, 0x870CBC, 0x8764D4), // Car Reflection Detail
    (0xA7A6_83AC, CustomKind::Slider, 0x870CC0, 0x8764D8), // Car Shadow / Neon
    (0xCFBE_DB50, CustomKind::OnOff, 0x870CC4, 0x8764DC),  // Car Headlight
    (0x4AC5_0BCF, CustomKind::LowHigh, 0x870CF8, 0x876510), // Car Geometry Detail
    (0x06CC_3750, CustomKind::OnOff, 0x870CD0, 0x8764E8),  // Crowds
    (0xBDE2_E94B, CustomKind::Slider, 0x870CF4, 0x87650C), // World Detail
    (0x2A2A_4AB3, CustomKind::Slider, 0x870CD4, 0x8764EC), // Road Reflection Detail
    (0x6F2C_7417, CustomKind::OnOff, 0x870CE0, 0x8764F8),  // Light Trails
    (0x6292_5361, CustomKind::OnOff, 0x870CE4, 0x8764FC),  // Light Glow
    (0x2522_A4E9, CustomKind::OnOff, 0x870CEC, 0x876504),  // Particle System
    (0x822E_4E9B, CustomKind::OnOff, 0x870CDC, 0x8764F4),  // Motion Blur
    (0x95B8_F38D, CustomKind::OnOff, 0x870CD8, 0x8764F0),  // Fog
    (0xA9C2_7DFD, CustomKind::OnOff, 0x870CF0, 0x876508),  // Depth Of Field
    (0x4CD7_75AC, CustomKind::Slider, 0x870D08, 0x876520), // Full Screen Anti-Aliasing
    (0x1150_4D0E, CustomKind::OnOff, 0x870D04, 0x87651C),  // Tinting
    (0x4E57_D355, CustomKind::OnOff, 0x870D0C, 0x876524),  // Horizon Fog
    (0xD6F5_F38C, CustomKind::OnOff, 0x870CFC, 0x876514),  // Over Bright
    (0x571F_6C00, CustomKind::OnOff, 0x870D00, 0x876518),  // Enhanced Contrast
    (0x4CDD_BEBB, CustomKind::OnOff, 0x870D10, 0x876528),  // Rain Splatter
    (0x61A5_0B8F, CustomKind::Slider, 0x870D18, 0x876530), // Texture Filtering
    (0x5B4A_A6E4, CustomKind::OnOff, 0x870D24, 0),         // Vsync (always listed)
];

/// The caps 0x5BE210 sets for a card with every capability it probes
/// (shaders, render targets, filtering): reflection rate 3, shadow 2,
/// world 3, road reflection 3 (2 + the second shader cap), texture
// prov: inferred frontend.options.CUSTOM_CAPS
/// filtering 2 (trilinear + anisotropic), the toggles 1. **Inferred**: the
/// clone asks no Direct3D device; full-screen anti-aliasing's cap is the
/// device's non-maskable multisample quality count − 1 (0x5D2724), taken
/// as 3 here. Vsync has no cap (1).
pub const CUSTOM_CAPS: [u8; 22] = [3, 1, 2, 1, 1, 1, 3, 3, 1, 1, 1, 1, 1, 1, 3, 1, 1, 1, 1, 1, 2, 1];

/// The custom values a Level Of Detail sets (0x5BE880, run by the detail
/// slider through 0x5BEA00 and by Default Settings, 0x5CED44), each then
/// held to its cap (0x5BE6A0): all 0 and texture filtering 1; above 1
/// shadow 2, headlight, crowds, glow, world 1, tinting; above 2 reflection
/// rate and detail, road reflection 2, fog, particles, geometry, texture
/// filtering 1, motion blur, horizon fog, world 2, depth of field, over
/// bright, light trails; above 3 reflection rate 2, rain, road reflection
/// 3, contrast, anti-aliasing 1; above 4 texture filtering 2, reflection
/// rate 3, anti-aliasing at its cap. Above 3 0x5BE880 also sets the
/// resolution index 0x870D1C = 1 (800 × 600) on a score-5 card
/// (0x5BE97C–0x5BE9BC; `Settings::reset_display`); the ATI case that
/// clears anti-aliasing (0x5BEA05) needs Win9x (0x5BF5B0 = dwPlatformId 1),
/// so never on NT (A-frontend-1 3-002 (a), (b)).
pub fn custom_presets(detail: u8) -> [u8; 22] {
    let mut v = [0u8; 22];
    v[20] = 1;
    if detail > 1 {
        for (i, x) in [(2, 2), (3, 1), (5, 1), (9, 1), (6, 1), (15, 1)] {
            v[i] = x;
        }
    }
    if detail > 2 {
        for (i, x) in [
            (0, 1),
            (1, 1),
            (7, 2),
            (12, 1),
            (10, 1),
            (4, 1),
            (20, 1),
            (11, 1),
            (16, 1),
            (6, 2),
            (13, 1),
            (17, 1),
            (8, 1),
        ] {
            v[i] = x;
        }
    }
    if detail > 3 {
        for (i, x) in [(0, 2), (19, 1), (7, 3), (18, 1), (14, 1)] {
            v[i] = x;
        }
    }
    if detail > 4 {
        v[20] = 2;
        v[0] = 3;
        v[14] = CUSTOM_CAPS[14];
    }
    for (x, cap) in v.iter_mut().zip(CUSTOM_CAPS) {
        *x = (*x).min(cap);
    }
    v
}

//! Options categories, pause context and Graphics groups.

use crate::frontend::fe_ids;
use super::*;

/// Where a pause's options were opened (UI_PauseOptions, screen 0x4D93C0,
/// rows 0x4C4BD0 by category).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Pause {
    /// The race's pause (else free roam's).
    pub race: bool,
    /// A career's free roam (0x500B60: game mode 1, no race): Career
    /// Difficulty is listed.
    pub career_roam: bool,
    /// A drag race (0x89E7D8 or 0x890108): no Transmission row.
    pub drag: bool,
    /// The dyno's test run (0x89E7E6): no Race Position, Lap Info, Split
    /// Time, Leaderboard or Score rows.
    pub test_run: bool,
}

/// The UI_Options category (0x83AA04).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Audio,
    Display,
    Gameplay,
    Player,
    /// UI_PC_Customize_Options (screen 0x4CF660, rows 0x4B6510).
    CustomDisplay,
    /// UI_Wheel_Options (screen 0x4CFBA0, rows 0x4B8A10).
    Wheel,
    /// The clone's Enhancements page (QoL, clone deviations), on
    /// UI_Options from the Display page's last row.
    Enhancements,
    /// The clone's Graphics page (graphics enhancements, clone
    /// deviations), from the Enhancements page's Graphics row.
    Graphics,
    /// Combined HD interface and detailed-map pack settings.
    HdUi,
    /// A group of the Graphics page's per-effect rows, from the
    /// Graphics page's group rows.
    GraphicsGroup(GraphicsGroup),
    /// docs/settings-page.md: a group page of the settings page,
    /// from the Enhancements hub's group rows.
    Settings(super::settings_model::UxGroup),
    /// The settings page's Advanced page: Compare, Show and the graphics
    /// groups, from the hub's Advanced row.
    Advanced,
}

/// The Graphics page's groups (docs/graphics-settings-audit.md): the
/// per-effect rows by area, under the preset row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphicsGroup {
    Image,
    Lighting,
    Reflections,
    Cars,
    Effects,
    /// docs/weather.md: the rain, the wet cars, spray and lens drops.
    Weather,
    Performance,
    /// HDR Output and its calibration (docs/hdr.md): the page is the
    /// calibration screen (its test pattern shows while it is open).
    Display,
}

pub const GRAPHICS_GROUPS: [GraphicsGroup; 8] = [
    GraphicsGroup::Image,
    GraphicsGroup::Lighting,
    GraphicsGroup::Reflections,
    GraphicsGroup::Cars,
    GraphicsGroup::Effects,
    GraphicsGroup::Weather,
    GraphicsGroup::Performance,
    GraphicsGroup::Display,
];

impl GraphicsGroup {
    /// The group's row label and page title.
    pub fn label(self) -> u32 {
        match self {
            GraphicsGroup::Image => GROUP_IMAGE,
            GraphicsGroup::Lighting => GROUP_LIGHTING,
            GraphicsGroup::Reflections => GROUP_REFLECTIONS,
            GraphicsGroup::Cars => GROUP_CARS,
            GraphicsGroup::Effects => GROUP_EFFECTS,
            GraphicsGroup::Weather => GROUP_WEATHER,
            GraphicsGroup::Performance => GROUP_PERFORMANCE,
            GraphicsGroup::Display => GROUP_DISPLAY,
        }
    }

    /// The group's rows in order ([`super::graphics_table::ROWS`]; the
    /// removed rows not included).
    pub fn rows(self) -> Vec<RowId> {
        super::graphics_table::ROWS.iter().filter(|r| r.group == self && !r.legacy).map(|r| r.id).collect()
    }

    /// One line on what the group holds (the detail line).
    pub fn help(self) -> &'static str {
        match self {
            GraphicsGroup::Image => "Anti-aliasing, sharpening and texture filtering.",
            GraphicsGroup::Lighting => "Ambient occlusion, shadows, lamps, headlight beams and light probes.",
            GraphicsGroup::Reflections => "Road reflection, wet roads and screen-space reflections.",
            GraphicsGroup::Cars => "Car paint and glass, car reflections and car model detail.",
            GraphicsGroup::Effects => {
                "Post-processing precision, LOD fading, particles, motion blur, depth of field and water."
            }
            GraphicsGroup::Weather => {
                "Rain on the cars, finer lit rain, splashes and spray, drops on the lens. Where and when it rains stays the original's."
            }
            GraphicsGroup::Performance => "Render scale and world detail (draw distance).",
            GraphicsGroup::Display => "HDR output and its calibration (test pattern on this page).",
        }
    }
}

impl Category {
    pub fn from_setting(n: u32) -> Option<Category> {
        match n {
            0 => Some(Category::Audio),
            1 => Some(Category::Display),
            2 => Some(Category::Gameplay),
            3 => Some(Category::Player),
            _ => None,
        }
    }

    /// The title (0x4CF750 sets 0x42ADB44C to the category's label).
    pub fn title(self) -> u32 {
        match self {
            Category::Audio => fe_ids::TXT_AUDIO,
            Category::Display => fe_ids::TXT_DISPLAY,
            Category::Gameplay => fe_ids::TXT_GAMEPLAY,
            Category::Player => fe_ids::TXT_PLAYER,
            // "Customize Display Settings" (0x4CF6A9).
            Category::CustomDisplay => fe_ids::OBJ_381FE1E9,
            Category::Wheel => fe_ids::OBJ_4DEAB931,
            Category::Enhancements => ENHANCEMENTS,
            Category::Graphics => GRAPHICS,
            Category::HdUi => HD_UI_PAGE,
            Category::GraphicsGroup(g) => g.label(),
            Category::Settings(g) => g.label(),
            Category::Advanced => ADVANCED,
        }
    }

    /// UI_PauseOptions' second header (Header_text2, 0x4C3370…): "AUDIO",
    /// "DISPLAY", "GAMEPLAY", "PLAYER".
    pub fn pause_header(self) -> u32 {
        match self {
            Category::Audio => 0x737D_1B97,
            // The Enhancements page sits under Display.
            Category::Display
            | Category::Enhancements
            | Category::Graphics
            | Category::HdUi
            | Category::GraphicsGroup(_)
            | Category::Settings(_)
            | Category::Advanced => 0xEF1B_B23B,
            Category::Gameplay => 0xE864_7E75,
            _ => 0xD9DC_2F12,
        }
    }

    /// The package the category's rows live in.
    pub fn package(self, pause: bool) -> &'static str {
        match self {
            Category::CustomDisplay => "UI_PC_Customize_Options",
            Category::Wheel => "UI_Wheel_Options",
            _ if pause => "UI_PauseOptions",
            _ => "UI_Options",
        }
    }
}

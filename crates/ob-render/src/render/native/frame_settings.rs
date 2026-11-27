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

pub use crate::qol::RenderOptions;

//! [`Quad`] (the canvas draw unit) and [`ScreenState`] (what a screen's code sets on its package).

use glam::Vec2;

pub use crate::draw::quad::{CANVAS, HAIRLINE_UV, Quad};

/// The state of a screen's objects: which script each runs and since when.
#[derive(Clone, Debug, Default)]
pub struct ScreenState {
    /// Per object hash: running script name and its start time (ms).
    pub scripts: crate::fxhash::FxHashMap<u32, (u32, f32)>,
    /// Objects hidden by the screen's code.
    pub hidden: crate::fxhash::FxHashSet<u32>,
    /// Objects the screen's code shows although the package hides them
    /// (flag 0x10000000 cleared, as the dialogs do with their buttons).
    pub shown: crate::fxhash::FxHashSet<u32>,
    /// Text set by the screen's code, per string object.
    pub text: crate::fxhash::FxHashMap<u32, String>,
    /// Texture set by the screen's code, per image object.
    pub image: crate::fxhash::FxHashMap<u32, u32>,
    /// Colour set by the screen's code (0x51D800), per object.
    pub color: crate::fxhash::FxHashMap<u32, [f32; 4]>,
    /// A colour (ARGB) 0x51D800 writes the way the game does: 0x517AD0 sets
    /// the base of every one of the object's scripts' colour tracks, so a
    /// running script's keys (deltas) still fade it.
    pub base_color: crate::fxhash::FxHashMap<u32, u32>,
    /// A colour (B, G, R, A) 0x535580 moves one script's colour track to,
    /// per object: (the script's name hash, the colour). The game shifts
    /// the track's base by the colour less the track's value at time 0
    /// (0x504A00(0), then 0x519D40 / 0x52BDD0), so its keys (deltas) still
    /// blink or fade it from there (the HUD's mail, 0x4ABAF0).
    pub script_color: crate::fxhash::FxHashMap<u32, (u32, [u32; 4])>,
    /// Rotation about z set by the screen's code (0x51CEE0), radians.
    pub angle: crate::fxhash::FxHashMap<u32, f32>,
    /// Size (x, y) set by the screen's code (0x50CC00, 0x50CB40).
    pub size: crate::fxhash::FxHashMap<u32, Vec2>,
    /// Texture corners (u0, v0, u1, v1) set by the screen's code (0x5179D0).
    pub uv: crate::fxhash::FxHashMap<u32, [f32; 4]>,
    /// Position (x, y) set by the screen's code (0x51D4E0).
    pub position: crate::fxhash::FxHashMap<u32, Vec2>,
    /// Let every zero-length INIT go on with its chain (the HUD: its
    /// elements' code shows its INIT → HIDE objects). Off, an object whose
    /// zero-length INIT chains to a script that does not loop (HIDE, or
    /// UI_pause's slot 6048A18B) and that no package message gave a script
    /// keeps INIT's state: a stand-in for the screens' code that runs
    /// scripts on such objects (e.g. 0x51DDA0 runs Fade_In / Fade_Out on the
    /// cash and player name, 0x537E70) or reads them before the first step
    /// (the icon scroller's option clones), which the clone's screens do
    /// not all port (**inferred** stand-in).
    pub chain_init: bool,
    /// Objects whose zero-length INIT goes on with its chain even without
    /// `chain_init` (traced: nothing in the screen's code shows them).
    pub chained: crate::fxhash::FxHashSet<u32>,
    /// Focus changes the screen's code makes, (time ms, object hash), in
    /// order: 0x52CE50 finds the button and 0x52B3B0 (flag set) sends the
    /// old focus Button_UnHighlight (0x55D1E635) and the new one
    /// Button_Highlight (0xABC08912), which the buttons' `MsgR` answer.
    pub focus: Vec<(f32, u32)>,
    /// Scripts the screen's code plays on an object after its INIT
    /// (0x537C10(package, object, name)): (object hash, script name, start
    /// ms), in time order. Unlike `scripts` the object's INIT and what the
    /// package's messages gave it run until then, and the played script's
    /// events reach the package's objects from its start.
    pub played: Vec<(u32, u32, f32)>,
}

impl ScreenState {
    /// Run `script` on `object` from `now` (ms). The game finds a script
    /// by its name's upper-case hash (0x51CF80 → 0x505450, which folds
    /// a–z to A–Z), as the packages store them: "Cash_Animate" runs
    /// CASH_ANIMATE.
    pub fn run(&mut self, object: u32, script: &str, now: f32) {
        self.scripts.insert(object, (crate::formats::hash::fe_hash(script), now));
    }
}

//! The strips' arrows' BLINK script.

use crate::frontend::fe_ids;
use crate::frontend::ScreenState;

/// The left / right arrows' "Blink": the FNG script BLINK of the
/// packages' arrows (ScrollBar_Arrow_1 / _2 and the like: 400 ms of colour
/// green and size 26 → 32, chained to STATIC, 0x7AB70D67) that a step
/// that way runs. Two ways the game runs it:
/// * restarted from 0 at every step (0x51CF80 with flag 1: UI_QRCarSelect's
///   step 0x4E2590 by 0x5426D0 on "ScrollBar_Arrow_1" / "_2", which
///   Main-menu Customize's car select shares; UI_PostRaceStats' paging
///   0x4A4480 by 0x537C70; the Quick Race track select, **observed**
///   under Wine (three Rights 0.15 s apart: three restarts), its call
///   site not traced);
/// * not while it runs (IconScroller_UpdateArrows 0x52FA80 for a step
///   right, 0x495810 for left, checked by 0x51D050 "running": script +0x10
///   time ≤ +0x0C length), the other arrow put on STATIC: the icon
///   scrollers. **Observed** under Wine on UI_Main (three Rights 0.15 s
///   apart: one blink).
///
/// The career car lot's (0x4AFFA0) and the crib's Car Select step (0x4B04C0)
/// name "ScrollBar_Arrow_1" / "_2" too, which UI_CareerCarLot and
/// UI_CareerCarSelect do not have (theirs are Scroll_Bar_Arrow_*): there
// prov: inferred frontend.screens.scroll_bar_at
/// nothing blinks (**inferred** from the packages; 0x5426D0's fallback
/// 0x52C580 not traced).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ArrowBlink(pub Option<(bool, f32)>);

/// The BLINK script's length (ms) in the packages' arrows.
pub const ARROW_BLINK_MS: f32 = 400.0;

impl ArrowBlink {
    /// A step right (or left) at `now` (real ms): the arrow that way
    /// blinks, from 0 when `restart`, else only when it is not blinking
    /// already. "Running" is 0x51D050's test on the script's own clock
    /// (script +0x10 ≤ +0x0C), which the package manager advances in
    /// package time (`super::FE_RATE`, 0x5662D0), so the 400 ms BLINK
    /// runs for 333 ms of real time.
    pub fn step(&mut self, right: bool, now: f32, restart: bool) {
        if !restart
            && let Some((r, at)) = self.0
            && r == right
            && (now - at) * crate::frontend::FE_RATE <= ARROW_BLINK_MS
        {
            return;
        }
        self.0 = Some((right, now));
    }

    /// Runs the blink on its arrow (`arrows`: left, right), timed from
    /// `origin` as the screen's layout is.
    pub fn apply(&self, state: &mut ScreenState, arrows: [u32; 2], origin: f32) {
        if let Some((right, at)) = self.0 {
            state.run(arrows[usize::from(right)], "BLINK", at - origin);
        }
    }
}

/// The icon scrollers' and the lots' arrows (FEng hashes of
/// "ScrollBar_Arrow_1" / "_2").
pub const SCROLLBAR_ARROWS: [u32; 2] = [fe_ids::POST_RACE_STATS_LEFT, fe_ids::POST_RACE_STATS_RIGHT];

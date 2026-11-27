//! The performance screens' stat bars.

use crate::frontend::fe_ids;
use glam::Vec2;

use crate::frontend::ScreenState;
use crate::frontend::screens::*;

/// A stat bar's objects, as 0x540C20 takes them from the names 0x547A10
/// (the package screen's acceleration, top speed and handling) and
/// 0x547E40 (UI_BuyPerformanceParts' torque and power) give it: the
/// backing, the handle marking the current value (the torque and power
/// bars have none), the cap at the left end, the current fill and the
/// preview fill.
#[derive(Clone, Copy, Debug)]
pub struct StatBar {
    pub backing: u32,
    pub handle: Option<u32>,
    pub cap: u32,
    pub current: u32,
    pub preview: u32,
}

/// UI_ChoosePerformancePackage's bars (0x547A10): Accel_*, top_speed_*,
/// handling_*.
pub const PACKAGE_BARS: [StatBar; 3] = [
    StatBar {
        backing: fe_ids::PACKAGE_BARS_0,
        handle: Some(fe_ids::PACKAGE_BARS_1),
        cap: fe_ids::PACKAGE_BARS_2,
        current: fe_ids::PACKAGE_BARS_3,
        preview: fe_ids::PACKAGE_BARS_4,
    },
    StatBar {
        backing: fe_ids::PACKAGE_BARS_5,
        handle: Some(fe_ids::PACKAGE_BARS_6),
        cap: fe_ids::PACKAGE_BARS_7,
        current: fe_ids::PACKAGE_BARS_8,
        preview: fe_ids::PACKAGE_BARS_9,
    },
    StatBar {
        backing: fe_ids::PACKAGE_BARS_10,
        handle: Some(fe_ids::PACKAGE_BARS_11),
        cap: fe_ids::PACKAGE_BARS_12,
        current: fe_ids::PACKAGE_BARS_13,
        preview: fe_ids::PACKAGE_BARS_14,
    },
];

/// UI_BuyPerformanceParts' bars (0x547E40): torque_*, horsepower_*.
pub const PART_BARS: [StatBar; 2] = [
    StatBar { backing: fe_ids::PART_BARS_0, handle: None, cap: fe_ids::PART_BARS_1, current: fe_ids::PART_BARS_2, preview: fe_ids::PART_BARS_3 },
    StatBar { backing: fe_ids::PART_BARS_4, handle: None, cap: fe_ids::PART_BARS_5, current: fe_ids::PART_BARS_6, preview: fe_ids::PART_BARS_7 },
];

/// Lay a bar out as its update (0x527180) does, for values on the bars'
/// 0–1 range (0x540C20 with 0 and 1): from the backing's top left (bx, by)
/// and width w, with the 46-pixel end pads of the constructor, the cap at
/// (bx + 42, by + 4); the fills run (w − 92 − 8, not below 0) for the
/// whole range; the current fill shows the lesser value from
/// (bx + 54, by + 9), the preview fill the difference after it — hidden
/// when there is none, else running ToLesser (a drop) or ToGreater — and
/// the handle sits at the current value's end less 9, at by + 7.
pub fn stat_bar(
    state: &mut ScreenState,
    package: &crate::frontend::feng::Package,
    bar: &StatBar,
    current: f32,
    preview: f32,
) {
    const PAD: f32 = 46.0;
    let Some((top_left, size)) = master_box(package, bar.backing) else { return };
    let place = |state: &mut ScreenState, hash: u32, at: Vec2, width: Option<f32>| {
        let Some(o) = package.object(hash) else { return };
        let s = o.attributes.size();
        let size = Vec2::new(width.unwrap_or(s.x), s.y);
        state.position.insert(hash, at + size * 0.5);
        if width.is_some() {
            state.size.insert(hash, size);
        }
    };
    let span = (size.x - 2.0 * PAD - 8.0).max(0.0);
    place(state, bar.cap, top_left + Vec2::new(PAD - 4.0, 4.0), None);
    let lesser = (current.min(preview)).clamp(0.0, 1.0) * span;
    let fill = top_left + Vec2::new(PAD + 8.0, 9.0);
    place(state, bar.current, fill, Some(lesser));
    let d = current - preview;
    let difference = d.abs() * span;
    let end = fill.x + lesser;
    if d == 0.0 {
        state.hidden.insert(bar.preview);
    } else {
        state.shown.insert(bar.preview);
        state.run(bar.preview, if d > 0.0 { "TOLESSER" } else { "TOGREATER" }, 0.0);
    }
    place(state, bar.preview, Vec2::new(end, fill.y), Some(difference));
    if let Some(handle) = bar.handle {
        let x = end - 9.0 + if d > 0.0 { difference } else { 0.0 };
        place(state, handle, Vec2::new(x, top_left.y + 7.0), None);
    }
}

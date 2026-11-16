use crate::frontend::fe_ids;
use crate::formats::hash::fe_hash;
use crate::frontend::{Assets, Quad, ScreenState, layout};

/// A slider (0x540C20 with 0–1 and the 46-pixel margin; drawn by
/// 0x540D30 as UI_Options' are): the knob along the base's track at the
/// fraction, the handle from the track's start as wide as the value's
/// part; the base and the caps stay where the package has them.
pub(crate) fn slider(
    state: &mut ScreenState,
    package: &crate::frontend::feng::Package,
    parts: [u32; 5],
    fraction: f32,
) {
    slider_with_margin(state, package, parts, fraction, 46.0);
}

pub(crate) fn slider_with_margin(
    state: &mut ScreenState,
    package: &crate::frontend::feng::Package,
    [base, knob, handle, beg, end]: [u32; 5],
    fraction: f32,
    margin: f32,
) {
    let Some(b) = package.object(base) else { return };
    let (bp, bs) = (b.attributes.position(), b.attributes.size());
    let left = bp.x - bs.x * 0.5;
    let track = bs.x - 2.0 * margin;
    let f = fraction.clamp(0.0, 1.0);
    if let Some(k) = package.object(knob) {
        state.position.insert(knob, glam::Vec2::new(left + margin + f * track, k.attributes.position().y));
    }
    if let Some(h) = package.object(handle) {
        let width = f * track;
        state.size.insert(handle, glam::Vec2::new(width, h.attributes.size().y));
        state.position.insert(handle, glam::Vec2::new(left + margin + width * 0.5, h.attributes.position().y));
    }
    for h in [base, knob, handle, beg, end] {
        state.shown.insert(h);
    }
}

/// A vertical slider (0x540C20 with flag 1 at s+4; its redraw 0x4FF530
/// takes vtable slot 7, 0x540EA0, instead of 0x540D30). The track is the
/// base's width (0x51D740, the package's size before the parts' 90°
/// turn) less the margin at each end (s+0x3C); with the base's position
/// (bx, by) (0x51D630) and f the value's fraction: the knob at
/// (bx + 4 − 3, by + track / 2 − 0.95·f·track, 7 higher once f > 0),
/// the caps vr_Beg / vr_End hidden (0x50CA00), the handle 0.95·f·track
/// wide (0x495590) at (bx + 4, by + track / 2 − width / 2 − 7)
/// (0x51D4E0 both).
pub(crate) fn slider_vertical(
    state: &mut ScreenState,
    package: &crate::frontend::feng::Package,
    [base, knob, handle, beg, end]: [u32; 5],
    fraction: f32,
    margin: f32,
) {
    let Some(b) = package.object(base) else { return };
    let (bp, bs) = (b.attributes.position(), b.attributes.size());
    let track = bs.x - 2.0 * margin;
    let f = fraction.clamp(0.0, 1.0);
    let bottom = bp.y + track * 0.5;
    let fill = f * track * 0.95;
    state.position.insert(knob, glam::Vec2::new(bp.x + 4.0 - 3.0, bottom - fill - if f > 0.0 { 7.0 } else { 0.0 }));
    if let Some(h) = package.object(handle) {
        state.size.insert(handle, glam::Vec2::new(fill, h.attributes.size().y));
    }
    state.position.insert(handle, glam::Vec2::new(bp.x + 4.0, bottom - fill * 0.5 - 7.0));
    state.hidden.extend([beg, end]);
}

/// The visual rating widget (vtable 0x79AA98, the object 0x8637D0 whose
/// +0x1C8 0x53D880 points at itself): 0x53D880 sets its rating (+0x4C,
/// 0x5234D0 of the player's car) with the comparison flag (+4) clear and
/// runs its update 0x523660, on UI_Status_Career's package (0x53DAB0
/// found the objects: per star k < 10 the full star 0x6EC2F0DE + k and
/// the halves REP_Half_Star_<2k + 1> / <2k + 2>; the bar vr_base,
/// vr_slider, vr_handle, vr_Beg, vr_End, 0–1 with a 72-pixel margin).
/// The update (nothing above 10): every star object hidden (0x50CA00);
/// the first floor(rating) full stars shown in colour +0xD0 (0x4A9DD0
/// (0xCA, 0xF4, 0x8A, 0xFF) = 0xFFCAF48A, 0x51D800); with the fraction
/// at least 0.5 (0x784260) the next star's first half shown in that
/// colour; the bar at the fraction (0x523AE3: rating − floor(rating)).
pub(crate) fn rating_stars(state: &mut ScreenState, package: &crate::frontend::feng::Package, rating: f32) {
    if rating > 10.0 {
        return;
    }
    let half = |n: u32| fe_hash(&format!("REP_Half_Star_{n}"));
    for k in 0..10u32 {
        state.hidden.extend([fe_ids::OBJ_6EC2F0DE + k, half(2 * k + 1), half(2 * k + 2)]);
    }
    let floor = rating.floor();
    let frac = rating - floor;
    let full = floor.max(0.0) as u32;
    let colour = crate::frontend::argb(0xFFCA_F48A);
    for k in 0..full.min(10) {
        state.hidden.remove(&(fe_ids::OBJ_6EC2F0DE + k));
        state.color.insert(0x6EC2_F0DE + k, colour);
    }
    if frac >= 0.5 && full < 10 {
        let h = half(2 * full + 1);
        state.hidden.remove(&h);
        state.color.insert(h, colour);
    }
    // The bar is the UI_Options slider class (0x53DAB0 initialises it
    // through its vtable slot 2, 0x540C20, flag 0: horizontal). The
    // update colours the widget's +0x20 / +0x24 / +0x28, the slider's
    // +0x18 / +0x1C / +0x20 = descriptor +0x10 / +0x14 / +0x18: vr_handle,
    // vr_Beg, vr_End (0x523AC5, colour +0xD0 with the flag clear).
    let [base, knob, handle, beg, end] = VR_PARTS.map(fe_hash);
    for h in [handle, beg, end] {
        state.color.insert(h, colour);
    }
    slider_with_margin(state, package, [base, knob, handle, beg, end], frac, 72.0);
}

/// The rating bar's parts in the order the clone's sliders take them; the
/// widgets' descriptors (0x53DAB0, 0x53D900) give them as +4 vr_base, +8
/// vr_slider, +0x10 vr_handle, +0x14 vr_Beg, +0x18 vr_End.
pub(crate) const VR_PARTS: [&str; 5] = ["vr_base", "vr_slider", "vr_handle", "vr_Beg", "vr_End"];

/// The rating widget's colours (constructor 0x5104D0, 0x4A9DD0(r, g, b,
/// a)): +0xD0 the rating (0xCA, 0xF4, 0x8A), +0xD4 a better preview
/// (0x80, 0xFF, 0x00), +0xD8 a worse one (0xCF, 0x80, 0x30).
pub const RATING_COLOURS: [u32; 3] = [0xFFCA_F48A, 0xFF80_FF00, 0xFFCF_8030];

/// The shops' Visual Rating widget: UI_Menu_Asset_Reputation, the second
/// widget of the object 0x8637D0 (+0xDC, vtable 0x79B82C set by its
/// constructor 0x5315C0). 0x55BCA0 runs on each screen change: on
/// UI_GenericParts_Browser, UI_ChoosePaintCategory, UI_Paint,
/// UI_DecalMain, UI_ChooseSpinner, UI_Rims_Browser and UI_DecalsOverlay
/// (the names 0x7FA1B8 …) it pushes the package if it is not up
/// (0x557ED0 → 0x555D10, then finds its objects, 0x53D900) and sets the
/// ratings (0x55B1B0(0x8392C0, 0x8389D0)); elsewhere it pops it
/// (0x53D8D0 → 0x537700). 0x55B1B0 updates twice: the car's own rating
/// (0x5234D0 of 0x8392C0, the FE car 0x838980's ride +0x940) with the
/// comparison flag (+4) clear, then the preview's (0x5234D0 of 0x8389D0,
/// the shown ride +0x50 that the shop screens fit the highlighted item
/// on, 0x55C180 / 0x55C200 / 0x55C3E0, and copy back from +0x940,
/// 0x55C470) with it set; each of those calls 0x55B1B0 again. The update
/// 0x54B010: with the flag, the text and bar from the preview (0x53DA20)
/// in colour +0xD8 when the preview is lower, +0xD4 when higher, +0xD0
/// when equal; without it, from the car's own in +0xD0. 0x53DA20(v): the
/// text 0x171D00CB "%$.2f" of v (0x537BF0, format 0x79BF3C), the bar at
/// v − floor(v) clamped to 0–1. 0x523610 colours vr_handle, vr_Beg,
/// vr_End, the text and the star 0x6EC2F0DE (which 0x53D900 shows,
/// 0x50CA50). So the shown state is the preview's.
pub fn shop_rating_quads(assets: &Assets, current: f32, preview: f32, now: f32) -> Vec<Quad> {
    let Some(package) = assets.package("UI_Menu_Asset_Reputation") else { return Vec::new() };
    let mut state = ScreenState::default();
    shop_rating(&mut state, package, current, preview);
    layout(assets, package, &state, now)
}

pub(crate) fn shop_rating(
    state: &mut ScreenState,
    package: &crate::frontend::feng::Package,
    current: f32,
    preview: f32,
) {
    let colour = crate::frontend::argb(if preview < current {
        RATING_COLOURS[2]
    } else if preview > current {
        RATING_COLOURS[1]
    } else {
        RATING_COLOURS[0]
    });
    state.text.insert(fe_ids::OBJ_171D00CB, crate::frontend::screens::fixed2(preview));
    let [base, knob, handle, beg, end] = VR_PARTS.map(fe_hash);
    for h in [handle, beg, end, fe_ids::OBJ_171D00CB, fe_ids::OBJ_6EC2F0DE] {
        state.color.insert(h, colour);
    }
    state.shown.insert(fe_ids::OBJ_6EC2F0DE);
    // 0x53D900 initialises the bar with flag 1 (vertical) and a 44-pixel
    // margin (descriptor +0x2C = 44.0).
    slider_vertical(state, package, [base, knob, handle, beg, end], (preview - preview.floor()).clamp(0.0, 1.0), 44.0);
}

//! The scroll bars of the text lists and icon strips.

use glam::Vec2;

use crate::formats::hash::fe_hash;
use crate::frontend::ScreenState;

/// The text list's vertical scroll bar (0x542AD0 finds SideScroll_Backing,
/// _Handle, _Arrow_1, _Arrow_2): when the lines fit, the handle and arrows
/// hide (0x538090); otherwise the handle's centre steps down the backing,
/// (backing − handle height) / max(lines − rows, 1) per line (0x52E330).
pub(crate) fn scroll_bar(
    state: &mut ScreenState,
    package: &crate::frontend::feng::Package,
    rows: usize,
    lines: usize,
    top: usize,
) {
    scroll_bar_named(state, package, "SideScroll", rows, lines, top);
}

/// [`scroll_bar`] for a bar named `<prefix>_Backing`, `_Handle`,
/// `_Arrow_1`, `_Arrow_2` (0x542AD0 takes the prefix; the option lists
/// of 0x552790 use "scrollbar").
pub(crate) fn scroll_bar_named(
    state: &mut ScreenState,
    package: &crate::frontend::feng::Package,
    prefix: &str,
    rows: usize,
    lines: usize,
    top: usize,
) {
    use crate::frontend::feng::Object;
    let find = |name: &str| package.objects.iter().find(|o| o.hash == fe_hash(&format!("{prefix}_{name}")));
    let (handle, arrows) = (
        fe_hash(&format!("{prefix}_Handle")),
        [fe_hash(&format!("{prefix}_Arrow_1")), fe_hash(&format!("{prefix}_Arrow_2"))],
    );
    if lines <= rows {
        state.hidden.extend([handle, arrows[0], arrows[1]]);
        return;
    }
    let (Some(backing), Some(h)) = (find("Backing"), find("Handle")) else { return };
    let size = |o: &Object| o.attributes.size().y;
    let travel = size(backing) - size(h);
    let step = travel / ((lines - rows) as f32).max(1.0);
    let back_top = backing.attributes.position().y - size(backing) * 0.5;
    let y = back_top + size(h) * 0.5 + (step * top as f32).min(travel);
    state.position.insert(handle, Vec2::new(h.attributes.position().x, y));
}

/// A scroll bar's handle (0x538090 → 0x52E330) for `pos` (from 1) of
/// `total` items, `visible` at once, along the bar's backing (the
/// `<prefix>_Backing` / `_Handle` / `_Arrow_1` / `_Arrow_2` objects,
/// 0x542AD0): with total ≤ visible or pos 0, the handle and arrows hide;
/// else the handle's centre sits at the backing's start + h/2 for the
/// first, its end − h/2 for the last, and start + h/2 + (L − h) / max(total
/// − visible, 1) · (pos − 1) between (L the backing's length, h the
/// handle's, along x for a horizontal bar, byte 0 of 0x542AD0 clear).
pub(crate) fn scroll_bar_at(
    state: &mut ScreenState,
    package: &crate::frontend::feng::Package,
    prefix: &str,
    horizontal: bool,
    visible: usize,
    total: usize,
    pos: usize,
) {
    let find = |name: &str| package.objects.iter().find(|o| o.hash == fe_hash(&format!("{prefix}_{name}")));
    let (handle, arrows) = (
        fe_hash(&format!("{prefix}_Handle")),
        [fe_hash(&format!("{prefix}_Arrow_1")), fe_hash(&format!("{prefix}_Arrow_2"))],
    );
    if total <= visible || pos == 0 {
        state.hidden.extend([handle, arrows[0], arrows[1]]);
        return;
    }
    // 0x538090 shows the bar, the handle (with a position) and both
    // arrows, over whatever the screen's code hid before.
    state.shown.extend([handle, arrows[0], arrows[1]]);
    for h in [handle, arrows[0], arrows[1]] {
        state.hidden.remove(&h);
    }
    let (Some(backing), Some(h)) = (find("Backing"), find("Handle")) else { return };
    let axis = |v: glam::Vec3| if horizontal { v.x } else { v.y };
    let (length, side) = (axis(backing.attributes.size()), axis(h.attributes.size()));
    let start = axis(backing.attributes.position()) - length * 0.5;
    let step = (length - side) / ((total - visible) as f32).max(1.0);
    let at = if pos == 1 {
        start + side * 0.5
    } else if pos == total {
        start + length - side * 0.5
    } else {
        start + side * 0.5 + step * (pos - 1) as f32
    };
    // The mouse's bar (0x538140): its layout, and a dragged handle stays
    // where the cursor has it (0x52E4F0; the layouts skip it while +2).
    crate::frontend::mouse::record_bar(
        &package.name,
        prefix,
        crate::frontend::mouse::Bar { horizontal, start, length, side, step, home: at },
    );
    let at = crate::frontend::mouse::dragged(&package.name, prefix).map_or(at, |tl| tl + side * 0.5);
    let p = h.attributes.position();
    state.position.insert(handle, if horizontal { Vec2::new(at, p.y) } else { Vec2::new(p.x, at) });
}

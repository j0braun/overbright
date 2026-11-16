//! The option list's rows with their sliders and arrows.

use crate::frontend::fe_ids;
use glam::Vec2;

use crate::formats::hash::fe_hash;
use crate::frontend::options::*;
use crate::frontend::{Assets, ScreenState};

/// The option list's rows (0x552790's layout, see [`OptionsScreen::quads`]):
/// `rows[top..]` on the masters, `selected` highlighted since `moved`, the
/// arrow that last moved blinking.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_rows(
    assets: &Assets,
    package: &crate::frontend::feng::Package,
    state: &mut ScreenState,
    rows: &[Row],
    top: usize,
    selected: usize,
    moved: f32,
    blink: Option<(bool, f32)>,
) {
    draw_rows_n(assets, package, state, rows, top, selected, moved, blink, VISIBLE);
}

/// [`draw_rows`] with `visible` rows shown (the settings page: 7).
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_rows_n(
    assets: &Assets,
    package: &crate::frontend::feng::Package,
    state: &mut ScreenState,
    rows: &[Row],
    top: usize,
    selected: usize,
    moved: f32,
    blink: Option<(bool, f32)>,
    visible: usize,
) {
    let label = |h: u32| assets.strings.get(h).or_else(|| clone_label(h)).unwrap_or("").to_string();
    let title = master_box(package, TITLE_MASTER);
    let data = master_box(package, DATA_MASTER);
    // The mouse's targets (0x52FC50 / 0x51FEB0 / 0x52DA00): each shown
    // row's box (the title master's size at the row's place) and arrows.
    let mut hit: Vec<crate::frontend::mouse::SliderRow> = Vec::new();
    for n in 1..=15 {
        let k = n - 1;
        let parts = RowParts::new(n);
        let row = if k < visible { rows.get(top + k) } else { None };
        let (Some(row), Some((tl, ts)), Some((dl, ds))) = (row, title, data) else {
            parts.hide(state);
            continue;
        };
        let y = tl.y + ts.y * k as f32;
        let dy = dl.y + ts.y * k as f32;
        // The name: top left on the title master's column.
        state.text.insert(parts.name, label(row.label));
        state.shown.insert(parts.name);
        state.position.insert(parts.name, Vec2::new(tl.x, y));
        // The backing group at the name's top left + (276, 10)
        // (0x52D810 with the item's −276, −10).
        state.position.insert(parts.backing, Vec2::new(tl.x + 276.0, y + 10.0));
        let centre = dl.x + ds.x * 0.5;
        {
            let (lw, lh) = object_size(package, parts.left).unwrap_or((24.0, 24.0));
            let (rw, rh) = object_size(package, parts.right).unwrap_or((24.0, 24.0));
            // A disabled row (+0x28 clear) is neither hovered nor clicked
            // (0x52FC50); a button row has no arrows.
            let (la, ra) = if !matches!(row.kind, Kind::Button) {
                ((Vec2::new(dl.x - rw, dy), Vec2::new(lw, lh)), (Vec2::new(dl.x + ds.x, dy), Vec2::new(rw, rh)))
            } else {
                ((Vec2::ZERO, Vec2::splat(-1.0)), (Vec2::ZERO, Vec2::splat(-1.0)))
            };
            if row.enabled {
                hit.push((top + k, Vec2::new(tl.x, y), ts, la, ra, None));
            }
        }
        let arrow = |state: &mut ScreenState| {
            // 0x54EF40 / 0x54F420 with 1: the arrows' texture 0x9943D584.
            if assets.textures.contains_key(&0x9943_D584) {
                state.image.insert(parts.left, fe_ids::OBJ_9943D584);
                state.image.insert(parts.right, fe_ids::OBJ_9943D584);
            }
            // 0x52DAD0: the left arrow's left edge a right arrow's width
            // left of the data master, the right arrow on its right
            // edge, both at the data cursor.
            let (lw, lh) = object_size(package, parts.left).unwrap_or((24.0, 24.0));
            let (rw, rh) = object_size(package, parts.right).unwrap_or((24.0, 24.0));
            state.position.insert(parts.left, Vec2::new(dl.x - rw + lw * 0.5, dy + lh * 0.5));
            state.position.insert(parts.right, Vec2::new(dl.x + ds.x + rw * 0.5, dy + rh * 0.5));
        };
        match row.kind {
            Kind::Cycle => {
                let text = match &row.value {
                    Some(Value::Label(h)) => label(*h),
                    Some(Value::Text(t)) => (*t).to_string(),
                    Some(Value::Owned(t)) => t.clone(),
                    None => String::new(),
                };
                state.text.insert(parts.data, text);
                state.position.insert(parts.data, Vec2::new(centre, dy));
                arrow(state);
                parts.hide_slider(state);
            }
            Kind::Slider { value, min, max } => {
                state.hidden.insert(parts.data);
                arrow(state);
                place_slider(package, state, &parts, centre, dy, (value - min) / (max - min));
            }
            // A settings page button with a value (a group's
            // "3 Changed") shows it centred, without arrows.
            Kind::Button if matches!(row.id, RowId::UxGroup(_)) && row.value.is_some() => {
                let text = match &row.value {
                    Some(Value::Label(h)) => label(*h),
                    Some(Value::Text(t)) => (*t).to_string(),
                    Some(Value::Owned(t)) => t.clone(),
                    None => String::new(),
                };
                state.text.insert(parts.data, text);
                state.position.insert(parts.data, Vec2::new(centre, dy));
                state.hidden.insert(parts.left);
                state.hidden.insert(parts.right);
                parts.hide_slider(state);
            }
            Kind::Button => {
                state.hidden.insert(parts.data);
                state.hidden.insert(parts.left);
                state.hidden.insert(parts.right);
                parts.hide_slider(state);
            }
        }
        // The selected row runs HIGHLIGHT, the others UNHIGHLIGHT
        // (slots +0x24 / +0x28); disabled rows DISABLE (0x51E3F0).
        // The settings page's focused greyed row reads highlighted,
        // without arrows (its value says why it is greyed).
        let focused_off = !row.enabled && top + k == selected && visible == PAGE_VISIBLE;
        let script = if !row.enabled && !focused_off {
            "DISABLE"
        } else if top + k == selected {
            "HIGHLIGHT"
        } else {
            "UNHIGHLIGHT"
        };
        // A disabled row's parts with no DISABLE (the arrows: 0x51E3F0
        // runs "Disable" on the row's text and backing objects only) stay
        // as the list's unhighlight left them: no arrows (Wine, menus15:
        // Quick Race's Laps / Opponents under Lap KO; the mechanism is
        // **inferred**, the look **verified**).
        // The Feedback row is its own class (vtable 0x791DB0, built at
        // 0x4B80BD / 0x4C4971 with 0x5B8250's force-feedback flag): its
        // Disable (0x4B5FA0) runs "Disabled" (DISABLED, 0x06B55037) on
        // the name and the data only (0x51CF80), not 0x51E3F0's
        // "Disable" — the grey (0x9B) "Feedback:" of the pause's Player
        // page (Wine, round 3A: the object's current script read from
        // memory is 0x06B55037).
        let feedback_off = !row.enabled && row.id == RowId::Feedback;
        for h in parts.all() {
            let has = |name: &str| {
                package
                    .object(h)
                    .is_some_and(|o| o.scripts.iter().any(|sc| sc.name == crate::formats::hash::fe_hash(name)))
            };
            if feedback_off && (h == parts.name || h == parts.data) {
                if has("DISABLED") {
                    state.run(h, "DISABLED", moved);
                }
            } else if feedback_off {
                if has("UNHIGHLIGHT") {
                    state.run(h, "UNHIGHLIGHT", moved);
                }
            } else if has(script) {
                state.run(h, script, moved);
            } else if script == "DISABLE" && has("UNHIGHLIGHT") {
                state.run(h, "UNHIGHLIGHT", moved);
            }
        }
        if focused_off {
            state.hidden.insert(parts.left);
            state.hidden.insert(parts.right);
        }
        // The arrow that moved blinks (0x51E6C0).
        if top + k == selected
            && let Some((right, at)) = blink
        {
            state.run(if right { parts.right } else { parts.left }, "BLINK", at);
        }
    }
    crate::frontend::mouse::set_slider_rows(hit);
}

pub(crate) const TITLE_MASTER: u32 = fe_ids::TITLE_MASTER;
pub(crate) const DATA_MASTER: u32 = fe_ids::DATA_MASTER;

/// An image's top left and size (0x51D080 / 0x51D740).
pub(crate) fn master_box(package: &crate::frontend::feng::Package, hash: u32) -> Option<(Vec2, Vec2)> {
    let o = package.object(hash)?;
    let (p, s) = (o.attributes.position(), o.attributes.size());
    let size = Vec2::new(s.x, s.y);
    Some((Vec2::new(p.x, p.y) - size * 0.5, size))
}

pub(crate) fn object_size(package: &crate::frontend::feng::Package, hash: u32) -> Option<(f32, f32)> {
    let s = package.object(hash)?.attributes.size();
    Some((s.x, s.y))
}

/// A row's objects (the list's name prefixes, 0x552790).
pub(crate) struct RowParts {
    pub(crate) name: u32,
    pub(crate) data: u32,
    pub(crate) backing: u32,
    pub(crate) left: u32,
    pub(crate) right: u32,
    pub(crate) base: u32,
    pub(crate) knob: u32,
    pub(crate) handle: u32,
    pub(crate) beg: u32,
    pub(crate) end: u32,
}

impl RowParts {
    pub(crate) fn new(n: usize) -> RowParts {
        let h = |p: &str| fe_hash(&format!("{p}{n}"));
        RowParts {
            name: h("OPTION_NAME_"),
            data: h("OPTION_DATA_"),
            backing: h("OPTION_BACKING_"),
            left: h("LEFT_ARROW_"),
            right: h("RIGHT_ARROW_"),
            base: h("SLIDER_BASE_"),
            knob: h("SLIDER_KNOB_"),
            handle: h("SLIDER_HANDLE_"),
            beg: h("SLIDER_BEG_"),
            end: h("SLIDER_END_"),
        }
    }

    pub(crate) fn all(&self) -> [u32; 8] {
        [self.name, self.data, self.backing, self.left, self.right, self.base, self.knob, self.handle]
    }

    /// Rows past the list: their INIT chains to HIDE.
    fn hide(&self, state: &mut ScreenState) {
        for h in [
            self.name,
            self.data,
            self.backing,
            self.left,
            self.right,
            self.base,
            self.knob,
            self.handle,
            self.beg,
            self.end,
        ] {
            state.hidden.insert(h);
        }
    }

    /// The slider's parts hide on rows without one (0x512240(0)).
    pub(crate) fn hide_slider(&self, state: &mut ScreenState) {
        for h in [self.base, self.knob, self.handle, self.beg, self.end] {
            state.hidden.insert(h);
        }
    }
}

/// A slider (0x52DF00 / 0x540D30): the base centred on the data master at
/// the data cursor; the knob along the base's track (its width less twice
/// the 46-pixel margin, item +0x94) at the value's fraction, a pixel
/// below the base's middle; the handle from the track's start as wide as
/// the value's part of the track. The begin / end caps stay hidden.
fn place_slider(
    package: &crate::frontend::feng::Package,
    state: &mut ScreenState,
    parts: &RowParts,
    centre: f32,
    top: f32,
    fraction: f32,
) {
    const MARGIN: f32 = 46.0;
    let Some((bw, bh)) = object_size(package, parts.base) else {
        return;
    };
    let (kw, kh) = object_size(package, parts.knob).unwrap_or((16.0, 16.0));
    let (_, hh) = object_size(package, parts.handle).unwrap_or((16.0, 5.75));
    let left = centre - bw * 0.5;
    state.position.insert(parts.base, Vec2::new(centre, top + bh * 0.5));
    let track = bw - 2.0 * MARGIN;
    let f = fraction.clamp(0.0, 1.0);
    let knob_top = bh * 0.5 + top - kh * 0.5 + 1.0;
    let knob_left = f * track - kw * 0.5 + MARGIN + left;
    state.position.insert(parts.knob, Vec2::new(knob_left + kw * 0.5, knob_top - 2.0 + kh * 0.5));
    let width = f * track;
    state.size.insert(parts.handle, Vec2::new(width, hh));
    state.position.insert(parts.handle, Vec2::new(left + MARGIN + width * 0.5, knob_top + hh * 0.5));
    state.hidden.insert(parts.beg);
    state.hidden.insert(parts.end);
}

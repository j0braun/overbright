//! UI_PerformanceBrandSelect.

use crate::frontend::fe_ids;
use glam::Vec2;

use crate::formats::hash::fe_hash;
use crate::frontend::options::*;
use crate::frontend::screens::Key;
use crate::frontend::{Assets, Quad, ScreenState, layout};

/// UI_PerformanceBrandSelect (class vtable 0x79CAC4, built by 0x5532C0 →
/// 0x550340, messages 0x55C940): after Yes on buying a performance part or
/// package whose parts carry brands, one row per part (0x521E40, option
/// vtable 0x79B6B0): for a part (0x83855D set) the part 0x839BB0, the
/// part-count group 0x6D017BC5 hidden; for a package (0x839BB4) each of
/// its first four parts with any brand (part +0x0C count, +0x10 hashes).
/// The row shows the part's name (+0x04), its brand's shown name and
/// DECAL_ICON_<name> (0x53B970: 0x4FFF00 / 0x4FFF40 over the brand table
/// 0x838400, chunk 0x34A14; 0x74CE8C0B without one). Left / Right
/// (0x9120409E / 0xB5971BF1, 0x521EA0) step the row's brand forward /
/// backward, wrapping;
/// Up / Down move between rows, the brand hint 0xB34A6C2B hidden while the
/// row has fewer than two. Select plays 0x47FF4E7C's 0xDE6EFF34, then
/// (0xE1FDE1D1) the rows' brands go to 0x8389B4… in order (0x521F00) and
/// the part is bought and fitted (0x521B10 + 0x5213E0) or the package
/// (0x54FE60 + 0x521350), back to the shop (0x55BD70).
pub struct BrandSelect {
    /// Each row: the part's name hash, its brands, the brand shown.
    pub rows: Vec<(u32, Vec<u32>, usize)>,
    pub selected: usize,
    pub opened: f32,
    pub moved: f32,
    pub blink: Option<(bool, f32)>,
    /// A single part (0x83855D) rather than a package.
    pub single: bool,
}

impl BrandSelect {
    pub fn new(rows: Vec<(u32, Vec<u32>, usize)>, single: bool, now: f32) -> BrandSelect {
        BrandSelect { rows, selected: 0, opened: now, moved: now, blink: None, single }
    }

    /// The rows' brands in order (0x521F00 → 0x8389B4).
    pub fn picked(&self) -> Vec<u32> {
        self.rows.iter().map(|(_, b, i)| b[*i]).collect()
    }

    pub fn key(&mut self, key: Key, now: f32) {
        match key {
            Key::Up if self.selected > 0 => {
                self.selected -= 1;
                self.moved = now;
            }
            Key::Down if self.selected + 1 < self.rows.len() => {
                self.selected += 1;
                self.moved = now;
            }
            Key::Left | Key::Right => {
                if let Some((_, brands, i)) = self.rows.get_mut(self.selected) {
                    let n = brands.len();
                    // 0x521EA0: Left (0x9120409E) the next brand, wrapping
                    // to the first; Right (0xB5971BF1) the one before,
                    // wrapping to the last (Wine: AEM, Right → HP Racing →
                    // Jackson Racing → Neuspeed, the table's order backwards).
                    *i = if key == Key::Left { (*i + 1) % n } else { (*i + n - 1) % n };
                    self.blink = Some((key == Key::Right, now));
                }
            }
            _ => {}
        }
    }

    /// The rows (Wine 2026-11-04, perf-shop round, `docs/perf-shop.md`;
    /// positions **measured**, the widget's own layout code not traced):
    /// a row a title master's height (76) apart from the masters
    /// (0x51FF90 / 0x54F830). In each, the brand's logo (OPTION_IMAGE_n,
    /// DECAL_ICON_<name> through 0x53B970) sits on top of the data
    /// master's centre, the brand's name (OPTION_DATA_n) under the logo,
    /// and the part's name (OPTION_NAME_n, the title master's column) and
    /// the arrows (0x52DAD0's places beside the data master) halfway down
    /// logo and name. The backings keep their authored places (four, one a
    /// row slot). Only the highlighted row shows its arrows; the list's
    /// bar is 0x538090(1, rows, selection): the bare track with one row,
    /// arrows and handle with two or more.
    pub fn quads(&self, assets: &Assets, brands: &[crate::frontend::career::Brand], now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("UI_PerformanceBrandSelect") else {
            return Vec::new();
        };
        let mut state = ScreenState::default();
        crate::frontend::screens::footer(&mut state, package, "Select");
        let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        state.hidden.insert(fe_ids::OBJ_3FBB7A4E);
        state.hidden.insert(TITLE_MASTER);
        state.hidden.insert(DATA_MASTER);
        // 0x550340 hides the "Change Selection" hint for a single part and
        // for a package with fewer than two branded rows.
        if self.single || self.rows.len() < 2 {
            state.hidden.insert(0x6D01_7BC5);
        }
        if self.rows.get(self.selected).is_none_or(|(_, b, _)| b.len() < 2) {
            state.hidden.insert(0xB34A_6C2B);
        }
        crate::frontend::screens::scroll_bar_at(
            &mut state,
            package,
            "ScrollBar",
            false,
            1,
            self.rows.len(),
            self.selected + 1,
        );
        let title = master_box(package, TITLE_MASTER);
        let data = master_box(package, DATA_MASTER);
        let mut hit: Vec<crate::frontend::mouse::SliderRow> = Vec::new();
        for n in 1..=4 {
            let k = n - 1;
            let parts = RowParts::new(n);
            let image = fe_hash(&format!("OPTION_IMAGE_{n}"));
            let (Some((name, list, index)), Some((tl, ts)), Some((dl, ds))) = (self.rows.get(k), title, data) else {
                for h in [
                    parts.name,
                    parts.data,
                    parts.left,
                    parts.right,
                    parts.base,
                    parts.knob,
                    parts.handle,
                    parts.beg,
                    parts.end,
                    image,
                ] {
                    state.hidden.insert(h);
                }
                continue;
            };
            let top = dl.y + ts.y * k as f32;
            let (iw, ih) = object_size(package, image).unwrap_or((89.6, 44.8));
            let centre_x = dl.x + ds.x * 0.5;
            let mid = top + (ih + ds.y) * 0.5;
            state.text.insert(parts.name, label(*name));
            state.shown.insert(parts.name);
            state.position.insert(parts.name, Vec2::new(tl.x, mid - ds.y * 0.5));
            let brand = brands.iter().find(|b| b.hash == list[*index]);
            state.text.insert(parts.data, brand.map_or(String::new(), |b| b.display.clone()));
            state.position.insert(parts.data, Vec2::new(centre_x, top + ih));
            let icon = brand.and_then(|b| assets.texture_for(&format!("DECAL_ICON_{}", b.name))).unwrap_or(0x74CE_8C0B);
            state.image.insert(image, icon);
            state.shown.insert(image);
            state.position.insert(image, Vec2::new(centre_x, top + ih * 0.5));
            let _ = iw;
            parts.hide_slider(&mut state);
            let (lw, lh) = object_size(package, parts.left).unwrap_or((24.0, 24.0));
            let (rw, rh) = object_size(package, parts.right).unwrap_or((24.0, 24.0));
            let left = (Vec2::new(dl.x - rw, mid - lh * 0.5), Vec2::new(lw, lh));
            let right = (Vec2::new(dl.x + ds.x, mid - rh * 0.5), Vec2::new(rw, rh));
            if list.len() > 1 && k == self.selected {
                if assets.textures.contains_key(&0x9943_D584) {
                    state.image.insert(parts.left, fe_ids::OBJ_9943D584);
                    state.image.insert(parts.right, fe_ids::OBJ_9943D584);
                }
                state.position.insert(parts.left, left.0 + left.1 * 0.5);
                state.position.insert(parts.right, right.0 + right.1 * 0.5);
            } else {
                state.hidden.insert(parts.left);
                state.hidden.insert(parts.right);
            }
            hit.push((k, Vec2::new(tl.x, tl.y + ts.y * k as f32), ts, left, right, None));
            let script = if k == self.selected { "HIGHLIGHT" } else { "UNHIGHLIGHT" };
            for h in parts.all() {
                if package
                    .object(h)
                    .is_some_and(|o| o.scripts.iter().any(|sc| sc.name == crate::formats::hash::fe_hash(script)))
                {
                    state.run(h, script, self.moved - self.opened);
                }
            }
            // The highlighted name's pulse is its own HIGHLIGHT script
            // (1.2 s, looping; Wine: the name fades in and out).
            if k == self.selected
                && let Some((right, at)) = self.blink
            {
                state.run(if right { parts.right } else { parts.left }, "BLINK", at - self.opened);
            }
        }
        crate::frontend::mouse::set_slider_rows(hit);
        layout(assets, package, &state, now - self.opened)
    }
}

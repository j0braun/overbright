use crate::frontend::fe_ids;
use super::*;

/// A row of UI_BuyPerformanceParts.
#[derive(Clone, Debug)]
pub struct PerfRow {
    pub id: u8,
    /// The part's name (language hash).
    pub name: u32,
    pub price: u32,
    /// Fitted to the car (upgrade state +0x50 + id).
    pub installed: bool,
    /// Bought for the car (0x530360).
    pub owned: bool,
    /// In a career (0x850078 = 1). Outside one a part that is not fitted
    /// reads "Not Installed" (0x53B859 in 0x53B7C0's non-career branch), not its price.
    pub career: bool,
}

/// UI_BuyPerformanceParts (screen 0x56C160 → 0x569CD0): the individual
/// parts of the performance category, pushed over
/// UI_ChoosePerformancePackage while its first option, Choose Individual
/// Parts, is highlighted (0x5620E0). An option list (0x552790) of the
/// category's parts, levels 1–3 in package order, each listed when bought
/// or when its package level is unlocked in the shop (0x5627A0 with
/// 0x50F0D0's flags 6 and 0x5155A0); with none, one "None" row (0x53B940).
/// Up and Down move along it, stopping at the ends (0x5390F0). The
/// Performance Stats panel shows the car's peak torque and power against
/// the car with the highlighted part fitted (0x562540).
#[derive(Clone, Debug)]
pub struct PerfParts {
    pub rows: Vec<PerfRow>,
    pub selected: usize,
    /// Peak torque (kN·m) and power (kW) of the car as it is.
    pub current: (f32, f32),
    /// The same with the highlighted part fitted.
    pub preview: (f32, f32),
    /// The same as bar values (0–1 on the 0x34A32 ranges).
    pub current_bars: (f32, f32),
    pub preview_bars: (f32, f32),
    /// The units option (0x83AA1A = 1): "Nm" / "kW" (0x5B81D613 /
    /// 0x5B81D5BA), else "ft-lbs" / "bhp" (0x562540; Wine, the finished-career
    /// save's km/h: "0.00 Nm", "0.00 kW").
    pub metric: bool,
    /// The first row shown: the list shows 4 (+0x134 = 4 at 0x569E1D, the
    /// option list's default being 7); under Wine with every part listed
    /// (12) four rows and the scroll bar.
    pub top: usize,
}

/// UI_BuyPerformanceParts' rows shown at once (0x569E1D).
pub const PERF_PARTS_ROWS: usize = 4;

pub const TITLE_MASTER: u32 = fe_ids::TITLE_MASTER;
pub const DATA_MASTER: u32 = fe_ids::DATA_MASTER;
pub const TORQUE: u32 = fe_ids::TORQUE;
pub const POWER: u32 = fe_ids::POWER;
/// Labels: "Installed", "Owned", "Not Installed" (NOT_INSTALLED, the
/// data text outside a career, 0x53B859 in 0x53B7C0), "None", "ft-lbs", "bhp".
pub const INSTALLED: u32 = fe_ids::INSTALLED;
pub const OWNED: u32 = fe_ids::OWNED;
pub const NOT_INSTALLED: u32 = fe_ids::NOT_INSTALLED;
pub const NONE: u32 = fe_ids::NONE;
pub const FT_LBS: u32 = fe_ids::FT_LBS;
pub const BHP: u32 = fe_ids::BHP;
pub const NM: u32 = fe_ids::NM;
pub const KW: u32 = fe_ids::KW;

impl PerfParts {
    pub fn key(&mut self, key: Key) {
        match key {
            Key::Up => self.selected = self.selected.saturating_sub(1),
            Key::Down => self.selected = (self.selected + 1).min(self.rows.len().saturating_sub(1)),
            _ => {}
        }
        self.keep_in_view();
    }

    // prov: inferred frontend.screens.keep_in_view
    /// The window follows the selection one row at a time (**inferred**
    /// for 0x5390F0: under Wine the list starts at its top).
    pub fn keep_in_view(&mut self) {
        if self.selected < self.top {
            self.top = self.selected;
        } else if self.selected >= self.top + PERF_PARTS_ROWS {
            self.top = self.selected + 1 - PERF_PARTS_ROWS;
        }
    }

    pub fn row(&self) -> Option<&PerfRow> {
        self.rows.get(self.selected)
    }

    /// The overlay's quads. The rows lie from TITLE_MASTER and
    /// DATA_MASTER as the option list does (0x51FF90 / 0x54F830), a title
    /// master's height apart: the names' top left at the title master's
    /// left, the data centred on the data master (0x52D810). A row's data
    /// (0x53B7C0) is "Installed" when fitted, "Owned" when bought, else the
    /// price (`%$d`); the highlighted row runs Highlight on its name, data
    /// and backing (0x52E130), the others Purchased on name and data when
    /// owned or fitted, else UnHighlight, and UnHighlight on the backing
    /// (0x530620). The readouts print the gain `%$2.2f %s` (0x562540):
    /// torque × 1000 × 0.7375621 in "ft-lbs", power × 1.34102209 in "bhp"
    /// (the clone's units are imperial; metric would be "Nm" and "kW").
    pub fn quads(&self, assets: &Assets, now: f32, opened: f32) -> Vec<Quad> {
        let Some(package) = assets.package("UI_BuyPerformanceParts") else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Select");
        let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        let title = master_box(package, perf_parts::TITLE_MASTER);
        let data_box = master_box(package, perf_parts::DATA_MASTER);
        let rows: Vec<(String, String, bool, bool)> = if self.rows.is_empty() {
            vec![(label(perf_parts::NONE), String::new(), true, false)]
        } else {
            self.rows
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let data = if r.installed {
                        label(perf_parts::INSTALLED)
                    } else if !r.career {
                        label(perf_parts::NOT_INSTALLED)
                    } else if r.owned {
                        label(perf_parts::OWNED)
                    } else {
                        crate::frontend::grouped(i64::from(r.price))
                    };
                    (label(r.name), data, i == self.selected, r.installed || r.owned)
                })
                .collect()
        };
        // The mouse's rows (the option list's 0x52FC50 over the rows'
        // title boxes; Wine: a move over a row highlights it).
        let mut hit: Vec<crate::frontend::mouse::SliderRow> = Vec::new();
        for n in 1..=12 {
            let name = fe_hash(&format!("OPTION_NAME_{n}"));
            let data = fe_hash(&format!("OPTION_DATA_{n}"));
            let backing = fe_hash(&format!("OPTION_BACKING_{n}"));
            match rows.get(self.top + n - 1).filter(|_| n <= PERF_PARTS_ROWS) {
                Some((text, value, highlighted, purchased)) => {
                    state.text.insert(name, text.clone());
                    state.text.insert(data, value.clone());
                    for o in [name, data, backing] {
                        state.shown.insert(o);
                    }
                    if let (Some((t, ts)), Some((d, ds))) = (title, data_box) {
                        let step = ts.y * (n - 1) as f32;
                        state.position.insert(name, Vec2::new(t.x, t.y + step));
                        state.position.insert(data, Vec2::new(d.x + ds.x * 0.5, d.y + step));
                        // The backing at the name's top left less the
                        // row's (−271, −10) (0x5627A0 → 0x52D810).
                        state.position.insert(backing, Vec2::new(t.x + 271.0, t.y + step + 10.0));
                        if !self.rows.is_empty() {
                            let none = (Vec2::ZERO, Vec2::splat(-1.0));
                            hit.push((self.top + n - 1, Vec2::new(t.x, t.y + step), ts, none, none, None));
                        }
                    }
                    if self.rows.is_empty() {
                        continue;
                    }
                    let script = if *highlighted {
                        "HIGHLIGHT"
                    } else if *purchased {
                        "PURCHASED"
                    } else {
                        "UNHIGHLIGHT"
                    };
                    // The row's unhighlight (0x530620) runs "Purchased"
                    // (0x79B610) on its +0x2C / +0x30 objects for a part
                    // owned or fitted, else UnHighlight: under Wine (the
                    // finished-career save, every category) only the data turns
                    // prov: inferred frontend.screens.quads-9
                    // orange, the name stays white (**inferred**: +0x2C /
                    // +0x30 are the data and the backing, whose package has
                    // no Purchased script).
                    state.run(name, if *highlighted { "HIGHLIGHT" } else { "UNHIGHLIGHT" }, 0.0);
                    state.run(data, script, 0.0);
                    state.run(backing, if *highlighted { "HIGHLIGHT" } else { "UNHIGHLIGHT" }, 0.0);
                    // 0x52E130 first moves the focus to the row's name
                    // (0x52CE50 → 0x52B3B0: Button_Highlight 0xABC08912),
                    // whose MsgR runs "Highlighted" (0xC2EA8AA0: 138, 244,
                    // 202, alpha 0 → 255 → 0 over 1.2 s, looping) once the
                    // queue is pumped, after its direct Highlight: the
                    // name pulses green, the data stays white (Wine
                    // menus103 perfparts_new_cust).
                    if *highlighted {
                        state.focus.push((0.0, name));
                    }
                }
                // INIT chains to HIDE.
                None => {
                    for o in [name, data, backing] {
                        state.hidden.insert(o);
                    }
                }
            }
        }
        // The masters only place the rows (INIT → HIDE).
        state.hidden.insert(perf_parts::TITLE_MASTER);
        state.hidden.insert(perf_parts::DATA_MASTER);
        // The option list's bar (0x552790's moves, e.g. 0x539324:
        // 0x538090(1, +0x12C − 1, the selection's index 0x520030)): one
        // visible, so the handle and arrows show whenever there are two
        // rows and the handle follows the selection, not the window (Wine,
        // a fresh 206's four engine parts: arrows and handle, the handle
        // at the top on the first row and near the end on the fourth).
        scroll_bar_at(
            &mut state,
            package,
            "scrollbar",
            false,
            1,
            rows.len(),
            if rows.is_empty() { 0 } else { self.selected + 1 },
        );
        let gain = |v: f32, unit: u32| format!("{} {}", fixed2(v), label(unit));
        if self.metric {
            state.text.insert(perf_parts::TORQUE, gain((self.preview.0 - self.current.0) * 1000.0, perf_parts::NM));
            state.text.insert(perf_parts::POWER, gain(self.preview.1 - self.current.1, perf_parts::KW));
        } else {
            state.text.insert(
                perf_parts::TORQUE,
                gain((self.preview.0 - self.current.0) * 1000.0 * 0.737_562_1, perf_parts::FT_LBS),
            );
            state
                .text
                .insert(perf_parts::POWER, gain((self.preview.1 - self.current.1) * 1.341_022_09, perf_parts::BHP));
        }
        stat_bar(&mut state, package, &PART_BARS[0], self.current_bars.0, self.preview_bars.0);
        stat_bar(&mut state, package, &PART_BARS[1], self.current_bars.1, self.preview_bars.1);
        crate::frontend::mouse::set_slider_rows(hit);
        layout(assets, package, &state, now - opened)
    }
}

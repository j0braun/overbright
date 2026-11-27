use crate::frontend::fe_ids;
use super::*;

/// A tuning parameter's name (0x50FE90): front / rear springs, shocks,
/// sway bars and ride height, steering ratio, final drive, gears 1–6,
/// nitrous, brake bias, front / rear tyre grip and downforce, ECU, turbo.
pub static TUNING_NAMES: crate::exe_tables::Table<[u32; 25]> = crate::exe_tables::Table::new(|t| &t.tuning_names);

/// The slider screens' rows by tuning category (0x557790 by 0x83A648:
/// 0x549B50 suspension, 0x549C30 drivetrain, 0x549DD0 aerodynamics,
/// 0x549E10 tyres & brakes; each row through 0x549860, which skips a
/// parameter that is not tunable): parameter and the end labels
/// (OPTION_DATA_B_n / _C_n) — "Low"/"High", "Soft"/"Hard", "Less"/"More",
/// "Top Speed"/"Acceleration", "Weak"/"Strong", "Rear"/"Front".
pub fn tuning_rows(category: usize) -> &'static [(u8, u32, u32)] {
    const LOW: (u32, u32) = (fe_ids::LOW_0, fe_ids::LOW_1);
    const SOFT: (u32, u32) = (fe_ids::SOFT_0, fe_ids::SOFT_1);
    const WEAK: (u32, u32) = (fe_ids::WEAK_0, fe_ids::WEAK_1);
    match category {
        0 => &[
            (3, LOW.0, LOW.1),
            (7, LOW.0, LOW.1),
            (0, SOFT.0, SOFT.1),
            (4, SOFT.0, SOFT.1),
            (1, SOFT.0, SOFT.1),
            (5, SOFT.0, SOFT.1),
            (2, SOFT.0, SOFT.1),
            (6, SOFT.0, SOFT.1),
            (8, 0xE669_2B0A, 0xE669_E1C6),
        ],
        1 => &[(10, 0, 0), (11, 0, 0), (12, 0, 0), (13, 0, 0), (14, 0, 0), (15, 0, 0), (9, 0xCEBF_AA36, 0x95A7_073D)],
        2 => &[(21, WEAK.0, WEAK.1), (22, WEAK.0, WEAK.1)],
        3 => &[(19, WEAK.0, WEAK.1), (20, WEAK.0, WEAK.1), (18, fe_ids::REAR, fe_ids::FRONT)],
        _ => &[],
    }
}

/// UI_PerformanceTuning_Sliders / _Drivetrain (screen 0x55DB20 →
/// 0x55AC20, rows 0x557790): a row per tunable parameter of the category,
/// 6 on screen (7 on the drivetrain's, `7 − (0x83A648 ≠ 1)`), each its
/// name (0x50FE90), the end labels and a slider whose handle shows the
/// setting. The package's row objects (OPTION_NAME_n, OPTION_DATA_B_n /
/// _C_n, SLIDER_BASE_n, SLIDER_HANDLE_n…) are templates below the screen
/// that the rows' widgets (0x522B00, placed by 0x530D30) move into the
// prov: inferred frontend.screens.TuneSliders
/// panel. **Inferred**: where (the clone stacks them 40 apart from the
/// panel's top with its own x offsets), the handle's travel (BASE's
/// width less 2 × 4 about its centre), the highlight colour of the selected row,
/// the scrolling past the last row, hiding the two header images
/// (0xC128B184 / 0xA753C46C, white without the texture the code sets).
/// A press of Left / Right is one step of the row's slider (verified,
/// 0x564F20 → 0x4FF590: (hi − lo)/30, see `tuning::nudge`).
pub struct TuneSliders {
    pub package: &'static str,
    /// (parameter, low label, high label), tunable rows only.
    pub rows: Vec<(u8, u32, u32)>,
    pub selected: usize,
    pub top: usize,
    pub category: usize,
    /// Speeds in km/h (0x83AA1A) for the drivetrain's gear chart.
    pub metric: bool,
    pub opened: f32,
    /// Each row's numbers (`tuning::slider_view`), set by the app when the
    /// screen opens and after each change.
    pub views: Vec<(u8, crate::tuning::SliderView)>,
}

/// A parameter's numbers from a screen's list (untunable when missing).
fn view_of(views: &[(u8, crate::tuning::SliderView)], p: u8) -> crate::tuning::SliderView {
    views.iter().find(|v| v.0 == p).map_or(crate::tuning::SliderView::UNTUNABLE, |v| v.1)
}

/// The slider's +0xAC after the row setup (4.0, stored at 0x549AB5): the
/// handle's travel is the base's width less 2 × this (0x530DA0, 0x549AE3).
const SLIDER_MARGIN: f32 = 4.0;

impl TuneSliders {
    pub fn visible(&self) -> usize {
        if self.category == 1 { 7 } else { 6 }
    }

    /// The selected row's parameter (the one BUTTON1 resets, 0x5650D0:
    /// the row widget +0x54 → +0x68).
    pub fn selected_param(&self) -> Option<u8> {
        self.rows.get(self.selected).map(|r| r.0)
    }

    /// Move the highlight; returns the parameter and the direction (±1)
    /// of a Left / Right press.
    pub fn key(&mut self, key: Key) -> Option<(u8, f32)> {
        match key {
            Key::Up if self.selected > 0 => self.selected -= 1,
            Key::Down if self.selected + 1 < self.rows.len() => self.selected += 1,
            Key::Left | Key::Right => {
                let p = self.rows.get(self.selected)?.0;
                return Some((p, if key == Key::Left { -1.0 } else { 1.0 }));
            }
            _ => {}
        }
        let n = self.visible();
        if self.selected < self.top {
            self.top = self.selected;
        } else if self.selected >= self.top + n {
            self.top = self.selected + 1 - n;
        }
        None
    }

    /// The rows as the option list lays them out (0x552790 / 0x51FF90 /
    /// 0x549860, **verified**): row k's top left at TITLE_MASTER's top left
    /// + k × its height (the list's +0x100 += +0x128 (0) + +0x108, the
    /// master's height), its data box at DATA_MASTER's top left + the same
    /// step. Each row is placed by 0x530AB0 (slot +0x10 of the row vtable
    /// 0x79B7C0, **verified** from the disassembly), with `off` −6 when the
    /// row has both end labels and `bc` the row's +0xBC (10 for the final
    /// drive, 0x549C30, else 0):
    /// * OPTION_NAME_ at (row x, row y + bc − 3), "%s (%s)" with the unit
    ///   (0x50FFB0) else the name (0x53CD40);
    /// * LEFT_ARROW_ at the data box's left, RIGHT_ARROW_ its right edge less
    ///   its width, both at data y + bc + off;
    /// * SLIDER_BASE_ between them: from data x + arrow width + 9 to data
    ///   right − arrow width − 9 (its width set, 0x495590), same top;
    /// * OPTION_DATA_B_ at the left arrow's left, OPTION_DATA_C_ right
    ///   aligned to the right arrow's right, TIP_TEXT_ centred on the base,
    ///   all at the base's bottom;
    /// * OPTION_DATA_ (the value, "%$2.2f", gears "%$2.1f") at (data x − 50,
    ///   data y + bc − 3);
    /// * the slider (0x5273B0, **verified**): travel = base width − 8 (+0xAC
    ///   4.0); with span = max − min (1 when 0) SLIDER_HANDLE_ (the green
    ///   fill) runs from base x + 4 + (lo − min)/span × travel to the value,
    ///   SLIDER_B_HANDLE_ (red, its "ToLesser" script) from the value to hi,
    ///   both at base y + 5; SLIDER_KNOB_'s top left at (value x − 4, base y
    ///   + 4).
    ///
    /// The arrows show 0x374F6465 / 0x760BDC9D (0x549860 with its fourth
    /// argument 0); the row's parts run HIGHLIGHT / UNHIGHLIGHT where they
    /// have them (the list's slots +0x24 / +0x28). The backing goes to the name's top left + (276, 10 + bc)
    // prov: inferred frontend.screens.quads-3
    /// (the item's −276, −10 − bc). **Inferred**: a string's 0x51D390 "top
    /// left" is its position (its size is its text scale).
    pub fn quads(&self, assets: &Assets, label: &dyn Fn(u32) -> String, now: f32) -> Vec<Quad> {
        let view = |p: u8| view_of(&self.views, p);
        let Some(package) = assets.package(self.package) else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Select");
        state.hidden.insert(fe_ids::DATA_MASTER);
        state.hidden.insert(fe_ids::TITLE_MASTER);
        let n = self.visible();
        // Every template row hides; the visible ones come back placed.
        let hide = [
            "OPTION_NAME_",
            "OPTION_DATA_",
            "OPTION_DATA_B_",
            "OPTION_DATA_C_",
            "TIP_TEXT_",
            "SLIDER_BASE_",
            "SLIDER_KNOB_",
            "SLIDER_HANDLE_",
            "SLIDER_B_HANDLE_",
            "SLIDER_BEG_",
            "SLIDER_END_",
            "LEFT_ARROW_",
            "RIGHT_ARROW_",
            "OPTION_BACKING_",
        ];
        for k in 1..=10 {
            for p in hide {
                state.hidden.insert(fe_hash(&format!("{p}{k}")));
            }
        }
        let master = |h: u32| {
            package.object(h).map(|o| {
                let (p, s) = (o.attributes.position(), o.attributes.size());
                (Vec2::new(p.x - s.x * 0.5, p.y - s.y * 0.5), Vec2::new(s.x, s.y))
            })
        };
        let (Some((tl, ts)), Some((dl, ds))) = (master(fe_ids::TITLE_MASTER), master(fe_ids::DATA_MASTER)) else {
            return layout(assets, package, &state, now - self.opened);
        };
        let size_of = |h: u32| {
            package.object(h).map_or(Vec2::splat(16.0), |o| {
                let s = o.attributes.size();
                Vec2::new(s.x, s.y)
            })
        };
        let mut hit = Vec::new();
        for k in 0..n {
            let Some(&(param, low, high)) = self.rows.get(self.top + k) else { break };
            let name = |p: &str| fe_hash(&format!("{p}{}", k + 1));
            let v = view(param);
            let bc = if param == 9 && self.category == 1 { 10.0 } else { 0.0 };
            let both = low != 0 && high != 0;
            let off = if both { -6.0 } else { 0.0 };
            let row = Vec2::new(tl.x, tl.y + ts.y * k as f32);
            let data = Vec2::new(dl.x, dl.y + ts.y * k as f32 + bc);
            let show = |state: &mut ScreenState, h: u32, centre: Vec2| {
                state.hidden.remove(&h);
                state.shown.insert(h);
                state.position.insert(h, centre);
            };
            // Name and value (strings: the position is the anchor).
            show(&mut state, name("OPTION_NAME_"), Vec2::new(row.x, row.y + bc - 3.0));
            let title = label(TUNING_NAMES[param as usize]);
            let title = match crate::tuning::unit_label(param, self.metric).map(label).filter(|u| !u.is_empty()) {
                Some(unit) => format!("{title} ({unit})"),
                None => title,
            };
            state.text.insert(name("OPTION_NAME_"), title);
            show(&mut state, name("OPTION_DATA_"), Vec2::new(data.x - 50.0, data.y - 3.0));
            state
                .text
                .insert(name("OPTION_DATA_"), fixed_grouped(v.value, if (10..=15).contains(&param) { 1 } else { 2 }));
            show(&mut state, name("OPTION_BACKING_"), Vec2::new(row.x + 276.0, row.y + 10.0 + bc));
            // Arrows and base.
            let (la, ra) = (name("LEFT_ARROW_"), name("RIGHT_ARROW_"));
            let (law, raw) = (size_of(la), size_of(ra));
            let top = data.y + off;
            show(&mut state, la, Vec2::new(data.x, top) + law * 0.5);
            show(&mut state, ra, Vec2::new(data.x + ds.x - raw.x, top) + raw * 0.5);
            let selected = self.top + k == self.selected;
            // 0x549860's fourth argument picks 0x9943D584 for both; every
            // tuning row passes 0 (0x549B50 … 0x549E10), so the left takes
            // 0x374F6465 and the right 0x760BDC9D; the selection shows
            // them through HIGHLIGHT / UNHIGHLIGHT below.
            for (h, tex) in [(la, 0x374F_6465), (ra, 0x760B_DC9D)] {
                if assets.textures.contains_key(&tex) {
                    state.image.insert(h, tex);
                }
            }
            let base = name("SLIDER_BASE_");
            let bh = size_of(base).y;
            let bx = data.x + raw.x + 9.0;
            let bw = ds.x - 2.0 * (raw.x + 9.0);
            show(&mut state, base, Vec2::new(bx + bw * 0.5, top + bh * 0.5));
            state.size.insert(base, Vec2::new(bw, bh));
            // The slider's fill and knob (0x5273B0).
            let span = if v.max - v.min == 0.0 { 1.0 } else { v.max - v.min };
            let travel = bw - 2.0 * SLIDER_MARGIN;
            let x0 = bx + SLIDER_MARGIN + (v.lo - v.min) / span * travel;
            let green = (v.value - v.lo) / span * travel;
            let red = (v.hi - v.value) / span * travel;
            let vx = x0 + green;
            for (h, x, w) in [(name("SLIDER_HANDLE_"), x0, green), (name("SLIDER_B_HANDLE_"), vx, red)] {
                let hh = size_of(h).y;
                show(&mut state, h, Vec2::new(x + w * 0.5, top + 5.0 + hh * 0.5));
                state.size.insert(h, Vec2::new(w, hh));
            }
            let b_handle = name("SLIDER_B_HANDLE_");
            if package.object(b_handle).is_some_and(|o| o.scripts.iter().any(|sc| sc.name == fe_hash("ToLesser"))) {
                state.run(b_handle, "ToLesser", 0.0);
            }
            let knob = name("SLIDER_KNOB_");
            show(&mut state, knob, Vec2::new(vx - 4.0, top + 4.0) + size_of(knob) * 0.5);
            // End labels and the tip mark under the base.
            let below = top + bh;
            if both {
                show(&mut state, name("OPTION_DATA_B_"), Vec2::new(data.x, below));
                show(&mut state, name("OPTION_DATA_C_"), Vec2::new(data.x + ds.x, below));
                state.text.insert(name("OPTION_DATA_B_"), label(low));
                state.text.insert(name("OPTION_DATA_C_"), label(high));
            }
            if !(9..=15).contains(&param) {
                show(&mut state, name("TIP_TEXT_"), Vec2::new(bx + bw * 0.5, below));
                state.text.insert(name("TIP_TEXT_"), label(fe_ids::TIP));
            }
            let script = if selected { "HIGHLIGHT" } else { "UNHIGHLIGHT" };
            for p in [
                "OPTION_NAME_",
                "OPTION_DATA_",
                "OPTION_BACKING_",
                "OPTION_DATA_B_",
                "OPTION_DATA_C_",
                "TIP_TEXT_",
                "LEFT_ARROW_",
                "RIGHT_ARROW_",
                "SLIDER_BASE_",
                "SLIDER_KNOB_",
                "SLIDER_HANDLE_",
                "SLIDER_B_HANDLE_",
            ] {
                let h = name(p);
                if package.object(h).is_some_and(|o| o.scripts.iter().any(|sc| sc.name == fe_hash(script))) {
                    state.run(h, script, 0.0);
                }
            }
            // The mouse's targets (0x52FC50 / 0x52DA00 / 0x565060): the row
            // (its title box), the arrows and the tip mark.
            hit.push((
                self.top + k,
                row,
                ts,
                (Vec2::new(data.x, top), law),
                (Vec2::new(data.x + ds.x - raw.x, top), raw),
                (!(9..=15).contains(&param)).then_some(Vec2::new(bx + bw * 0.5, below)),
            ));
        }
        // The drivetrain's gear chart (0x549C30, verified): the eight speed
        // labels 0xAEA3C019 + k read `%d` of k × 35 (mph), or k × 60 when
        // 0x83AA1A is 1.
        if self.category == 1 {
            let step = if self.metric { 60 } else { 35 };
            for k in 0..8u32 {
                state.text.insert(0xAEA3_C019 + k, format!("{}", k * step));
            }
        }
        crate::frontend::mouse::set_slider_rows(hit);
        // The row list's bar as the option lists' (0x5397D8 …:
        // 0x538090(1, count, selection)): hidden with one row, else both
        // arrows and the handle by the selection (Wine, the finished-career save:
        // Aerodynamics' single row shows the bare track, Tires & Brakes'
        // first of three the handle at the top; the clone left the
        // package's handle in the middle).
        scroll_bar_at(
            &mut state,
            package,
            "ScrollBar",
            false,
            1,
            self.rows.len(),
            if self.rows.is_empty() { 0 } else { self.selected + 1 },
        );
        layout(assets, package, &state, now - self.opened)
    }
}

/// UI_PerformanceTuning_Graph / _NOS (factory 0x55DB80 → 0x55AD80 →
/// 0x5578D0, messages 0x5654E0; **verified** unless marked): one widget
/// by the category 0x83A648 — 4 the ECU (parameter 23) and 5 the turbo
/// (24), a curve widget (0x53CFD0, vtable 0x79BEC0, 455 × 241) of nine
/// points (0x550590: PULLPOINT_n / GRAPHBAR_n…, one slider each); 6 the
/// nitrous widget (0x523060 → 0x550850) with one slider for parameter
/// 16 that moves 17 with it (the nitrous writer). Keys: on the curve
/// Left / Right (0x9120409E / 0xB5971BF1) pick the point (0x522EC0,
/// wrapping, "Highlight" / "UnHighlight" scripts) and Up / Down
/// (0x72619778 / 0x911C0A4B) move it (0x522F20); on the nitrous widget
/// Left / Right move the slider (0x523220). Back (0x911AB364) plays
/// 0xDE6EFF34 and returns to UI_PerformanceTuning_Master (0x55DDB0,
/// 0x731BF236). The curve's drawing (`curve_bars`, 0x531050 per point) takes
/// the axis and the nine points in display units from `curve`; the
/// widget's box is 455 × 241 at (65 − 292, −49 + 213) about the screen's
// prov: inferred frontend.screens.TuneGraph
/// centre (0x550590). The bars' colours are **inferred**.
pub struct TuneGraph {
    pub package: &'static str,
    /// 23 (ECU), 24 (turbo) or 16 (nitrous).
    pub param: u8,
    pub point: usize,
    pub category: usize,
    pub opened: f32,
    /// The curve's inputs (`tuning::curve_graph`, set by the app when the
    /// screen opens and after each press); None for the nitrous widget or
    /// when the car's records cannot be read, and then nothing is drawn.
    pub curve: Option<crate::tuning::CurveGraph>,
    /// The nitrous widget's numbers for 16 and 17 (`tuning::slider_view`).
    pub views: Vec<(u8, crate::tuning::SliderView)>,
    /// The units option (0x83AA1A = 1): the torque axis' "Nm", else
    /// "ft-lbs" (0x5507D6 / 0x550D61).
    pub metric: bool,
}

impl TuneGraph {
    pub fn new(category: usize, now: f32) -> TuneGraph {
        let (package, param) = match category {
            4 => ("UI_PerformanceTuning_Graph", 23),
            5 => ("UI_PerformanceTuning_Graph", 24),
            _ => ("UI_PerformanceTuning_NOS", 16),
        };
        TuneGraph { package, param, point: 0, category, opened: now, curve: None, views: Vec::new(), metric: false }
    }

    /// A key: the parameter, point and direction (±1) of a move.
    pub fn key(&mut self, key: Key) -> Option<(u8, usize, f32)> {
        let curve = self.param != 16;
        match key {
            Key::Left if curve => self.point = (self.point + 8) % 9,
            Key::Right if curve => self.point = (self.point + 1) % 9,
            Key::Up if curve => return Some((self.param, self.point, 1.0)),
            Key::Down if curve => return Some((self.param, self.point, -1.0)),
            Key::Left => return Some((16, 0, -1.0)),
            Key::Right => return Some((16, 0, 1.0)),
            _ => {}
        }
        None
    }

    /// The widget drawn with the package's own sprites (so their colours
    /// and textures are the game's):
    /// * the curve (0x549E60 / 0x531050, **verified**): per point k the
    ///   value bar GRAPHBAR_k (+0x4), the low bar GRAPHBAR_B_k (+0x8), the
    ///   high bar GRAPHBAR_C_k (+0xC), each the pitch wide on the baseline
    ///   and scale × (v − min) / span tall, and the handle PULLPOINT_k
    ///   (+0x10) centred on the column at the value bar's top (the mapping
    ///   from 0x549E60's four 0x50B990 names to the setters 0x53CE50 …
    ///   0x53CF70, **verified** from the disassembly);
    /// * the nitrous widget (0x5312D0, **verified** for the geometry; placed
    ///   at (0, 0) by 0x5578D0, so its origin is (−148, 122), 0x550850): the
    ///   line 0x11495CD6 from (rLo16 / max16 × W, rHi17 / max17 × H) to
    ///   (rHi16 / max16 × W, rLo17 / max17 × H) above the origin (its length
    ///   the distance, turned by the angle 0x43E6D0 gives, centred between),
    ///   the vertical 0x01183698 (H tall, on the origin's line) at the
    ///   value of 16, the horizontal 0xD35A7923 (W wide, from the origin's
    ///   x) at the value of 17, the dot 0x9C90D375 where they cross. W and H
    ///   are the widget's +0x18 / +0x1C; the constructor (0x523060) stores
    ///   455 / 241, but the frames under Wine match the two markers' own
    // prov: inferred frontend.screens.quads-4
    ///   sizes (319.3 / 202.1), which the clone uses (**inferred**: which
    ///   code sets them; the markers' ±1 nudges are not ported).
    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        let view = |p: u8| view_of(&self.views, p);
        let Some(package) = assets.package(self.package) else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Select");
        let size_of = |h: u32| {
            package.object(h).map(|o| {
                let s = o.attributes.size();
                Vec2::new(s.x, s.y)
            })
        };
        // The axis titles (verified): the y title 0xCB9A2C64 is "Torque ( %s
        // )" (0x38D548D2) with "Nm" (0x5B81D613) when 0x83AA1A is 1, else
        // "ft-lbs" (0xACB8DFF3) (0x5507B0 for the curve, 0x550D3B for the
        // nitrous widget); the x title 0x83841A5E "RPM (x1000)" (0x3389F685,
        // 0x549FE0) or "Duration (s)" (0x035C9376, 0x54A270). Under Wine (the
        // finished-career save, km/h): "Torque ( Nm )", "RPM (x1000)", "Duration (s)";
        // the clone kept the package's "Torque N-M" / "X 1000 RPM" /
        // "Duration in seconds".
        let text = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        let unit = text(if self.metric { fe_ids::NM } else { fe_ids::FT_LBS });
        state.text.insert(0xCB9A_2C64, text(0x38D5_48D2).replace("%s", &unit));
        state.text.insert(0x8384_1A5E, text(if self.param == 16 { 0x035C_9376 } else { 0x3389_F685 }));
        if self.param == 16 {
            let (d, p) = (view(16), view(17));
            let origin = Vec2::new(-148.0, 122.0);
            let w = size_of(fe_ids::OBJ_D35A7923).map_or(455.0, |s| s.x);
            let h = size_of(fe_ids::OBJ_01183698).map_or(241.0, |s| s.y);
            let fx = |v: f32| origin.x + v / if d.max == 0.0 { 1.0 } else { d.max } * w;
            let fy = |v: f32| origin.y - v / if p.max == 0.0 { 1.0 } else { p.max } * h;
            let (a, b) = (Vec2::new(fx(d.lo), fy(p.hi)), Vec2::new(fx(d.hi), fy(p.lo)));
            let line = 0x1149_5CD6;
            let thick = size_of(line).map_or(3.0, |s| s.y);
            state.position.insert(line, (a + b) * 0.5);
            state.size.insert(line, Vec2::new((b - a).length(), thick));
            state.angle.insert(line, (b.y - a.y).atan2(b.x - a.x));
            let (vx, hy) = (fx(d.value), fy(p.value));
            let vt = size_of(fe_ids::OBJ_01183698).map_or(3.0, |s| s.x);
            state.position.insert(fe_ids::OBJ_01183698, Vec2::new(vx, origin.y - h * 0.5));
            state.size.insert(fe_ids::OBJ_01183698, Vec2::new(vt, h));
            let ht = size_of(fe_ids::OBJ_D35A7923).map_or(3.0, |s| s.y);
            state.position.insert(fe_ids::OBJ_D35A7923, Vec2::new(origin.x + w * 0.5, hy));
            state.size.insert(fe_ids::OBJ_D35A7923, Vec2::new(w, ht));
            state.position.insert(0x9C90_D375, Vec2::new(vx, hy));
            // The axis numbers (0x54A270, verified): RPM_DATA_c (c = 0..9)
            // read `%d` of 2c, their box's top left at (2c / max16 · W +
            // x0, y0 + 6), hidden at or past the widget's right end;
            // TORQUE_DATA_c+1 (c = 0..4) `%$d` of c · step, step 50 when
            // _ftol(max17) / 50 < 5, else 100, or 200 when _ftol(max17) /
            // 100 > 4, anchored at x0 − 8 with its box's top at y0 − (H −
            // 16) · c · step / max17, hidden above y0 − H (0x51D2E0 /
            // 0x51D390 place a string's box by its justification). Wine,
            // the finished-career save: 0 … 10 across, 0 and 50 up; the clone kept
            // the package's 0 … 12.
            let place = |state: &mut ScreenState,
                         h: u32,
                         text: String,
                         top_left: Option<Vec2>,
                         anchor_x: Option<f32>,
                         top: f32| {
                let Some(o) = package.object(h) else { return };
                let scale = o.attributes.size();
                let font = package.resources.get(o.resource as usize).and_then(|r| assets.font_for(&r.name));
                let (w, lh) = font.map_or((0.0, 0.0), |f| (f.measure(&text) * scale.x, f.line_height * scale.y));
                let jx = if o.justify & 1 != 0 {
                    w * 0.5
                } else if o.justify & 2 != 0 {
                    w
                } else {
                    0.0
                };
                let jy = if o.justify & 4 != 0 {
                    lh * 0.5
                } else if o.justify & 8 != 0 {
                    lh
                } else {
                    0.0
                };
                let x = match (top_left, anchor_x) {
                    (Some(tl), _) => tl.x + jx,
                    (None, Some(ax)) => ax,
                    _ => o.attributes.position().x,
                };
                state.position.insert(h, Vec2::new(x, top + jy));
                state.text.insert(h, text);
            };
            let max16 = if d.max == 0.0 { 1.0 } else { d.max };
            for c in 0..10u32 {
                let h = fe_hash(&format!("RPM_DATA_{c}"));
                let v = 2 * c;
                let x = v as f32 / max16 * w + origin.x;
                if x >= origin.x + w {
                    state.hidden.insert(h);
                } else {
                    place(&mut state, h, v.to_string(), Some(Vec2::new(x, 0.0)), None, origin.y + 6.0);
                }
            }
            // The scale is the widget's +0x78 (0x550850: parameter 17's
            // max, 1.0 when 0) × 100.0 (0x784258, at 0x54A3D5), _ftol'd for
            // the step's test and its inverse for the places (Wine, the
            // finished-career save: 0 and 50).
            let max17 = if p.max == 0.0 { 1.0 } else { p.max } * 100.0;
            let top17 = max17 as i32;
            let step = if top17 / 50 < 5 {
                50
            } else if top17 / 100 > 4 {
                200
            } else {
                100
            };
            for c in 0..5 {
                let obj = fe_hash(&format!("TORQUE_DATA_{}", c + 1));
                let y = origin.y - (h - 16.0) * c as f32 / max17 * step as f32;
                if y < origin.y - h {
                    state.hidden.insert(obj);
                } else {
                    place(
                        &mut state,
                        obj,
                        crate::frontend::grouped(i64::from(step * c)),
                        None,
                        Some(origin.x - 8.0),
                        y,
                    );
                    state.hidden.remove(&obj);
                    state.shown.insert(obj);
                }
            }
            // The widget's slider (+0x20, vtable 0x79AE5C) on parameter 16,
            // drawn by 0x5273B0 as a row's (see `TuneSliders::quads`) about
            // prov: inferred frontend.screens.quads-5
            // SLIDER_BASE_1 where the package has it (**inferred**: that
            // 0x54A1A0 leaves the base there and its +0xAC at 4, as the
            // frames under Wine show).
            if let Some(base) = package.object(fe_hash("SLIDER_BASE_1")) {
                let (bp, bs) = (base.attributes.position(), base.attributes.size());
                let (bx, top) = (bp.x - bs.x * 0.5, bp.y - bs.y * 0.5);
                let span = if d.max - d.min == 0.0 { 1.0 } else { d.max - d.min };
                let travel = bs.x - 2.0 * SLIDER_MARGIN;
                let x0 = bx + SLIDER_MARGIN + (d.lo - d.min) / span * travel;
                let green = (d.value - d.lo) / span * travel;
                let red = (d.hi - d.value) / span * travel;
                for (h, x, w) in
                    [(fe_hash("SLIDER_HANDLE_1"), x0, green), (fe_hash("SLIDER_B_HANDLE_1"), x0 + green, red)]
                {
                    let hh = size_of(h).map_or(5.75, |s| s.y);
                    state.position.insert(h, Vec2::new(x + w * 0.5, top + 5.0 + hh * 0.5));
                    state.size.insert(h, Vec2::new(w, hh));
                }
                let knob = fe_hash("SLIDER_KNOB_1");
                state.position.insert(
                    knob,
                    Vec2::new(x0 + green - 4.0, top + 4.0) + size_of(knob).unwrap_or(Vec2::splat(16.0)) * 0.5,
                );
            }
            return layout(assets, package, &state, now - self.opened);
        }
        // Without the curve's inputs the bars keep their templates.
        if let Some(curve) = &self.curve {
            const PITCH: f32 = 455.0 * 0.111_111_111_938_953_4;
            const SCALE: f32 = 241.0;
            let origin = Vec2::new(65.0 - 292.0, -49.0 + 213.0);
            let (min, max) = curve.axis;
            let span = if max - min == 0.0 { 1.0 } else { max - min };
            // The y axis' six numbers (0x54A13C, verified): TORQUE_DATA_k
            // `%$d` of _ftol(axis min) + (k − 1) · _ftol((max − min) · 0.2)
            // (Wine, the finished-career save's ECU: 50, 150 … 550; the clone kept
            // the package's 0 … 500).
            let step = ((max - min) * 0.2) as i64;
            for k in 0..6i64 {
                state.text.insert(
                    fe_hash(&format!("TORQUE_DATA_{}", k + 1)),
                    crate::frontend::grouped(min as i64 + k * step),
                );
            }
            let height = |v: f32| (v - min) / span * SCALE;
            for (k, point) in curve.points.iter().enumerate() {
                let col = origin.x + PITCH * k as f32;
                for (name, v) in [("GRAPHBAR_", point.value), ("GRAPHBAR_B_", point.low), ("GRAPHBAR_C_", point.high)] {
                    let hsh = fe_hash(&format!("{name}{}", k + 1));
                    let hgt = height(v);
                    state.position.insert(hsh, Vec2::new(col + PITCH * 0.5, origin.y - hgt * 0.5));
                    state.size.insert(hsh, Vec2::new(PITCH, hgt));
                }
                let pull = fe_hash(&format!("PULLPOINT_{}", k + 1));
                state.position.insert(pull, Vec2::new(col + PITCH * 0.5, origin.y - height(point.value)));
                // The selected point runs "Highlight", the others
                // "UnHighlight" (0x522EC0).
                let script = if k == self.point { "Highlight" } else { "UnHighlight" };
                if package.object(pull).is_some_and(|o| o.scripts.iter().any(|sc| sc.name == fe_hash(script))) {
                    state.run(pull, script, 0.0);
                }
                // The mouse's columns (0x522FA0: the point's +0x38 / +0x40).
                crate::frontend::mouse::record_rect(
                    self.package,
                    crate::frontend::mouse::CURVE_COLUMN + k as u32,
                    Vec2::new(col + 320.0, 0.0),
                    Vec2::new(col + PITCH + 320.0, 480.0),
                    Default::default(),
                );
            }
        }
        layout(assets, package, &state, now - self.opened)
    }
}

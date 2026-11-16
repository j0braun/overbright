//! The Graphics page's rows as one table (docs/graphics-settings-audit.md
//! "Tiers"): each row is a list of named quality tiers (Off / Low / Medium /
//! High / Very High / Ultra, or the original's own values), each tier with
//! a one-line description and its GPU cost, mapped onto the stored values
//! (`Settings::custom` for the original's Customize Display values,
//! `qol::Enhancements` for the clone's).
//!
//! One concept, one row (no two settings should do the same
//! thing): where the clone enhances something the original
//! already has a Customize Display row for (car reflection detail and update
//! rate, car shadow, car geometry detail, world detail, road reflection
//! detail, texture filtering), the row's first tiers ARE the original's
//! values (same storage, `custom.N` in options.txt) and the enhanced tiers
//! follow above the original's top ([`GraphicsRow::native`]). The
//! Customize Display page still shows the original's row; it is the same
//! setting (see `options::change` for `RowId::Custom`).
//!
//! Rows sharing an original Customize Display row use its label (the
//! game's string).
//!
//! Adding a row (e.g. the dynamic lamps): add a `RowId` variant and its label
//! (`options::clone_label`), the stored value (`qol::Enhancements` field,
//! its `parse` / `write` / `apply`), then one [`GraphicsRow`] entry here in
//! its group's place with its tiers, `get` and `set`. Presets: `qol::Preset::apply`.

use crate::frontend::options::{
    AA_FILTER, AA_MSAA, ENHANCED_PAINT, ENHANCED_REFLECTIONS, HDR_POST, LOD_FADE, POST_AA, RENDER_SCALE, REVERSED_Z,
    SHARPEN, SSAO, SSR, WET_ROADS,
};
use crate::frontend::options::{AMBIENT_DETAIL, ANIMATED_WATER, PARTICLES, SKY, SPARKS_FIRE, UPSCALE, VOLUMETRIC_FOG};
use crate::frontend::options::{CONTACT_SHADOWS, DYNAMIC_LAMPS, LAMP_SHADOWS};
use crate::frontend::options::{CUSTOM_CAPS, GraphicsGroup, RowId, Settings, custom_presets};
use crate::frontend::options::{ENHANCED_GLASS, HEADLIGHT_BEAMS, LIGHT_PROBES, SURFACE_DETAIL};
use crate::frontend::options::{HDR_GAMMA, HDR_OUTPUT, HDR_PAPER, HDR_PEAK};
use crate::frontend::options::{LENS_DROPS, RAIN_DETAIL, SPRAY, WET_CAR, WET_INTERACTION};
use crate::frontend::options::{MODERN_LIGHTING, MODERN_STRENGTH};
use crate::frontend::options::{SMOOTH_CARS, TEXTURE_PACK};
use crate::qol;

mod costs;
mod rows_cars;
mod rows_display;
mod rows_effects;
mod rows_image;
mod rows_lighting;
mod rows_weather;
use costs::*;

/// One value of a row.
#[derive(Clone, Copy, Debug)]
pub struct Tier {
    pub name: &'static str,
    /// What this value does (the detail line).
    pub help: &'static str,
    /// GPU busy ms it adds over Original at 3840 × 2160 on the reference desktop GPU
    /// (scaled by the output's pixels) and ms that do not scale (CPU);
    /// None: native / free of any measurable cost.
    pub cost: Option<(f32, f32)>,
}

const fn t(name: &'static str, help: &'static str) -> Tier {
    Tier { name, help, cost: None }
}

const fn tc(name: &'static str, help: &'static str, gpu_4k: f32, fixed: f32) -> Tier {
    Tier { name, help, cost: Some((gpu_4k, fixed)) }
}

/// A row of the Graphics group pages.
#[derive(Clone, Copy)]
pub struct GraphicsRow {
    pub id: RowId,
    pub label: u32,
    pub group: GraphicsGroup,
    /// What the row does (the detail line's first part).
    pub help: &'static str,
    pub tiers: &'static [Tier],
    /// The Customize Display row (`CUSTOM_ROWS` index) whose values 0 to
    /// its cap are this row's first tiers; the tiers after them are the
    /// clone's enhancements and keep that value at least at World Detail
    /// 5's ([`GraphicsRow::native_top`]).
    pub native: Option<usize>,
    /// The current tier.
    pub get: fn(&Settings) -> usize,
    /// Put tier `i` in the stored values.
    pub set: fn(&mut Settings, usize),
    /// Whether tier `i` is offered on this adapter and output (None: all).
    pub offered: Option<fn(&Settings, usize) -> bool>,
    /// A removed row (audit): listed only while not on tier 0, and any
    /// change puts it on tier 0.
    pub legacy: bool,
}

impl GraphicsRow {
    /// The number of tiers that are the original's values.
    pub fn native_tiers(&self) -> usize {
        self.native.map_or(1, |i| usize::from(CUSTOM_CAPS[i]) + 1)
    }

    /// The original's value the enhanced tiers build on: World Detail 5's
    /// (0x5BE880; World Detail's is 2 of its cap 3).
    pub fn native_top(&self) -> usize {
        self.native.map_or(0, |c| usize::from(custom_presets(5)[c]))
    }

    /// Whether the current value is one of the clone's enhanced tiers
    /// above the original's own values.
    pub fn enhanced(&self, s: &Settings) -> bool {
        (self.get)(s) >= self.native_tiers()
    }

    /// Whether tier `i` can be chosen: offered on this adapter and output
    /// and not taken out by a settings-model rule in force (
    /// `settings_model::RULES`, e.g. SSAA under FSR 1).
    pub fn is_offered(&self, s: &Settings, i: usize) -> bool {
        self.offered.is_none_or(|f| f(s, i)) && !super::settings_model::blocked(self, s, i)
    }
}

fn index_of<T: PartialEq + PartialOrd + Copy>(list: &[T], v: T) -> usize {
    list.iter().position(|&x| x == v).unwrap_or_else(|| list.iter().rposition(|&x| x <= v).unwrap_or(0))
}

/// Native value `i` of Customize Display row `c` (≤ its cap); for an
/// enhanced tier (returned as 1, 2, …) the value is held at least at World
/// Detail 5's, and the player's own value before that raise is remembered
/// (`Settings::custom_raised`) for [`normalize`] to give back. A native
/// value chosen here is the player's own: nothing is remembered.
fn custom_part(s: &mut Settings, c: usize, i: usize) -> Option<usize> {
    let cap = usize::from(CUSTOM_CAPS[c]);
    if i <= cap {
        s.custom[c] = i as u8;
        s.custom_raised[c] = None;
        None
    } else {
        let raised = s.custom[c].max(custom_presets(5)[c]).min(cap as u8);
        if raised != s.custom[c] && s.custom_raised[c].is_none() {
            s.custom_raised[c] = Some(s.custom[c]);
        }
        s.custom[c] = raised;
        Some(i - cap)
    }
}

fn custom_get(s: &Settings, c: usize, enhanced: usize) -> usize {
    if enhanced > 0 { usize::from(CUSTOM_CAPS[c]) + enhanced } else { usize::from(s.custom[c].min(CUSTOM_CAPS[c])) }
}

/// Every row, the groups in the page's order (`rows_*.rs`).
const ALL_LEN: usize = rows_image::ROWS.len()
    + rows_lighting::ROWS.len()
    + rows_cars::ROWS.len()
    + rows_effects::ROWS.len()
    + rows_weather::ROWS.len()
    + rows_display::ROWS.len();
const fn concat() -> [GraphicsRow; ALL_LEN] {
    let parts: [&[GraphicsRow]; 6] = [
        rows_image::ROWS,
        rows_lighting::ROWS,
        rows_cars::ROWS,
        rows_effects::ROWS,
        rows_weather::ROWS,
        rows_display::ROWS,
    ];
    let mut out = [parts[0][0]; ALL_LEN];
    let (mut p, mut k) = (0, 0);
    while p < parts.len() {
        let mut i = 0;
        while i < parts[p].len() {
            out[k] = parts[p][i];
            k += 1;
            i += 1;
        }
        p += 1;
    }
    out
}
static ALL: [GraphicsRow; ALL_LEN] = concat();
pub static ROWS: &[GraphicsRow] = &ALL;

/// The settings' program key: a digest of the Graphics rows' Enhanced
/// tiers (a row that shares its first tiers with a Customize Display row
/// counts only above them), which decide the Enhanced shader programs a
/// drive makes. The prepare step's stamp carries it, so an Enhanced level
/// changed since (in the launcher's Graphics row or in the game) shows as
/// not prepared. The Customize Display values are left out: the game's
/// first run sets them for the hardware after a first prepare; a change
/// there is made by the in-menu build as before.
pub fn program_key(s: &Settings) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for r in ROWS {
        let tier = (r.get)(s);
        let tier = if r.native.is_some() && tier < r.native_tiers() { 0 } else { tier };
        for b in format!("{:?}={tier};", r.id).bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

/// The table row of `id`.
pub fn row(id: RowId) -> Option<&'static GraphicsRow> {
    ROWS.iter().find(|r| r.id == id)
}

/// The row that shares its first tiers with Customize Display row `c`.
pub fn for_custom(c: usize) -> Option<&'static GraphicsRow> {
    ROWS.iter().find(|r| r.native == Some(c))
}

/// Group `g`'s rows in order: the legacy ones only while on.
pub fn group_rows(g: GraphicsGroup, s: &Settings) -> Vec<&'static GraphicsRow> {
    ROWS.iter().filter(|r| r.group == g && (!r.legacy || (r.get)(s) != 0)).collect()
}

/// Step row `r` one offered tier left or right, wrapping; a legacy row
/// goes to tier 0.
pub fn step(r: &GraphicsRow, s: &mut Settings, right: bool) {
    if r.legacy {
        (r.set)(s, 0);
        return;
    }
    let at = (r.get)(s);
    let offered: Vec<usize> = (0..r.tiers.len()).filter(|&i| r.is_offered(s, i)).collect();
    if offered.is_empty() {
        return;
    }
    let next = if right {
        offered.iter().copied().find(|&i| i > at).unwrap_or(offered[0])
    } else {
        offered.iter().rev().copied().find(|&i| i < at).unwrap_or(offered[offered.len() - 1])
    };
    (r.set)(s, next);
}

/// Keep the shared rows consistent: an enhanced tier keeps its Customize
/// Display value at the cap, and the removed clone rows that duplicated a
/// Customize Display row (enh.anisotropy, enh.reflection_rate) become
/// that row's value. A row no longer on an enhanced tier (Restore Native,
/// the Original preset, a preset without it) gets back the player's value an
/// enhanced tier raised. A no-op on native values.
pub fn normalize(s: &mut Settings) {
    for r in ROWS.iter() {
        if let Some(c) = r.native
            && !r.enhanced(s)
            && let Some(v) = s.custom_raised[c].take()
        {
            s.custom[c] = v.min(CUSTOM_CAPS[c]);
        }
    }
    for r in ROWS.iter().filter(|r| r.native.is_some()) {
        let i = (r.get)(s);
        (r.set)(s, i);
    }
}

#[cfg(test)]
mod water_tier_tests {
    use super::*;

    /// Animated Water's row steps Off / On / Realistic /
    /// Next-Gen through `enh.water` 0 / 1 / 2 / 3.
    #[test]
    fn animated_water_has_a_realistic_tier() {
        let r = row(RowId::AnimatedWater).unwrap();
        assert_eq!(r.tiers.iter().map(|t| t.name).collect::<Vec<_>>(), ["Off", "On", "Realistic", "Next-Gen"]);
        let mut s = Settings::default();
        assert_eq!((r.get)(&s), 0);
        for i in 0..4 {
            (r.set)(&mut s, i);
            assert_eq!(s.enh.water, i as u8);
            assert_eq!((r.get)(&s), i);
        }
    }
}

#[cfg(test)]
mod program_key_tests {
    use super::*;

    /// The prepare stamp's key follows the Enhanced level (Original, the
    /// presets) and not the Customize Display values the game's first run
    /// sets for the hardware.
    #[test]
    fn the_program_key_follows_the_enhanced_level_only() {
        let base = Settings::default();
        let key = program_key(&base);
        let mut detail = base.clone();
        detail.estimated_detail(1);
        assert_eq!(program_key(&detail), key, "the first run's World Detail values");
        let mut keys = vec![key];
        for preset in crate::qol::PRESETS.iter().copied().filter(|p| *p != crate::qol::Preset::Original) {
            let mut s = base.clone();
            preset.apply(&mut s.enh);
            keys.push(program_key(&s));
        }
        let mut distinct = keys.clone();
        distinct.sort();
        distinct.dedup();
        assert_eq!(distinct.len(), keys.len(), "every preset has its own key");
        let mut ssao = base.clone();
        ssao.enh.ssao = 50;
        assert_ne!(program_key(&ssao), key, "one Enhanced row changed");
    }
}

#[cfg(test)]
mod cost_tests {
    use super::*;

    /// Every hint the budget table measured (docs/evidence/perf-budget/
    /// hints-4k.tsv: 4K, a maxed play setup, leave-one-out) stays within
    /// 1.5x of it or 0.3 ms (the table's run-to-run noise at 4K under max).
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn cost_hints_follow_the_budget_table() {
        let table = &crate::testkit::private_str("docs/evidence/perf-budget/hints-4k.tsv");
        let mut checked = 0;
        for line in table.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let f: Vec<&str> = line.split('\t').collect();
            let (row, tier, measured) = (f[0], f[1], f[2].parse::<f32>().unwrap());
            let r = ROWS.iter().find(|r| format!("{:?}", r.id) == row).unwrap_or_else(|| panic!("no row {row}"));
            let t = r.tiers.iter().find(|t| t.name == tier).unwrap_or_else(|| panic!("{row}: no tier {tier}"));
            let hint = t.cost.map_or(0.0, |c| c.0);
            let ratio = hint / measured;
            assert!(
                (hint - measured).abs() <= 0.3 || (1.0 / 1.5..=1.5).contains(&ratio),
                "{row} {tier}: hint {hint} ms, the budget table {measured} ms ({})",
                f[3]
            );
            checked += 1;
        }
        assert!(checked >= 40, "{checked} hints checked");
    }
}

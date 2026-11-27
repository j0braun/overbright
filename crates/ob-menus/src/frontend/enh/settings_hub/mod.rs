//! The settings page (docs/developing/settings-page.md) — the Enhancements entry
//! of the Display options as one page for every clone setting, built on
//! The settings model ([`super::settings_model`]).
//!
//! * The hub ([`super::options::Category::Enhancements`]): the preset row
//!   (Original / Balanced / High / Ultra / Extreme / "60 fps on this
//!   machine"), Compare (Original ↔ current, live A/B), Show (a filter for
//!   the group pages), the seven settings groups with how many of their rows are
//!   changed, Restore Native.
//! * A group page ([`super::options::Category::Settings`]): the group's rows
//!   in [`super::settings_model::MODEL`]'s order (shared, enhanced and
//!   convenience rows; the original's Customize Display rows an enhancement
//!   needs, as the same setting), then Reset Group to Native. LTrigger /
//!   RTrigger ([ / ], the pad's shoulders) step to the previous / next
//!   group, Button1 (C, the pad's X) toggles Compare anywhere on the page.
//! * Every row's detail line says, in plain words, whether it is the
//!   original's or an enhancement, when it applies, its measured cost
//!   ("~+0.35 ms at 4K, +0.09 here"), the rule in force (overridden /
//!   needs / blocked, from the settings model) and what it does.
//!
//! Nothing here is native: the original has no such page. The native
//! Customize Display / options screens are unchanged (their rows, look and
//! flow); the page only reads and writes the same stored values, so
//! options.txt keeps its keys. Compare, Show and the 60 fps pick are
//! session state (not saved).
//!
//! Strings: every text the page adds is in [`text`] (one table, keyed by
//! [`Text`]; labels by id in `options::clone_label`), ready for a
//! translation table.

use std::sync::atomic::{AtomicU8, Ordering};

use super::graphics_table::{self, GraphicsRow};
use super::settings_model::{self, Applies, MODEL, Origin, UxGroup};
use crate::frontend::options::{self, Category, Kind, Row, RowId, Settings, Value};
use crate::qol::{self, GpuClass, Preset};

mod detail;
mod main_page;
mod presets;
mod rows;
mod text;
pub use detail::*;
pub use main_page::*;
pub use presets::*;
pub use rows::*;
pub use text::*;

// ---------------------------------------------------------------------
// Session state (not saved).

/// Show: 0 all rows, 1 changed only, 2 costly only.
static FILTER: AtomicU8 = AtomicU8::new(0);
/// The 60 fps entry's pick (`PAGE_PRESETS` index + 1), 0 when not chosen.
static FPS60: AtomicU8 = AtomicU8::new(0);

pub fn filter() -> u8 {
    FILTER.load(Ordering::Relaxed)
}

pub fn set_filter(v: u8) {
    FILTER.store(v % 3, Ordering::Relaxed);
}

/// The page's session state back to its start (leaving the page): Compare
/// off (the stored values in force again), Show all, no 60 fps pick shown.
pub fn reset_session(s: &Settings) {
    let was = qol::comparing();
    qol::set_compare(false);
    FILTER.store(0, Ordering::Relaxed);
    FPS60.store(0, Ordering::Relaxed);
    if was {
        s.apply_enhancements();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native() -> Settings {
        let mut s = Settings::default();
        s.custom = options::custom_presets(5);
        s
    }

    /// Every model row is on exactly one group page, every group has rows,
    /// and each page ends with its reset row.
    #[test]
    fn every_row_on_one_page() {
        let s = native();
        for m in MODEL {
            let n = GROUPS.iter().filter(|&&g| group_ids(g).contains(&m.id)).count();
            assert_eq!(n, 1, "{:?}", m.id);
        }
        for g in GROUPS {
            let rows = group_rows(g, &s);
            assert!(rows.len() >= 2, "{g:?}");
            assert_eq!(rows.last().map(|r| r.id), Some(RowId::UxReset(g)));
            for r in &rows {
                // The clone's labels by id; shared rows use the game's string.
                let shared = settings_model::model(r.id).is_some_and(|m| matches!(m.origin, Origin::Shared(_)));
                assert!(
                    options::clone_label(r.label).is_some() || shared || matches!(r.id, RowId::Custom(_)),
                    "{:?} has no label",
                    r.id
                );
                assert!(detail(Category::Settings(g), r.id, &s).is_some_and(|d| d.len() > 10), "{:?}: no detail", r.id);
            }
        }
    }

    /// The defaults show nothing changed; a preset changes rows and the
    /// group reset puts them back to native (the options file to its
    /// defaults); switching back after an override keeps the value.
    #[test]
    fn reset_group_restores_native() {
        let base = native();
        for g in GROUPS {
            assert!(group_ids(g).iter().all(|&id| !changed(id, &base)), "{g:?} changed on defaults");
        }
        let mut s = base.clone();
        Preset::Extreme.apply(&mut s.enh);
        graphics_table::normalize(&mut s);
        s.enh.fov_offset = 10;
        s.overlay.layout = 2;
        assert!(group_ids(UxGroup::ImageQuality).iter().any(|&id| changed(id, &s)));
        for g in GROUPS {
            reset_group(g, &mut s);
        }
        assert_eq!(s.enh, base.enh);
        assert_eq!(s.custom, base.custom);
        assert_eq!(s.options_text(), base.options_text());
    }

    /// The hub and a group page build the native
    /// reference once per page (not two whole-Settings copies per row and
    /// frame), and every row's "changed" answer is the one the old per-row
    /// copy-and-reset gave, on the defaults and on every preset.
    #[test]
    fn pages_build_the_native_reference_once_and_answer_as_before() {
        let old_changed = |id: RowId, s: &Settings| {
            if matches!(id, RowId::Custom(_)) {
                return false;
            }
            let mut n = s.clone();
            reset_row(id, &mut n);
            n != *s
        };
        let base = native();
        let mut cases = vec![base.clone()];
        for p in [Preset::Clean, Preset::Modern, Preset::Ultra, Preset::Extreme] {
            let mut s = base.clone();
            p.apply(&mut s.enh);
            graphics_table::normalize(&mut s);
            s.enh.fov_offset = 10;
            cases.push(s);
        }
        for s in &cases {
            for g in GROUPS {
                for id in group_ids(g) {
                    assert_eq!(changed(id, s), old_changed(id, s), "{id:?}");
                }
            }
            let refs = || rows::NATIVE_REFERENCES.with(|n| n.get());
            let before = refs();
            let hub = hub_rows(s);
            assert!(refs() - before <= 1, "hub: {} native references", refs() - before);
            let before = refs();
            group_rows(UxGroup::LightingEffects, s);
            assert!(refs() - before <= 1, "group page: {} native references", refs() - before);
            assert!(!hub.is_empty());
        }
    }

    /// The preset row steps Original, Balanced, High, Ultra, Extreme, 60 FPS
    /// and wraps; the 60 fps entry applies a preset from the measured table.
    #[test]
    fn preset_row_cycles_with_60fps() {
        let mut s = native();
        let mut names = Vec::new();
        let n = offered_presets().len();
        for _ in 0..n + 1 {
            step_preset(&mut s, true);
            names.push(preset_value(&s));
        }
        assert_eq!(&names[..4], ["Balanced", "High", "Ultra", "Extreme"]);
        assert!(names[n - 1].starts_with("60 FPS ("), "{names:?}");
        assert_eq!(names[n], "Original");
        step_preset(&mut s, false);
        assert!(preset_value(&s).starts_with("60 FPS ("), "Left from Original: the 60 fps entry");
        step_preset(&mut s, true);
        assert_eq!(s.enh, qol::Enhancements::default());
        FPS60.store(0, Ordering::Relaxed);
    }

    /// 60 fps: Original on an integrated GPU at 4K, a richer preset at 1080p
    /// than at 4K on the High class, never above the budget unless Original.
    #[test]
    fn fps60_follows_measured_costs() {
        for class in [GpuClass::High, GpuClass::Mid, GpuClass::Integrated] {
            let mut last = usize::MAX;
            for out in [(1280, 720), (1920, 1080), (2560, 1440), (3440, 1440), (3840, 2160), (7680, 4320)] {
                let (p, ms) = fps60_pick(class, out);
                assert!(p == Preset::Original || ms <= FPS60_BUDGET_MS, "{class:?} {out:?}");
                let i = PRESET_COSTS.iter().position(|c| c.preset == p).unwrap();
                assert!(i <= last, "{class:?}: a bigger screen never picks a costlier preset");
                last = i;
            }
        }
        assert_eq!(fps60_pick(GpuClass::Integrated, (3840, 2160)).0, Preset::Original);
        assert_ne!(fps60_pick(GpuClass::High, (1920, 1080)).0, Preset::Original);
        // Costlier presets cost more (the table is ordered).
        for w in PRESET_COSTS.windows(2) {
            assert!(
                w[0].high_1080 <= w[1].high_1080
                    && w[0].high_4k <= w[1].high_4k
                    && w[0].integrated_1080 <= w[1].integrated_1080
            );
        }
    }

    /// A row a rule overrides reads the reason in its detail line and is
    /// greyed; the group pages and the hub use new label ids only from the
    /// The settings page's range.
    #[test]
    fn rules_show_on_the_page() {
        let mut s = native();
        s.enh.modern_lighting = 2;
        s.enh.dynamic_lamps = true;
        let rows = group_rows(UxGroup::LightingEffects, &s);
        let lamps = rows.iter().find(|r| r.id == RowId::DynamicLamps).unwrap();
        assert!(!lamps.enabled);
        let d = detail(Category::Settings(UxGroup::LightingEffects), RowId::DynamicLamps, &s).unwrap();
        assert!(d.contains("Overridden by Modern Lighting"), "{d}");
        let d = detail(Category::Settings(UxGroup::ImageQuality), RowId::Ssao, &s).unwrap_or_default();
        let _ = d;
        let d = detail(Category::Settings(UxGroup::LightingEffects), RowId::Ssao, &s).unwrap();
        assert!(d.starts_with("Enhancement, off") && d.contains("Applies at once"), "{d}");
        s.enh.ssao = 50;
        let d = detail(Category::Settings(UxGroup::LightingEffects), RowId::Ssao, &s).unwrap();
        assert!(d.contains("ms at 4K"), "{d}");
    }

    /// Show: Changed lists only the changed rows (or the no-match row).
    #[test]
    fn filter_lists_changed_rows() {
        let mut s = native();
        s.enh.ssao = 50;
        set_filter(1);
        let rows = group_rows(UxGroup::LightingEffects, &s);
        let display = group_rows(UxGroup::Display, &s);
        set_filter(0);
        assert_eq!(
            rows.iter().map(|r| r.id).collect::<Vec<_>>(),
            [RowId::Ssao, RowId::UxReset(UxGroup::LightingEffects)]
        );
        assert_eq!(display[0].id, RowId::UxNoMatch);
    }
}

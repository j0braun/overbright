//! The settings model (docs/developing/settings-model.md) — the data the settings
//! page is built from, and the conflict & redundancy rules of the
//! audit (docs/developing/settings-model.md "Audit").
//!
//! * [`MODEL`]: every clone row's settings group, whether it is the original's
//!   (native), a native row with the clone's tiers above it (shared) or an
//!   enhancement, and when a change takes effect (live, on the next load,
//!   after a restart). The tiers, their help and measured cost stay in
//!   [`super::graphics_table::ROWS`].
//! * [`RULES`]: dependencies and exclusions, each with the plain-words
//!   reason the page shows and the measurement behind it. A rule either
//!   overrides the whole row (shown disabled, "Overridden by X"), blocks
//!   some of its tiers (shown "… off under X"), or notes a missing
//!   dependency. The stored value is never changed by a rule, so turning
//!   the overriding row off gives the row its own value back. Rules marked
//!   `engine` are also resolved by the renderer
//!   ([`crate::qol::Enhancements::effective`]); the others describe what the
//!   renderer already does.
//!
//! Nothing here is native: these are the clone's own options. The
//! original's Customize Display rows are described where an enhancement
//! depends on them (traced in the clone's code, addresses in the reasons'
//! docs).

use super::graphics_table::GraphicsRow;
#[cfg(any(test, feature = "dev-harness"))]
use super::graphics_table::{ROWS, normalize};
use crate::frontend::options::{RowId, Settings};
#[cfg(any(test, feature = "dev-harness"))]
use crate::frontend::options::CUSTOM_CAPS;
use crate::qol;

mod model;
mod rules;
pub use model::*;
pub use rules::*;

/// The combination matrix (scripts/checks/matrix.sh): the settings to
/// draw a few frames each with, from `start` (the bench's own settings).
/// `spec`: comma-separated parts —
/// * `orig`: Original + each row at each of its tiers;
/// * `max`: every row at its top offered tier, then each row at each tier
///   and each Customize Display value (the original's rows under every
///   enhancement);
/// * `pairs`: Original + every two rows at their top tiers;
/// * `rules`: each rule's two rows at the tiers that make it fire, on
///   Original and on max;
/// * `levels`: each Enhanced quality level alone, with ray tracing, and
///   with each main-page Upscaling and Anti-Aliasing choice.
#[cfg(any(test, feature = "dev-harness"))]
pub fn matrix_configs(start: &Settings, spec: &str) -> Vec<(String, Settings)> {
    let rows: Vec<&GraphicsRow> = ROWS.iter().filter(|r| !r.legacy).collect();
    let mut native = start.clone();
    qol::Preset::Original.apply(&mut native.enh);
    normalize(&mut native);
    let top = |r: &GraphicsRow, s: &Settings| (0..r.tiers.len()).rev().find(|&i| r.is_offered(s, i)).unwrap_or(0);
    let mut max = native.clone();
    // Twice: rows offered only with another on (Modern Strength).
    for _ in 0..2 {
        for r in &rows {
            let t = top(r, &max);
            (r.set)(&mut max, t);
        }
    }
    normalize(&mut max);
    let name = |r: &GraphicsRow, i: usize| format!("{:?}={}", r.id, r.tiers[i].name);
    let mut out = Vec::new();
    let each = |base: &Settings, tag: &str, out: &mut Vec<(String, Settings)>| {
        for r in &rows {
            for i in 0..r.tiers.len() {
                let mut s = base.clone();
                (r.set)(&mut s, i);
                normalize(&mut s);
                out.push((format!("{tag} {}", name(r, i)), s));
            }
        }
    };
    for part in spec.split(',').map(str::trim) {
        match part {
            "orig" => each(&native, "orig", &mut out),
            "max" => {
                out.push(("max".to_string(), max.clone()));
                each(&max, "max", &mut out);
                for (c, &cap) in CUSTOM_CAPS.iter().enumerate() {
                    for v in 0..=cap {
                        let mut s = max.clone();
                        s.custom[c] = v;
                        normalize(&mut s);
                        out.push((format!("max custom.{c}={v}"), s));
                    }
                }
            }
            "pairs" => {
                for (k, a) in rows.iter().enumerate() {
                    for b in &rows[k + 1..] {
                        let mut s = native.clone();
                        let ta = top(a, &s);
                        (a.set)(&mut s, ta);
                        let tb = top(b, &s);
                        (b.set)(&mut s, tb);
                        normalize(&mut s);
                        out.push((format!("pair {} {}", name(a, ta), name(b, tb)), s));
                    }
                }
            }
            "rules" => {
                for (k, rule) in RULES.iter().enumerate() {
                    for (tag, base) in [("orig", &native), ("max", &max)] {
                        let Some(r) = crate::frontend::graphics_table::row(rule.row) else { continue };
                        for i in 0..r.tiers.len() {
                            let mut s = base.clone();
                            (r.set)(&mut s, i);
                            trigger(rule, &mut s);
                            normalize(&mut s);
                            out.push((format!("rule {k} {tag} {} by {:?}", name(r, i), rule.by), s));
                        }
                    }
                }
            }
            "levels" => {
                // The main page's quality levels, alone, with ray tracing,
                // and with each Upscaling and Anti-Aliasing choice.
                for l in qol::LEVELS {
                    let mut base = native.clone();
                    l.apply(&mut base.enh);
                    normalize(&mut base);
                    out.push((format!("level {}", l.name()), base.clone()));
                    let mut s = base.clone();
                    qol::set_ray_traced(&mut s.enh, true);
                    normalize(&mut s);
                    out.push((format!("level {} RayTracing=On", l.name()), s));
                    for (id, list) in [(RowId::Upscale, &LEVEL_UPSCALE[..]), (RowId::PostAa, &LEVEL_AA[..])] {
                        let Some(r) = crate::frontend::graphics_table::row(id) else { continue };
                        for &i in list {
                            let mut s = base.clone();
                            (r.set)(&mut s, i);
                            normalize(&mut s);
                            out.push((format!("level {} {}", l.name(), name(r, i)), s));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The matrix's `levels` part: the main page's Upscaling modes and
/// Anti-Aliasing choices other than the level's own (Off, Native).
#[cfg(any(test, feature = "dev-harness"))]
const LEVEL_UPSCALE: [usize; 3] = [1, 2, 3];
#[cfg(any(test, feature = "dev-harness"))]
const LEVEL_AA: [usize; 6] = [qol::TAA as usize, 6, 7, 8, 9, 10];

/// Values that make `rule` fire (for the matrix): its `by` row stepped
/// until `when` holds, else its first non-native tier.
#[cfg(any(test, feature = "dev-harness"))]
fn trigger(rule: &Rule, s: &mut Settings) {
    if (rule.when)(s) {
        return;
    }
    if let RowId::Custom(c) = rule.by {
        s.custom[usize::from(c)] = if s.custom[usize::from(c)] == 0 { CUSTOM_CAPS[usize::from(c)] } else { 0 };
        return;
    }
    if let Some(by) = crate::frontend::graphics_table::row(rule.by) {
        for i in 0..by.tiers.len() {
            let mut t = s.clone();
            (by.set)(&mut t, i);
            if (rule.when)(&t) {
                *s = t;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every Graphics row and every Enhancements page row is in the model
    /// once (builds its page from it).
    #[test]
    fn model_covers_every_row() {
        for r in ROWS.iter() {
            assert_eq!(MODEL.iter().filter(|m| m.id == r.id).count(), 1, "{:?}", r.id);
        }
        let s = Settings::default();
        for row in crate::frontend::options::qol_rows(&s) {
            if matches!(row.id, RowId::Graphics | RowId::RestoreNative | RowId::HdUiPage) {
                continue;
            }
            assert_eq!(MODEL.iter().filter(|m| m.id == row.id).count(), 1, "{:?}", row.id);
        }
        for m in MODEL {
            if let Origin::Shared(c) = m.origin {
                assert_eq!(crate::frontend::graphics_table::row(m.id).and_then(|r| r.native), Some(c), "{:?}", m.id);
            }
        }
    }

    /// Every rule names rows that exist, a reason in plain words and its
    /// evidence; Blocks rules block some tier but never all of them (the
    /// row keeps a value to step to); no rule fires on the Original preset
    /// with the native World Detail 5 values (defaults stay native and
    /// unannotated).
    #[test]
    fn rules_are_well_formed() {
        let mut native = Settings::default();
        native.custom = crate::frontend::options::custom_presets(5);
        for r in RULES {
            assert!(crate::frontend::graphics_table::row(r.row).is_some(), "{:?}", r.row);
            assert!(
                matches!(r.by, RowId::Custom(_)) || crate::frontend::graphics_table::row(r.by).is_some(),
                "{:?}",
                r.by
            );
            assert!(r.reason.len() > 20 && r.evidence.len() > 10 && !r.by_name.is_empty());
            if r.effect == Effect::Blocks {
                let row = crate::frontend::graphics_table::row(r.row).unwrap();
                let b = r.blocks.expect("Blocks needs blocks");
                let n = (0..row.tiers.len()).filter(|&i| b(i)).count();
                assert!(n > 0 && n < row.tiers.len(), "{:?}", r.row);
            }
            // A Needs rule may show on Original (Modern Strength without Modern Lighting).
            assert!(r.effect == Effect::Needs || !(r.when)(&native), "{:?} by {:?} fires on Original", r.row, r.by);
        }
    }

    /// Engine rules and the renderer agree: `Enhancements::effective`
    /// resolves exactly the stacks the engine rules name, and is a no-op on
    /// every preset's own values where no rule fires and on Original.
    #[test]
    fn engine_rules_match_effective() {
        use crate::qol::{Enhancements, PRESETS, Preset};
        let mut e = Enhancements::default();
        assert_eq!(e.effective(), e);
        for p in PRESETS {
            p.apply(&mut e);
            let f = e.effective();
            let mut g = e;
            if g.wet_interaction != 0 {
                g.wet_roads = true;
                g.spray = true;
            }
            assert_eq!(f, g, "{p:?}");
        }
        let _ = Preset::Original;
        let mut s = Settings::default();
        s.enh.upscale = 5;
        s.enh.post_aa = 6;
        s.enh.sharpen = 75;
        let f = s.enh.effective();
        assert_eq!((f.post_aa, f.sharpen), (0, 0));
        assert!(rule_for(RowId::Sharpen, &s).is_some_and(|r| r.engine));
        s.enh.post_aa = 5;
        assert_eq!(s.enh.effective().post_aa, 5, "MSAA stays under FSR 1");
        s.enh.upscale = 0;
        s.enh.modern_lighting = 2;
        s.enh.dynamic_lamps = true;
        assert!(!s.enh.effective().dynamic_lamps);
        assert_eq!(s.enh.dynamic_lamps, true, "the stored value is kept (switching back restores it)");
    }

    /// No stored tier is ever lost to a rule: overriding leaves the value,
    /// and the row reads its own tier again once the overriding row is off.
    #[test]
    fn switching_back_restores_the_value() {
        let mut s = Settings::default();
        s.enh.dynamic_lamps = true;
        s.enh.modern_lighting = 2;
        let r = crate::frontend::graphics_table::row(RowId::DynamicLamps).unwrap();
        assert_eq!(shown(RowId::DynamicLamps, "On", &s), Some(("Overridden by Modern Lighting".to_string(), false)));
        s.enh.modern_lighting = 0;
        assert_eq!(shown(RowId::DynamicLamps, "On", &s), None);
        assert_eq!((r.get)(&s), 1);
    }

    /// The matrix covers every row's every tier on Original and max, every
    /// Customize Display value under max, and every pair of rows.
    #[test]
    fn matrix_covers_every_tier() {
        let mut s = Settings::default();
        s.custom = crate::frontend::options::custom_presets(5);
        let tiers: usize = ROWS.iter().filter(|r| !r.legacy).map(|r| r.tiers.len()).sum();
        let n = ROWS.iter().filter(|r| !r.legacy).count();
        assert_eq!(matrix_configs(&s, "orig").len(), tiers);
        let customs: usize = CUSTOM_CAPS.iter().map(|&c| usize::from(c) + 1).sum();
        assert_eq!(matrix_configs(&s, "max").len(), 1 + tiers + customs);
        assert_eq!(matrix_configs(&s, "pairs").len(), n * (n - 1) / 2);
        assert!(!matrix_configs(&s, "rules").is_empty());
        let levels = matrix_configs(&s, "levels");
        assert_eq!(levels.len(), qol::LEVELS.len() * (2 + LEVEL_UPSCALE.len() + LEVEL_AA.len()));
        for l in qol::LEVELS {
            assert!(levels.iter().any(|(n, c)| *n == format!("level {}", l.name()) && qol::Level::of(&c.enh) == Some(l)));
        }
    }
}

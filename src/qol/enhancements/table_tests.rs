//! The option table's generated code against the hand-written code it
//! replaced (`super::oracle`): options.txt text byte for byte, both parsers,
//! the deviation list, `apply_now`'s stores in order and the presets'
//! keep-list; and every value the settings page can set kept by the table.

use super::super::atomics::APPLY_LOG;
use super::super::*;
use super::oracle::{Old, old_preset_base};

/// The keys of the nested groups (`extras`, `pad`; their own code).
const NESTED_KEYS: [&str; 17] = [
    "enh.low_latency",
    "enh.replay_cameras",
    "enh.hud_palette",
    "enh.hud_minimap",
    "enh.hud_speedo",
    "enh.hud_info",
    "enh.reduced_flashing",
    "enh.power_saving",
    "enh.mark_spot",
    "enh.pad_layout",
    "enh.pad_deadzone",
    "enh.pad_trigger_deadzone",
    "enh.pad_saturation",
    "enh.pad_steer_curve",
    "enh.pad_rumble",
    "enh.pad_pause_on_disconnect",
    "enh.pad_safe_bind",
];

fn table_keys() -> Vec<&'static str> {
    OPTION_ROWS.iter().filter_map(|r| r.key).collect()
}

fn all_keys() -> Vec<&'static str> {
    let mut k = table_keys();
    k.extend(NESTED_KEYS);
    k
}

/// Values tried on every key: in range, out of range, not numbers, spaced.
const PROBES: [&str; 51] = [
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "10",
    "11",
    "12",
    "13",
    "15",
    "16",
    "20",
    "35",
    "70",
    "75",
    "80",
    "85",
    "90",
    "95",
    "125",
    "150",
    "-1",
    "-5",
    "-10",
    "-11",
    "25",
    "49",
    "99",
    "101",
    "128",
    "200",
    "255",
    "256",
    "300",
    "511",
    "512",
    "513",
    "1000",
    "1001",
    "2048",
    "10001",
    "65535",
    "65536",
    "4294967296",
    "x",
    "",
];
const PROBES_ODD: [&str; 8] = [" 3 ", "1.5", "+2", "0x10", "1 # comment", "true", "on", "00"];

/// A small deterministic generator (an LCG): the test objects are the
/// same on every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as usize
    }
}

/// Three settings objects in which every key (the nested groups' too) is
/// off its default: each key gets the first of a seeded draw of probes that
/// the OLD parser takes to a value that is not the default.
fn objects() -> Vec<Enhancements> {
    let d = Enhancements::default();
    (1..=3u64)
        .map(|seed| {
            let mut rng = Lcg(seed * 0x9E37_79B9);
            let mut e = Enhancements::default();
            for key in all_keys() {
                let mut done = false;
                for _ in 0..400 {
                    let v = PROBES[rng.next() % PROBES.len()];
                    let mut t = e;
                    t.old_parse(key, v);
                    let mut alone = d;
                    alone.old_parse(key, v);
                    if alone != d {
                        e = t;
                        done = true;
                        break;
                    }
                }
                assert!(done, "seed {seed}: no non-default value for {key}");
            }
            e
        })
        .collect()
}

fn old_text(e: &Enhancements) -> String {
    let mut out = String::new();
    e.old_write(&mut out);
    out
}

fn new_text(e: &Enhancements) -> String {
    let mut out = String::new();
    e.write(&mut out);
    out
}

/// options.txt's line split (`Settings::from_text`) through one parser.
fn read(text: &str, old: bool) -> (Enhancements, Vec<bool>) {
    let mut e = Enhancements::default();
    let mut known = Vec::new();
    for line in text.lines() {
        let (k, v) = line.split_once(' ').unwrap_or((line, ""));
        known.push(if old { e.old_parse(k, v) } else { e.parse(k, v) });
    }
    (e, known)
}

#[test]
fn the_table_is_well_formed() {
    assert_eq!(OPTION_ROWS.len(), 71);
    let mut fields: Vec<_> = OPTION_ROWS.iter().map(|r| r.field).collect();
    fields.sort();
    fields.dedup();
    assert_eq!(fields.len(), OPTION_ROWS.len(), "a field twice");
    let mut keys = all_keys();
    keys.extend(LEGACY_KEYS);
    let n = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), n, "a key twice (table, nested groups, legacy)");
    for rank in [|r: &OptionRow| r.write_rank, |r: &OptionRow| r.dev_rank, |r: &OptionRow| r.apply_rank] {
        let mut v: Vec<u8> = OPTION_ROWS.iter().map(rank).filter(|&r| r != 0).collect();
        let n = v.len();
        v.sort();
        v.dedup();
        assert_eq!(v.len(), n, "a rank twice");
    }
    assert!(OPTION_ROWS.iter().all(|r| r.write_rank != 0), "every row is written");
    // Every nested key is one of its group's (parsed by it).
    for k in NESTED_KEYS {
        let mut e = Enhancements::default();
        assert!(e.old_parse(k, "1") && e.parse(k, "1"), "{k}");
    }
}

/// (a) options.txt: the new writer's text is the old one's byte for byte,
/// and both parsers read it back to the object.
#[test]
fn golden_text_and_round_trip() {
    let mut all = vec![Enhancements::default()];
    all.extend(objects());
    for (i, e) in all.iter().enumerate() {
        let old = old_text(e);
        let new = new_text(e);
        assert_eq!(new, old, "object {i}: written text");
        if i > 0 {
            for k in all_keys() {
                assert!(new.lines().any(|l| l.split_once(' ').unwrap().0 == k), "object {i}: {k} not written");
            }
        }
        let (a, ka) = read(&new, false);
        let (b, kb) = read(&new, true);
        assert_eq!(a, *e, "object {i}: new parser");
        assert_eq!(b, *e, "object {i}: old parser");
        assert!(ka.iter().all(|&k| k) && kb.iter().all(|&k| k));
        assert_eq!(e.deviations(), e.old_deviations(), "object {i}: deviations");
    }
    // The three objects differ from one another.
    let o = objects();
    assert!(o[0] != o[1] && o[1] != o[2] && o[0] != o[2]);
}

/// (a) Every key with every probe value, from the default and from each
/// object: the same value, the same "known" answer, the same deviations.
#[test]
fn out_of_range_and_odd_values_parse_as_before() {
    let mut starts = vec![Enhancements::default()];
    starts.extend(objects());
    for start in &starts {
        for key in all_keys() {
            for v in PROBES.iter().chain(PROBES_ODD.iter()) {
                let mut a = *start;
                let mut b = *start;
                let ka = a.parse(key, v);
                let kb = b.old_parse(key, v);
                assert_eq!((a, ka), (b, kb), "{key} {v:?}");
                assert_eq!(new_text(&a), old_text(&a), "{key} {v:?}");
                assert_eq!(a.deviations(), a.old_deviations(), "{key} {v:?}");
            }
        }
    }
}

/// (a) Unknown keys, comments, duplicates and old key spellings.
#[test]
fn unknown_comment_duplicate_and_legacy_lines_as_before() {
    let text = "enh.fov 5\n\
                # enh.fov 10\n\
                // enh.hud_scale 70\n\
                enh.nope 1\n\
                enh. 1\n\
                enh\n\
                enh.raised.3 2\n\
                detail 3\n\
                ENH.FOV 10\n\
                enh.fov  10\n\
                enh.sharpen 25 # a note\n\
                enh.fov 10\n\
                enh.fov 15\n\
                enh.water on\n\
                enh.headlight_beams yes\n\
                enh.vector_fonts 1\n\
                enh.hud_palette 2\n\
                enh.hud_palette 9\n\
                enh.pad_rumble 70\n\
                \n\
                enh.ssr 3\n\
                enh.ssr x\n";
    let (mut a, ka) = read(text, false);
    let (b, kb) = read(text, true);
    assert_eq!(a, b);
    assert_eq!(ka, kb);
    assert_eq!(a.fov_offset, 15, "the last of duplicate lines wins");
    assert_eq!((a.water, a.headlight_beams), (1, 1), "the per-key legacy rules");
    assert!(!a.parse("enh.nope", "1") && a.parse("enh.vector_fonts", "1"));
    // Every legacy key: known, changes nothing, never written.
    for k in LEGACY_KEYS {
        let mut e = Enhancements::default();
        assert!(e.parse(k, "1") && e.is_native());
        for o in objects() {
            assert!(!new_text(&o).contains(k));
        }
    }
}

/// (b) is `enhancements_round_trip`'s 79 (src/qol/mod.rs), unchanged.
/// (c) `apply_now` stores the same values in the same globals in the same
/// order.
#[test]
fn apply_now_stores_as_before() {
    let _g = test_globals();
    let mut all = vec![Enhancements::default()];
    all.extend(objects());
    // Single rows on (an engine rule each: FSR 1 with SSAA, Modern
    // Lighting with Dynamic Lamps, Wet Road Interaction).
    for (k, v) in [("enh.upscale", "5"), ("enh.modern_lighting", "2"), ("enh.wet_interaction", "1")] {
        for mut e in all.clone() {
            e.parse(k, v);
            e.parse("enh.post_aa", "6");
            e.parse("enh.dynamic_lamps", "1");
            all.push(e);
        }
    }
    let log = |f: &dyn Fn()| {
        APPLY_LOG.with(|l| *l.borrow_mut() = Some(Vec::new()));
        f();
        APPLY_LOG.with(|l| l.borrow_mut().take().unwrap())
    };
    for (i, e) in all.iter().enumerate() {
        let old = log(&|| e.old_apply_now());
        let new = log(&|| e.apply_now());
        assert!(old.len() >= 66, "{}", old.len());
        assert_eq!(new, old, "object {i}");
    }
}

/// The presets' keep-list: the same rows kept.
#[test]
fn preset_keep_list_as_before() {
    let mut all = vec![Enhancements::default()];
    all.extend(objects());
    for e in all {
        let mut old = e;
        old_preset_base(&mut old);
        assert_eq!(e.preset_base(), old);
        for p in PRESETS {
            for c in [GpuClass::High, GpuClass::Mid, GpuClass::Integrated] {
                let mut a = e;
                p.apply_for(c, &mut a);
                assert_eq!(a.preset_base(), old, "{p:?} {c:?}: the kept rows stay");
            }
        }
    }
}

/// Every value the settings page can set (each Graphics row's tiers, each
/// QoL row stepped both ways) is written under a key of the table (or of a
/// nested group) and read back to the same value. Lists the keys no row
/// sets.
#[test]
fn every_ui_value_is_kept_by_the_table() {
    use crate::frontend::enh::graphics_table;
    use crate::frontend::options::{RowId, Settings, change};
    let mut seen = std::collections::BTreeSet::new();
    let mut check = |s: &Settings, what: &str| {
        let text = new_text(&s.enh);
        let (back, known) = read(&text, false);
        assert_eq!(back, s.enh, "{what}: {text}");
        assert!(known.iter().all(|&k| k), "{what}");
        for l in text.lines() {
            seen.insert(l.split_once(' ').unwrap().0.to_string());
        }
    };
    for r in graphics_table::ROWS.iter() {
        for i in 0..r.tiers.len() {
            let mut s = Settings::default();
            (r.set)(&mut s, i);
            check(&s, &format!("{:?} tier {i}", r.id));
        }
    }
    let qol = [
        RowId::FovOffset,
        RowId::HudScale,
        RowId::HudLayout,
        RowId::FrameCap,
        RowId::DurableSaves,
        RowId::SaveBackups,
        RowId::CrashReports,
        RowId::ListsOpenInstalled,
        RowId::OutrunVariety,
        RowId::FixModels,
        RowId::TrafficDensity,
        RowId::SkipMovies,
        RowId::SkipTitle,
        RowId::PhotoMode,
        RowId::LowLatency,
        RowId::MarkSpot,
        RowId::PowerSaving,
        RowId::ReplayCameras,
        RowId::HudPalette,
        RowId::HudMinimap,
        RowId::HudSpeedo,
        RowId::HudInfo,
        RowId::ReducedFlashing,
        RowId::PadLayout,
        RowId::PadDeadzone,
        RowId::PadTriggerDeadzone,
        RowId::PadSaturation,
        RowId::PadSteerCurve,
        RowId::PadRumble,
        RowId::PadPauseOnDisconnect,
        RowId::PadSafeBind,
        RowId::HdUi,
        RowId::ReloadPacks,
    ];
    for id in qol {
        for right in [true, false] {
            let mut s = Settings::default();
            for n in 0..24 {
                change(id, &mut s, 0, right, false);
                check(&s, &format!("{id:?} step {n} {right}"));
            }
        }
    }
    let unset: Vec<_> = all_keys().into_iter().filter(|k| !seen.contains(*k)).collect();
    // enh.anisotropy and enh.reflection_rate are removed clone rows: their
    // Customize Display rows (Texture Filtering, Car Reflection Update
    // Rate) read them and put them to 0 on any change (graphics_table);
    // boot_loading_min_ms has no row (options.txt only); launcher.skip is
    // the launcher's (L1).
    assert_eq!(
        unset,
        ["enh.anisotropy", "enh.reflection_rate", "enh.boot_loading_min_ms", "launcher.skip"],
        "keys no settings-page row sets"
    );
}

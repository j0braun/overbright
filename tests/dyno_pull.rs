//! The Dyno Run's pull against the original (docs/formats.md, "Dyno
//! chart"): the finished-career save's Lancer (tests/fixtures/native_saves/career_complete.sav,
//! Circuit settings, all sliders at 100) on the dyno, measured under Wine
//! on 2026-11-01 by reading car +0x434 (gear), +0x440 (rpm) and +0x330
//! (wheel spin) every ~13 ms of game time (g_SimTicks 0x865188).
//!
//! The pull's third gear depends on the force pass's dyno boost (0x5B0530:
//! the held car's engine power +0x80 = 1.5, 0x7A1A28, while the manager is
//! in state 2); without it the clone's third gear took 8.97 s instead of
//! 5.12 s. Ignored without NFSU2_ROOT.
mod common;

use nfsu2::car::Car;
use nfsu2::dyno::DynoPull;

/// the finished-career save's Lancer as the dyno builds it: its fitted performance parts and
/// unique parts (`Game::with_uniques`), Circuit's settings.
const PERF: [u8; 39] = [0, 1, 2, 3, 4, 5, 6, 7, 10, 11, 14, 16, 19, 20, 21, 22, 23, 24, 25, 28, 30, 35, 36, 37, 39, 42, 43, 44, 45, 47, 48, 49, 50, 51, 52, 55, 58, 60, 63];

/// The original's time in each forward gear of the pull, s: first (gear
/// 2) 84.912 → 86.340, second 86.340 → 87.170, third 87.170 → 92.289
/// (the shift down after the pull).
const ORIGINAL_GEAR_TIMES: [f32; 3] = [1.428, 0.830, 5.119];

/// The original's rpm and wheel spin (rad/s) at these times into third gear.
const ORIGINAL_THIRD: [(f32, f32, f32); 6] = [
    (0.5, 2742.0, 51.33),
    (1.0, 3437.0, 64.29),
    (2.0, 5014.5, 93.71),
    (3.0, 6569.5, 122.49),
    (4.0, 7832.6, 146.02),
    (5.0, 8871.1, 165.27),
];

fn lancer() -> Car {
    let root = common::game_root();
    let career = nfsu2::formats::jdlz::decompress(&std::fs::read(root.join("GLOBAL/GlobalB.lzc")).expect("reading GLOBAL/GlobalB.lzc")).expect("decompressing GlobalB.lzc");
    let group = nfsu2::formats::bundle::chunks(&career, 0).find(|c| c.id == 0x8003_4A10).expect("career chunk 0x34A10");
    let packages = nfsu2::performance::parse(group.child(0x34A15).expect("performance packages (0x34A15)").body);
    let table = nfsu2::tuning::parse(group.child(0x34A1A).expect("tuning table (0x34A1A)").body);
    let settings = nfsu2::tuning::Settings::default();
    Car::load_physics(&root, "LANCEREVO8", &packages, &|id| PERF.contains(&id), Some((&table, &settings))).expect("loading the LANCEREVO8")
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn dyno_pull_matches_the_original() {
    let car = lancer();
    let mut pull = DynoPull::new(car);
    let frame = nfsu2::dyno::FRAME;
    // (time, gear, rpm, spin) each frame until the pull ends.
    let mut trace = Vec::new();
    let mut t = 0.0;
    while t < 12.0 && pull.stage() != 3 {
        pull.step(frame);
        t += frame;
        trace.push((t, pull.gear(), pull.rpm(), pull.car_state().wheels[0].spin));
    }
    // Gear 3 is left by the shift down at the pull's end (state 3).
    for _ in 0..30 {
        pull.step(frame);
        t += frame;
        trace.push((t, pull.gear(), pull.rpm(), pull.car_state().wheels[0].spin));
    }
    let entered = |gear: usize| trace.iter().find(|r| r.1 == gear).map(|r| r.0).unwrap();
    let left_third = trace.iter().skip_while(|r| r.1 != 4).find(|r| r.1 != 4).map(|r| r.0).unwrap();
    let times = [entered(3) - entered(2), entered(4) - entered(3), left_third - entered(4)];
    eprintln!("dyno pull gear times: clone {times:.3?}, original {ORIGINAL_GEAR_TIMES:?}");
    // First and second gear: within 0.06 s each — the original was read
    // every ~13 ms and ran ~120 frames a second under Wine, while the clone
    // steps the manager at 1/60 s, which asks for each shift two frames
    // after it raises the target (0x56E6F0) — and within 3% together.
    // Third gear (the pull) within 3%.
    for (i, (clone, original)) in times.iter().zip(ORIGINAL_GEAR_TIMES).enumerate() {
        let ok = if i == 2 { (clone - original).abs() / original < 0.03 } else { (clone - original).abs() < 0.06 };
        assert!(ok, "gear {} of the pull: clone {clone:.3} s, original {original:.3} s", i + 1);
    }
    let (low, original_low) = (times[0] + times[1], ORIGINAL_GEAR_TIMES[0] + ORIGINAL_GEAR_TIMES[1]);
    assert!((low - original_low).abs() / original_low < 0.03, "first and second gear: clone {low:.3} s, original {original_low:.3} s");
    let third = entered(4);
    for (dt, rpm, spin) in ORIGINAL_THIRD {
        let r = trace.iter().min_by(|a, b| (a.0 - third - dt).abs().total_cmp(&(b.0 - third - dt).abs())).unwrap();
        assert!((r.2 - rpm).abs() / rpm < 0.03, "{dt} s into third: rpm {:.0}, original {rpm}", r.2);
        assert!((r.3 - spin).abs() / spin < 0.03, "{dt} s into third: spin {:.2}, original {spin}", r.3);
    }
}

//! The difficulty against the shipped data: what Easy, Medium and Hard
//! change (see `nfsu2::difficulty` and docs/formats.md "Difficulty").

mod common;

use nfsu2::ai::route::AiTuning;
use nfsu2::difficulty::RaceGlobals;
use nfsu2::frontend::career::{CareerData, EventKind};

fn root() -> std::path::PathBuf {
    common::game_root()
}

/// Career events: each opponent's skill row rises with the difficulty
/// (+0x08 … +0x0A, read at +0x08 + 0x89E7F4), its Block row (+0x0B →
/// racer +0x902) is 1, and the throttle level (+0x7D → 0x89E804) is 0 or
/// 1 in drag events and 3 in every other.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn career_events_by_difficulty() {
    let root = root();
    let data = CareerData::load(&root).unwrap();
    let mut rows = [0usize; 3];
    let mut n = 0;
    for e in &data.events {
        if e.kind == EventKind::Drag {
            assert!(matches!(e.throttle, 0 | 1), "{} {}", e.name, e.throttle);
        } else {
            assert_eq!(e.throttle, 3, "{}", e.name);
        }
        for o in &e.opponents {
            assert_eq!(o.block, 1, "{}", e.name);
            assert!(o.difficulty[0] < o.difficulty[1] && o.difficulty[1] < o.difficulty[2], "{} {:?}", e.name, o.difficulty);
            for d in 0..3 {
                rows[d] += usize::from(o.difficulty[d]);
            }
            n += 1;
        }
    }
    assert!(n > 600);
    // The mean row per difficulty (679 entries): about 1.1, 5.0 and 8.0.
    let mean = rows.map(|r| r as f32 / n as f32);
    assert!(mean[0] < 1.5 && (4.5..5.5).contains(&mean[1]) && mean[2] > 7.5, "{mean:?}");
}

/// The skill rows the difficulty picks: the route speed factor (+0x538)
/// and, for a Quick Race or free roam's racers, the drawn row (0x4FED00
/// on the ten cumulative floats at +0x9F4 + 0x28·difficulty) climbs with
/// it.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn random_rows_climb_with_the_difficulty() {
    let root = root();
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let t = AiTuning::parse(&globalb).expect("0x34B00");
    let mean = |d: usize| (0..1000).map(|i| t.random_row(d, (i as f32 + 0.5) / 1000.0) as f32).sum::<f32>() / 1000.0;
    let (easy, medium, hard) = (mean(0), mean(1), mean(2));
    assert!(easy < medium && medium < hard, "{easy} {medium} {hard}");
    let speed = |d: usize| (0..1000).map(|i| t.speed_factor(t.random_row(d, (i as f32 + 0.5) / 1000.0))).sum::<f32>() / 1000.0;
    assert!(speed(0) < speed(1) && speed(1) < speed(2));
    // The Block rows the racers use (row 1) and the catch-up scale table,
    // all 1.0 as shipped.
    for d in 0..3 {
        for class in 0..4 {
            assert_eq!(t.catch_up_scale(d, class), 1.0);
        }
    }
}

/// What else the difficulty picks: the AI's drift points rate (0x79DAF0),
/// the downhill drift board, the Quick Race drag shift window.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drift_and_drag_by_difficulty() {
    assert_eq!(nfsu2::drift::AI_DIFFICULTY, [1.0, 1.25, 1.5]);
    let s = [0, 1, 2].map(|d| nfsu2::drift::virtual_scores(4174, 3, d).unwrap()[0]);
    assert!(s[0] < s[1] && s[1] < s[2], "{s:?}");
    assert_eq!([0, 1, 2].map(|d| nfsu2::handling::drag_shift_scale(true, false, d)), [3.0, 1.5, 1.0]);
    assert_eq!([0, 1, 2].map(|d| nfsu2::handling::drag_shift_scale(true, true, d)), [1.0; 3]);
}

/// The globals over a session: a career race takes the career difficulty
/// and its event's level, a Quick Race circuit keeps the level, drag sets
/// it from its difficulty.
#[test]
fn globals_over_a_session() {
    let mut g = RaceGlobals::default();
    assert_eq!(g.throttle_cap(), 1.0);
    g.quick_race(0, false);
    assert_eq!((g.difficulty, g.throttle_cap()), (0, 1.0));
    g.quick_race(0, true);
    assert_eq!(g.throttle_cap(), 0.93);
    g.quick_race(2, false);
    assert_eq!((g.difficulty, g.throttle_cap()), (2, 0.93));
    g.career_race(1, 3);
    assert_eq!((g.difficulty, g.throttle_cap()), (1, 1.0));
    g.city(2);
    assert_eq!((g.difficulty, g.throttle_cap()), (2, 1.0));
}

//! The in-race HUD on the tick. Its update runs once per tick
//! (`Game::step` → `overlay_update`), its quads are a pure read of that
//! state (`overlay_layout`), so a scripted race stepped through the window's
//! cadence at 30 / 60 / 120 / 200 Hz and a jittered schedule has the same
//! HUD state (the rate gate's `hud` column) and the same HUD quads at the
//! same tick, and drawing them twice or not at all changes nothing.
use crate::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// A stage-1 circuit race with a career profile, the gas held from tick 0
/// (keyed on the tick, `input_at`).
fn race_game() -> Game {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("HUD_TICK_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    let e = game
        .game_data
        .career
        .stage_events(&game.game_data.root, 1)
        .iter()
        .find(|e| e.kind == career::EventKind::Circuit)
        .map(|e| e.index)
        .expect("a stage-1 circuit");
    game.start_race(e, false, false).unwrap();
    let tick = game.pacing.clocks.tick;
    game.input_at(tick, InputEvent::press(KeyCode::KeyW));
    game
}

/// The HUD's state digest (the state hash's `hud` column) and its quads.
fn hud_now(game: &Game) -> (u64, String) {
    let hud = crate::dev::state_hash::components_v2(game)[3];
    (hud, format!("{:?}", game.overlay_layout(game.now())))
}

/// Display frames of 1/36 000 s units: exact rates, or 4–40 ms jitter with
/// a 120 ms stall about every 5 s (as the rate gate's `jitter:<seed>`).
fn schedule(kind: u32) -> impl FnMut() -> f32 {
    let mut x: u32 = 0x9E37_79B9;
    let mut since_stall = 0u32;
    move || {
        let units = match kind {
            0 => {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                since_stall += 1;
                if since_stall > 200 {
                    since_stall = 0;
                    120 * 36
                } else {
                    (4 + x % 37) * 36
                }
            }
            hz => 36_000 / hz,
        };
        units as f32 / 36_000.0
    }
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn hud_state_and_quads_are_the_same_at_every_rate() {
    testkit::game();
    const TICKS: u64 = 1500;
    // Per schedule: per tick reached, the HUD state digest and its quads.
    let mut runs: Vec<(u32, std::collections::BTreeMap<u64, (u64, String)>)> = Vec::new();
    for kind in [60u32, 30, 120, 200, 0] {
        let mut game = race_game();
        let t0 = game.pacing.clocks.tick;
        let mut next = schedule(kind);
        let mut seen = std::collections::BTreeMap::new();
        while game.pacing.clocks.tick - t0 < TICKS {
            game.advance(next()).unwrap();
            let n = game.pacing.clocks.tick - t0;
            let now = hud_now(&game);
            // A frame without a tick draws the last tick's HUD.
            if let Some(before) = seen.get(&n) {
                assert_eq!(before, &now, "schedule {kind}: tick {n} drawn twice differs");
            }
            seen.insert(n, now);
        }
        runs.push((kind, seen));
    }
    let (_, reference) = &runs[0];
    let racing = reference.values().filter(|v| !v.1.is_empty() && v.1 != "[]").count();
    assert!(racing > 600, "the HUD drew on only {racing} ticks");
    for (kind, seen) in &runs[1..] {
        let mut common = 0;
        for (n, v) in seen {
            if let Some(r) = reference.get(n) {
                common += 1;
                assert_eq!(r.0, v.0, "schedule {kind}: HUD state at tick {n} differs from 60 Hz");
                assert_eq!(r.1, v.1, "schedule {kind}: HUD quads at tick {n} differ from 60 Hz");
            }
        }
        assert!(common > 300, "schedule {kind}: {common} ticks in common");
    }
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drawing_the_hud_changes_nothing() {
    testkit::game();
    let mut game = race_game();
    for _ in 0..900 {
        game.advance(1.0 / 60.0).unwrap();
    }
    let before = crate::dev::state_hash::components_v2(&game);
    let rear = game.hud_state.rear_view_on;
    let a = format!("{:?}", game.overlay_layout(game.now()));
    let b = format!("{:?}", game.overlay_layout(game.now()));
    assert!(a.len() > 100, "the HUD draws in the race");
    assert_eq!(a, b, "the same quads twice");
    assert_eq!(before, crate::dev::state_hash::components_v2(&game), "no state written by the layout");
    assert_eq!(rear, game.hud_state.rear_view_on);
    // A tick then gives the same HUD whether or not frames drew in between.
    let mut other = race_game();
    for _ in 0..900 {
        other.advance(1.0 / 60.0).unwrap();
        let _ = other.overlay_layout(other.now());
        let _ = other.overlay_layout(other.now());
    }
    assert_eq!(hud_now(&game), hud_now(&other));
}

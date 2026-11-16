//! The free-roam zone lookup (0x5EB050, every
//! 8th update) follows the world updates, not the drawn frames. Through
//! the window cadence (`Game::advance`, a drawn frame counted after each
//! interval) at 30 / 60 / 120 / 200 / 240 Hz and under an uneven display
//! (4–40 ms frames, a 120 ms stall), the car put from one zone into another
//! takes the new zone within 8 updates at every rate. Before, the draw
//! count's phase at the updates could skip every lookup at 120/240 Hz.
use super::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn zone_follows_updates_at_display_rates() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("ZONE_RATE_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    // Two road points (shops' GPS points) in different zones.
    let points: Vec<(Vec3, usize)> = game
        .game_data.career
        .shops
        .iter()
        .filter_map(|s| game.sim.route_triggers.find(s.trigger))
        .filter_map(|t| game.sim.route_triggers.zone_at(t.gps.truncate()).map(|z| (t.gps, z)))
        .collect();
    let a = points.first().copied().expect("a shop in a zone");
    let b = points.iter().copied().find(|p| p.1 != a.1).expect("shops in two zones");
    let put = |game: &mut Game, at: Vec3| {
        let d = game.sim.drive.as_mut().unwrap();
        d.state.position = at + Vec3::Z * 0.5;
        d.state.velocity = Vec3::ZERO;
    };
    let mut report = Vec::new();
    // 0.0: the uneven display.
    let interval = |hz: f32, k: usize| -> f32 {
        if hz > 0.0 {
            return 1.0 / hz;
        }
        if k % 50 == 49 {
            return 0.120;
        }
        let x = (k as u64 + 7).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40;
        0.004 + (x % 3601) as f32 * 1e-5
    };
    for hz in [30.0f32, 60.0, 120.0, 200.0, 240.0, 0.0] {
        // At each phase of the draw count against the updates.
        for shift in 0..4u32 {
            game.pacing.frame_count = game.pacing.frame_count.wrapping_add(shift);
            let (mut first, mut updates) = (None, 0usize);
            for (at, zone) in [a, b, a, b] {
                let mut taken = None;
                for k in 0..(hz.max(60.0) as usize) {
                    put(&mut game, at);
                    game.advance(interval(hz, k)).unwrap();
                    game.frame_drawn();
                    updates += game.pacing.frame_updates;
                    if game.sim.zone.1 == Some(zone) {
                        taken = Some(updates);
                        break;
                    }
                }
                let n = taken.unwrap_or_else(|| panic!("{hz} Hz shift {shift}: zone {zone} never taken"));
                // The first put may find no zone yet (looked up at once).
                if first.is_some() {
                    assert!(n <= 8, "{hz} Hz shift {shift}: {n} updates to take zone {zone}");
                }
                first = Some(n);
                report.push((hz, shift, zone, n));
                updates = 0;
            }
        }
    }
    eprintln!("zone rate: {report:?}");
}

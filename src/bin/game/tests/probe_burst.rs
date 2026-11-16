//! Light Probes' arrival burst (the first frame after a load bakes every
//! probe near the car): drawn and read back with one wait for the GPU for
//! the whole burst, the field is the one a wait after each probe gives.
//! A real free roam (the game files under $NFSU2_ROOT), drawn offscreen.
use super::*;

/// The probes' cubes after a free-roam load's first frame, the burst in
/// its serial form or not.
fn field_after_the_first_frame(serial: bool) -> Vec<Option<[Vec3; 6]>> {
    nfsu2::render::probes::set_burst_serial(serial);
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("PROBE_BURST_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    super::tick_effects::offscreen_window(&mut game, 320, 240);
    game.settings.enh.parse("enh.light_probes", "1");
    game.settings.apply_enhancements();
    game.pacing.clock = Some(0.0);
    game.explore(None).unwrap();
    game.step_for(1.0 / 60.0).unwrap();
    game.draw().unwrap();
    nfsu2::render::probes::published_baked().expect("the probe field after a frame with Light Probes")
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_arrival_burst_bakes_the_same_field_with_one_wait() {
    // The Graphics rows and the burst's form are process-wide.
    super::test_isolation::exclusive();
    let serial = field_after_the_first_frame(true);
    let together = field_after_the_first_frame(false);
    let baked = serial.iter().flatten().count();
    assert!(baked >= 2, "the free-roam start bakes a burst of several probes (baked {baked})");
    assert_eq!(serial.len(), together.len());
    for (i, (a, b)) in serial.iter().zip(&together).enumerate() {
        assert_eq!(a, b, "probe {i}");
    }
}

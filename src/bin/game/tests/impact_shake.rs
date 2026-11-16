//! The impact shake on the drive (docs/native-fixes/impact-shake.tsv):
//! the player's car acceleration after its world update starts it
//! (0x572E60 from 0x60A154), the chase view of the same frame takes it
//! (0x572A60 at 0x453B55) unless the world's time scale is below 1, and
//! the world frame's end steps it (0x5728D0 at 0x5814C2). The input is
//! staged directly: the original's car +0x80 on crash_wall's hit.

use crate::*;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_hard_hit_shakes_the_chase_view_on_the_tick() {
    super::testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("ImpactShake");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let mut rng = game.rng.clone();
    let d = game.sim.drive.as_mut().unwrap();
    let dt = 1.0 / 60.0;
    for _ in 0..30 {
        d.chase_view(dt);
    }
    // Below 250 m/s² weighted nothing starts.
    d.state.world_acceleration = Vec3::new(300.0, 0.0, 0.0);
    d.impact_trigger();
    assert!(!d.impact.shake.running());
    // crash_wall's hit: the original's slot vector (−0.0770, −0.1083, −0.0011).
    d.state.world_acceleration = Vec3::new(398.185, 784.247, 2.02513);
    d.impact_trigger();
    assert!(d.impact.shake.running());
    let offset = d.impact.shake.offset();
    let mut plain = d.chase.clone();
    d.chase_view(dt);
    plain.set_impact(Vec3::ZERO);
    let unshaken = plain.update(&d.state, dt, d.fov_offset);
    assert_eq!(d.impact.applied, offset);
    let moved = d.camera.position - unshaken.position;
    assert!((moved + offset).length() < 0.02, "the eye moves by −offset: {moved} vs {offset}");
    assert!(d.camera.pitch != unshaken.pitch || d.camera.yaw != unshaken.yaw, "the view turns too");
    // Slow motion (time scale < 1): the slot runs on but no view takes it.
    d.impact.time_scale = 0.4;
    d.chase_view(dt);
    assert_eq!(d.impact.applied, Vec3::ZERO);
    d.impact.time_scale = 1.0;
    // The world frame's end steps it; at 60 fps a full-scale hit lasts
    // 0.564 s: 34 frames.
    let mut frames = 0;
    while d.impact.shake.running() {
        d.tick_clock(dt, &mut rng);
        frames += 1;
        assert!(frames < 60);
    }
    assert_eq!(frames, 34);
    d.chase_view(dt);
    assert_eq!(d.impact.applied, Vec3::ZERO);
}

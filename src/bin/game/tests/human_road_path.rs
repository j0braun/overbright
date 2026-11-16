//! The human's road path at 10 m/s and above against
//! the original's recording (the maintainer's private capture).

use crate::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// Native (0x42CAA0 → 0x4221F0 → 0x41E860 / 0x404CD0, **verified**): at
/// car +0x42C ≥ 10 the human path runs from the walk's current entry, each
/// point at the entry's +0x54 with the car's raw offset off the line held
/// as a running clamp in each entry's room. The human walk is driven
/// by the recorded car (+0x60, +0x70, forward, +0x430) and the path's first
/// two points and the count compared with the original's (input +0x10).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn human_road_path_matches_the_recording() {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("HumanPath");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    let e = game.sim.encounters.as_ref().unwrap();
    let d = game.sim.drive.as_ref().unwrap();
    let half = d.car.car_type.handling.dimensions[1] * 0.5;
    let text = &testkit::private_str("docs/evidence/n-mix/hpath-r5-freeroam.tsv");
    let mut road = nfsu2::ai::route::HumanRoad::default();
    let (mut errors, mut rows) = (Vec::new(), 0);
    let mut prev: Option<Vec<f32>> = None;
    for line in text.lines().filter(|l| !l.starts_with('#')).skip(1) {
        let row: Vec<f32> = line.split('\t').map(|x| x.parse().unwrap_or(f32::NAN)).collect();
        // A row holds the car after the frame's motion and the path its
        // controller built before it: from the previous row's car.
        let Some(v) = prev.replace(row.clone()) else { continue };
        let at = Vec3::new(v[1], v[2], v[3]);
        let vel = Vec3::new(v[4], v[5], v[6]);
        let fwd = Vec3::new(v[7], v[8], v[9]);
        let speed = v[10];
        road.update(&e.routes, at, vel, fwd, speed, half);
        if !(v[11] >= nfsu2::ai::route::ROAD_PATH_SPEED) {
            continue;
        }
        rows += 1;
        let p = road.road_path(&e.routes, at, vel, fwd, speed).unwrap_or_default();
        let native = [Vec3::new(row[13], row[14], row[15]), Vec3::new(row[16], row[17], row[18])];
        for (k, n) in native.iter().enumerate() {
            let err = p.get(k).map_or(f32::INFINITY, |q| q.truncate().distance(n.truncate()));
            errors.push((err, row[0] as i32, k, p.len(), row[12] as i32));
        }
    }
    errors.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = errors.len();
    eprintln!(
        "HPATH rows {rows} points {n}: median {:.4} p90 {:.4} max {:?}",
        errors[n / 2].0,
        errors[n * 9 / 10].0,
        errors.last()
    );
    for e in errors.iter().rev().take(8) {
        eprintln!("HPATH worst {e:?}");
    }
    assert!(rows > 100);
    assert!(errors.iter().all(|e| e.0 <= 1e-3 && e.3 as i32 == e.4), "{:?}", errors.last());
}

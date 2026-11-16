//! docs/uncapped-fps.md, "Tick snapshots": the frame's values
//! come from the two tick snapshots and α; the draw writes no simulation
//! state. On a real free roam (the player's game files).
use super::*;
use nfsu2::interpolate::CarPose;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// Free roam in a car moving at 25 m/s, three 60 Hz ticks on.
fn moving_explore() -> Game {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("SNAPSHOT_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    for _ in 0..3 {
        let d = game.sim.drive.as_mut().unwrap();
        d.state.velocity = d.state.orientation * Vec3::X * 25.0;
        game.advance(1.0 / 60.0).unwrap();
    }
    game
}

fn bits(v: Vec3) -> [u32; 3] {
    v.to_array().map(f32::to_bits)
}

/// α = 0 draws S_{n−1} exactly, α = 1 S_n exactly, between stays between;
/// then each cut source draws S_n (the tick's live values).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn frame_values_are_the_snapshots_and_cut_by_rule() {
    testkit::game();
    let mut game = moving_explore();
    let (prev, cur) = (game.interp.prev.clone(), game.interp.cur.clone());
    let live = game.sim.drive.as_ref().unwrap().state.clone();
    let live_camera = game.sim.drive.as_ref().unwrap().camera;
    assert_ne!(prev.poses[0].position, cur.poses[0].position, "the car moves between the ticks");
    assert_eq!(cur.poses[0], CarPose::of(&live), "S_n is the tick's state");
    // α = 0: the older tick's pose and camera, bit for bit.
    let f = game.frame_poses(0.0);
    assert_eq!(bits(f.player().position), bits(prev.poses[0].position));
    assert_eq!(f.player().orientation.to_array().map(f32::to_bits), prev.poses[0].orientation.to_array().map(f32::to_bits));
    let c = f.camera.expect("the camera is blended");
    assert_eq!(bits(c.position), bits(prev.camera.unwrap().position));
    assert_eq!(c.yaw.to_bits(), prev.camera.unwrap().yaw.to_bits());
    assert_eq!(f.clocks.blur.to_bits(), prev.clocks.world_time.to_bits());
    assert_eq!(f.clocks.world.map(f32::to_bits), prev.clocks.world.map(|w| (w as f32).to_bits()));
    // α = 1: the newest, as the tick left it.
    let f = game.frame_poses(1.0);
    assert_eq!(bits(f.player().position), bits(live.position));
    assert!(f.camera.is_none());
    assert_eq!(f.clocks.blur.to_bits(), cur.clocks.world_time.to_bits());
    // Between: every coordinate between the two.
    let f = game.frame_poses(0.5);
    for k in 0..3 {
        let (a, b) = (prev.poses[0].position[k], cur.poses[0].position[k]);
        let m = f.player().position[k];
        assert!(m >= a.min(b) && m <= a.max(b), "axis {k}: {m} not in [{a}, {b}]");
    }
    let drawn_cam = f.camera.unwrap().position;
    assert_ne!(bits(drawn_cam), bits(live_camera.position));
    // Cut sources (each draws the tick's values):
    let player_live = |game: &Game, f: &crate::render::snapshot::FramePoses| {
        bits(f.player().position) == bits(game.sim.drive.as_ref().unwrap().state.position)
    };
    // - the camera mover changed between the ticks: camera and crowd at S_n;
    game.interp.cur.cut = true;
    let f = game.frame_poses(0.5);
    assert!(f.camera.is_none(), "mover cut");
    assert!(!player_live(&game, &f), "a mover cut keeps the cars blended (as before)");
    game.interp.cur.cut = false;
    // - another car at the player's index in S_{n−1};
    let key = game.interp.prev.keys[0];
    game.interp.prev.keys[0] = key ^ 0xFFFF;
    assert!(player_live(&game, &game.frame_poses(0.5)), "another car");
    game.interp.prev.keys[0] = key;
    // - a write after the tick (the harness's frozen car);
    let (held, held_camera) = (live.position, live_camera);
    game.sim.drive.as_mut().unwrap().state.position.z += 0.01;
    assert!(player_live(&game, &game.frame_poses(0.5)), "late write");
    game.sim.drive.as_mut().unwrap().state.position = held;
    // - a camera written after the tick (the dyno's, photo mode's) wins;
    game.sim.drive.as_mut().unwrap().camera.position.z += 0.01;
    assert!(game.frame_poses(0.5).camera.is_none(), "late camera write");
    game.sim.drive.as_mut().unwrap().camera = held_camera;
    assert!(game.frame_poses(0.5).camera.is_some(), "restored");
    // - no S_{n−1} yet (the first tick, a fresh world);
    let kept = std::mem::take(&mut game.interp.prev.poses);
    let f = game.frame_poses(0.5);
    assert!(player_live(&game, &f) && f.camera.is_none(), "no previous tick");
    game.interp.prev.poses = kept;
    // - a teleport inside a tick (a reset to the road, a respawn): over
    //   30 m between the two snapshots;
    game.interp.prev.poses[0].position += Vec3::X * 100.0;
    assert!(player_live(&game, &game.frame_poses(0.5)), "teleport");
    game.interp.prev.poses[0].position -= Vec3::X * 100.0;
    // - a move outside the ticks (the harness, a load, a menu action) is
    //   not a cut: the next tick's S_{n−1} is where the car was put
    //   (`refresh_snapshot`), as the old interpolation's tick-start state.
    let put = game.sim.drive.as_ref().unwrap().state.position + Vec3::X * 100.0;
    game.sim.drive.as_mut().unwrap().state.position = put;
    game.advance(1.0 / 60.0).unwrap();
    assert_eq!(bits(game.interp.prev.poses[0].position), bits(put));
    assert!(!player_live(&game, &game.frame_poses(0.5)), "blended from where it was put");
}

/// The carried particles (the nitrous flame) move with their owner's drawn
/// pose in the frame's copy only.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn carried_particles_follow_the_drawn_car_in_a_copy() {
    testkit::game();
    let mut game = moving_explore();
    let d = game.sim.drive.as_mut().unwrap();
    let at = d.state.position + Vec3::Z;
    let mut p = d.effects.system.particles.first().copied().unwrap_or(nfsu2::particles::Particle {
        description: 0,
        palette: 0,
        key: 0,
        life: 100,
        angle: 0,
        speed: 0,
        carried: 0,
        position: Vec3::ZERO,
        direction: [0; 3],
        frame: 0,
        id: 0,
    });
    p.key = FLAME_KEY;
    // A particle born in the newest tick (an id S_{n−1} has not).
    p.id = u32::MAX;
    p.position = at;
    d.effects.system.particles.push(p);
    let i = d.effects.system.particles.len() - 1;
    let f = game.frame_poses(0.5);
    let live = game.sim.drive.as_ref().unwrap().effects.system.particles.clone();
    let drawn = f.particles(&live);
    let now = glam::Mat4::from_rotation_translation(live_state(&game).orientation, live_state(&game).position);
    let moved = glam::Mat4::from_rotation_translation(f.player().orientation, f.player().position);
    assert_eq!(bits(drawn[i].position), bits((moved * now.inverse()).transform_point3(at)));
    assert_eq!(bits(live[i].position), bits(at), "the drive's particle is not written");
    assert_ne!(bits(drawn[i].position), bits(at));
}

fn live_state(game: &Game) -> &CarState {
    &game.sim.drive.as_ref().unwrap().state
}

/// An offscreen window on the real draw path (as `--bench` makes it; the
/// user_reports tests' helper).
pub(crate) fn offscreen_window(game: &mut Game, width: u32, height: u32) {
    super::test_isolation::exclusive();
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
    let format = wgpu::TextureFormat::Bgra8Unorm;
    let offscreen = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("snapshot test target"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        color_space: Default::default(),
    };
    let depth = create_depth(&gpu, width, height);
    let ui = UiRenderer::new(&gpu, format);
    game.platform.window = Some(WindowState::new(None, None, Some(offscreen), config, gpu, depth, ui));
}

/// A draw between the ticks writes no simulation state (the state hash's
/// sim columns and the car, camera and particles are the same after it),
/// and two draws from the same pair take the same frame values.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_draw_between_ticks_writes_no_simulation_state() {
    testkit::game();
    let mut game = moving_explore();
    offscreen_window(&mut game, 320, 240);
    game.draw().unwrap();
    let sim = |game: &Game| {
        let (a, b) = (crate::dev::state_hash::components(game), crate::dev::state_hash::components_v2(game));
        // player, race, rivals, traffic; gsim, clk, enc, fx (the global
        // column counts drawn frames; hud / fe / vis / envf hold what
        // the draw stepped then: the HUD, the front end, the visuals, the env cube).
        let d = game.sim.drive.as_ref().unwrap();
        (a[1..].to_vec(), [b[0], b[1], b[2], b[5]], CarPose::of(&d.state), bits(d.camera.position), d.effects.system.particles.len())
    };
    let before = sim(&game);
    let values = |game: &Game| {
        let f = game.frame_poses(0.5);
        (bits(f.player().position), f.camera.map(|c| bits(c.position)), f.clocks.blur.to_bits())
    };
    let first = values(&game);
    game.draw_between(0.5).unwrap();
    assert_eq!(sim(&game), before, "the draw wrote simulation state");
    assert_eq!(values(&game), first, "the same pair gives the same frame values");
    game.draw_between(0.5).unwrap();
    assert_eq!(sim(&game), before, "the second draw wrote simulation state");
}

/// The capture's cost per tick in free roam (traffic and encounter cars
/// in the ring), and no allocation once the slots have their size.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn capture_cost_per_tick() {
    testkit::game();
    let mut game = moving_explore();
    for _ in 0..120 {
        game.advance(1.0 / 60.0).unwrap();
    }
    let cars = game.interp.cur.keys.len();
    let caps = |g: &Game| (g.interp.cur.keys.capacity(), g.interp.prev.keys.capacity());
    game.capture_tick(1.0 / 60.0);
    game.capture_tick(1.0 / 60.0);
    let before = caps(&game);
    let n = 2000;
    let t = std::time::Instant::now();
    for _ in 0..n {
        game.capture_tick(1.0 / 60.0);
    }
    let us = t.elapsed().as_secs_f64() * 1e6 / f64::from(n);
    assert_eq!(caps(&game), before, "the slots keep their buffers");
    // The refresh before each tick copies the drawn objects again.
    let t = std::time::Instant::now();
    for _ in 0..n {
        game.refresh_snapshot();
    }
    let refresh = t.elapsed().as_secs_f64() * 1e6 / f64::from(n);
    let (particles, posed) = (game.interp.cur.particles.len(), game.interp.cur.posed.len());
    let pcaps = |g: &Game| (g.interp.cur.particles.capacity(), g.interp.cur.posed.capacity(), g.interp.cur.owners.capacity());
    let held = pcaps(&game);
    for _ in 0..100 {
        game.capture_tick(1.0 / 60.0);
    }
    assert!(pcaps(&game).0 >= held.0.min(particles) && pcaps(&game).1 >= posed);
    eprintln!("capture: {cars} cars, {particles} particles, {posed} moved placements, {us:.2} µs per tick, refresh {refresh:.2} µs");
}

/// A digest of the drawn objects of a frame's values: the
/// particles (id, place, life, frame, size factor), the rain's streaks
/// (seen from a fixed view) and lens drops, the moved placements, photo
/// mode's rig and the dyno's rollers.
fn drawn_objects(game: &Game, f: &crate::render::snapshot::FramePoses) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    let Some(d) = game.sim.drive.as_ref() else { return 0 };
    for p in f.particles(&d.effects.system.particles) {
        (p.id, bits(p.position), p.life, p.frame).hash(&mut h);
    }
    for s in f.particle_scale() {
        s.to_bits().hash(&mut h);
    }
    let view = nfsu2::weather::Frame {
        camera: Vec3::new(0.0, 0.0, 1.2),
        camera_forward: Vec3::X,
        camera_velocity: Vec3::ZERO,
        car: Vec3::ZERO,
        forward: Vec3::X,
        clock: 0.0,
    };
    let (streaks, lens): (Vec<_>, Vec<_>) = match f.rain() {
        Some(r) => (r.streaks(&view), r.lens_drops().collect()),
        None => (d.weather.streaks(&view), d.weather.lens_drops().collect()),
    };
    for s in &streaks {
        s.corners.map(bits).hash(&mut h);
    }
    for l in &lens {
        [l.x, l.y, l.size, l.alpha].map(f32::to_bits).hash(&mut h);
    }
    let mut posed: Vec<_> = f.posed(&d.posed).iter().map(|(&p, t)| (p, t.to_cols_array().map(f32::to_bits))).collect();
    posed.sort_unstable();
    posed.hash(&mut h);
    f.photo_rig.map(|r| [r.yaw, r.pitch, r.distance, r.roll, r.fov].map(f32::to_bits)).hash(&mut h);
    f.rollers.map(|r| r.map(f32::to_bits)).hash(&mut h);
    h.finish()
}

/// The world kept raining and the car moving at 25 m/s before each tick
/// (the same writes at the same tick indices whatever the display rate).
fn keep_moving_in_rain(game: &mut Game) {
    let d = game.sim.drive.as_mut().unwrap();
    d.state.velocity = d.state.orientation * Vec3::X * 25.0;
    d.weather.amount = d.weather.amount.max(0.9);
}

/// For every drawn kind, α = 0 draws S_{n−1} (the particles as
/// S_{n−1} held them, the rain's streaks and lens drops, the moved
/// placements), α = 1 the tick's live state, α = 0.5 values within the
/// two (a particle in both ticks halfway, a newborn half size).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drawn_objects_are_the_ticks_at_the_ends_and_between_inside() {
    testkit::game();
    let mut game = moving_explore();
    let (mut seen_particles, mut seen_rain, mut seen_posed) = (0, 0, 0);
    let view = nfsu2::weather::Frame {
        camera: Vec3::new(0.0, 0.0, 1.2),
        camera_forward: Vec3::X,
        camera_velocity: Vec3::ZERO,
        car: Vec3::ZERO,
        forward: Vec3::X,
        clock: 0.0,
    };
    let streak_bits = |s: Vec<nfsu2::weather::Streak>| s.iter().map(|q| q.corners.map(bits)).collect::<Vec<_>>();
    for _ in 0..240 {
        keep_moving_in_rain(&mut game);
        game.advance(1.0 / 60.0).unwrap();
        let (prev, d) = (&game.interp.prev, game.sim.drive.as_ref().unwrap());
        let live = &d.effects.system.particles;
        // α = 0: S_{n−1}.
        let f = game.frame_poses(0.0);
        let drawn = f.particles(live);
        assert_eq!(drawn.len(), prev.particles.len());
        for (a, b) in drawn.iter().zip(&prev.particles) {
            assert_eq!((a.id, bits(a.position), a.life, a.frame), (b.id, bits(b.position), b.life, b.frame));
        }
        let rain = f.rain().expect("the frame's rain");
        assert_eq!(streak_bits(rain.streaks(&view)), streak_bits(prev.rain.streaks(&view)));
        assert_eq!(rain.lens_drops().collect::<Vec<_>>(), prev.rain.lens_drops().collect::<Vec<_>>());
        for (p, t) in f.posed(&d.posed) {
            if let Ok(i) = prev.posed.binary_search_by_key(p, |e| e.0) {
                assert_eq!(t.to_cols_array().map(f32::to_bits), prev.posed[i].1.to_cols_array().map(f32::to_bits));
            } else {
                let rest = d.world.placements[*p].transform;
                assert_eq!(t.to_cols_array().map(f32::to_bits), rest.to_cols_array().map(f32::to_bits));
            }
        }
        // α = 1: the tick's.
        let f = game.frame_poses(1.0);
        assert!(std::ptr::eq(f.particles(live), live.as_slice()) && f.rain().is_none());
        assert!(std::ptr::eq(f.posed(&d.posed), &d.posed));
        // α = 0.5: within.
        let f = game.frame_poses(0.5);
        let drawn = f.particles(live);
        assert_eq!(drawn.len(), live.len());
        for (i, (p, c)) in drawn.iter().zip(live).enumerate() {
            match prev.particles.iter().find(|q| q.id == c.id) {
                Some(a) if a.position.distance(c.position) <= nfsu2::particles::BETWEEN_JUMP && c.key < FLAME_KEY => {
                    seen_particles += 1;
                    for k in 0..3 {
                        let (lo, hi) = (a.position[k].min(c.position[k]), a.position[k].max(c.position[k]));
                        assert!(p.position[k] >= lo && p.position[k] <= hi, "particle {}: {p:?} not in {a:?} .. {c:?}", c.id);
                    }
                }
                Some(_) => {}
                None => assert_eq!(f.particle_scale()[i], 0.5, "a newborn at half size"),
            }
        }
        seen_rain += f.rain().unwrap().streaks(&view).len();
        for (p, t) in f.posed(&d.posed) {
            let a = prev.posed.binary_search_by_key(p, |e| e.0).map_or(d.world.placements[*p].transform, |i| prev.posed[i].1);
            let b = d.posed[p];
            if a != b {
                seen_posed += 1;
                let (ta, tb, tm) = (a.w_axis, b.w_axis, t.w_axis);
                for k in 0..3 {
                    assert!(tm[k] >= ta[k].min(tb[k]) - 1e-4 && tm[k] <= ta[k].max(tb[k]) + 1e-4);
                }
            }
        }
    }
    eprintln!("blended: {seen_particles} particles, {seen_rain} streaks, {seen_posed} placements");
    assert!(seen_rain > 0, "it rained");
}

/// The same scripted free-roam drive in the rain (throttle held),
/// drawn at 30, 60, 120 and 200 Hz: the tick state is the same at every
/// common tick, and at every tick the frame values at α = 0 and 0.5, and at
/// every drawn frame the values at its (tick, α), are the same at every
/// rate where that (tick, α) is drawn.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drawn_objects_are_the_same_at_every_display_rate() {
    testkit::game();
    // Alone among the game's tests: the runs compare tick states across
    // four games, and process-wide state other tests step at the same
    // time (the enhancement effects' globals) must not reach them (one
    // run in the full suite under load saw a tick state differ).
    super::test_isolation::exclusive();
    // Per tick: the tick state's digest (the state hash's sim columns:
    // player, race, rivals, traffic; gsim, clk, enc, fx) and the drawn
    // objects at α = 0 and 0.5; per drawn frame: the drawn objects at its
    // (tick, α).
    type Run = (std::collections::BTreeMap<u64, ([u64; 8], u64, u64)>, std::collections::BTreeMap<(u64, u32), u64>);
    let state = |g: &Game| {
        let (a, b) = (crate::dev::state_hash::components(g), crate::dev::state_hash::components_v2(g));
        [a[1], a[2], a[3], a[4], b[0], b[1], b[2], b[5]]
    };
    let run = |hz: f32| -> Run {
        let mut game = moving_explore();
        // Nothing is written between ticks (a frame between two ticks
        // would draw the write: the live state is its S_n): the rain set
        // once before the first frame (it eases off over the run), the
        // throttle held through the input queue (at a tick index).
        game.sim.drive.as_mut().unwrap().weather.amount = 1.0;
        game.hold_next(KeyCode::ArrowUp, true);
        let mut frames = crate::dev::ticks::Frames::new(hz);
        let (mut at_ticks, mut at_frames) = (std::collections::BTreeMap::new(), std::collections::BTreeMap::new());
        for _ in 0..300 {
            frames
                .tick_with(&mut game, &mut |g, d| {
                    let n = g.interp.cur.tick;
                    at_frames.insert((n, d.alpha.to_bits()), drawn_objects(g, &g.frame_poses(d.alpha)));
                    at_ticks.insert(n, (state(g), drawn_objects(g, &g.frame_poses(0.0)), drawn_objects(g, &g.frame_poses(0.5))));
                    Ok(())
                })
                .unwrap();
        }
        (at_ticks, at_frames)
    };
    let (ref_ticks, ref_frames) = run(60.0);
    let all: Vec<(f32, Run)> = [30.0, 120.0, 200.0].into_iter().map(|hz| (hz, run(hz))).collect();
    for (hz, (ticks, frames)) in &all {
        let mut common = 0;
        for (n, v) in ticks {
            if let Some(r) = ref_ticks.get(n) {
                // The drawn values are a function of the two ticks' state:
                // compared where the tick state is the 60 Hz run's (a tick
                // state that differs is the simulation's rate dependence,
                // which the rate gate's columns report, not the draw's).
                assert_eq!(v.0, r.0, "{hz} Hz, tick {n}: the TICK STATE differs from 60 Hz (not a drawn value; player race rivals traffic gsim clk enc fx)");
                assert_eq!((v.1, v.2), (r.1, r.2), "{hz} Hz, tick {n}: the drawn objects at α = 0 / 0.5 differ from 60 Hz");
                common += 1;
            }
        }
        assert!(common > 100, "{hz} Hz: {common} ticks in common");
        for (k, v) in frames {
            for (_, (_, other)) in all.iter().filter(|(h, _)| h != hz) {
                if let Some(o) = other.get(k) {
                    assert_eq!(v, o, "{hz} Hz, (tick, α) {k:?}");
                }
            }
            if let Some(r) = ref_frames.get(k) {
                assert_eq!(v, r, "{hz} Hz, (tick, α) {k:?} vs 60 Hz");
            }
        }
    }
}

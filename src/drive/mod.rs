//! Driving: a rigid body on four suspended wheels, following the game's
//! PhysicsMover (see docs/formats.md, "Handling"). Per step:
//!
//! 1. steering input → front wheel angles (PlayerSteering 0x416BB0, two-wheel
//!    Ackermann 0x59DF00);
//! 2. suspension compression and force per wheel (0x5A87A0), wheel load
//!    (0x59D170);
//! 3. brakes, drivetrain torque and tyre forces (0x5A8290), five passes of
//!    the rolling constraint with the engine coupled in (0x5ABE00,
//!    0x594DB0), the friction circle (0x59D610);
//! 4. aerodynamics (0x59E630), gravity, integration.
//!
//! After each frame the car's convex hull meets the track's polygons
//! ([`crate::contact`], the game's world collision step 0x5FFA60).

pub mod nis;

use glam::{Mat3, Mat4, Quat, Vec3};

use crate::car::{Car, WheelPose};
use crate::contact;
use crate::handling::{Drivetrain, Handling, NEUTRAL};
use crate::narrow;
use crate::world::World;

mod collide;
mod force;
mod input;
mod integrate;
mod rest;
pub(crate) mod rigid;
mod simulate;
mod state;
mod tables;

pub use collide::*;
pub use force::*;
pub use input::*;
pub use state::*;
pub use tables::*;

#[cfg(test)]
mod tests {
    #[test]
    fn traffic_on_the_belt_meets_no_track() {
        use super::{AiControls, Input, takes_track_contacts};
        let c = AiControls::default();
        assert!(!takes_track_contacts(true, &Input::Belt(c)));
        assert!(takes_track_contacts(false, &Input::Belt(c)), "a racer's belt keeps its contacts");
        assert!(takes_track_contacts(true, &Input::Ai(c)));
        assert!(takes_track_contacts(true, &Input::Melo(c)));
    }

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn world_acceleration_matches_160_original_pc24_cases() {
        let raw = &crate::testkit::private("docs/evidence/outrun/acceleration-followup/fixtures.bin");
        let mut checked = 0;
        for row in raw.chunks_exact(60) {
            let u = |i: usize| u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap());
            if u(0) != 0x007f {
                continue;
            }
            let f = |i| f32::from_bits(u(i));
            let v = |i| glam::Vec3::new(f(i), f(i + 1), f(i + 2));
            let mut state = super::CarState::new(glam::Vec3::ZERO, 0.0);
            state.world_acceleration = glam::Vec3::new(11.0, 22.0, 33.0);
            state.velocity = v(2);
            let sample = state.begin_world_update(f(1));
            state.velocity = v(5);
            state.finish_world_update(sample);
            assert_eq!(state.world_acceleration.to_array().map(f32::to_bits), [u(11), u(12), u(13)]);
            assert_eq!(state.acceleration_along(v(8)).to_bits(), u(14));
            checked += 1;
        }
        assert_eq!(checked, 160);
    }

    #[test]
    fn world_acceleration_retains_outer_interval_and_contact_impulse() {
        let mut state = super::CarState::new(glam::Vec3::ZERO, 0.0);
        state.velocity = glam::Vec3::new(10.0, 0.0, 0.0);
        let sample = state.begin_world_update(0.05);
        // Two force substeps followed by a contact impulse: only the
        // completed world interval is the published acceleration.
        state.velocity.x += 0.2;
        assert_eq!(state.world_acceleration, glam::Vec3::ZERO);
        state.velocity.x += 0.3;
        state.velocity += glam::Vec3::new(-2.0, 1.0, 3.0);
        state.finish_world_update(sample);
        assert!((state.world_acceleration - glam::Vec3::new(-30.0, 20.0, 60.0)).length() < 0.001);
        let sample = state.begin_world_update(0.125);
        state.velocity += glam::Vec3::new(1.0, -2.0, 0.5);
        state.finish_world_update(sample);
        assert_eq!(state.world_acceleration, glam::Vec3::new(8.0, -16.0, 4.0));
        let kept = state.world_acceleration;
        for dt in [0.0, -0.0, -0.1, f32::NAN] {
            let sample = state.begin_world_update(dt);
            state.velocity += glam::Vec3::splat(100.0);
            state.finish_world_update(sample);
            assert_eq!(state.world_acceleration, kept);
        }
        // A controller's placement before the native snapshot must not
        // appear as physical acceleration in the next completed frame.
        state.velocity = glam::Vec3::splat(5.0);
        let sample = state.begin_world_update(0.05);
        state.finish_world_update(sample);
        assert_eq!(state.world_acceleration, glam::Vec3::ZERO);
        state.world_acceleration = glam::Vec3::new(2.0, 0.0, 8.0);
        state.orientation = glam::Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
        assert!((state.forward_acceleration() - 8.0).abs() < 0.001);
    }

    use super::*;
    use crate::race::RaceMode;

    #[test]
    #[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
    fn native_ground_boundary_replay() {
        // Original L4RA assets (ignored without NFSU2_ROOT).
        let root = crate::testkit::game_root();
        let world = World::load(&root, "L4RA").unwrap();
        // Native entry +4 is not the asset-table ordinal. Resolve the
        // captured names/hashes in the production table instead.
        let barriers: Vec<u8> = ["PLAYER_BARRIERS_4014", "BARRIERS_4014"]
            .iter()
            .map(|name| {
                world
                    .barrier_groups
                    .iter()
                    .position(|group| group.name == *name && group.hash == crate::formats::hash::bin_hash(name))
                    .expect("captured barrier missing") as u8
            })
            .collect();
        let rows: Vec<Vec<f32>> =
            crate::testkit::private_str("docs/evidence/full-phase-inputs/ground.csv")
                .lines()
                .skip(1)
                .map(|line| line.split(',').map(|x| x.parse().unwrap()).collect())
                .collect();
        assert_eq!(rows.len(), 559 * 4);
        let mut state = CarState::new(Vec3::ZERO, 0.0);
        for group in rows.chunks_exact(4) {
            let first = &group[0];
            let anchor = glam::Vec2::new(first[4], first[5]);
            let recent: Vec<usize> = first[8..8 + first[7] as usize].iter().map(|&x| x as usize).collect();
            state.ground = world.ground.native_history(&world.visibility, anchor, first[6], &recent, barriers.clone());
            for (i, r) in group.iter().enumerate() {
                assert_eq!(r[1] as usize, i);
                let rest = Vec3::new(r[12], r[13], r[14]);
                assert!(rest.truncate().distance(anchor) <= 9.0 + first[6], "unsupported section boundary");
                let up = Vec3::new(r[15], r[16], r[17]);
                let (compression, normal, surface) = state.wheel_compression(&world, rest, up, r[18]);
                let actual = compression.clamp(r[19], r[20]);
                assert_eq!(
                    actual.to_bits(),
                    r[21].to_bits(),
                    "step {} wheel {i}: compression {actual} != {}",
                    r[0],
                    r[21]
                );
                let expected = Vec3::new(r[22], r[23], r[24]);
                assert!(
                    (normal.expect("native ground contact") - expected).length() <= 1e-5,
                    "step {} wheel {i}: normal",
                    r[0]
                );
                assert_eq!(surface, r[25] as u8, "step {} wheel {i}: surface", r[0]);
            }
            let expected: Vec<usize> = first[27..27 + first[26] as usize].iter().map(|&x| x as usize).collect();
            assert_eq!(world.ground.native_recent(&state.ground), expected, "step {}: recent history", first[0]);
        }
    }

    #[test]
    #[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
    fn native_next_force_uses_final_suspension_loads() {
        let h = crate::handling::parse_handling(&crate::testkit::handling_peugot());
        let rows: Vec<Vec<f32>> =
            crate::testkit::private_str("docs/evidence/suspension-load-phase/loads.csv")
                .lines()
                .skip(1)
                .map(|line| line.split(',').map(|cell| cell.parse().unwrap()).collect())
                .collect();
        let mut counts = [0usize; 3];
        for group in rows.chunks_exact(4) {
            // Only the suspension portion of CarState is projected. The
            // existing rest classification is not replayed or replaced.
            let mut state = CarState::new(Vec3::ZERO, 0.0);
            state.rest = group[0][2] as u8;
            let bump_visited = group[0][3] != 0.0;
            let zero_post = state.rest != 0 || bump_visited;
            assert_eq!(zero_post, group[0][4] != 0.0);
            counts[if state.rest != 0 {
                0
            } else if bump_visited {
                1
            } else {
                2
            }] += 1;
            let compressions = std::array::from_fn(|i| group[i][6]);
            for (i, row) in group.iter().enumerate() {
                assert_eq!(row[1] as usize, i);
                state.wheels[i].compression = row[5];
                state.wheels[i].compression_rate = row[7];
                state.wheels[i].on_ground = row[9] != 0.0;
            }
            for (i, row) in group.iter().enumerate() {
                let other = state.suspension_neighbour_compression(i, &compressions, bump_visited);
                state.wheels[i].compression = compressions[i];
                let actual = if state.wheels[i].on_ground {
                    state.wheels[i].suspension_load(&h.suspension[i / 2], other, row[8])
                } else {
                    0.0
                };
                // 5B0590..5B05A9 reads this final raw suspension load,
                // then 59D170 supplies wheel+164 to the tyre force pass.
                assert!(
                    (actual - row[11]).abs() < 0.00001,
                    "next-force step {} wheel {i}, rest={} bump={bump_visited}: {actual} != {}",
                    row[0],
                    state.rest,
                    row[11]
                );
            }
        }
        assert_eq!(rows.len(), 2200);
        assert_eq!(counts, [112, 15, 423]);
    }

    #[test]
    #[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
    fn native_zero_refresh_recomputes_suspension_loads() {
        let h = crate::handling::parse_handling(&crate::testkit::handling_peugot());
        let rows: Vec<Vec<f32>> =
            crate::testkit::private_str("docs/evidence/boundary-replay/loads.csv")
                .lines()
                .skip(1)
                .map(|line| line.split(',').map(|cell| cell.parse().unwrap()).collect())
                .collect();
        let mut changed_left = 0;
        for group in rows.chunks_exact(4) {
            // A projection of the native suspension boundary, not a whole
            // CarState/substep restore: this component reads only these
            // wheel fields, the captured Handling and captured preload.
            let mut state = CarState::new(Vec3::ZERO, 0.0);
            for (i, row) in group.iter().enumerate() {
                assert_eq!(row[1] as usize, i);
                state.wheels[i].compression = row[6];
                state.wheels[i].compression_rate = row[8];
            }
            for (i, row) in group.iter().enumerate() {
                state.wheels[i].compression = row[7];
                let actual =
                    state.wheels[i].suspension_load(&h.suspension[i / 2], state.wheels[i ^ 1].compression, row[9]);
                assert!(
                    (actual - row[10]).abs() < 0.00001,
                    "normal step {} wheel {i}: {actual} != {}",
                    row[0],
                    row[10]
                );
            }
            // dt=0 preserves compression/rate but repeats the load calculation
            // with both neighbours now current. Production still lacks this
            // second traversal; this test diagnoses its required inputs/output.
            for (i, row) in group.iter().enumerate() {
                let actual =
                    state.wheels[i].suspension_load(&h.suspension[i / 2], state.wheels[i ^ 1].compression, row[9]);
                assert!((actual - row[11]).abs() < 0.00001, "zero step {} wheel {i}: {actual} != {}", row[0], row[11]);
                if (row[10] - row[11]).abs() > 0.00001 {
                    assert_eq!(i % 2, 0);
                    changed_left += 1;
                }
            }
        }
        assert_eq!(rows.len(), 60);
        assert!(changed_left > 0);
    }

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn bump_stop_loop_matches_native_hard_landing() {
        // Generated from actual bump-pre/post raw snapshots by
        // replay_bumps.py --fixture; expected momentum is native, not simulated.
        let fixture =
            &crate::testkit::private_str("docs/evidence/positive-dt-native/bump-groups.csv");
        let rows: Vec<Vec<f32>> =
            fixture.lines().skip(1).map(|line| line.split(',').map(|s| s.parse().unwrap()).collect()).collect();
        let mut groups = 0;
        let mut start = 0;
        while start < rows.len() {
            let first = &rows[start];
            let end = (start + 1..rows.len()).find(|&i| rows[i][0] != first[0]).unwrap_or(rows.len());
            let vec = |row: &[f32], offset: usize| Vec3::new(row[offset], row[offset + 1], row[offset + 2]);
            let mut contacts = [None; 4];
            for row in &rows[start..end] {
                assert_eq!(vec(row, 4), vec(first, 4));
                assert_eq!(vec(row, 7), vec(first, 7));
                contacts[row[2] as usize] = Some((vec(row, 10), vec(row, 13)));
            }
            let (linear, angular, visited) = bump_stop_impulses(vec(first, 4), vec(first, 7), first[3], contacts);
            let last = &rows[end - 1];
            let expected_linear = vec(last, 19) - vec(first, 16);
            let expected_angular = vec(last, 25) - vec(first, 22);
            assert!(visited);
            assert!(
                (linear - expected_linear).length() < 0.00001,
                "native step {} linear: {linear:?} != {expected_linear:?}",
                first[0]
            );
            assert!(
                (angular - expected_angular).length() < 0.00001,
                "native step {} angular: {angular:?} != {expected_angular:?}",
                first[0]
            );
            groups += 1;
            start = end;
        }
        assert_eq!((groups, rows.len()), (17, 39));
    }

    #[test]
    fn bump_stop_visits_refresh_even_without_impulse_and_caps_each_wheel() {
        let contact = Some((Vec3::X, Vec3::Z));
        let (linear, angular, visited) = bump_stop_impulses(Vec3::Z, Vec3::ZERO, 1.0, [contact, None, None, None]);
        assert!(visited);
        assert_eq!((linear, angular), (Vec3::ZERO, Vec3::ZERO));
        let (linear, angular, visited) =
            bump_stop_impulses(-Vec3::Z * 100000.0, Vec3::ZERO, 1.0, [contact, contact, None, None]);
        assert!(visited);
        assert_eq!(linear, Vec3::Z * 2000.0);
        assert_eq!(angular, -Vec3::Y * 20.0);
        assert!(!bump_stop_impulses(Vec3::ZERO, Vec3::ZERO, 1.0, [None; 4]).2);
    }

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn other_car_acceleration_matches_105_original_pc24_cases() {
        let fixture = &crate::testkit::private("docs/evidence/outrun/other-acceleration-followup/pc24-entry.bin");
        let mut checked = 0;
        for row in fixture.chunks_exact(28) {
            let words: Vec<u32> = row.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect();
            let v = |i| Vec3::new(f32::from_bits(words[i]), f32::from_bits(words[i + 1]), f32::from_bits(words[i + 2]));
            let mut state = CarState::new(Vec3::ZERO, 0.0);
            state.velocity = v(0);
            state.world_acceleration = v(3);
            assert_eq!(state.velocity_acceleration().to_bits(), words[6], "native case {checked}");
            checked += 1;
        }
        assert_eq!(checked, 105);
    }

    /// Captured by executing the original 0x59D360 bytes under Wine.
    /// The fixture uses radius .5, inertia 1, spin 2 and averaged torque 3.
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn wheel_slip_correction_matches_native_submillisecond_boundary() {
        let fixture =
            &crate::testkit::private_str("docs/evidence/reverse-refresh/wheel-spin-native.csv");
        let mut checked = 0;
        for line in fixture.lines().skip(1) {
            let cells: Vec<_> = line.split(',').collect();
            if cells[2] != "0" || cells[3] != "0" {
                continue;
            }
            let dt = f32::from_bits(u32::from_str_radix(&cells[1][2..], 16).unwrap());
            let slip: f32 = cells[4].parse().unwrap();
            let expected: f32 = cells[6].parse().unwrap();
            let actual = gripping_wheel_spin_rate(2.996000051498413, slip, 0.5, dt);
            assert!(
                (actual - expected).abs() < 0.00002,
                "native slip derivative at dt={dt:?}, slip={slip}: {actual} != {expected}"
            );
            checked += 1;
        }
        assert_eq!(checked, 12);
    }

    /// PlayerSteering +0x7C: 1/65536 of a turn, truncated toward zero.
    /// The shown rpm (car +0x440) replayed from the original: a COROLLA
    /// rival's launch (circuit_s1, docs/notes/parity/tuning-physics-parity.md), each
    /// frame's value from the last one, the engine's speed after the update
    /// and the update's ticks; the fall is held to 2.5 / 0.0002 rpm/s, the
    /// rest closes 55 % a frame. Within 0.02 rpm over 50 frames.
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn shown_rpm_matches_native_launch() {
        let rows: Vec<Vec<f32>> = crate::testkit::private_str("docs/evidence/tuning-physics-parity/shown-rpm-corolla.tsv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| l.split('\t').map(|x| x.parse().unwrap()).collect())
        .collect();
        for w in rows.windows(2) {
            let (prev, row) = (&w[0], &w[1]);
            let got = shown_rpm_step(prev[2], row[3] * 9.549304, 0.0002, false, row[1] / 1800.0);
            assert!((got - row[2]).abs() < 0.02, "frame {}: {got} vs native {}", row[0], row[2]);
        }
        // Rising: 55 % of the gap, whatever the dt (and a zero-length
        // re-step still smooths).
        assert_eq!(shown_rpm_step(1000.0, 2000.0, 0.0002, false, 0.0), 1550.0);
        // Shifting up past 1st: the fall may be 6 × faster.
        let slow = shown_rpm_step(7000.0, 3000.0, 0.0002, false, 1.0 / 60.0);
        let fast = shown_rpm_step(7000.0, 3000.0, 0.0002, true, 1.0 / 60.0);
        assert!((7000.0 - fast) > 5.9 * (7000.0 - slow));
    }

    /// 0x4061C0: a wheel's input is clamped to ±Wheel Range and stretched
    /// over it, shaped by 0x7E9E88 (half a range is −0.288), and the angle
    /// is set at once (no 0x4060E0 rate limit): at rest with the pedals up
    /// the lock is 45° × (1 × 0.5 + 1) = 67.5°, capped at 45°.
    #[test]
    fn wheel_steering() {
        let mut h = crate::handling::parse_handling(&vec![0u8; 0x1000]);
        h.steering_ratio = 1.0;
        let wheel = |steer: f32, range: f32| {
            let mut s = CarState::new(Vec3::ZERO, 0.0);
            s.steer(&h, Controls { steer, wheel: Some(range), ..Default::default() }, 1.0 / 60.0);
            (s.steer_degrees, s.last_steer_input)
        };
        assert_eq!(wheel(0.5, 0.5), (-45.0, -1.0));
        assert_eq!(wheel(0.9, 0.5), (-45.0, -1.0));
        assert_eq!(wheel(-0.5, 0.5), (45.0, 1.0));
        let half = quantise_steer(-0.288 * 67.5);
        assert_eq!(wheel(0.25, 0.5).0, half);
        assert_eq!(wheel(0.5, 1.0).0, half);
        assert_eq!(wheel(0.0, 0.3), (0.0, 0.0));
        // The pad path moves at most 180°/s × the speed factor a tick.
        let mut s = CarState::new(Vec3::ZERO, 0.0);
        s.steer(&h, Controls { steer: 1.0, ..Default::default() }, 1.0 / 60.0);
        assert!(s.steer_degrees.abs() < 10.0, "{}", s.steer_degrees);
    }

    #[test]
    fn steer_angle_word() {
        assert_eq!(quantise_steer(0.0), 0.0);
        assert_eq!(quantise_steer(0.005), 0.0);
        assert_eq!(quantise_steer(-0.005), 0.0);
        assert_eq!(quantise_steer(45.0), 45.0);
        // 10° is 1820.44 steps: 1820 kept, toward zero on both sides.
        let ten = 1820.0 * 360.0 / 65536.0;
        assert!((quantise_steer(10.0) - ten).abs() < 1e-4);
        assert!((quantise_steer(-10.0) + ten).abs() < 1e-4);
    }

    /// 0x576AB0 with the steering's 0.55 s window: a key held from rest
    /// at 25 frames a second climbs by 1/14 a frame (as the original's
    /// +0x20 did live), and lets go as the old samples leave.
    #[test]
    fn steer_average_window() {
        let mut a = SteerAverage::default();
        for _ in 0..20 {
            a.add(0.0, 0.04);
        }
        a.add(1.0, 0.04);
        assert!((a.mean - 1.0 / 14.0).abs() < 1e-4, "{}", a.mean);
        for _ in 0..13 {
            a.add(1.0, 0.04);
        }
        assert!((a.mean - 1.0).abs() < 1e-4, "{}", a.mean);
        a.add(-1.0, 0.04);
        assert!((a.mean - 12.0 / 14.0).abs() < 1e-4, "{}", a.mean);
    }

    /// 0x4060E0's +0x48 mean at 20 frames a second: a key press (3.1)
    /// with three steady samples (1.0) in the 0.15 s window (the oldest
    /// exactly 0.15 s old on the whole-tick clock is kept), gone 0.2 s on.
    #[test]
    fn steer_rate_boost() {
        let mut a = RateAverage::default();
        for _ in 0..10 {
            a.add(1.0, 0.05);
        }
        a.add(3.1, 0.05);
        assert!((a.mean - 6.1 / 4.0).abs() < 1e-5, "{}", a.mean);
        for _ in 0..4 {
            a.add(1.0, 0.05);
        }
        assert!((a.mean - 1.0).abs() < 1e-5, "{}", a.mean);
    }

    /// 0x5F62C0: the player's and the traffic's brake lights follow the
    /// pedal (> 0.05); the racers' stay on 0.5 s after it is let go.
    #[test]
    fn brake_light_hold() {
        let mut s = CarState::new(Vec3::ZERO, 0.0);
        assert!(!s.brake_lights(true) && !s.brake_lights(false));
        s.note_brake(0.3, 0.1);
        assert!(s.brake_lights(true) && s.brake_lights(false));
        s.note_brake(0.0, 0.3);
        assert!(s.brake_lights(true) && !s.brake_lights(false));
        s.note_brake(0.04, 0.3);
        assert!(!s.brake_lights(true));
    }

    /// 0x5D3CB0 by hand: corner (i, j) takes table entry
    /// (−5ij − 3(i+1)(j+1)) & 15; the cell's floor; × the row's amplitude.
    #[test]
    fn table_arctangent() {
        // 0x43C570: quadrants, the diagonal, and the dead zone under 1/256.
        assert_eq!(atan16(1.0, 0.0), 0);
        assert_eq!(atan16(0.0, 1.0), 0x4000);
        assert_eq!(atan16(1.0, 1.0), 0x2000);
        assert_eq!(atan16(-1.0, 0.0), 0x8000);
        assert_eq!(atan16(10.0, 0.03), 0);
        assert_eq!(atan16(10.0, 0.04), 32);
        assert_eq!(atan16(10.0, -0.04), 0u16.wrapping_sub(32));
        // Within a step of the float arctangent elsewhere.
        for k in 1..60 {
            let a = k as f32 * 0.05;
            let (x, y) = (a.cos(), a.sin());
            let got = atan16(x, y) as f32 * 9.587_38e-5;
            assert!((got - a).abs() < 0.008, "{a} {got}");
        }
    }

    #[test]
    fn surface_bump_is_the_games_noise() {
        // Cell (0, 0) at its middle: entries 13, 10, 10, 15 = 0, 0.4, 0.4, 0.1.
        assert!((surface_bump(0.5, 0.5, 8) - 0.225 * 0.05).abs() < 1e-7);
        // Cell (−1, 0): entries 0, 13, 5, 10 = −0.5, 0, −0.1, 0.4.
        assert!((surface_bump(-0.5, 0.5, 8) + 0.05 * 0.05).abs() < 1e-7);
        // Surface 11's lattice is three to the metre.
        assert!((surface_bump(0.5 / 3.0, 0.5 / 3.0, 11) - 0.225 * 0.03).abs() < 1e-6);
        // At a lattice point the corner's entry alone.
        assert!((surface_bump(1.0, 0.0, 15) - 0.4 * 0.05).abs() < 1e-7);
        assert_eq!(surface_bump(3.3, -7.1, 0), 0.0);
        assert_eq!(surface_bump(3.3, -7.1, 22), 0.0);
    }

    /// The race start's hold and the drag launch (0x41E37E, 0x410430,
    /// 0x407310, 0x407A30): held, full brake; in a drag race in neutral
    /// the gas climbs at 1/s with a 0.6 floor under idle + 1000 rpm; the
    /// release asks a manual gearbox for first gear and opens the 0.25 s
    /// launch window, as backing out of reverse does.
    #[test]
    /// The hop's start test reads the load term the force pass after
    /// the frame's last integration wrote (0x5B664F, +0x164), `hop_loads`,
    /// not the last sub-step's start loads. The finished-career save's Skyline (AWD, class 0)
    /// at the green of S4_H_DRAG_15 (run n1, the maintainer's tool `subtrap.py`
    /// sub-step capture, the maintainer's private captures): the end-state loads
    /// and the frame's end slips give the original's count 3 and grip
    /// 0.195188; the loads one sub-step older pick another wheel.
    fn hop_reads_the_end_state_load_term() {
        let mut h = crate::handling::parse_handling(&vec![0u8; 0x1000]);
        h.gearbox.rear_split = 0.5;
        h.performance_class = 0;
        h.race = RaceMode::Drag;
        let mut car = CarState::new(Vec3::ZERO, 0.0);
        car.velocity = Vec3::new(0.314658, 0.0, 0.0);
        // The hop reads the exported forward speed (car +0x42C).
        car.tyre_speed = 0.314658;
        // +0x118 at the frame's end (FL, FR, RR, RL as mover +0x38 + 4i).
        for (w, slip) in car.wheels.iter_mut().zip([0.577454, 0.577454, 0.697963, 0.697657]) {
            w.slip = slip;
        }
        // +0x164 after the frame's last integration (subtrap B, frame 4's fourth).
        car.hop_loads = [3.175766, 3.190298, 2.585473, 2.581244];
        car.wheel_hop(&h, 1.0 / 60.0);
        assert_eq!(car.hop, 3);
        assert!((car.hop_grip - 0.195188).abs() < 2e-5, "{}", car.hop_grip);
        // The loads at the last sub-step's start (subtrap A): the front
        // wheel's slip caps the term and wins instead.
        let mut old = CarState::new(Vec3::ZERO, 0.0);
        old.velocity = car.velocity;
        old.tyre_speed = car.tyre_speed;
        for (w, slip) in old.wheels.iter_mut().zip([0.577454, 0.577454, 0.697963, 0.697657]) {
            w.slip = slip;
        }
        old.hop_loads = [3.245182, 3.258793, 2.43779, 2.438696];
        old.wheel_hop(&h, 1.0 / 60.0);
        assert!((old.hop_grip - 0.195188).abs() > 5e-5, "{}", old.hop_grip);
    }

    #[test]
    fn drag_hold_and_launch() {
        let mut r = vec![0u8; 0x1000];
        r[0x300..0x304].copy_from_slice(&800.0f32.to_le_bytes());
        let mut h = crate::handling::parse_handling(&r);
        h.race = RaceMode::Drag;
        let mut car = CarState::new(Vec3::ZERO, 0.0);
        car.start_race(RaceMode::Drag);
        car.held = true;
        assert!(car.manual && car.gear_request.is_none() && car.launch_timer == 0.0);
        let gas = Controls { throttle: 1.0, ..Default::default() };
        let p = car.pedals(&h, gas, crate::handling::NEUTRAL, 900.0, 0.1);
        assert_eq!(p.brake, 1.0);
        // Under idle + 1000 rpm the pedal over 0.6 gets 0.6 at once.
        assert!((p.throttle - 0.6).abs() < 1e-6, "{}", p.throttle);
        // Above it the gas climbs at most 1/s from last frame's.
        let p = car.pedals(&h, gas, crate::handling::NEUTRAL, 3000.0, 0.1);
        assert!((p.throttle - 0.7).abs() < 1e-5, "{}", p.throttle);
        // In gear the pedal passes as it is.
        let p = car.pedals(&h, gas, crate::handling::NEUTRAL + 1, 3000.0, 0.1);
        assert_eq!(p.throttle, 1.0);
        car.release_hold(RaceMode::Drag);
        assert!(!car.held);
        assert_eq!(car.gear_request, Some(crate::handling::NEUTRAL + 1));
        assert_eq!(car.launch_timer, 0.25);
        // The launch window: the gas doubled, at least 0.7.
        let p =
            car.pedals(&h, Controls { throttle: 0.2, ..Default::default() }, crate::handling::NEUTRAL + 1, 3000.0, 0.1);
        assert!((p.throttle - 0.7).abs() < 1e-6, "{}", p.throttle);
        // The automatic gearbox: no request, no window.
        let mut auto = CarState::new(Vec3::ZERO, 0.0);
        auto.start_race(RaceMode::FreeRoam);
        auto.held = true;
        auto.release_hold(RaceMode::FreeRoam);
        assert!(auto.gear_request.is_none() && auto.launch_timer == 0.0);
        // Out of reverse a manual gearbox asks for first gear again.
        car.gear_request = None;
        car.launch_timer = 0.0;
        car.reverse_requested = true;
        car.pedals(&h, Controls::default(), crate::handling::NEUTRAL + 1, 3000.0, 0.1);
        assert_eq!(car.gear_request, Some(crate::handling::NEUTRAL + 1));
        assert_eq!(car.launch_timer, 0.25);
    }

    #[test]
    fn scripted_spin_replaces_stale_momentum_and_roundtrips() {
        let mut h = crate::handling::parse_handling(&vec![0u8; 0x1000]);
        h.inertia = [2.0, 3.0, 4.0];
        h.unknown_270 = 1.7;
        for ground in [false, true] {
            let mut state = CarState::new(Vec3::ZERO, 0.0);
            state.orientation = Quat::from_rotation_z(0.7) * Quat::from_rotation_x(0.3);
            state.ground_inertia = ground;
            state.angular_momentum = Vec3::new(12.0, -8.0, 5.0);
            let spin = Vec3::new(0.2, -0.4, 0.8);
            state.set_scripted_spin(&h, spin);
            state.spin_from_momentum(&h);
            assert!(state.spin.distance(spin) < 1e-6);
            state.set_scripted_spin(&h, Vec3::ZERO);
            assert_eq!(state.angular_momentum, Vec3::ZERO);
            state.spin_from_momentum(&h);
            assert_eq!(state.spin, Vec3::ZERO);
        }
    }

    #[test]
    fn drag_pushes_total_a_car() {
        let mut h = crate::handling::parse_handling(&vec![0u8; 0x1000]);
        let mut car = CarState::new(Vec3::ZERO, 0.0);
        h.race = RaceMode::Drag;
        car.check_totaled(&h, 29.9);
        assert!(!car.totaled);
        car.check_totaled(&h, 30.0);
        assert!(car.totaled);
        // On its roof alone does not (the +0x38 test reads a zero).
        let mut car = CarState::new(Vec3::ZERO, 0.0);
        car.orientation = Quat::from_rotation_x(std::f32::consts::PI);
        car.check_totaled(&h, 1.0);
        assert!(!car.totaled);
        // Only in a drag race.
        h.race = RaceMode::FreeRoam;
        let mut car = CarState::new(Vec3::ZERO, 0.0);
        car.check_totaled(&h, 40.0);
        assert!(!car.totaled);
    }
}

#[cfg(test)]
mod native_hub_replay;

#[cfg(test)]
mod native_camber_replay;

#[cfg(test)]
mod native_cached_count;

#[cfg(test)]
mod native_cached_speed;

#[cfg(test)]
mod native_export_lifetime;

#[cfg(test)]
mod native_phys;

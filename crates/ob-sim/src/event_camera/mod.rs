//! The player's event cameras and the world's slow motion, traced in
//! SPEED2.EXE (verified unless marked): the crash camera (event kind 6,
//! camera mover 0x0F) a car-against-car crash starts, its follow-up (kind
//! 7, mover 0x10) and the time scale both run under.
//!
//! * **Slow motion** (the world, 0x890080: +0x108 rate, +0x10C seconds
//!   left, +0x110 / +0x114 the callback and its argument; set by 0x5EA290).
//!   The frame's time (0x5EA370, from the world update 0x5813DA) counts
//!   +0x10C down by the real frame time; once it reaches 0 the rate goes
//!   back to 1, the time to 0 and the callback runs (and is cleared); the
//!   world then steps by the frame time × the rate. So the slow motion
//!   lasts its seconds of real time, and the simulation clock (g_SimTicks,
//!   4000 a second) runs at the rate meanwhile ([`SlowMotion`]).
//! * **Starting an event** (Player_StartEventCamera 0x6022F0(kind, key,
//!   _, flag) on the player's camera director, the per-player object of
//!   0x8900B4[]): no drift race (0x89E7D9, 0x890104); without `key` 5 s of
//!   simulation since the same kind last started (+0x68 + 4·kind, 0 or
//!   0x7FFFFFFF meaning never), with it 0.1 s since +0x68; only while
//!   racing (race status 4), kinds 0..7, one player; kinds 6 / 7 need
//!   +0x24 ≠ 0 — the Jump Camera option (profile settings +0x1C, 0x83AA20,
//!   copied by the director's constructor at 0x5F180B: 127 on, 0 off) —
//!   and kinds 1..5 0x83AA24 (settings +0x20, 127.5). The current kind (+0x58)
//!   ends first when 0x8020E0[kind] (kinds 1, 2); any player's current
//!   kind with 0x8020E8[kind] (kinds 3..7) refuses. Then +0x98 = the real
//!   clock, +0x68 + 4·kind = the simulation clock (without `key`) and by
//!   kind: 1 → mover 0x0D, 2 → 0x0E (both with the world's +0xFC timer),
//!   3 → slow motion 1/3 for 8 s, 4 → 1/3 for 2 s, 5 → 2.0 for 4 s, 6 → 0.4
//!   for 8 s and mover 0x0F (+0x644 = +0x648 = −1), 7 → 0.4 for 8 s and
//!   mover 0x10 (movers not in game flow state 5); the callback is 0x6022E0
//!   (0x5FFE80(1)). +0x225 = 1, +0x58 = kind, +0x90 = flag, +0x94 = 0 with
//!   the flag else 1.0.
//! * **Ending** (0x5FFE80, with a kind running): +0x225 = 0; kinds 1, 2
//!   stamp +0x68 (outside replays, 0x890084); kinds 1, 2, 6, 7 restore the
//!   saved mover (Player_RestoreCamera, not in game flow state 5); every
//!   kind resets the slow motion (rate 1, time 0, the callback cleared, the
//!   world's +0xFC 0), +0x5C = the kind, +0x58 = 0, +0x90 = 0, +0x94 = 0.
//! * **The crash trigger** (Car_Crash 0x59FB80, after it puts a car in
//!   crash mode: 0x5A0095..0x5A0105): no drift race, the pair car against
//!   car (0x592C70), the push ≥ 30 (0x7A186C), the car a human racer's
//!   (0x431DD0: racer +4 = 1, +0x900 clear), its director's kind not 6 or
//!   7, and a crash camera placement (0x5F1EF0, [`placement`]); then kind
//!   6 (key off, flag 1). Refused, 0x5EA650 clears +0x644 / +0x648.
//! * **While it runs** (Car_UpdateRaceEvents 0x604750, each world update):
//!   with kind 6 or 7, a car in crash mode (+0x38 = 1) ends it once all
//!   four wheels are down (+0x424 = 4) more than 0.5 s after the crash
//!   began (+0x548, the simulation clock kept by 0x5F6BF0 for a human
//!   car); a car out of crash mode ends it with any wheel down. Every
//!   eighth update ((counter + 3) & 7 = 0, 0x60498A), with kind 6 and the
//!   car in crash mode: more than 3 s after the crash began it ends kind 6
//!   and starts kind 7 (the same gates: no drift race, 5 s since kind 7
//!   last started via +0x84, racing, one player, the option, the kind
//!   tables) — the second crash camera, 0x10, again 0.4 for 8 s.
//!
//! * **The mover** ([`CrashShot`], [`plan_shot`]; Camera_Factory 0x0F /
//!   0x10 → 0x44BEA0, update 0x44C350): a still eye beside the road ahead
//!   on the human car's list (the game's `crash_route`), field of view 20°,
//!   the car off-centre by ±0.4 of the half view. Compared under Wine
//!   (captured): the place agrees to 1 cm and the framing matches; the whole
//!   HUD is hidden meanwhile (0x6000A0, `hud::Input::event_camera`).
//!
//! **Not started (verified)**: kinds 1..5 — 0x83AA24 is 127.5, but
//! no trigger exists in the PC game (see `may_start`).
// prov: inferred event_camera.module
//! **Inferred**: one world update per clone frame for the eighth-update
//! counter; the human car's list as a fresh list from its node.

mod crash_shot;
mod director;
mod slowmo;

pub use self::crash_shot::*;
pub use self::director::*;
pub use self::slowmo::*;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn racing() -> RaceState {
        RaceState { racing: true, ..Default::default() }
    }

    #[test]
    fn slow_motion_runs_in_real_time() {
        let mut s = SlowMotion::default();
        s.start(0.4, 8.0);
        let (dt, done) = s.advance(1.0);
        assert!((dt - 0.4).abs() < 1e-6 && !done);
        let (dt, done) = s.advance(7.5);
        assert!(done && (dt - 7.5).abs() < 1e-6 && s.rate == 1.0);
    }

    #[test]
    fn crash_starts_kind_six_then_seven() {
        let mut e = EventCameras::new(true);
        let mut s = racing();
        s.crash_mode = true;
        assert!(e.crash(&s, true));
        assert_eq!((e.kind, e.mover()), (6, Some(0x0F)));
        assert_eq!(e.slow.rate, 0.4);
        // Airborne for over 3 s of simulation: kind 7 on an eighth update.
        for _ in 0..400 {
            e.update(0.01, &s);
        }
        assert_eq!(e.kind, 7);
        assert_eq!(e.previous, 6);
        // Down on four wheels: it ends, the rate back to 1.
        s.wheels_down = 4;
        e.update(0.01, &s);
        assert_eq!(e.kind, 0);
        assert_eq!(e.slow.rate, 1.0);
        // 5 s between two crash cameras.
        assert!(!e.crash(&s, true));
        for _ in 0..200 {
            e.update(0.01, &RaceState { racing: true, ..Default::default() });
        }
        assert!(e.crash(&s, true));
    }

    #[test]
    fn option_and_drift_refuse() {
        let mut e = EventCameras::new(false);
        assert!(!e.crash(&racing(), true));
        let mut e = EventCameras::new(true);
        assert!(!e.crash(&RaceState { racing: true, drift: true, ..Default::default() }, true));
        assert!(!e.crash(&racing(), false));
        assert!(!e.crash(&RaceState::default(), true));
    }

    #[test]
    fn a_ramp_flight_is_predicted() {
        let flat = |_: Vec3| (0.0f32, Vec3::Z);
        // 3 m up at 30 m/s level: falls 3 m in 0.78 s.
        let f = predict_flight(Vec3::new(0.0, 0.0, 3.0), Vec3::new(30.0, 0.0, 0.0), 30.0, 1.8, 0.0, &flat, true);
        assert!((f.height - 3.0).abs() < 1e-4);
        assert!(f.airtime > 0.6 && f.airtime < 0.9, "{f:?}");
        // On the ground: no flight.
        let g = predict_flight(Vec3::ZERO, Vec3::new(30.0, 0.0, 0.0), 30.0, 1.8, 0.0, &flat, true);
        assert_eq!(g.height, 0.0);
        assert!(graph(JUMP_SPEED, 127.0) > 30.0 && graph(JUMP_SPEED, 127.0) < 30.1);
    }

    #[test]
    fn jump_starts_kind_six_and_landing_ends_it() {
        let mut e = EventCameras::new(true);
        let mut s = RaceState { racing: true, speed: 35.0, ..Default::default() };
        let mut flight = |_: f32| Flight { height: 2.0, airtime: 1.2, mean: 1.0 };
        for _ in 0..8 {
            e.update_with(0.01, &s, &mut flight, &mut || true);
        }
        assert_eq!((e.kind, e.jumps, e.started_now), (6, 1, Some(6)));
        s.wheels_down = 1;
        e.update_with(0.01, &s, &mut flight, &mut || true);
        assert_eq!(e.kind, 0);
        // Too slow: none.
        let mut e = EventCameras::new(true);
        let s = RaceState { racing: true, speed: 25.0, ..Default::default() };
        for _ in 0..16 {
            e.update_with(0.01, &s, &mut flight, &mut || true);
        }
        assert_eq!(e.kind, 0);
    }

    #[test]
    fn the_crash_shot_frames_the_car() {
        // A straight road along +x, 10 m nodes, 6 m half widths.
        let nodes: Vec<NodeEdges> = (0..12)
            .map(|i| {
                let x = i as f32 * 10.0;
                NodeEdges {
                    s: x,
                    inner: [Vec3::new(x, 4.0, 0.0), Vec3::new(x, -4.0, 0.0)],
                    edge: [Vec3::new(x, 6.0, 0.0), Vec3::new(x, -6.0, 0.0)],
                }
            })
            .collect();
        let car = Vec3::new(0.0, 0.0, 0.5);
        let shot = plan_shot(&nodes, 8, car, Vec3::new(30.0, 0.0, 0.0), &|_| 0.0, &|_, _| false).unwrap();
        // Node 6 (8 − 2); both sides equally far along: the right one.
        assert_eq!(shot.eye, Vec3::new(60.0, -4.0, 1.0));
        assert_eq!((shot.side, shot.second), (-0.4, Vec3::new(20.0, 4.0, 1.0)));
        let c = shot.view(car, 0.0);
        let fov = c.game_fov.unwrap().to_degrees();
        assert!((fov - 20.0).abs() < 0.01, "{fov}");
        assert!(shot.visible && !plan_shot(&nodes, 8, car, Vec3::X, &|_| 0.0, &|_, _| true).unwrap().visible);
        assert!(plan_shot(&nodes, 3, car, Vec3::X, &|_| 0.0, &|_, _| false).is_some());
        let second = shot.second_shot(Vec3::new(30.0, 0.0, 3.0), 0.5, &|_| 0.0, &|_, _| false);
        assert_eq!((second.eye.z, second.side), (3.0, 0.4));
    }

    #[test]
    fn eighth_updates() {
        let mut e = EventCameras::new(true);
        let mut eighth = Vec::new();
        for _ in 0..16 {
            e.update(0.01, &racing());
            eighth.push(e.eighth());
        }
        // Counter 1, 2, …: (n + 3) & 7 = 0 at n = 5 and 13.
        let at: Vec<usize> = eighth.iter().enumerate().filter(|(_, b)| **b).map(|(i, _)| i + 1).collect();
        assert_eq!(at, [5, 13]);
    }

    #[test]
    fn placement_needs_seventy_five_metres() {
        let nodes = |step: f32, n: usize| -> Vec<NodeEdges> {
            (0..n).map(|i| NodeEdges { s: i as f32 * step, inner: [Vec3::ZERO; 2], edge: [Vec3::ZERO; 2] }).collect()
        };
        // The Quick Race grid's list (8 m points in): 84.4 m at node 8.
        assert_eq!(far_node(&nodes(10.0, 20)), Some(8));
        // 75 m within four nodes: refused; never 75 m: refused.
        assert_eq!(far_node(&nodes(25.0, 20)), None);
        assert_eq!(far_node(&nodes(10.0, 5)), None);
    }
}

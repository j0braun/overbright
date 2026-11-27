//! The AI racers' low-level driver: the game's "CarDriver" for controller
//! type 3 (constructor 0x425AB0, vtable 0x784EB0, base 0x423330), run once
//! a frame after the controllers have planned (0x421030 through the world
//! update 0x609CF0). It steers at a look-ahead point on the path the
//! action layer hands it and holds the target speed it is given, with
//! adaptive PID controllers, and leaves a front-wheel angle, pedals and a
//! gear request for the car (see docs/formats.md, "AI driver").
//!
//! Angles go through the game's 16-bit binary angle (0x10000 = 360°).

use glam::{Vec2, Vec3};

use crate::drive::{AiControls, CarState};
use crate::handling::Handling;

mod car;
mod drift;
mod math;
mod pid;
mod unstuck;

pub use car::*;
pub use drift::*;
pub use math::*;
pub use pid::*;
pub use unstuck::*;

/// The steering PID's bounds and rates per frame (0x421030 writes them;
/// tables 0x78385C, 0x78388C): rates, then (min, max) of Kp, Ki and Kd.
/// Full-physics cars (car +0x28 = 1, every racer here), car or SUV
/// (record +0x88A).
const STEER_GAINS_CAR: ([f32; 3], [(f32, f32); 3]) = ([5e-5, 5e-5, 5e-6], [(0.15, 0.5), (0.0, 0.5), (0.0, 0.08)]);
const STEER_GAINS_SUV: ([f32; 3], [(f32, f32); 3]) = ([5e-5, 5e-5, 5e-6], [(0.4, 0.7), (0.0, 0.5), (0.0, 0.08)]);

/// Look-ahead over forward speed, m (0x7E9D94: 0..160 m/s).
const LOOKAHEAD: [f32; 10] = [22.8972, 22.8972, 24.0, 27.9, 33.0, 39.0, 45.8, 49.4, 53.1, 55.9];
/// The look-ahead of the table modes: forward speed × the time, at least the
/// least, by mode (0x7839B0, 0x7839CC; 0x415F80 reads modes 3 and 4).
const LOOK_TIME: [f32; 7] = [0.55, 0.4, 0.35, 0.55, 0.9, 0.4, 0.0];
const LOOK_LEAST: [f32; 7] = [15.0, 15.0, 15.0, 5.0, 9.0, 15.0, 0.0];
/// Its factor over the performance class 0..3 (0x7E9DF8, full physics).
const LOOKAHEAD_CLASS: [f32; 2] = [1.3, 1.1];
/// The steering gains a slow car starts from, over |v| (0x7E9D44, 0x7E9D80,
/// 0x7E9D58: 0..160 m/s).
const SEED_KP: [f32; 10] = [0.328, 0.22, 0.148, 0.115, 0.09, 0.074, 0.057, 0.049, 0.043, 0.04];
const SEED_KI: [f32; 10] = [0.21, 0.244, 0.267, 0.29, 0.305, 0.321, 0.328, 0.336, 0.341, 0.344];
const SEED_KD: [f32; 10] =
    [0.0, 0.075829, 0.075829, 0.073826, 0.060403, 0.04698, 0.04698, 0.040268, 0.040268, 0.040268];
/// Steering lock over signed speed, degrees (0x7E9E0C: 0..160 m/s).
static LOCK: crate::exe_tables::Table<[f32; 10]> = crate::exe_tables::Table::new(|t| &t.ai_lock);
/// The table steering's rate limit: over |error|° (0x7E9E34: 0..95) and
/// speed (0x7E9E20: 0..160 m/s), per s.
const TABLE_RATE_ERROR: [f32; 20] = [1.0; 20];
static TABLE_RATE_SPEED: crate::exe_tables::Table<[f32; 10]> = crate::exe_tables::Table::new(|t| &t.ai_table_rate_speed);
/// How fast the pedal may move over |v|, per s (0x7E9D10: 0..160 m/s).
const PEDAL_RATE: [f32; 10] = [30.0, 15.0, 7.5, 6.0, 6.0, 6.0, 6.0, 6.0, 6.0, 6.0];
/// The weight of the direction of travel against the heading by speed
/// (0x81E7E4, from 0x7E9D34).
const TRAVEL_WEIGHT: [(f32, f32); 2] = [(5.0, 0.0), (10.0, 1.0)];
/// The rate the steering error should shrink at, °/s, by |error|°
/// (0x827A18, from 0x7E9C78).
const SHRINK_RATE: [(f32, f32); 10] = [
    (0.0, 0.0),
    (1.0, -0.5),
    (3.0, -1.5),
    (5.0, -2.5),
    (10.0, -5.0),
    (50.0, -5.0),
    (60.0, -5.0),
    (70.0, -5.0),
    (80.0, -5.0),
    (90.0, -5.0),
];
/// The speed PID's derivative target by speed over target (0x81E7F4, from
/// 0x7E9CC8).
const SPEED_RATE: [(f32, f32); 9] = [
    (-20.0, 100.0),
    (-10.0, 10.0),
    (-5.0, 7.0),
    (-2.0, 3.0),
    (0.0, 0.0),
    (2.0, 0.0),
    (10.0, -3.0),
    (20.0, -10.0),
    (30.0, -100.0),
];
/// The table steering gains by mode (0x78395C P, 0x783978 I, 0x783994 D).
const TABLE_P: [f32; 7] = [0.9, 1.0, 0.8, 0.7, 0.85, 0.5, 0.0];
const TABLE_I: [f32; 7] = [0.2, 0.0, 0.1, 1.42, 0.79, 1.0, 0.0];
const TABLE_D: [f32; 7] = [0.3, 0.08, 0.18, 0.0, 0.12, 0.0, 0.0];
/// The table speed gains by mode: accelerating (0x7E8FCC, 0x7E8FE8,
/// 0x7E9004) and braking (0x7E9020, 0x7E903C, 0x7E9058).
const SPEED_PA: [f32; 7] = [0.1, 0.1, 0.1, 0.1, 0.005, 0.1, 0.1];
const SPEED_IA: [f32; 7] = [0.02, 0.02, 0.02, 0.02, 0.005, 0.02, 0.02];
const SPEED_DA: [f32; 7] = [0.02, 0.02, 0.01, 0.01, 0.01, 0.01, 0.01];
const SPEED_PB: [f32; 7] = [0.1, 0.1, 0.1, 0.1, 0.005, 0.1, 0.1];
const SPEED_IB: [f32; 7] = [0.02, 0.02, 0.02, 0.02, 0.0, 0.02, 0.02];
const SPEED_DB: [f32; 7] = [0.02, 0.02, 0.01, 0.01, 0.008, 0.01, 0.01];
/// The steering integral's limit, rad (0x81E5AC, set at start-up by
/// 0x77A7D0: 6°).
const I_LIMIT: f32 = 0.104_719_75;

/// The AI racers' throttle cap by 0x89E804 (0x783A28); other drivers 1.
pub const THROTTLE_CAP: [f32; 4] = [0.93, 0.97, 1.0, 1.0];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn drift_driver_pedals_and_reverse() {
        // Standing, asked for 20 m/s straight ahead: full gas, no brake,
        // the automatic gears, wheels straight.
        let car = CarState::new(Vec3::ZERO, 0.0);
        let path = [Vec3::new(50.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 0.0)];
        let mut d = DriftDriver::default();
        let c = d.update(&car, &path, 20.0, 10.0, 1.0 / 60.0);
        assert_eq!((c.gas, c.brake, c.handbrake, c.gear), (1.0, 0.0, 0.0, -1));
        assert!(c.steer_degrees.abs() < 0.1);
        // The target behind: back up (gear 0), the wheels turning at most
        // 360°/s (6° in a 60th) towards the reversed aim.
        let c = d.update(&car, &[Vec3::new(-50.0, 1.0, 0.0), Vec3::new(-100.0, 1.0, 0.0)], 20.0, 10.0, 1.0 / 60.0);
        assert_eq!(c.gear, 0);
        assert!(c.steer_degrees.abs() <= 6.01);
        // Over 70 mph the target speed is held to 31.29 m/s; asked under
        // 2 m/s it brakes.
        let c = d.update(&car, &path, 1.0, 10.0, 1.0 / 60.0);
        assert_eq!((c.gas, c.brake), (0.0, 1.0));
        assert!((wrap_pi(3.0 * std::f32::consts::PI).abs() - std::f32::consts::PI).abs() < 1e-3);
    }

    /// 0x43E6D0's table stays within 8 units (0.05°) of the exact angle
    /// all the way round, with the axes exact.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn table_angle_follows_atan2() {
        assert_eq!(
            (angle16(1.0, 0.0), angle16(0.0, 1.0), angle16(-1.0, 0.0), angle16(0.0, -1.0)),
            (0, 0x4000, 0x8000, 0xC000)
        );
        for k in 0..3600 {
            let t = k as f32 * std::f32::consts::TAU / 3600.0;
            let exact = (t / std::f32::consts::TAU * 65536.0).round() as i32;
            let d = (angle16(t.cos() * 7.0, t.sin() * 7.0) as i32 - exact).rem_euclid(65536);
            assert!(d.min(65536 - d) <= 8, "{k}: {d}");
        }
    }

    /// The look-ahead point is measured from the car's centre of mass
    /// (car +0x60: 0x403AF0 with no path, 0x415630 along one), not from the
    /// body origin. Under Wine, on the pinned stage-1 circuit's grid, the
    /// three rivals' points stood 25.187 m ahead of their centres of mass
    /// (PEUGOT, 240SX, MIATA alike); the clone's had stood that far ahead
    /// of the origins, 0.23–0.29 m short of the original's, and the 240SX
    /// steered two 16-bit steps further from the first frame on
    /// (docs/evidence/grid-stance/launch.txt).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_look_ahead_is_measured_from_the_centre_of_mass() {
        let looks: Vec<(Vec2, Vec3)> = [[0.3, 0.0, 0.1], [0.24, 0.0, 0.05]]
            .iter()
            .map(|c| {
                let mut h = crate::handling::parse_handling(&vec![0u8; 0x1000]);
                h.centre_of_mass = *c;
                let car = CarState::new(Vec3::new(-35.0, 367.0, 1.6), 0.0);
                let com = car.position + car.orientation * Vec3::from(h.centre_of_mass);
                let mut d = AiDriver::new(&h, false, 1.0);
                let input = DriverInputs {
                    path: &[],
                    target_speed: 0.0,
                    enabled: true,
                    green: true,
                    countdown: false,
                    since_green: 0.0,
                    clock: 0.0,
                };
                d.update(&car, &h, false, &input, 1.0 / 60.0);
                (d.target, com)
            })
            .collect();
        let ahead: Vec<Vec2> = looks.iter().map(|(t, com)| *t - com.truncate()).collect();
        assert!((ahead[0] - ahead[1]).length() < 1e-4, "{ahead:?}");
        assert!(ahead[0].x >= 5.0 && ahead[0].y.abs() < 1e-4, "{ahead:?}");
        let gap = looks[0].0.x - looks[1].0.x;
        assert!((gap - 0.06).abs() < 1e-4, "{gap}");
    }

    /// The same on the captured grid with the cars' own handling: each
    /// rival put on the original's frame-0 centre of mass and forward axis
    /// (pinned stage-1 circuit, the green), standing, no path yet: its
    /// look-ahead point lands on the original's (x within 5 mm; y differs
    /// by the original's straight-ahead direction, the car's 16-bit heading
    /// +0x40C, which is not ported), 25.187 m from the centre of mass.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn grid_look_ahead_points_land_on_the_originals() {
        // (car, centre of mass, forward, the original's look-ahead x)
        let grid = [
            ("PEUGOT", [-29.629143, 363.43701, 1.8613359], [0.9992957, -0.02730233, 0.025744643], -4.449808),
            ("240SX", [-34.900604, 367.18973, 1.7029082], [0.9996553, -0.0013895795, 0.026217049], -9.713684),
            ("MIATA", [-35.00402, 363.56027, 1.656018], [0.9995097, -0.017842844, 0.025730494], -9.8200645),
        ];
        for (name, com, fwd, native_x) in grid {
            let car = crate::car::Car::load(&crate::testkit::game_root(), name).unwrap();
            let h = &car.car_type.handling;
            let (com, fwd) = (Vec3::from(com), Vec3::from(fwd));
            let q = glam::Quat::from_rotation_z(fwd.y.atan2(fwd.x)) * glam::Quat::from_rotation_y(-fwd.z.asin());
            let mut state = CarState::new(Vec3::ZERO, 0.0);
            state.orientation = q;
            state.position = com - q * Vec3::from(h.centre_of_mass);
            let mut d = AiDriver::new(h, false, 1.0);
            let input = DriverInputs {
                path: &[],
                target_speed: 0.0,
                enabled: true,
                green: true,
                countdown: false,
                since_green: 0.0,
                clock: 0.0,
            };
            d.update(&state, h, false, &input, 1.0 / 60.0);
            let far = d.target.distance(com.truncate());
            assert!((d.target.x - native_x).abs() < 0.005, "{name}: {} against {native_x}", d.target.x);
            assert!((far - 25.187).abs() < 0.002, "{name}: {far} m ahead");
        }
    }

    /// A standing car (velocity 0) looks along the path from its first
    /// point ahead of +x (0x404EF0 with 0x43CE50's (1, 0) for a zero
    /// vector), not from the path's last point: under Wine the circuit
    /// start's rivals keep the same look point before and after they
    /// roll (circuit_s1f MIATA: (−4.17, 369.0) from the green on), the
    /// clone's took the last point and steered 3.1° instead of 1.05°
    /// until the car moved.
    #[test]
    fn a_standing_car_looks_from_the_first_point_ahead() {
        let car = CarState::new(Vec3::ZERO, 0.0);
        assert_eq!(car.velocity, Vec3::ZERO);
        let path = [
            Vec3::new(10.0, 1.0, 0.0),
            Vec3::new(20.0, 1.0, 0.0),
            Vec3::new(30.0, 5.0, 0.0),
            Vec3::new(40.0, 20.0, 0.0),
        ];
        let p = path_point(&car, car.position, &path, 15.0, 10.0);
        assert!((p.x - 15.0).abs() < 0.2 && (p.y - 1.0).abs() < 0.05, "{p:?}");
        assert_eq!(flat_unit(Vec3::ZERO), Vec2::X);
        assert_eq!(flat_unit(Vec3::new(0.0, -3.0, 7.0)), Vec2::new(0.0, -1.0));
    }

    #[test]
    fn running_mean_counts_up_then_slides() {
        let mut m = RunningMean::new(3);
        m.push(3.0);
        assert_eq!((m.count, m.sum, m.mean), (1, 3.0, 3.0));
        m.push(6.0);
        m.push(9.0);
        m.push(12.0);
        assert_eq!((m.count, m.sum, m.mean), (3, 27.0, 9.0));
    }

    #[test]
    fn tracker_integrates_and_differentiates() {
        let mut t = Tracker::new(5, 5, 30.0);
        t.update(1.0, 0.5, false, false);
        t.update(2.0, 0.5, false, false);
        // Areas: (0.5·1 + 0)·0.5 = 0.25 and (0.5·1 + 1)·0.5 = 0.75; rates 2, 2.
        assert_eq!(t.rate(), 2.0);
        assert!((t.integral() - (1.0 * 2.0 / (1.0 * 30.0))).abs() < 1e-6);
        t.reset(4.0);
        assert_eq!((t.cur, t.prev, t.areas.count, t.areas.sum), (0.0, 4.0, 0, 20.0));
    }

    #[test]
    fn window_mean_forgets_old_samples() {
        let mut w = WindowMean::new(0.2, 45.0);
        assert_eq!(w.samples.len(), 9);
        w.push(1.0, 1.0);
        w.push(3.0, 1.1);
        assert_eq!(w.mean, 2.0);
        w.push(5.0, 1.5);
        assert_eq!((w.count, w.mean), (1, 5.0));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn curves_clamp_and_interpolate() {
        assert_eq!(table(&*LOCK, 0.0, 160.0, -5.0), 45.0);
        assert!((table(&*LOCK, 0.0, 160.0, 160.0 / 18.0) - 37.5).abs() < 1e-4);
        assert_eq!(pairs(&TRAVEL_WEIGHT, 7.5), 0.5);
        assert_eq!(pairs(&SPEED_RATE, 40.0), -100.0);
    }

    #[test]
    fn leaving_the_road() {
        // A road along +x, 5 m each side; heading 0 is +x.
        let edge = RoadEdge {
            position: Vec2::ZERO,
            tangent: Vec2::X,
            travel: Vec2::X,
            half_width: [5.0, 5.0],
            backward: false,
        };
        let mut car = CarState::new(Vec3::new(0.0, -4.0, 0.0), 0.0);
        // Stopped, nose along the road: on it.
        assert_eq!(leaving_road(&car, &edge, None, false), 0);
        // Stopped, turned right towards the edge 1 m off: the nose is past it.
        car = CarState::new(Vec3::new(0.0, -4.0, 0.0), -std::f32::consts::FRAC_PI_2);
        assert_eq!(leaving_road(&car, &edge, None, false), 1);
        // Taken back when the controller's point is ahead of the nose.
        assert_eq!(leaving_road(&car, &edge, Some(Vec3::new(0.0, -20.0, 0.0)), true), 0);
        // Moving right at 2 m/s, 1 m from the edge: 0.5 s to it, heading
        // along the road with forward speed: 0; turned across it: 2.
        car = CarState::new(Vec3::new(0.0, -4.0, 0.0), 0.0);
        car.velocity = Vec3::new(1.0, -2.0, 0.0);
        car.publish_forward_speed();
        let forward = car.exported_forward_speed() > 0.0;
        assert!(forward);
        assert_eq!(leaving_road(&car, &edge, None, false), 0);
        car = CarState::new(Vec3::new(0.0, -4.0, 0.0), -1.4);
        car.velocity = Vec3::new(0.3, -2.0, 0.0);
        car.publish_forward_speed();
        assert_eq!(leaving_road(&car, &edge, None, false), 2);
        // Drifting left towards the other edge is never timed (as built).
        car = CarState::new(Vec3::new(0.0, 4.0, 0.0), 0.0);
        car.velocity = Vec3::new(1.0, 2.0, 0.0);
        car.publish_forward_speed();
        assert_eq!(leaving_road(&car, &edge, None, false), 0);
    }

    /// The forward test of 0x40DF50 reads car +0x42C (0x40E142), the speed
    /// the mover exports. A racer on the BeltMover through a corner (the
    /// career circuit's rival at (199, -1039), 25.8 s, with the human parked
    /// at the grid) had its body sliding 90° off its heading while the belt
    /// exported 20.7 m/s: under Wine none of the three rivals backed up
    /// there (no reverse target in 60 s); reading the body's velocity along
    /// the heading (−0.57 m/s) gave a 1, GetUnstuck, and the rival reversed.
    #[test]
    fn leaving_the_road_reads_the_exported_forward_speed() {
        let edge = RoadEdge {
            position: Vec2::ZERO,
            tangent: Vec2::X,
            travel: Vec2::X,
            half_width: [5.0, 5.0],
            backward: false,
        };
        // Heading along the road, the body moving right towards the edge
        // 1 m off (0.07 s to it) and slightly backward.
        let mut car = CarState::new(Vec3::new(0.0, -4.0, 0.0), 0.0);
        car.velocity = Vec3::new(-0.5, -14.8, 0.0);
        assert!(car.speed() < 0.0);
        car.set_exported_forward_speed(20.7);
        assert_eq!(leaving_road(&car, &edge, None, false), 0, "the belt's speed is forward: not off the road");
        // The same body with the mover exporting a stop or reverse: a yes.
        car.set_exported_forward_speed(0.0);
        assert_eq!(leaving_road(&car, &edge, None, false), 1);
    }

    #[test]
    fn contacts_in_the_frame_of_travel() {
        // Heading +x, moving +x: a point ahead and to the right.
        let mut car = CarState::new(Vec3::ZERO, 0.0);
        car.velocity = Vec3::new(10.0, 0.0, 0.0);
        let (right, along) = travel_frame(&car, Vec3::new(2.0, -0.5, 0.0));
        assert!((right - 0.5).abs() < 1e-5 && (along - 2.0).abs() < 1e-5);
        // Reversing: the frame keeps facing the nose.
        car.velocity = Vec3::new(-10.0, 0.0, 0.0);
        let (right, along) = travel_frame(&car, Vec3::new(2.0, -0.5, 0.0));
        assert!((right - 0.5).abs() < 1e-5 && (along - 2.0).abs() < 1e-5);
        // Past the edge: 3 m beyond a 5 m right half width, 2 m inside it.
        let edge = RoadEdge {
            position: Vec2::ZERO,
            tangent: Vec2::X,
            travel: Vec2::X,
            half_width: [4.0, 5.0],
            backward: false,
        };
        let car = CarState::new(Vec3::new(0.0, -8.0, 0.0), 0.0);
        assert!((past_edge(&car, &edge) - 3.0).abs() < 1e-5);
        let car = CarState::new(Vec3::new(0.0, 2.0, 0.0), 0.0);
        assert!((past_edge(&car, &edge) + 2.0).abs() < 1e-5);
    }

    #[test]
    fn cross_track_is_positive_to_the_right() {
        let path = [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), Vec3::new(20.0, 0.0, 0.0)];
        assert!((cross_track(&path, Vec3::new(12.0, -2.0, 0.0)) - 2.0).abs() < 1e-5);
        assert!((cross_track(&path, Vec3::new(5.0, 3.0, 0.0)) + 3.0).abs() < 1e-5);
    }
}

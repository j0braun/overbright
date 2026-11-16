//! The career intro's guidance layer: the D-Day script (career +0x713C =
//! 0x861E7C, stepped by 0x5342A0), Rachel's hint queue (career +0x887C =
//! 0x8635BC: 0x5243E0 posts, 0x54BD40 counts down, 0x53E0C0 fires) and
//! the GPS (0x838590: 0x529490 engages, 0x5010E0 clears, 0x501100 reads
//! its state through the route finder 0x81CBC0).
//!
//! Times are the game's 4000 Hz clock (0x865188) in milliseconds here.

#[cfg(test)]
use crate::formats::ai_routes::RouteFile;
#[cfg(test)]
use glam::{Vec2, Vec3};

mod gps_aid;
mod hints;
mod script;

pub use gps_aid::*;
pub use hints::*;
pub use script::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::speech::lines;

    impl Script {
        fn step0(&mut self, now: f32) -> Step {
            self.step(now, true).0
        }
    }

    #[test]
    fn the_script_waits_for_rachel() {
        let mut s = Script { state: 3, ..Script::default() };
        assert_eq!(s.step(0.0, false), (Step::None, None));
        assert_eq!(s.step(1.0, true), (Step::Instruction, Some(lines::career(0x800, 0x20))));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn milestone_hints_are_speech_once_per_cover() {
        let mut h = Hints::default();
        h.milestone(1, 0x6014_17C4, 0.0);
        h.milestone(3, 0x6014_17C4, 0.0);
        assert_eq!(h.pending(), vec![(4000, 0xE)]);
        // A DVD only while +0x28 is set.
        h.milestone(2, 0x9E10_1FB3, 3.0);
        assert_eq!(h.pending().len(), 1);
        h.fitted = true;
        h.milestone(2, 0x9E10_1FB3, 3.0);
        assert!(h.dvd && h.ratings[3] && h.pending().len() == 2);
        assert!(h.tick(1.0, 0.0, true).is_none());
        assert!(!h.fitted);
        // The magazine's line (slot 0 first at a tie), then the DVD's.
        let f = h.tick(1.0 + 1.0, 0.0, true).unwrap();
        assert_eq!(f.line, Some(lines::magazine(0x4, 1)));
        let f = h.tick(2.01, 0.0, true).unwrap();
        assert_eq!(f.line, Some(lines::dvd(0x8, 1)));
        assert!(!h.dvd && h.pending().is_empty());
    }

    #[test]
    fn hints_round_trip_through_the_career_bytes() {
        let mut h = Hints::default();
        h.car_lot();
        h.crib();
        h.complaints = 2;
        h.covers = vec![1, 2];
        let b = h.to_bytes();
        assert_eq!(b.len(), HINTS_LEN);
        assert_eq!(&b[0xA8..0xB4], &[0xA0, 0x0F, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0]);
        let g = Hints::from_bytes(&b);
        assert_eq!((g.garage, g.events, g.complaints, g.covers.clone(), g.slots), (true, true, 2, vec![1, 2], h.slots));
        assert_eq!(g.to_bytes(), b);
    }
    #[test]
    fn the_aid_looks_ahead_along_the_route() {
        let pts = [Vec3::ZERO, Vec3::new(50.0, 0.0, 0.0), Vec3::new(50.0, 100.0, 0.0)];
        // From x = 10: 100 m on is 60 m up the second stretch.
        let p = look_ahead(&pts, Vec3::new(10.0, 2.0, 0.0), 100.0).unwrap();
        assert!((p - Vec3::new(50.0, 60.0, 0.0)).length() < 1e-3);
        assert_eq!(look_ahead(&pts, Vec3::new(10.0, 2.0, 0.0), 200.0), None);
        assert_eq!(aid_ahead(10.0), 100.0);
        assert_eq!(aid_ahead(80.0), 120.0);
    }

    /// MARKER_GPS_AID on the intro's way to the car lot (player report: the
    /// clone's arrow pointed right at the airport where the original's
    /// points left). The original under Wine (2026-11-01, new
    /// career, Rachel's 350Z driven up the airport road): the car's centre
    /// of mass (car +0x60), its forward row (+0x90) and the kept aid point
    /// (player +0x120, +0x114 = 1). Over the stage-0 network the clone's
    /// aid lies the same way from the car; the network pruned for stage 4
    /// or 5 (a later career's free roam kept into a new career) starts
    /// the route south instead, which turned the arrow right.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_intro_aid_points_as_the_original() {
        let root = crate::testkit::game_root().join("TRACKS/ROUTESL4RA");
        let routes = std::fs::read(root.join("RoutesFreeRoam.bin")).expect("RoutesFreeRoam.bin");
        let paths = std::fs::read(root.join("PathsFreeRoam.bin")).expect("PathsFreeRoam.bin");
        // The car lot's trigger, its GPS point (−723, −913, 17).
        let lot = super::super::career::Trigger {
            name: 0,
            position: Vec3::new(-727.0, -877.0, 18.0),
            gps: Vec3::new(-723.0, -913.0, 17.0),
            min: Vec2::ZERO,
            max: Vec2::ZERO,
            polygon: [Vec2::ZERO; 8],
            points: 0,
        };
        let start = (Vec3::new(1695.3, -883.6, 24.1), Vec2::new(0.982, -0.190));
        let route = |stage: u32| {
            let mut f = RouteFile::parse(&routes);
            f.finalize_free_roam_paths(&paths, 4000, stage);
            let mut g = Gps::engage(0, &lot, Some(start));
            for _ in 0..10_000 {
                g.update(&f, start.0, start.1);
                if matches!(g.follower.state, 1 | 2) {
                    break;
                }
            }
            assert!(matches!(g.follower.state, 1 | 2), "stage {stage}: no route");
            (f, g)
        };
        // (car, forward, the original's aid point).
        let samples = [
            ((1695.3, -883.6), (0.982, -0.190), (1725.2, -787.3)),
            ((1717.5, -878.0), (0.201, 0.980), (1726.9, -778.0)),
            ((1716.7, -850.0), (-0.073, 0.997), (1731.7, -751.1)),
            ((1717.3, -781.6), (0.170, 0.985), (1743.5, -684.5)),
            ((1728.3, -720.3), (0.177, 0.984), (1749.6, -622.4)),
            ((1743.1, -636.8), (0.174, 0.985), (1693.7, -568.0)),
        ];
        let side = |f: Vec2, d: Vec2| f.perp_dot(d.normalize());
        let (f, mut g) = route(0);
        for (car, fwd, aid) in samples {
            let (car, fwd, want) = (Vec2::from(car), Vec2::from(fwd), Vec2::from(aid) - Vec2::from(car));
            let at = car.extend(24.1);
            g.update(&f, at, fwd);
            let got = g.update_aid(&f, at, 0.0).expect("an aid point").truncate() - car;
            // The same side of the car (+ left) and within 10° of the
            // original's bearing (the clone's look-ahead polyline is an
            // approximation, see docs/formats.md).
            assert_eq!(side(fwd, got) > 0.0, side(fwd, want) > 0.0, "at {car}: {got} against {want}");
            assert!(got.angle_to(want).abs().to_degrees() < 10.0, "at {car}: {got} against {want}");
        }
        // At the start the aid is to the left (the way north).
        let (f4, mut g4) = route(4);
        let at = start.0;
        let got = g4.update_aid(&f4, at, 0.0).unwrap().truncate() - at.truncate();
        assert!(side(start.1, got) < 0.0, "stage 4's network runs south: {got}");
    }

    #[test]
    fn the_aid_floats_in_view_turned_from_the_point() {
        // Looking down +y from the origin, z up: the aid sits ahead and
        // above the centre (a < 0, y down), its z row away from the target.
        let m = aid_matrix(Vec3::ZERO, Vec3::X, -Vec3::Z, Vec3::Y, 1.0, Vec3::new(0.0, 100.0, 0.0));
        let p = m.w_axis.truncate();
        assert!(p.y > 5.0 && p.y < 10.0 && p.z > 0.0 && p.x.abs() < 1e-4);
        assert!(m.z_axis.truncate().y < -0.9);
        assert!((m.x_axis.truncate().length() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn the_script_waits_for_the_message() {
        let mut s = Script::default();
        s.begin();
        assert_eq!(s.step0(0.0), Step::Prompt);
        assert_eq!(s.hud_features(), Some((0, 2)));
        assert_eq!(s.step0(1000.0), Step::None);
        assert_eq!(s.state, 2);
        s.step0(2001.0);
        assert_eq!(s.step0(2002.0), Step::Instruction);
        assert_eq!(s.state, 5);
        // Not until the message is read and the screens closed.
        assert_eq!(s.step0(9000.0), Step::None);
        s.on_read();
        assert_eq!(s.hud_features(), Some((0, 0)));
        s.on_sms_closed();
        s.step0(9001.0);
        assert_eq!(s.state, 6);
        assert_eq!(s.hud_features(), Some((1, 2)));
        assert_eq!(s.step0(10000.0), Step::None);
        assert_eq!(s.step0(11002.0), Step::GpsToA);
        assert!(s.shows_marker(true) && !s.shows_marker(false));
        assert_eq!(s.step0(11003.0), Step::None);
        assert_eq!(s.step0(11004.0), Step::Done);
        assert!(!s.running() && s.hud_features().is_none());
    }

    #[test]
    fn hints_fire_after_a_second_of_driving() {
        let mut h = Hints::default();
        assert_eq!(h.car_lot(), Some(lines::fe_screen(0x100_0000, 0x20)));
        assert_eq!(h.car_lot(), None);
        assert_eq!(h.pending().len(), 1);
        assert!(h.tick(10.0, 0.0, true).is_none());
        assert!(h.tick(10.5, 0.0, true).is_none());
        // Due at 0 ticks; with slot 0 the one counting it fires.
        let f = h.tick(11.0, 0.0, true).unwrap();
        assert_eq!(
            f,
            Fired { fire: Some(Fire::Message(SMS_LOC_GO_TO_GARAGE)), line: Some(lines::career(0x2000, 0x20)) }
        );
        assert_eq!(h.crib(), Some(lines::fe_screen(0x10_0000, 0x10)));
        // Not while Rachel is busy: it waits, then fires.
        assert!(h.tick(12.0, 0.0, false).is_none());
        assert_eq!(h.tick(12.1, 0.0, true).unwrap().fire, Some(Fire::Message(SMS_LOC_EVENTS_OUT_THERE)));
        assert_eq!(h.tick(50.0, 0.0, true).unwrap().line, Some(lines::career(0x80_0000, 0x10)));
        assert!(h.pending().is_empty());
    }

    #[test]
    fn stage0_races_open_then_rachel_closes_them() {
        let mut h = Hints::default();
        h.dday_a();
        h.dday_a();
        assert_eq!(h.pending().len(), 2);
        assert!(h.tick(1.0, 0.0, true).is_none());
        assert!(h.tick(15.9, 0.0, true).is_none());
        let f = h.tick(16.1, 0.0, true).unwrap();
        assert_eq!(f.fire, Some(Fire::Reveal));
        assert_eq!(f.line, Some(lines::intro_events(0x40_0000, 8)));
        let mut t = 16.1;
        for (n, line) in [(1, 1), (2, 4), (3, 8)] {
            h.race_over(STAGE0_RACES[0], true, false, 0, false);
            h.race_over(STAGE0_RACES[1], true, false, 0, false);
            t += 1.0;
            let f = h.tick(t, 0.0, true).unwrap();
            assert_eq!(f, Fired { fire: Some(Fire::Complain { close: n == 3 }), line: Some(lines::time(line)) });
        }
        h.race_over(STAGE0_RACES[2], true, false, 0, false);
        t += 1.0;
        assert_eq!(h.tick(t, 0.0, true), Some(Fired::default()));
    }

    #[test]
    fn stage_lines_name_the_new_stage() {
        let mut h = Hints::default();
        h.tick(1.0, 0.0, true);
        h.stage_up(3);
        h.stage_up(4);
        assert_eq!(h.pending(), vec![(60_000, 12)]);
        assert_eq!(h.tick(16.0, 0.0, true).unwrap().line, Some(lines::intro_area(4, 0x20)));
        assert_eq!(h.stage_line, 0);
        // A stage-1 win: kind 5 once, the final event of a stage kind 7.
        h.race_over(0x1234, true, false, 1, true);
        assert_eq!(h.tick(17.0, 0.0, true).unwrap().line, Some(lines::career(1, 0x20)));
        assert_eq!(h.tick(17.1, 0.0, true).unwrap().line, Some(lines::career(0x40, 0x20)));
        h.race_over(0x1234, true, false, 1, false);
        assert!(h.pending().is_empty());
    }

    #[test]
    fn zone_line_waits_300_s_of_driving_and_unique_posts_8_or_9() {
        let mut h = Hints::default();
        assert!(!h.zone_changed(0, true));
        assert!(!h.zone_changed(7, true));
        assert_eq!((h.zone, h.zone_time), (7, 0));
        h.drive_sum = 1_199_999;
        assert!(!h.zone_changed(8, true));
        h.drive_sum = 1_200_000;
        assert!(!h.zone_changed(8, true));
        assert!(!h.zone_changed(9, false));
        assert!(h.zone_changed(7, true));
        let b = h.to_bytes();
        assert_eq!(Hints::from_bytes(&b).zone, 7);
        h.unique_taken(false);
        h.unique_taken(true);
        assert_eq!(h.pending(), vec![(SECOND, 8), (SECOND, 9)]);
        assert_eq!(h.showcase_line(), Some(lines::career(0x1_0000, 0x20)));
        assert_eq!(h.showcase_line(), None);
    }

    #[test]
    fn neighbourhood_hashes_match_the_bit_table() {
        use crate::formats::hash::bin_hash;
        assert_eq!(crate::frontend::career::neighbourhood_hash(bin_hash("JACKSON_HEIGHTS")), 0x5755_251F);
        assert_eq!(crate::frontend::career::neighbourhood_hash(bin_hash("COAL_HARBOUR_WEST")), 0xBB12_679E);
        assert_eq!(crate::frontend::career::neighbourhood_hash(0x1234), 0);
    }

    #[test]
    fn the_shop_screens_lines_follow_0x5243e0() {
        let mut h = Hints { fe_clock: 4000, ..Hints::default() };
        let lot = Some((4u8, 0xEC17_E618u32));
        // The car lot: the once-line, then (+9 set, the time unset) the
        // later line, then nothing for an hour of the front-end clock.
        assert_eq!(h.fe_line(4, lot), Some(lines::fe_screen(0x100_0000, 0x20)));
        assert_eq!(h.fe_line(4, lot), Some(lines::fe_screen(0x20, 0x20)));
        assert_eq!(h.fe_line(4, lot), None);
        h.fe_clock += 3600 * 4000 + 1;
        assert_eq!(h.fe_line(4, lot), Some(lines::fe_screen(0x20, 0x20)));
        // Performance categories: first (0x800000), Megalow's kind 0.
        assert_eq!(h.fe_line(1, Some((3, 0x4E79_CD0A))), Some(lines::fe_screen(0x80_0000, 0x20)));
        assert_eq!(h.fe_line(1, Some((3, 0x7FC8_D0D0))), Some(lines::fe_screen(0x40_0000, 0x20)));
        // In the garage kinds 1, 2, 0x12 are the crib's (once, then none).
        let crib = Some((0u8, 0x5544_6B8F));
        assert_eq!(h.fe_line(2, crib), Some(lines::fe_screen(0x10_0000, 0x10)));
        assert_eq!(h.fe_line(0x12, crib), None);
        // The body shop's parts browser: line 8 and +0x28.
        h.fitted = false;
        assert_eq!(h.fe_line(2, Some((2, 0x2033_AE7C))), Some(lines::fe_screen(8, 0x20)));
        assert!(h.fitted);
        assert_eq!(h.fe_line(0x1B, Some((3, 0x4E79_CD0A))), Some(lines::fe_option(0x100, 0x10)));
        assert_eq!(h.fe_line(5, Some((1, 0xCB1B_31D5))), Some(lines::fe_screen(0x40, 0x20)));
    }
}

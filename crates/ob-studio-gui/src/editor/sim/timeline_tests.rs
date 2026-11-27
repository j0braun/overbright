use glam::Vec3;

use super::super::cams::{self, CamMode, ChaseCars};
use super::super::record::{Phase, Playback, Recording};
use super::super::summary::tests::{findings, lines, race};
use super::*;

fn done(rec: &Recording) -> Playback {
    let mut p = Playback::default();
    p.start();
    p.observe(rec, false);
    assert_eq!(p.phase, Phase::Done);
    p
}

/// The same real time played shows the same moment at 30, 60, 120 and
/// 200 frames a second, at every speed.
#[test]
fn playback_is_the_same_at_any_display_rate() {
    let rec = race(1, true, false);
    for speed in 0..SPEEDS.len() {
        let mut at = Vec::new();
        for fps in [30u32, 60, 120, 200] {
            let mut p = done(&rec);
            p.speed = speed;
            p.seek(&rec, 0.0);
            p.toggle(&rec);
            for _ in 0..fps * 10 {
                p.tick(&rec, 1.0 / fps as f32);
            }
            at.push(p.time);
        }
        let want = (10.0 * SPEEDS[speed]).min(60.0);
        for (k, t) in at.iter().enumerate() {
            assert!((t - want).abs() < 1e-4, "speed {}x, rate {k}: {t} want {want}", SPEEDS[speed]);
        }
    }
    // A speed change mid-play goes on from the moment shown.
    let mut p = done(&rec);
    p.seek(&rec, 0.0);
    p.toggle(&rec);
    for _ in 0..60 {
        p.tick(&rec, 1.0 / 60.0);
    }
    p.faster();
    for _ in 0..60 {
        p.tick(&rec, 1.0 / 60.0);
    }
    assert!((p.time - 3.0).abs() < 1e-4, "{}", p.time);
}

/// Stepping and scrubbing land on a recorded frame's time exactly.
#[test]
fn step_and_scrub_land_on_recorded_frames() {
    let rec = race(1, true, false);
    let on_frame = |t: f32| rec.frames.iter().any(|f| f.time == t);
    let mut p = done(&rec);
    p.seek(&rec, 3.1);
    p.step(&rec, 1);
    assert_eq!(p.time, 3.25);
    p.step(&rec, -1);
    assert_eq!(p.time, 3.0);
    p.seek(&rec, 3.1);
    p.step(&rec, -1);
    assert_eq!(p.time, 3.0, "back from between two frames: the one before");
    p.step(&rec, 4);
    assert_eq!(p.time, 4.0);
    p.step(&rec, -10_000);
    assert_eq!(p.time, -1.0, "stops at the first frame");
    p.step(&rec, 10_000);
    assert_eq!(p.time, 60.0, "and at the last");
    assert!(!p.playing);
    for (want, at) in [(3.0, 3.1), (3.25, 3.2), (-1.0, -9.0), (60.0, 99.0), (12.5, 12.5)] {
        p.seek_frame(&rec, at);
        assert_eq!(p.time, want, "scrub to {at}");
        assert!(on_frame(p.time));
    }
    // A track 600 points wide: every pixel lands on a frame.
    let r = Rect::from_min_size(Pos2::new(100.0, 0.0), egui::vec2(600.0, 20.0));
    for x in (100..=700).step_by(7) {
        p.seek_frame(&rec, time_at(r, -1.0, 60.0, x as f32));
        assert!(on_frame(p.time), "x {x}: {}", p.time);
    }
    assert_eq!(x_of(r, -1.0, 60.0, -1.0), 100.0);
    assert_eq!(x_of(r, -1.0, 60.0, 60.0), 700.0);
}

/// The markers are What went wrong's findings of the run shown, by time,
/// with a colour per kind; another seed's run has none here.
#[test]
fn markers_are_the_runs_findings() {
    let rec = race(1, true, true);
    let other = race(2, false, false);
    let batch = vec![findings(&other), findings(&rec)];
    let m = markers(&batch, &rec);
    let want: Vec<_> = batch[1].findings.iter().filter(|f| f.kind != Kind::Spread).collect();
    assert_eq!(m.len(), want.len());
    assert!(m.len() >= 3, "{m:?}");
    for f in &want {
        assert!(m.contains(f), "{f:?}");
    }
    assert!(m.windows(2).all(|w| w[0].time <= w[1].time), "by time");
    assert!(m.iter().any(|f| f.kind == Kind::Stuck));
    assert!(m.iter().any(|f| matches!(f.kind, Kind::CrashProp(_))));
    assert_ne!(kind_colour(&Kind::Stuck), kind_colour(&Kind::CrashWall));
    assert_ne!(kind_colour(&Kind::WrongWay), kind_colour(&Kind::Missed));
    assert_eq!(kind_colour(&Kind::CrashCar), kind_colour(&Kind::CrashWall));
    assert!(markers(&batch[..1], &rec).is_empty());
}

/// The Follow camera sits behind and above the chosen car and looks at it.
#[test]
fn follow_camera_targets_the_chosen_car() {
    let rec = race(1, true, false);
    let mut cars = ChaseCars::default();
    for id in [0, 2] {
        let t = 7.3;
        let cam = cams::view(&rec, CamMode::Follow, &mut cars, std::path::Path::new("/nonexistent"), id, t).unwrap();
        let car = rec.cars_at(t).into_iter().find(|c| c.id == id).unwrap();
        let to = (car.position + Vec3::Z - cam.position).normalize();
        assert!(cam.forward().dot(to) > 0.999, "car {id}: looks at it");
        assert!(cam.position.x < car.position.x && cam.position.z > car.position.z, "behind and above");
    }
    // No game data: Chase falls back to Follow.
    let a = cams::view(&rec, CamMode::Chase, &mut cars, std::path::Path::new("/nonexistent"), 2, 7.3).unwrap();
    let b = cams::view(&rec, CamMode::Follow, &mut cars, std::path::Path::new("/nonexistent"), 2, 7.3).unwrap();
    assert_eq!(a.position, b.position);
}

/// With the game's files: the chase camera rides behind the chosen car
/// (the game's Far view), and the view at a time does not depend on the way
/// there.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn chase_camera_rides_with_the_chosen_car() {
    let root = crate::testkit::game_root();
    let car = nfsu2::studio_api::chase::physics_car(&root, "PEUGOT").unwrap();
    let rec = race(1, true, false);
    for (id, t) in [(0u32, 20.0f32), (2, 25.1)] {
        let cam = cams::chase(&rec, &car, id, t).unwrap();
        let c = rec.cars_at(t).into_iter().find(|c| c.id == id).unwrap();
        let behind = c.position.x - cam.position.x;
        assert!((2.0..12.0).contains(&behind), "car {id}: {behind} m behind");
        assert!((cam.position.y - c.position.y).abs() < 1.0 && cam.position.z > c.position.z);
        assert!(cam.forward().x > 0.9, "looks along the road: {}", cam.forward());
        assert_eq!(cams::chase(&rec, &car, id, t).unwrap().position, cam.position, "the same each time");
    }
}

/// Through the whole editor: a recorded run under the view's timeline;
/// Left / Right step a frame, Shift a second, Space plays; a click on a
/// marker shows its finding; a click on the track scrubs to a frame; the
/// Follow camera rides with the car chosen.
#[test]
fn the_timeline_steps_scrubs_and_jumps_to_markers() {
    use crate::editor::app::App;
    use egui::{Event, Modifiers, PointerButton};

    let dir = crate::testkit::Scratch::new("sim-timeline");
    let p = dir.path().join("seed1.run");
    std::fs::write(&p, lines(1, true, false).iter().map(|l| l.write() + "\n").collect::<String>()).unwrap();
    let mut a = App::new(dir.path().join("game"), dir.path().join("maps"), Box::new(|_, _| Ok(Default::default())));
    a.new_mod("r", "Races", "L4RA").unwrap();
    let frame = |a: &mut App, events: Vec<Event>, modifiers: Modifiers| {
        let events = [Event::ModifiersChanged(modifiers)].into_iter().chain(events).collect();
        let _ = a.frame(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 1000.0))),
            events,
            ..Default::default()
        });
    };
    let key = |k: egui::Key, modifiers: Modifiers| {
        let e = |pressed| Event::Key { key: k, physical_key: None, pressed, repeat: false, modifiers };
        vec![e(true), e(false)]
    };
    let click = |a: &mut App, at: Pos2| {
        let b = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        frame(a, vec![Event::PointerMoved(at)], Modifiers::NONE);
        frame(a, vec![b(true)], Modifiers::NONE);
        frame(a, vec![b(false)], Modifiers::NONE);
        frame(a, vec![], Modifiers::NONE);
    };
    super::super::panel::load_recording(&a.ctx, &format!("{}@5", p.display()));
    for _ in 0..3 {
        frame(&mut a, vec![], Modifiers::NONE);
    }
    let sim = a.panels.sim.clone();
    let track = sim.borrow().track_rect;
    assert!(track.width() > 200.0, "the timeline is drawn: {track:?}");
    assert!(
        track.bottom() <= a.view_rect.top() + 1.0 || track.top() >= a.view_rect.bottom() - 1.0,
        "beside the view, not over it"
    );
    assert_eq!(sim.borrow().play.time, 5.0);
    frame(&mut a, key(egui::Key::ArrowRight, Modifiers::NONE), Modifiers::NONE);
    assert_eq!(sim.borrow().play.time, 5.25);
    frame(&mut a, key(egui::Key::ArrowLeft, Modifiers::NONE), Modifiers::NONE);
    frame(&mut a, key(egui::Key::ArrowLeft, Modifiers::NONE), Modifiers::NONE);
    assert_eq!(sim.borrow().play.time, 4.75);
    frame(&mut a, key(egui::Key::ArrowRight, Modifiers::SHIFT), Modifiers::SHIFT);
    assert_eq!(sim.borrow().play.time, 5.75);
    frame(&mut a, key(egui::Key::Space, Modifiers::NONE), Modifiers::NONE);
    assert!(sim.borrow().play.playing);
    frame(&mut a, key(egui::Key::Space, Modifiers::NONE), Modifiers::NONE);
    assert!(!sim.borrow().play.playing);
    // A marker: its moment, its car.
    let (lo, hi) = sim.borrow().rec.span().unwrap();
    let m = {
        let s = sim.borrow();
        let m = markers(&s.batch, &s.rec);
        assert!(!m.is_empty());
        m.iter().find(|f| f.kind == Kind::Stuck).copied().cloned().unwrap()
    };
    click(&mut a, Pos2::new(x_of(track, lo, hi, m.time), track.center().y));
    {
        let s = sim.borrow();
        assert_eq!(s.play.time, m.time, "the marker's moment");
        assert_eq!(s.play.follow, Some(m.car));
    }
    // The track between markers: a recorded frame near the pointer.
    let x = x_of(track, lo, hi, 40.1);
    click(&mut a, Pos2::new(x, track.center().y));
    {
        let s = sim.borrow();
        assert!(s.rec.frames.iter().any(|f| f.time == s.play.time), "{}", s.play.time);
        assert!((s.play.time - 40.1).abs() < 0.3, "{}", s.play.time);
    }
    // Follow car 2 in the 3D view.
    {
        let mut s = sim.borrow_mut();
        s.play.follow = Some(2);
        s.cam = CamMode::Follow;
    }
    frame(&mut a, vec![], Modifiers::NONE);
    let t = sim.borrow().play.time;
    let want = cams::follow(&sim.borrow().rec.cars_at(t).into_iter().find(|c| c.id == 2).unwrap());
    assert_eq!(a.state.as_ref().unwrap().camera.position, want.position);
}

//! Recorded runs made in code, for the tests of the crates that read runs (not part of any program).

use glam::{Quat, Vec3};

use super::format::{CarInfo, CarKind, CarSample, Event, EventKind, Frame, Header, Line, ResultRow};
use super::record::Recording;

/// A recorded sprint along y = 0 (x = 20 m/s × t): racer 1 stops at
/// x = 200 from 10 s and never moves again; racer 2 hits the mod's lamp at
/// x = 400 at 20 s and passes over checkpoint 2; the player's slot leaves
/// the road (y = 40) from 30 to 34 s. `start_crash`: racer 2 also hits a
/// wall 1.5 s after the green at its grid spot.
pub fn lines(seed: u64, prop_crash: bool, start_crash: bool) -> Vec<Line> {
    let mut r = Vec::new();
    r.push(Line::Header(Header { version: 1, seed, race: "r".into(), checkpoints: 4, ..Default::default() }));
    for (id, kind, name) in [(0, CarKind::Player, ""), (1, CarKind::Racer, "Civic"), (2, CarKind::Racer, "Golf")] {
        r.push(Line::Car(CarInfo { id, kind, name: name.into() }));
    }
    let pos = |id: u32, t: f32| -> (Vec3, f32) {
        let t0 = t.max(0.0);
        match id {
            1 if t0 >= 10.0 => (Vec3::new(200.0, 0.0, 0.0), 0.0),
            0 if (30.0..=34.0).contains(&t0) => (Vec3::new(20.0 * t0, 40.0, 0.0), 20.0),
            _ => (Vec3::new(20.0 * t0 - 10.0 * id as f32, 0.0, 0.0), if t < 0.0 { 0.0 } else { 20.0 }),
        }
    };
    let mut tick = 0;
    let mut t = -1.0f32;
    while t <= 60.0 {
        let cars = (0..3)
            .map(|id| {
                let (p, v) = pos(id, t);
                CarSample {
                    id,
                    kind: if id == 0 { CarKind::Player } else { CarKind::Racer },
                    position: p,
                    rotation: Quat::IDENTITY,
                    speed: v,
                    lap: 0,
                    checkpoint: 0,
                    place: id + 1,
                    wheels: None,
                }
            })
            .collect();
        r.push(Line::Frame(Frame { tick, time: t, cars }));
        tick += 15;
        t += 0.25;
    }
    let ev = |time: f32, kind, car, checkpoint, value| {
        Line::Event(Event { tick: ((time + 1.0) * 60.0) as u64, time, kind, car, checkpoint, value })
    };
    r.push(ev(0.0, EventKind::Start, 0, 0, 0.0));
    if start_crash {
        r.push(ev(1.5, EventKind::Crash, 2, 1, 8.0));
    }
    r.push(ev(5.0, EventKind::Checkpoint, 1, 1, 0.0));
    r.push(ev(5.0, EventKind::Checkpoint, 2, 1, 0.0));
    r.push(ev(8.0, EventKind::Checkpoint, 1, 2, 0.0));
    r.push(ev(14.0, EventKind::Stuck, 1, 3, 4.0));
    if prop_crash {
        r.push(ev(20.0, EventKind::Crash, 2, 2, 10.0));
        r.push(ev(25.0, EventKind::Checkpoint, 2, 3, 0.0));
    }
    r.push(ev(50.0, EventKind::Finish, 0, 0, 50.0));
    r.push(ev(58.0, EventKind::Finish, 2, 0, 58.0));
    for (car, place, time) in [(0, 1, Some(50.0)), (2, 2, Some(58.0)), (1, 0, None)] {
        r.push(Line::Result(ResultRow { car, place, time, best: None, laps: Vec::new() }));
    }
    r.push(Line::End("limit".into()));
    r
}

pub fn race(seed: u64, prop_crash: bool, start_crash: bool) -> Recording {
    let mut r = Recording::default();
    for l in lines(seed, prop_crash, start_crash) {
        r.push(l);
    }
    r
}


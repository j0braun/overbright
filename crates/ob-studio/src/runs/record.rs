//! A simulation as the editor keeps it: every record the game streamed,
//! and the playback over it (play / pause, speed, scrub, follow). Plain
//! data, so the state machine is tested without a window or a game.

use std::collections::HashMap;

use glam::Vec3;

use super::format::{CarInfo, CarKind, CarLook, CarSample, Event, EventKind, Frame, Header, Line, ResultRow};

/// What the player's slot is called in the results, events and follow list:
/// in a simulation the AI drives it.
pub const PLAYER_SLOT: &str = "Your slot (AI)";

/// The playback speeds offered.
pub const SPEEDS: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];

/// Every record of one run, in order.
#[derive(Clone, Debug, Default)]
pub struct Recording {
    pub header: Option<Header>,
    pub cars: HashMap<u32, CarInfo>,
    /// Each car's looks in order: the time of the last frame before the
    /// look came (it holds for the frames after), the look.
    pub looks: HashMap<u32, Vec<(f32, CarLook)>>,
    pub frames: Vec<Frame>,
    pub events: Vec<Event>,
    pub results: Vec<ResultRow>,
    /// The state hashes: the last tick each covers, the hash (a second run
    /// of the seed parts from this one at the first that differs).
    pub hashes: Vec<(u64, u64)>,
    /// The end line (`finish`, `quit`, `limit`, `error …`).
    pub end: Option<String>,
    /// Lines that did not read (kept for the log, at most a few).
    pub bad: Vec<String>,
}

impl Recording {
    /// Take one line of the stream.
    pub fn push_line(&mut self, text: &str) {
        match Line::parse(text) {
            Ok(Some(l)) => self.push(l),
            Ok(None) => {}
            Err(e) => {
                if self.bad.len() < 8 {
                    self.bad.push(e);
                }
            }
        }
    }

    pub fn push(&mut self, l: Line) {
        match l {
            Line::Header(h) => self.header = Some(h),
            Line::Car(c) => {
                self.cars.insert(c.id, c);
            }
            Line::Look(l) => {
                let after = self.frames.last().map_or(f32::NEG_INFINITY, |f| f.time);
                self.looks.entry(l.id).or_default().push((after, l));
            }
            Line::Frame(f) => {
                // The race clock set back (the start scene sets it once the
                // grid is loaded): the frames before it are dropped, so the
                // times stay in order; the looks so far hold from the start.
                if self.frames.last().is_some_and(|l| f.time < l.time) {
                    self.frames.clear();
                    for list in self.looks.values_mut() {
                        for l in list.iter_mut() {
                            l.0 = f32::NEG_INFINITY;
                        }
                    }
                }
                self.frames.push(f);
            }
            Line::Event(e) => self.events.push(e),
            Line::Result(r) => self.results.push(r),
            Line::Hash { tick, hash } => self.hashes.push((tick, hash.0)),
            Line::End(why) => self.end = Some(why),
        }
    }

    /// The whole stream read at once (a saved file).
    pub fn read(text: &str) -> Recording {
        let mut r = Recording::default();
        for l in text.lines() {
            r.push_line(l);
        }
        r
    }

    /// The times covered: first and last frame.
    pub fn span(&self) -> Option<(f32, f32)> {
        Some((self.frames.first()?.time, self.frames.last()?.time))
    }

    /// The frame at or just before `time`.
    pub fn frame_at(&self, time: f32) -> Option<&Frame> {
        let k = self.frames.partition_point(|f| f.time <= time);
        self.frames.get(k.saturating_sub(1))
    }

    /// The time of the recorded frame nearest `time`.
    pub fn nearest_frame(&self, time: f32) -> Option<f32> {
        let k = self.frames.partition_point(|f| f.time <= time);
        let before = k.checked_sub(1).map(|i| self.frames[i].time);
        let after = self.frames.get(k).map(|f| f.time);
        match (before, after) {
            (Some(a), Some(b)) => Some(if time - a <= b - time { a } else { b }),
            (a, b) => a.or(b),
        }
    }

    /// The car's look at `time` (none in a stream without looks).
    pub fn look_at(&self, id: u32, time: f32) -> Option<&CarLook> {
        let list = self.looks.get(&id)?;
        list.iter().rev().find(|(after, _)| *after < time).or(list.first()).map(|(_, l)| l)
    }

    /// The cars at `time`, each between the frame at or before it and the
    /// next one (the game's 60 Hz ticks drawn smoothly at any rate); a car
    /// not in the next frame as it was.
    pub fn cars_at(&self, time: f32) -> Vec<CarSample> {
        let k = self.frames.partition_point(|f| f.time <= time);
        let Some(a) = self.frames.get(k.saturating_sub(1)) else { return Vec::new() };
        let Some(b) = self.frames.get(k).filter(|b| b.time > a.time && time > a.time) else { return a.cars.clone() };
        let t = ((time - a.time) / (b.time - a.time)).clamp(0.0, 1.0);
        a.cars
            .iter()
            .map(|c| match b.cars.iter().find(|n| n.id == c.id) {
                Some(n) => CarSample::lerp(c, n, t),
                None => *c,
            })
            .collect()
    }

    /// The car's name ("Racer 3" when the stream gave none; the player's
    /// slot, which the AI drives in a simulation, is "Your slot (AI)").
    pub fn name(&self, id: u32) -> String {
        match self.cars.get(&id) {
            Some(c) if c.kind == CarKind::Player => PLAYER_SLOT.into(),
            Some(c) if !c.name.is_empty() => c.name.clone(),
            Some(c) => match c.kind {
                CarKind::Player => PLAYER_SLOT.into(),
                CarKind::Racer => format!("Racer {id}"),
                CarKind::Traffic => format!("Traffic {id}"),
                CarKind::Encounter => format!("Street racer {id}"),
            },
            None => format!("Car {id}"),
        }
    }

    /// A car's path up to `time`, every `step`-th frame (a racing line; a
    /// jump over 30 m — a reset or a car placed again — starts a new piece).
    pub fn trace(&self, id: u32, upto: f32, step: usize) -> Vec<Vec<Vec3>> {
        let mut pieces: Vec<Vec<Vec3>> = vec![Vec::new()];
        for f in self.frames.iter().take_while(|f| f.time <= upto).step_by(step.max(1)) {
            if let Some(c) = f.cars.iter().find(|c| c.id == id) {
                let piece = pieces.last_mut().unwrap();
                if piece.last().is_some_and(|p: &Vec3| p.distance(c.position) > 30.0 * step as f32) {
                    pieces.push(vec![c.position]);
                } else {
                    piece.push(c.position);
                }
            }
        }
        pieces.retain(|p| p.len() > 1);
        pieces
    }

    /// The racers (player slot and opponents) by id.
    pub fn racers(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.cars.values().filter(|c| c.kind.races()).map(|c| c.id).collect();
        v.sort();
        v
    }

    /// The results table at `time`, in order: the stream's result lines
    /// once the run has ended there, else the race as it stands (finished
    /// racers by finish time, then the rest by their place on the road).
    /// Racers without a finish when the run ended are last, marked
    /// `dnf`; before the green light nobody has a place yet (grid order).
    pub fn standings(&self, time: f32) -> Vec<Standing> {
        let ended = self.end.is_some() && self.span().is_some_and(|(_, end)| time >= end);
        let laps_done = |id: u32| -> Vec<f32> {
            self.events
                .iter()
                .filter(|e| e.car == id && e.kind == EventKind::Lap && e.time <= time)
                .map(|e| e.value)
                .collect()
        };
        if ended && !self.results.is_empty() {
            let mut v: Vec<Standing> = self
                .results
                .iter()
                .map(|r| Standing {
                    car: r.car,
                    place: r.place,
                    time: r.time.filter(|_| r.place > 0),
                    best: r.best,
                    laps: r.laps.clone(),
                    dnf: r.place == 0,
                })
                .collect();
            v.sort_by(|a, b| {
                (a.dnf, a.place, a.time.unwrap_or(f32::MAX), a.car)
                    .partial_cmp(&(b.dnf, b.place, b.time.unwrap_or(f32::MAX), b.car))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            return v;
        }
        let Some(f) = self.frame_at(time) else { return Vec::new() };
        let started = time >= 0.0;
        let mut v: Vec<(Standing, u32)> = f
            .cars
            .iter()
            .filter(|c| c.kind.races())
            .map(|c| {
                let laps = laps_done(c.id);
                let finish =
                    self.events.iter().find(|e| e.car == c.id && e.kind == EventKind::Finish && e.time <= time);
                let s = Standing {
                    car: c.id,
                    place: 0,
                    time: finish.map(|e| e.value),
                    best: laps.iter().copied().reduce(f32::min),
                    laps,
                    dnf: ended && finish.is_none(),
                };
                (s, c.place)
            })
            .collect();
        // Finished first by time, then on the road by the game's place
        // (none = last), the unfinished at the end last of all.
        v.sort_by(|(a, pa), (b, pb)| {
            let key = |s: &Standing, p: u32| {
                (s.dnf, s.time.is_none(), s.time.unwrap_or(0.0), if p == 0 { u32::MAX } else { p }, s.car)
            };
            key(a, *pa).partial_cmp(&key(b, *pb)).unwrap_or(std::cmp::Ordering::Equal)
        });
        v.into_iter()
            .enumerate()
            .map(|(k, (mut s, _))| {
                if started && !s.dnf {
                    s.place = k as u32 + 1;
                }
                s
            })
            .collect()
    }

    /// The problems (stuck, wrong way, crashes) for the race's validation,
    /// one per car and kind at its first checkpoint.
    pub fn problems(&self) -> Vec<Problem> {
        let mut out: Vec<Problem> = Vec::new();
        for e in self.events.iter().filter(|e| e.kind.is_problem()) {
            if let Some(p) = out.iter_mut().find(|p| p.kind == e.kind && p.checkpoint == e.checkpoint) {
                if !p.cars.contains(&e.car) {
                    p.cars.push(e.car);
                }
                continue;
            }
            let at = self
                .frames
                .iter()
                .find(|f| f.tick >= e.tick)
                .and_then(|f| f.cars.iter().find(|c| c.id == e.car))
                .map(|c| c.position);
            out.push(Problem { kind: e.kind, checkpoint: e.checkpoint, cars: vec![e.car], at, time: e.time });
        }
        out
    }
}

/// One row of the results table.
#[derive(Clone, Debug, PartialEq)]
pub struct Standing {
    pub car: u32,
    pub place: u32,
    pub time: Option<f32>,
    pub best: Option<f32>,
    pub laps: Vec<f32>,
    /// Did not finish: the run ended without this racer's finish.
    pub dnf: bool,
}

/// A problem the simulation found, for the Validation list.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub kind: EventKind,
    /// The checkpoint the cars were heading for (1-based; 0 none).
    pub checkpoint: u32,
    pub cars: Vec<u32>,
    pub at: Option<Vec3>,
    pub time: f32,
}

impl Problem {
    /// "AI got stuck before checkpoint 3 (2 cars)".
    pub fn text(&self) -> String {
        let what = match self.kind {
            EventKind::Stuck => "AI got stuck",
            EventKind::WrongWay => "AI drove the wrong way",
            _ => "AI crashed",
        };
        let at = if self.checkpoint > 0 { format!(" before checkpoint {}", self.checkpoint) } else { String::new() };
        let n = if self.cars.len() > 1 { format!(" ({} cars)", self.cars.len()) } else { String::new() };
        format!("Simulation: {what}{at}{n}")
    }
}

/// The run's state as the panel shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    /// Nothing started.
    Idle,
    /// The game started; no record yet.
    Starting,
    /// Records arriving.
    Running,
    /// The stream ended (its END line or the game's exit).
    Done,
    /// The game ended with no stream, or the stream says why it failed.
    Failed(String),
}

/// The playback over a recording.
#[derive(Clone, Debug)]
pub struct Playback {
    pub phase: Phase,
    pub playing: bool,
    /// Index into [`SPEEDS`].
    pub speed: usize,
    /// The time shown (stream seconds).
    pub time: f32,
    /// The car the 3D camera follows.
    pub follow: Option<u32>,
    /// Hold at the newest frame while the run streams (live).
    pub live: bool,
    /// The time shown when the playback last started, was moved or changed
    /// speed, the speed then, and the real seconds played since (summed in
    /// f64): the time shown is `anchor + played × speed`, so it is the same
    /// after the same real time at any display rate.
    anchor: f32,
    anchor_speed: usize,
    played: f64,
    /// The time as the last tick left it (moved since: anchored again).
    ticked: f32,
}

impl Default for Playback {
    fn default() -> Playback {
        Playback {
            phase: Phase::Idle,
            playing: true,
            speed: 2,
            time: f32::NEG_INFINITY,
            follow: None,
            live: true,
            anchor: f32::NEG_INFINITY,
            anchor_speed: 2,
            played: 0.0,
            ticked: f32::NEG_INFINITY,
        }
    }
}

impl Playback {
    pub fn speed(&self) -> f32 {
        SPEEDS[self.speed.min(SPEEDS.len() - 1)]
    }

    /// A run started: the playback from its first frame.
    pub fn start(&mut self) {
        *self = Playback { phase: Phase::Starting, follow: self.follow, speed: self.speed, ..Playback::default() };
    }

    /// Records arrived / the run ended: the phase follows.
    pub fn observe(&mut self, rec: &Recording, child_running: bool) {
        self.phase = match (&self.phase, rec.end.as_deref()) {
            (Phase::Idle, _) => Phase::Idle,
            (_, Some(why)) if why.starts_with("error") => {
                Phase::Failed(why.trim_start_matches("error").trim().to_string())
            }
            (_, Some(_)) => Phase::Done,
            (_, None) if !child_running && rec.frames.is_empty() => {
                Phase::Failed("the game ended without a stream".into())
            }
            (_, None) if !child_running => Phase::Done,
            (_, None) if rec.header.is_some() => Phase::Running,
            (p, None) => p.clone(),
        };
        if let Some((lo, _)) = rec.span()
            && !self.time.is_finite()
        {
            self.time = lo;
        }
    }

    /// Advance by `dt` real seconds over `rec`: stops at the newest frame
    /// (waiting there while the run streams).
    pub fn tick(&mut self, rec: &Recording, dt: f32) {
        let Some((lo, hi)) = rec.span() else { return };
        if !self.time.is_finite() {
            self.time = lo;
        }
        // Moved (scrub, step, a jump), paused or another speed: play on
        // from here.
        if !self.playing || self.time != self.ticked || self.speed != self.anchor_speed {
            self.anchor = self.time;
            self.anchor_speed = self.speed;
            self.played = 0.0;
        }
        if self.playing {
            self.played += f64::from(dt);
            self.time = (f64::from(self.anchor) + self.played * f64::from(self.speed())) as f32;
        }
        if self.time >= hi {
            // Held at the newest frame: play on from there.
            self.time = hi;
            self.anchor = hi;
            self.played = 0.0;
            if self.phase == Phase::Done {
                self.playing = false;
            }
        }
        self.time = self.time.clamp(lo, hi);
        self.ticked = self.time;
    }

    /// Step `n` recorded frames on (back when negative) from the time
    /// shown, landing on a frame's time exactly (pauses).
    pub fn step(&mut self, rec: &Recording, n: i32) {
        let f = &rec.frames;
        if f.is_empty() {
            return;
        }
        // The frame at or before the time shown (-1: before the first).
        let k = f.partition_point(|x| x.time <= self.time) as i64 - 1;
        let on = k >= 0 && f[k as usize].time == self.time;
        let to = if n < 0 && !on { k + i64::from(n) + 1 } else { k + i64::from(n) };
        self.time = f[to.clamp(0, f.len() as i64 - 1) as usize].time;
        self.playing = false;
    }

    /// Scrub to the recorded frame nearest `time` (pauses).
    pub fn seek_frame(&mut self, rec: &Recording, time: f32) {
        if let Some(t) = rec.nearest_frame(time) {
            self.time = t;
            self.playing = false;
        }
    }

    /// Scrub to `time` (pauses).
    pub fn seek(&mut self, rec: &Recording, time: f32) {
        if let Some((lo, hi)) = rec.span() {
            self.time = time.clamp(lo, hi);
            self.playing = false;
        }
    }

    pub fn toggle(&mut self, rec: &Recording) {
        // Play again from the start once at the end of a finished run.
        if !self.playing && self.phase == Phase::Done && rec.span().is_some_and(|(_, hi)| self.time >= hi) {
            self.time = rec.span().map_or(0.0, |s| s.0);
        }
        self.playing = !self.playing;
    }

    pub fn faster(&mut self) {
        self.speed = (self.speed + 1).min(SPEEDS.len() - 1);
    }

    pub fn slower(&mut self) {
        self.speed = self.speed.saturating_sub(1);
    }

    /// The cars at the time shown, between the ticks ([`Recording::cars_at`]).
    pub fn cars(&self, rec: &Recording) -> Vec<CarSample> {
        rec.cars_at(self.time)
    }
}

#[cfg(test)]
mod tests {
    use glam::Quat;

    use crate::runs::format::{CarInfo, SimMode};
    use super::*;

    fn car(id: u32, kind: CarKind, x: f32, place: u32) -> CarSample {
        CarSample {
            id,
            kind,
            position: Vec3::new(x, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            speed: 10.0,
            lap: 0,
            checkpoint: 0,
            place,
            wheels: None,
            drive: None,
        }
    }

    fn race(frames: usize, end: Option<&str>) -> Recording {
        let mut r = Recording::default();
        r.push(Line::Header(Header { version: 1, mode: SimMode::Race, race: "r".into(), ..Default::default() }));
        r.push(Line::Car(CarInfo { id: 0, kind: CarKind::Player, name: String::new() }));
        r.push(Line::Car(CarInfo { id: 1, kind: CarKind::Racer, name: "Rival".into() }));
        for k in 0..frames {
            let t = k as f32 / 60.0 - 1.0;
            r.push(Line::Frame(Frame {
                tick: k as u64,
                time: t,
                cars: vec![car(0, CarKind::Player, k as f32, 2), car(1, CarKind::Racer, k as f32 + 1.0, 1)],
            }));
        }
        if let Some(e) = end {
            r.push(Line::End(e.into()));
        }
        r
    }

    #[test]
    fn the_phase_follows_the_run() {
        let mut p = Playback::default();
        assert_eq!(p.phase, Phase::Idle);
        p.start();
        assert_eq!(p.phase, Phase::Starting);
        p.observe(&Recording::default(), true);
        assert_eq!(p.phase, Phase::Starting, "no record yet");
        p.observe(&race(10, None), true);
        assert_eq!(p.phase, Phase::Running);
        assert_eq!(p.time, -1.0, "from the first frame");
        p.observe(&race(10, Some("finish")), true);
        assert_eq!(p.phase, Phase::Done);
        p.start();
        p.observe(&race(10, Some("error no race x")), false);
        assert_eq!(p.phase, Phase::Failed("no race x".into()));
        p.start();
        p.observe(&Recording::default(), false);
        assert!(matches!(p.phase, Phase::Failed(_)), "the game ended with nothing");
        p.start();
        p.observe(&race(10, None), false);
        assert_eq!(p.phase, Phase::Done, "the game ended mid-stream: what came is kept");
    }

    #[test]
    fn cars_are_drawn_between_ticks_with_their_looks() {
        let mut rec = race(0, None);
        let look = |id: u32, model: &str| CarLook { id, model: model.into(), camber_bits: 0, parts: vec![(0x3f, 7)] };
        rec.push(Line::Look(look(0, "RX8")));
        rec.push(Line::Look(look(1, "COROLLA")));
        for k in 0..4 {
            let t = k as f32 / 60.0;
            rec.push(Line::Frame(Frame { tick: k, time: t, cars: vec![car(0, CarKind::Player, k as f32, 1), car(1, CarKind::Racer, 0.0, 2)] }));
            if k == 1 {
                // The slot takes another car after tick 1.
                rec.push(Line::Look(look(1, "SUPRA")));
            }
        }
        // A quarter of the way from tick 1 to tick 2.
        let cars = rec.cars_at(1.25 / 60.0);
        assert!((cars[0].position.x - 1.25).abs() < 1e-4, "{}", cars[0].position.x);
        // At a tick exactly, and past the last one, that tick.
        assert_eq!(rec.cars_at(2.0 / 60.0)[0].position.x, 2.0);
        assert_eq!(rec.cars_at(9.0)[0].position.x, 3.0);
        assert_eq!(rec.look_at(0, 0.0).unwrap().model, "RX8");
        assert_eq!(rec.look_at(1, 1.0 / 60.0).unwrap().model, "COROLLA");
        assert_eq!(rec.look_at(1, 1.5 / 60.0).unwrap().model, "SUPRA");
        assert_eq!(rec.look_at(1, -5.0).unwrap().model, "COROLLA", "before the first frame: the first look");
        assert!(rec.look_at(7, 0.0).is_none());
        // The clock set back: the looks hold from the new start.
        rec.push(Line::Frame(Frame { tick: 9, time: -3.0, cars: Vec::new() }));
        assert_eq!(rec.look_at(1, -2.0).unwrap().model, "SUPRA");
    }

    #[test]
    fn playback_plays_scrubs_and_waits_at_the_newest_frame() {
        let rec = race(121, None);
        let mut p = Playback::default();
        p.start();
        p.observe(&rec, true);
        assert_eq!(p.speed(), 1.0);
        p.tick(&rec, 0.5);
        assert!((p.time - -0.5).abs() < 1e-5);
        p.faster();
        p.faster();
        assert_eq!(p.speed(), 4.0);
        p.tick(&rec, 1.0);
        assert_eq!(p.time, 1.0, "held at the newest frame while it streams");
        assert!(p.playing, "still playing: more comes");
        p.seek(&rec, 0.0);
        assert!(!p.playing && p.time == 0.0);
        assert_eq!(p.cars(&rec)[0].position.x, 60.0);
        p.seek(&rec, 99.0);
        assert_eq!(p.time, 1.0, "scrub clamps to the recording");
        for _ in 0..10 {
            p.slower();
        }
        assert_eq!(p.speed(), 0.25);
        for _ in 0..10 {
            p.faster();
        }
        assert_eq!(p.speed(), 4.0);
        // A finished run stops at its end; play again restarts it.
        let rec = race(121, Some("finish"));
        p.observe(&rec, false);
        p.playing = true;
        p.tick(&rec, 10.0);
        assert!(!p.playing);
        p.toggle(&rec);
        assert!(p.playing && p.time == -1.0);
    }

    #[test]
    fn standings_problems_and_traces() {
        let mut rec = race(121, None);
        let st = rec.standings(0.5);
        assert_eq!(st.iter().map(|s| s.car).collect::<Vec<_>>(), vec![1, 0], "by place");
        for (car, cp) in [(0, 3), (1, 3), (1, 3), (0, 5)] {
            rec.push(Line::Event(Event {
                tick: 30,
                time: -0.5,
                kind: EventKind::Stuck,
                car,
                checkpoint: cp,
                value: 4.0,
            }));
        }
        let p = rec.problems();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].text(), "Simulation: AI got stuck before checkpoint 3 (2 cars)");
        assert_eq!(p[0].at, Some(Vec3::new(30.0, 0.0, 0.0)));
        assert_eq!(p[1].text(), "Simulation: AI got stuck before checkpoint 5");
        rec.push(Line::Result(ResultRow {
            car: 0,
            place: 1,
            time: Some(80.0),
            best: Some(40.0),
            laps: vec![40.0, 40.0],
        }));
        rec.push(Line::Result(ResultRow { car: 1, place: 0, time: None, best: None, laps: Vec::new() }));
        rec.push(Line::End("finish".into()));
        let st = rec.standings(1.0);
        assert_eq!((st[0].car, st[0].place, st[1].place), (0, 1, 0), "the final results, unfinished last");
        assert_eq!(rec.name(1), "Rival");
        assert_eq!(rec.name(0), "Your slot (AI)");
        assert!(st[1].dnf && st[1].time.is_none(), "did not finish");
        let t = rec.trace(0, 0.0, 10);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].len(), 7);
        assert_eq!(rec.racers(), vec![0, 1]);
    }

    #[test]
    fn results_sort_by_finish_and_put_the_unfinished_last() {
        let ev = |time: f32, kind, car| Event { tick: 0, time, kind, car, checkpoint: 0, value: time };
        let row = |car, place, time: Option<f32>| ResultRow { car, place, time, best: time, laps: Vec::new() };
        // Before the green: grid order, nobody placed (the player's slot is
        // not "first" yet).
        let mut rec = race(121, None);
        rec.push(Line::Car(CarInfo { id: 0, kind: CarKind::Player, name: "PEUGOT".into() }));
        let st = rec.standings(-0.5);
        assert_eq!(st.iter().map(|s| (s.car, s.place)).collect::<Vec<_>>(), vec![(1, 0), (0, 0)]);
        assert_eq!(rec.name(0), "Your slot (AI)", "the slot's label, not its car");
        // A finished racer goes ahead of one still on the road, whatever
        // the road place says.
        rec.push(Line::Event(ev(0.5, EventKind::Finish, 0)));
        let st = rec.standings(1.0);
        assert_eq!(st.iter().map(|s| (s.car, s.place)).collect::<Vec<_>>(), vec![(0, 1), (1, 2)]);
        assert!(st.iter().all(|s| !s.dnf), "the run goes on");
        // The run ended without result lines (time limit): no finish = DNF.
        rec.push(Line::End("limit".into()));
        let st = rec.standings(1.0);
        assert_eq!((st[0].car, st[0].place, st[0].dnf), (0, 1, false));
        assert_eq!((st[1].car, st[1].place, st[1].dnf), (1, 0, true));
        // Result lines in the stream's order (by car) come out by place,
        // the unfinished last; before the end the race as it stood shows.
        let mut rec = race(121, Some("finish"));
        for (car, place, time) in [(0, 0, None), (1, 2, Some(14.7)), (2, 3, Some(15.3)), (3, 1, Some(14.1))] {
            rec.push(Line::Result(row(car, place, time)));
        }
        let st = rec.standings(1.0);
        assert_eq!(st.iter().map(|s| s.car).collect::<Vec<_>>(), vec![3, 1, 2, 0]);
        assert_eq!(st.iter().map(|s| s.dnf).collect::<Vec<_>>(), vec![false, false, false, true]);
        assert_eq!(rec.standings(0.5).len(), 2, "mid-run: the frame's racers");
    }

    #[test]
    fn a_stream_reads_line_by_line() {
        let text: String = race(5, Some("finish"))
            .frames
            .iter()
            .map(|f| Line::Frame(f.clone()).write() + "\n")
            .chain(["garbage here\n".to_string(), "F 1 nope\n".into(), "END quit\n".into()])
            .collect();
        let r = Recording::read(&text);
        assert_eq!(r.frames.len(), 5);
        assert_eq!(r.bad.len(), 1);
        assert_eq!(r.end.as_deref(), Some("quit"));
        // A clock set back drops what came before.
        let mut r = race(5, None);
        r.push(Line::Frame(Frame { tick: 9, time: -10.0, cars: Vec::new() }));
        assert_eq!(r.frames.len(), 1);
        assert_eq!(r.span(), Some((-10.0, -10.0)));
    }
}

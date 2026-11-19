//! A simulation as the editor keeps it: every record the game streamed,
//! and the playback over it (play / pause, speed, scrub, follow). Plain
//! data, so the state machine is tested without a window or a game.

use std::collections::HashMap;

use glam::Vec3;

use super::format::{CarInfo, CarKind, CarSample, Event, EventKind, Frame, Header, Line, ResultRow};

/// The playback speeds offered.
pub const SPEEDS: [f32; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];

/// Every record of one run, in order.
#[derive(Clone, Debug, Default)]
pub struct Recording {
    pub header: Option<Header>,
    pub cars: HashMap<u32, CarInfo>,
    pub frames: Vec<Frame>,
    pub events: Vec<Event>,
    pub results: Vec<ResultRow>,
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
            Line::Frame(f) => {
                // The race clock set back (the start scene sets it once the
                // grid is loaded): the frames before it are dropped, so the
                // times stay in order.
                if self.frames.last().is_some_and(|l| f.time < l.time) {
                    self.frames.clear();
                }
                self.frames.push(f);
            }
            Line::Event(e) => self.events.push(e),
            Line::Result(r) => self.results.push(r),
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

    /// The car's name ("Racer 3" when the stream gave none).
    pub fn name(&self, id: u32) -> String {
        match self.cars.get(&id) {
            Some(c) if !c.name.is_empty() => match c.kind {
                CarKind::Player => format!("{} (player slot)", c.name),
                _ => c.name.clone(),
            },
            Some(c) => match c.kind {
                CarKind::Player => "Player slot".into(),
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

    /// The results table: the stream's result lines when it ended, else
    /// the race as it stands at `time` (place, laps done, finish time).
    pub fn standings(&self, time: f32) -> Vec<Standing> {
        let laps_done = |id: u32| -> Vec<f32> {
            self.events
                .iter()
                .filter(|e| e.car == id && e.kind == EventKind::Lap && e.time <= time)
                .map(|e| e.value)
                .collect()
        };
        if !self.results.is_empty() && self.end.is_some() && self.span().is_some_and(|(_, end)| time >= end) {
            let mut v: Vec<Standing> = self
                .results
                .iter()
                .map(|r| Standing { car: r.car, place: r.place, time: r.time, best: r.best, laps: r.laps.clone() })
                .collect();
            v.sort_by_key(|s| (if s.place == 0 { u32::MAX } else { s.place }, s.car));
            return v;
        }
        let Some(f) = self.frame_at(time) else { return Vec::new() };
        let mut v: Vec<Standing> = f
            .cars
            .iter()
            .filter(|c| c.kind.races())
            .map(|c| {
                let laps = laps_done(c.id);
                let finish =
                    self.events.iter().find(|e| e.car == c.id && e.kind == EventKind::Finish && e.time <= time);
                Standing {
                    car: c.id,
                    place: c.place,
                    time: finish.map(|e| e.value),
                    best: laps.iter().copied().reduce(f32::min),
                    laps,
                }
            })
            .collect();
        v.sort_by_key(|s| (if s.place == 0 { u32::MAX } else { s.place }, s.car));
        v
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
}

impl Default for Playback {
    fn default() -> Playback {
        Playback { phase: Phase::Idle, playing: true, speed: 2, time: f32::NEG_INFINITY, follow: None, live: true }
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
        if self.playing {
            self.time += dt * self.speed();
        }
        if self.time >= hi {
            self.time = hi;
            if self.phase == Phase::Done {
                self.playing = false;
            }
        }
        self.time = self.time.clamp(lo, hi);
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

    /// The cars at the time shown.
    pub fn cars<'r>(&self, rec: &'r Recording) -> &'r [CarSample] {
        rec.frame_at(self.time).map_or(&[], |f| f.cars.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use glam::Quat;

    use super::super::format::{CarInfo, SimMode};
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
        assert_eq!(p.speed(), 8.0);
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
        assert_eq!(rec.name(0), "Player slot");
        let t = rec.trace(0, 0.0, 10);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].len(), 7);
        assert_eq!(rec.racers(), vec![0, 1]);
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

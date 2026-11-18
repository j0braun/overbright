//! The simulation stream: what `game --simulate` / `--simulate-world`
//! writes and the editor's Simulate tab reads. One line per record, plain
//! text (UTF-8, `\n`), so a stream can be read in a terminal and diffed:
//!
//! ```text
//! OBSIM 1 mode=race seed=7 race=dock_sprint track=4014 laps=1 checkpoints=5
//! C 0 P Player car name             a car's first appearance: id, kind, name
//! F 12 -2.483 0 P -310.25 88.12 4.50 0.0000 0.0000 0.7071 0.7071 0.00 0 0 1|1 R …
//!                                   a tick: tick, time, then per car (`|`):
//!                                   id kind x y z qx qy qz qw speed lap checkpoint place
//! E 640 8.217 checkpoint 2 3 0.000  an event: tick time kind car checkpoint value
//! R 0 1 83.410 41.200 42.210,41.200 a result: car place time best laps
//! END finish                        the end and why
//! ```
//!
//! Times are seconds on the race clock (negative before the green) or, in
//! world mode, since the start. Positions are world metres, the rotation a
//! unit quaternion, speed m/s. `lap` counts from 0 (the lap under way),
//! `checkpoint` is the number of the race's checkpoints passed in that lap,
//! `place` 1-based (0 outside a race). Readers skip records they do not
//! know, so a newer game's stream still reads; the version in the first
//! line changes only when a known record changes meaning.

use std::fmt::Write as _;

use glam::{Quat, Vec3};

/// The stream's version (the header's second word).
pub const VERSION: u32 = 1;

/// What runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SimMode {
    /// A custom race, every car (the player's slot too) driven by the AI.
    #[default]
    Race,
    /// Free roam from a spot: traffic and street racers, the player's car
    /// parked.
    World,
}

/// The first line.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Header {
    pub version: u32,
    pub mode: SimMode,
    pub seed: u64,
    /// The custom race's id (empty in world mode).
    pub race: String,
    pub track: u16,
    pub laps: u32,
    pub checkpoints: u32,
}

/// A car's role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CarKind {
    /// The player's slot (AI-driven in a simulation).
    Player,
    /// A race opponent.
    Racer,
    Traffic,
    /// A street racer met in free roam.
    Encounter,
}

impl CarKind {
    pub fn letter(self) -> char {
        match self {
            CarKind::Player => 'P',
            CarKind::Racer => 'R',
            CarKind::Traffic => 'T',
            CarKind::Encounter => 'X',
        }
    }

    pub fn from_letter(s: &str) -> Option<CarKind> {
        Some(match s {
            "P" => CarKind::Player,
            "R" => CarKind::Racer,
            "T" => CarKind::Traffic,
            "X" => CarKind::Encounter,
            _ => return None,
        })
    }

    /// A racer of the race (the player's slot or an opponent).
    pub fn races(self) -> bool {
        matches!(self, CarKind::Player | CarKind::Racer)
    }
}

/// A car's first appearance.
#[derive(Clone, Debug, PartialEq)]
pub struct CarInfo {
    pub id: u32,
    pub kind: CarKind,
    pub name: String,
}

/// One car at one tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarSample {
    pub id: u32,
    pub kind: CarKind,
    pub position: Vec3,
    pub rotation: Quat,
    pub speed: f32,
    pub lap: i32,
    pub checkpoint: u32,
    pub place: u32,
}

impl CarSample {
    /// The heading around +z from +x, radians (the car's forward is +x).
    pub fn heading(&self) -> f32 {
        let f = self.rotation * Vec3::X;
        f.y.atan2(f.x)
    }
}

/// Every car at one tick.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Frame {
    pub tick: u64,
    pub time: f32,
    pub cars: Vec<CarSample>,
}

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// The green light.
    Start,
    /// A checkpoint passed (`checkpoint` its 1-based number).
    Checkpoint,
    /// A lap completed (`value` its time).
    Lap,
    /// The racer finished (`value` its time).
    Finish,
    /// A hard hit: speed lost in one tick (`value` m/s lost).
    Crash,
    /// Barely moving for a while after the green (`value` seconds).
    Stuck,
    /// Driving against the race's direction for a while.
    WrongWay,
}

impl EventKind {
    pub const ALL: [EventKind; 7] = [
        EventKind::Start,
        EventKind::Checkpoint,
        EventKind::Lap,
        EventKind::Finish,
        EventKind::Crash,
        EventKind::Stuck,
        EventKind::WrongWay,
    ];

    pub fn word(self) -> &'static str {
        match self {
            EventKind::Start => "start",
            EventKind::Checkpoint => "checkpoint",
            EventKind::Lap => "lap",
            EventKind::Finish => "finish",
            EventKind::Crash => "crash",
            EventKind::Stuck => "stuck",
            EventKind::WrongWay => "wrong-way",
        }
    }

    pub fn from_word(s: &str) -> Option<EventKind> {
        EventKind::ALL.into_iter().find(|k| k.word() == s)
    }

    /// A problem the race's validation reports.
    pub fn is_problem(self) -> bool {
        matches!(self, EventKind::Crash | EventKind::Stuck | EventKind::WrongWay)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub tick: u64,
    pub time: f32,
    pub kind: EventKind,
    pub car: u32,
    /// The checkpoint passed, or for a problem the next one to reach
    /// (1-based; 0: none).
    pub checkpoint: u32,
    pub value: f32,
}

/// A racer's result at the end.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultRow {
    pub car: u32,
    /// 1-based; 0: did not finish.
    pub place: u32,
    pub time: Option<f32>,
    pub best: Option<f32>,
    pub laps: Vec<f32>,
}

/// One record.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    Header(Header),
    Car(CarInfo),
    Frame(Frame),
    Event(Event),
    Result(ResultRow),
    /// The end: `finish`, `quit`, `limit` or `error <why>`.
    End(String),
}

fn opt(v: Option<f32>) -> String {
    v.map_or_else(|| "-".to_string(), |t| format!("{t:.3}"))
}

fn parse_opt(s: &str) -> Result<Option<f32>, String> {
    if s == "-" { Ok(None) } else { s.parse().map(Some).map_err(|_| format!("bad number `{s}`")) }
}

/// One-line names: no line breaks, no `|`.
fn clean(name: &str) -> String {
    name.chars().map(|c| if c.is_control() || c == '|' { ' ' } else { c }).collect::<String>().trim().to_string()
}

impl Line {
    /// The record as one line, without its `\n`.
    pub fn write(&self) -> String {
        let mut o = String::new();
        match self {
            Line::Header(h) => {
                let mode = match h.mode {
                    SimMode::Race => "race",
                    SimMode::World => "world",
                };
                let race = if h.race.is_empty() { "-" } else { h.race.as_str() };
                let _ = write!(
                    o,
                    "OBSIM {} mode={mode} seed={} race={race} track={} laps={} checkpoints={}",
                    h.version, h.seed, h.track, h.laps, h.checkpoints
                );
            }
            Line::Car(c) => {
                let _ = write!(o, "C {} {} {}", c.id, c.kind.letter(), clean(&c.name));
            }
            Line::Frame(f) => {
                let _ = write!(o, "F {} {:.3}", f.tick, f.time);
                for (i, c) in f.cars.iter().enumerate() {
                    let (p, q) = (c.position, c.rotation);
                    let _ = write!(
                        o,
                        "{}{} {} {:.2} {:.2} {:.2} {:.4} {:.4} {:.4} {:.4} {:.2} {} {} {}",
                        if i == 0 { " " } else { "|" },
                        c.id,
                        c.kind.letter(),
                        p.x,
                        p.y,
                        p.z,
                        q.x,
                        q.y,
                        q.z,
                        q.w,
                        c.speed,
                        c.lap,
                        c.checkpoint,
                        c.place
                    );
                }
            }
            Line::Event(e) => {
                let _ =
                    write!(o, "E {} {:.3} {} {} {} {:.3}", e.tick, e.time, e.kind.word(), e.car, e.checkpoint, e.value);
            }
            Line::Result(r) => {
                let laps: Vec<String> = r.laps.iter().map(|t| format!("{t:.3}")).collect();
                let laps = if laps.is_empty() { "-".to_string() } else { laps.join(",") };
                let _ = write!(o, "R {} {} {} {} {laps}", r.car, r.place, opt(r.time), opt(r.best));
            }
            Line::End(why) => {
                let _ = write!(o, "END {}", clean(why));
            }
        }
        o
    }

    /// One line read back. `Ok(None)`: a record this reader does not know
    /// (skipped); `Err`: a known record that is malformed.
    pub fn parse(line: &str) -> Result<Option<Line>, String> {
        let line = line.trim_end_matches(['\r', '\n']);
        let (tag, rest) = line.split_once(' ').unwrap_or((line, ""));
        let num = |s: Option<&str>, what: &str| -> Result<f32, String> {
            let s = s.ok_or_else(|| format!("{tag}: no {what}"))?;
            s.parse::<f32>().map_err(|_| format!("{tag}: bad {what} `{s}`"))
        };
        let int = |s: Option<&str>, what: &str| -> Result<i64, String> {
            let s = s.ok_or_else(|| format!("{tag}: no {what}"))?;
            s.parse::<i64>().map_err(|_| format!("{tag}: bad {what} `{s}`"))
        };
        Ok(Some(match tag {
            "OBSIM" => {
                let mut w = rest.split_whitespace();
                let version = int(w.next(), "version")? as u32;
                let mut h = Header { version, ..Default::default() };
                for (k, v) in w.filter_map(|kv| kv.split_once('=')) {
                    match k {
                        "mode" => h.mode = if v == "world" { SimMode::World } else { SimMode::Race },
                        "seed" => h.seed = v.parse().map_err(|_| format!("OBSIM: bad seed `{v}`"))?,
                        "race" => h.race = if v == "-" { String::new() } else { v.to_string() },
                        "track" => h.track = v.parse().map_err(|_| format!("OBSIM: bad track `{v}`"))?,
                        "laps" => h.laps = v.parse().map_err(|_| format!("OBSIM: bad laps `{v}`"))?,
                        "checkpoints" => h.checkpoints = v.parse().map_err(|_| format!("OBSIM: bad count `{v}`"))?,
                        _ => {}
                    }
                }
                Line::Header(h)
            }
            "C" => {
                let mut w = rest.splitn(3, ' ');
                let id = int(w.next(), "id")? as u32;
                let kind = w.next().and_then(CarKind::from_letter).ok_or("C: bad kind")?;
                Line::Car(CarInfo { id, kind, name: w.next().unwrap_or("").to_string() })
            }
            "F" => {
                let mut w = rest.splitn(3, ' ');
                let tick = int(w.next(), "tick")? as u64;
                let time = num(w.next(), "time")?;
                let mut cars = Vec::new();
                if let Some(list) = w.next().filter(|l| !l.trim().is_empty()) {
                    for car in list.split('|') {
                        let v: Vec<&str> = car.split_whitespace().collect();
                        if v.len() < 13 {
                            return Err(format!("F: a car with {} fields", v.len()));
                        }
                        let f = |i: usize, what: &str| num(Some(v[i]), what);
                        cars.push(CarSample {
                            id: int(Some(v[0]), "id")? as u32,
                            kind: CarKind::from_letter(v[1]).ok_or("F: bad kind")?,
                            position: Vec3::new(f(2, "x")?, f(3, "y")?, f(4, "z")?),
                            rotation: Quat::from_xyzw(f(5, "qx")?, f(6, "qy")?, f(7, "qz")?, f(8, "qw")?),
                            speed: f(9, "speed")?,
                            lap: int(Some(v[10]), "lap")? as i32,
                            checkpoint: int(Some(v[11]), "checkpoint")? as u32,
                            place: int(Some(v[12]), "place")? as u32,
                        });
                    }
                }
                Line::Frame(Frame { tick, time, cars })
            }
            "E" => {
                let mut w = rest.split_whitespace();
                let tick = int(w.next(), "tick")? as u64;
                let time = num(w.next(), "time")?;
                let Some(kind) = w.next().and_then(EventKind::from_word) else { return Ok(None) };
                Line::Event(Event {
                    tick,
                    time,
                    kind,
                    car: int(w.next(), "car")? as u32,
                    checkpoint: int(w.next(), "checkpoint")? as u32,
                    value: num(w.next(), "value")?,
                })
            }
            "R" => {
                let mut w = rest.split_whitespace();
                let car = int(w.next(), "car")? as u32;
                let place = int(w.next(), "place")? as u32;
                let time = parse_opt(w.next().ok_or("R: no time")?)?;
                let best = parse_opt(w.next().ok_or("R: no best")?)?;
                let laps = match w.next() {
                    None | Some("-") => Vec::new(),
                    Some(l) => l
                        .split(',')
                        .map(|t| t.parse().map_err(|_| format!("R: bad lap `{t}`")))
                        .collect::<Result<_, _>>()?,
                };
                Line::Result(ResultRow { car, place, time, best, laps })
            }
            "END" => Line::End(rest.trim().to_string()),
            _ => return Ok(None),
        }))
    }
}

/// The values a written line keeps (positions to 1 cm, rotations to 1e-4,
/// times to 1 ms): what a sample reads back as.
pub fn quantize(l: &Line) -> Line {
    match Line::parse(&l.write()) {
        Ok(Some(q)) => q,
        _ => l.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: u32, kind: CarKind, x: f32) -> CarSample {
        CarSample {
            id,
            kind,
            position: Vec3::new(x, -12.5, 3.25),
            rotation: Quat::from_rotation_z(0.7),
            speed: 31.4,
            lap: 1,
            checkpoint: 3,
            place: 2,
        }
    }

    fn all() -> Vec<Line> {
        vec![
            Line::Header(Header {
                version: VERSION,
                mode: SimMode::Race,
                seed: 7,
                race: "dock_sprint".into(),
                track: 4014,
                laps: 2,
                checkpoints: 5,
            }),
            Line::Header(Header { version: VERSION, mode: SimMode::World, seed: 1, ..Default::default() }),
            Line::Car(CarInfo { id: 0, kind: CarKind::Player, name: "Peugeot 206".into() }),
            Line::Car(CarInfo { id: 104, kind: CarKind::Traffic, name: String::new() }),
            Line::Frame(Frame {
                tick: 12,
                time: -2.5,
                cars: vec![sample(0, CarKind::Player, 10.0), sample(3, CarKind::Racer, -310.25)],
            }),
            Line::Frame(Frame { tick: 13, time: 0.0, cars: Vec::new() }),
            Line::Event(Event { tick: 640, time: 8.25, kind: EventKind::Stuck, car: 2, checkpoint: 3, value: 4.0 }),
            Line::Result(ResultRow { car: 0, place: 1, time: Some(83.41), best: Some(41.2), laps: vec![42.21, 41.2] }),
            Line::Result(ResultRow { car: 3, place: 0, time: None, best: None, laps: Vec::new() }),
            Line::End("finish".into()),
        ]
    }

    #[test]
    fn every_record_reads_back_as_written() {
        for l in all() {
            let text = l.write();
            assert!(!text.contains('\n'), "{text}");
            let back = Line::parse(&text).unwrap().unwrap();
            // Written again, the same text: the round trip loses nothing
            // beyond the written precision.
            assert_eq!(back.write(), text);
            match (&l, &back) {
                (Line::Frame(a), Line::Frame(b)) => {
                    for (x, y) in a.cars.iter().zip(&b.cars) {
                        assert!(x.position.distance(y.position) < 0.01);
                        assert!(x.rotation.angle_between(y.rotation) < 1e-3);
                        assert_eq!(
                            (x.id, x.kind, x.lap, x.checkpoint, x.place),
                            (y.id, y.kind, y.lap, y.checkpoint, y.place)
                        );
                    }
                    assert_eq!(a.cars.len(), b.cars.len());
                }
                _ => assert_eq!(&back, &l),
            }
        }
    }

    #[test]
    fn unknown_records_are_skipped_and_bad_ones_refused() {
        assert_eq!(Line::parse("Z something new"), Ok(None));
        assert_eq!(Line::parse("E 1 0.5 jumped 2 0 1.0"), Ok(None), "an event kind from a newer game");
        assert!(Line::parse("F 1 x").is_err());
        assert!(Line::parse("F 1 0.0 3 P 1 2").is_err());
        assert!(Line::parse("C 1 Q name").is_err());
        assert!(Line::parse("R 1 1 abc - -").is_err());
        // Names keep to one line.
        let l = Line::Car(CarInfo { id: 1, kind: CarKind::Racer, name: "a|b\nc".into() });
        assert_eq!(l.write(), "C 1 R a b c");
    }

    #[test]
    fn heading_follows_the_rotation() {
        let s = sample(0, CarKind::Player, 0.0);
        assert!((s.heading() - 0.7).abs() < 1e-5);
    }
}

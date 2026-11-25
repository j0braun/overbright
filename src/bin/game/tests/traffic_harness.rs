//! The traffic parity harness, this game's side (docs/traffic-harness.md):
//! seeded headless sessions that log every traffic car each tick to the
//! harness's common table, the same columns `tools/parity/trafh_native.py`
//! writes for the original, so `tools/parity/trafh.py` computes one set of
//! statistics for both.
//!
//! `TRAFH_OUT=<dir> cargo test --features dev-harness --bin game -- --ignored --exact
//! tests::traffic_harness::traffic_harness --nocapture` with
//! - `TRAFH_SCEN`: `stand` (free roam, the player standing), `bot` (free roam,
//!   the scripted player driving a route through the traffic), `qr:<mode>`
//!   (a Quick Race of that mode, its first track: circuit, sprint, drag;
//!   drift, url and streetx run but have no traffic);
//! - `TRAFH_SECONDS` (120), `TRAFH_SEEDS` (`0` or `0,1,2` or `0-3`: the traffic
//!   seed `NFSU2_TRAFFIC_SEED`, 0 the default stream; a seed over 0 also seeds the shared
//!   generator as a boot of its own unless `TRAFH_BOOT=0`), `TRAFH_EVERY` (2: a row
//!   set every second 1/60 s tick), `TRAFH_AT` (`x,y,z,heading°`, the free-roam
//!   stand and the route's start), `TRAFH_LEVEL` (3: a Quick Race's traffic
//!   level), `TRAFH_DRIVER` (a race's player: `throttle`, `auto` the
//!   autopilot, `drag` the drag bot; default `drag` in a drag, else `throttle`).
//!
//! Writes `<dir>/ours-<scen>-s<seed>.tsv` and, for `bot`, `<dir>/route.tsv`
//! (the route the original's bot drives too).
use crate::*;
use std::collections::HashMap;
use std::io::Write;

/// The common table's columns (docs/traffic-harness.md "The table").
pub(crate) const HEADER: &str =
    "t\tcar\tmodel\tmode\tx\ty\tz\thd\tspeed\tlane\taction\ttarget\tstopped\tactive\tbrake\tev\tnote";

/// One traffic car as the table lists it.
struct Row<'a> {
    car: String,
    model: &'a str,
    /// 0 not placed (parked / paused), 2 TrafficMover, 3 MeloMvr, 4 BeltMover, -1 the player.
    mode: i32,
    pos: Vec3,
    hd: f32,
    speed: f32,
    lane: String,
    action: i32,
    target: String,
    stopped: bool,
    active: bool,
    brake: bool,
    note: String,
}

/// The table writer: the spawn / despawn column by one rule for both games
/// (`S` placed after not being placed or moved over 40 m since its last
/// row, `D` placed before and not now).
struct Table {
    w: std::io::BufWriter<std::fs::File>,
    last: HashMap<String, (bool, Vec2)>,
}

impl Table {
    fn create(path: &std::path::Path, meta: &str) -> Table {
        let mut w = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        writeln!(w, "# trafh 1\tside=ours\t{meta}").unwrap();
        writeln!(w, "{HEADER}").unwrap();
        Table { w, last: HashMap::new() }
    }

    fn row(&mut self, t: f32, r: Row) {
        let placed = r.mode > 0;
        let p = r.pos.truncate();
        let ev = if r.mode < 0 {
            "-"
        } else {
            match self.last.insert(r.car.clone(), (placed, p)) {
                Some((true, _)) if !placed => "D",
                Some((true, q)) if q.distance(p) > 40.0 => "S",
                Some((false, _)) | None if placed => "S",
                _ => "-",
            }
        };
        let _ = writeln!(
            self.w,
            "{t:.3}\t{}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.1}\t{:.3}\t{}\t{}\t{}\t{}\t{}\t{}\t{ev}\t{}",
            r.car,
            r.model,
            r.mode,
            r.pos.x,
            r.pos.y,
            r.pos.z,
            r.hd,
            r.speed,
            r.lane,
            r.action,
            r.target,
            u8::from(r.stopped),
            u8::from(r.active),
            u8::from(r.brake),
            if r.note.is_empty() { "-" } else { &r.note },
        );
    }

    fn traffic(&mut self, t: f32, tr: &Traffic) {
        for (i, c) in tr.cars.iter().enumerate() {
            let l = c.lane.as_ref();
            let (mode, speed) = match (l, &c.mover, &c.belt.belt, &c.state.melo) {
                (None, ..) => (0, c.state.speed()),
                (_, Some(m), _, _) => (2, m.speed),
                (_, _, Some(b), _) => (4, b.speed()),
                (_, _, _, Some(_)) => (3, c.state.speed()),
                _ => (0, c.state.speed()),
            };
            let (lane, action, target) = match l {
                Some(l) => {
                    let lane = l
                        .current(&tr.routes)
                        .map_or("-".into(), |(_, _, b, lane, _)| nfsu2::formats::ai_routes::lane(lane, b).to_string());
                    let act = if l.avoiding() {
                        2
                    } else if l.obeying() {
                        17
                    } else {
                        7
                    };
                    let tgt = l.avoid_target().map_or("-".into(), |t| match t {
                        usize::MAX => "P".to_string(),
                        // Rival k is `usize::MAX - 1 - k` in the traffic's
                        // entries and row `R<k + 1>`.
                        t if t >= usize::MAX - 64 => format!("R{}", usize::MAX - t),
                        t => t.to_string(),
                    });
                    (lane, act, tgt)
                }
                None => ("-".into(), -1, "-".into()),
            };
            self.row(
                t,
                Row {
                    car: i.to_string(),
                    model: &c.car.car_type.name,
                    mode,
                    pos: c.state.position,
                    hd: c.state.heading().to_degrees(),
                    speed,
                    lane,
                    action,
                    target,
                    stopped: c.stopped,
                    active: c.active,
                    brake: c.state.brake_lights(false),
                    note: String::new(),
                },
            );
        }
    }

    /// The encounter cars out (mode −2 rows, `R<k>` as the original's world
    /// list slot): the mover in `lane`, the action type in `action`, its
    /// tracked car in `target` (`P`, `R<k>`, or a traffic car's slot), the
    /// goal in `note` (9 the pursuit, 5 / 6 an outrun's, 1 after it).
    fn racers(&mut self, t: f32, e: &crate::sim::encounters::Encounters) {
        for (i, c) in e.cars.iter().enumerate().filter(|(_, c)| c.active) {
            let a = c.controller.actions.as_ref();
            let goal = match (a.map(|a| a.goal), c.controller.goal) {
                (Some(nfsu2::ai::actions::Goal::OffsetPursuit { .. }), _) => 9,
                (_, Some(g)) => g.type_id(),
                _ => -1,
            };
            let target = a.and_then(|a| a.target.filter(|_| a.kind != nfsu2::ai::actions::Kind::OffsetPursuit)).map_or(
                "-".into(),
                |id| match id {
                    0 => "P".to_string(),
                    id if id >= 1000 => (id - 1000).to_string(),
                    id => format!("R{id}"),
                },
            );
            self.row(
                t,
                Row {
                    car: format!("R{}", i + 1),
                    model: &c.car.car_type.name,
                    mode: -2,
                    pos: c.state.position,
                    hd: c.state.heading().to_degrees(),
                    speed: c.state.speed(),
                    lane: crate::sim::encounters::ai::encounter_mover_kind(c).to_string(),
                    action: if c.unstuck.since.is_some() { 9 } else { a.map_or(-1, |a| a.kind.type_id()) },
                    target,
                    stopped: false,
                    active: true,
                    brake: c.state.brake_lights(false),
                    note: format!("goal{goal}"),
                },
            );
        }
    }

    /// The race's rivals: `R<k>` (k from 1), mode −2, the drag lane record,
    /// the action type, its tracked car, `stopped` = totaled, `active` = a
    /// live contact record (a wall or a car), the note `frac=<road fraction>`
    /// (car +0x8F8), `imp=<the largest summed pushes (record +0x60) of its
    /// live contact records with a racer>`, the lane record's `po=<offset>,pr=<rate>`, the
    /// driven path's `pl=<reach from the car>,pn=<points>`, AvoidAvoidable's `rc=<reach time>` and,
    /// once over, `fin=<race clock>`.
    fn rivals(&mut self, t: f32, rivals: &[Rival], cars: &nfsu2::drive::CarField) {
        let records = cars.records();
        for (k, r) in rivals.iter().enumerate() {
            let me = k as u64 + 1;
            let imp = records
                .iter()
                .filter(|&&(a, b, _, _)| (a == me && b < 1000) || (b == me && a < 1000))
                .map(|&(_, _, _, j)| j)
                .fold(0.0f32, f32::max);
            let f = (r.state.orientation * Vec3::X).truncate();
            let fin = r.progress.finished.map_or(String::new(), |c| format!(",fin={c:.3}"));
            self.row(
                t,
                Row {
                    car: format!("R{}", k + 1),
                    model: &r.car.car_type.name,
                    mode: -2,
                    pos: r.state.position,
                    hd: f.y.atan2(f.x).to_degrees(),
                    speed: r.state.speed(),
                    lane: r.lane.lane.map_or("-".into(), |l| l.to_string()),
                    action: r.actions.kind.type_id(),
                    // As the rows name them: the player, a traffic car's
                    // slot (road car 1000 + slot), rival k + 1 (`R<k + 1>`).
                    target: r.actions.target.map_or("-".into(), |id| match id {
                        0 => "P".to_string(),
                        id if id >= 1000 => (id - 1000).to_string(),
                        id => format!("R{id}"),
                    }),
                    stopped: r.state.totaled,
                    active: !r.state.contacts.is_empty(),
                    brake: r.state.brake_lights(false),
                    note: format!(
                        "frac={:.4},imp={imp:.2},po={:.2},pr={:.3},pl={:.1},pn={},rc={:.2}{fin}",
                        r.progress.locator.fraction,
                        r.actions.plan.offset,
                        r.actions.plan.rate,
                        r.actions.last_path.last().map_or(f32::NAN, |q| (*q - r.state.position).truncate().length()),
                        r.actions.last_path.len(),
                        r.actions.reach
                    ),
                },
            );
        }
    }

    /// The contact records that involve a traffic car (`# rec` lines, as
    /// the original's side writes them): each record when it ends, the
    /// time it was first seen, the kinds of the two cars (P the player, R
    /// an encounter car, T traffic) and its largest summed pushes (+0x60).
    fn contacts(&mut self, t: f32, field: &nfsu2::drive::CarField, live: &mut HashMap<u64, (String, f32, f32)>) {
        let kind = |id: u64| match id {
            0 => 'P',
            id if id < 1000 => 'R',
            _ => 'T',
        };
        let mut seen = std::collections::HashSet::new();
        for (a, b, rec, imp) in field.records() {
            let mut k = [kind(a), kind(b)];
            if !k.contains(&'T') {
                continue;
            }
            seen.insert(rec);
            k.sort_unstable();
            let e = live.entry(rec).or_insert((format!("{}-{}", k[0], k[1]), 0.0, t));
            e.1 = e.1.max(imp);
        }
        let mut gone: Vec<u64> = live.keys().copied().filter(|r| !seen.contains(r)).collect();
        gone.sort_unstable();
        for r in gone {
            if let Some((k, imp, t0)) = live.remove(&r) {
                let _ = writeln!(self.w, "# rec\t{t0:.2}\t{k}\t{imp:.1}");
            }
        }
    }

    fn player(&mut self, t: f32, d: &Drive, note: String) {
        let f = (d.state.orientation * Vec3::X).truncate();
        self.row(
            t,
            Row {
                car: "P".into(),
                model: &d.car.car_type.name,
                mode: -1,
                pos: d.state.position,
                hd: f.y.atan2(f.x).to_degrees(),
                speed: d.state.speed(),
                lane: "-".into(),
                action: -1,
                target: "-".into(),
                stopped: false,
                active: true,
                brake: d.state.brake_lights(false),
                note,
            },
        );
    }
}

/// The route the scripted player drives (free roam): from the forward lane
/// nearest `at` running along `heading`, control point to control point by
/// the traffic's own road walk (junctions by a fixed seed), a point every 5
/// m on the lane's line, `metres` long or to a dead end.
pub(crate) fn bot_route(f: &nfsu2::formats::ai_routes::RouteFile, at: Vec2, heading: f32, metres: f32) -> Vec<Vec2> {
    use nfsu2::formats::ai_routes::{lane, lane_offset};
    let fw = Vec2::new(heading.cos(), heading.sin());
    let start = f
        .records
        .iter()
        .enumerate()
        .flat_map(|(ri, r)| (0..r.points.len()).flat_map(move |ci| [false, true].into_iter().map(move |b| (ri, ci, b))))
        .filter(|&(ri, ci, b)| f.direction(ri, ci, b).dot(fw) > 0.7 && (2..6).any(|l| f.records[ri].lanes[lane(l, b)]))
        .min_by(|a, b| {
            let d = |x: &(usize, usize, bool)| f.records[x.0].points[x.1].position.truncate().distance_squared(at);
            d(a).total_cmp(&d(b))
        });
    let Some(mut s) = start else { return Vec::new() };
    let mut rng = nfsu2::opponents::GameRng::seeded(1);
    let mut out: Vec<Vec2> = Vec::new();
    let mut len = 0.0;
    let mut off = 0.0;
    while len < metres {
        let r = &f.records[s.0];
        if let Some(l) = (2..6).map(|l| lane(l, s.2)).find(|&l| r.lanes[l]) {
            off = lane_offset(&r.points[s.1], l);
        }
        let seg = f.segment(s.0, s.1, s.2).max(0.0);
        let mut d = 0.0;
        loop {
            let p = f.point(s.0, s.1, s.2, d, off).truncate();
            if let Some(q) = out.last() {
                len += q.distance(p);
            }
            if out.last().is_none_or(|q| q.distance(p) > 0.5) {
                out.push(p);
            }
            d += 5.0;
            if d >= seg {
                break;
            }
        }
        match nfsu2::ai::traffic::road_walk(f, s.0, s.1, s.2, 1e-3, &mut rng) {
            Some(n) => s = n,
            None => break,
        }
    }
    out
}

/// The scripted player (docs/traffic-harness.md "The player bot"; the same
/// rules as `tools/parity/trafbot.py` for the original): it drives the
/// route at a cruise speed, closes in behind a car ahead in its line,
/// follows it, then overtakes on the left and returns to the route's line.
/// Keyboard only (binary keys), a decision every 50 ms of game time.
pub(crate) struct Bot {
    route: Vec<Vec2>,
    cum: Vec<f32>,
    idx: usize,
    pub(crate) state: &'static str,
    lead: Option<String>,
    timer: f32,
    stuck: f32,
    reverse: f32,
    next: f32,
    keys: [bool; 4],
    pub(crate) overtakes: u32,
    pub(crate) stucks: u32,
    lead_ds: f32,
}

pub(crate) const BOT_CRUISE: f32 = 22.0;
const BOT_GAP: f32 = 10.0;
const BOT_CLOSE: f32 = 15.0;
const BOT_FOLLOW: f32 = 4.0;
const BOT_PASS_LAT: f32 = -3.5;
const BOT_PASS_MARGIN: f32 = 12.0;
const BOT_PASS_EXTRA: f32 = 6.0;
const BOT_THINK: f32 = 0.05;
const BOT_DEAD: f32 = 0.04;

fn wrap(a: f32) -> f32 {
    a.sin().atan2(a.cos())
}

impl Bot {
    pub(crate) fn new(route: Vec<Vec2>) -> Bot {
        let mut cum = vec![0.0];
        for w in route.windows(2) {
            cum.push(cum.last().unwrap() + w[0].distance(w[1]));
        }
        Bot {
            route,
            cum,
            idx: 0,
            state: "cruise",
            lead: None,
            timer: 0.0,
            stuck: 0.0,
            reverse: 0.0,
            next: 0.0,
            keys: [false; 4],
            overtakes: 0,
            stucks: 0,
            lead_ds: 0.0,
        }
    }

    /// The nearest route piece in [from, to): its index, the distance along
    /// the route, the signed offset (right positive) and the distance.
    fn locate(&self, p: Vec2, from: usize, to: usize) -> (usize, f32, f32, f32) {
        let mut best = (from, 0.0, 0.0, f32::INFINITY);
        for i in from..to.min(self.route.len() - 1) {
            let (a, b) = (self.route[i], self.route[i + 1]);
            let l = a.distance(b).max(1e-3);
            let d = (b - a) / l;
            let t = (p - a).dot(d).clamp(0.0, l);
            let r = p - (a + d * t);
            if r.length() < best.3 {
                best = (i, self.cum[i] + t, r.dot(Vec2::new(d.y, -d.x)), r.length());
            }
        }
        best
    }

    fn at(&self, s: f32) -> (Vec2, Vec2) {
        let i = self.cum.partition_point(|&c| c <= s).clamp(1, self.route.len() - 1) - 1;
        let (a, b) = (self.route[i], self.route[i + 1]);
        let d = (b - a).normalize_or_zero();
        (a + d * (s - self.cum[i]).clamp(0.0, a.distance(b)), d)
    }

    /// The keys (gas, brake, left, right) for this tick from the player's
    /// place, facing and speed and the traffic cars (id, place, velocity).
    pub(crate) fn think(&mut self, t: f32, p: Vec2, fwd: Vec2, v: f32, cars: &[(String, Vec2, Vec2)]) -> [bool; 4] {
        if t < self.next || self.route.len() < 2 {
            return self.keys;
        }
        self.next = t + BOT_THINK;
        let n = self.route.len();
        let (i, s, lat, _) = self.locate(p, self.idx.saturating_sub(5), (self.idx + 40).min(n - 1));
        self.idx = i;
        let total = *self.cum.last().unwrap();
        if s > total - 30.0 {
            self.state = "end";
        }
        let mut cand: Option<(f32, String, f32)> = None;
        let mut lead_seen = None;
        for (id, cp, cv) in cars {
            if cp.distance(p) > 80.0 {
                continue;
            }
            let (ci, cs, clat, cd) = self.locate(*cp, self.idx.saturating_sub(10), (self.idx + 30).min(n - 1));
            if cd > 12.0 {
                continue;
            }
            let dir = (self.route[ci + 1] - self.route[ci]).normalize_or_zero();
            let along = cv.dot(dir);
            if along < -1.0 {
                continue;
            }
            let ds = cs - s;
            if self.lead.as_deref() == Some(id.as_str()) {
                lead_seen = Some((ds, along));
            }
            if ds > 2.0 && ds < 60.0 && (clat - lat).abs() < 2.2 && cand.as_ref().is_none_or(|c| ds < c.0) {
                cand = Some((ds, id.clone(), along));
            }
        }
        let mut lat_t = 0.0;
        let mut vt = BOT_CRUISE;
        match self.state {
            "cruise" => {
                if let Some(c) = &cand {
                    self.state = "follow";
                    self.lead = Some(c.1.clone());
                    self.timer = 0.0;
                }
            }
            "follow" => match &cand {
                Some(c) => {
                    if self.lead.as_deref() != Some(c.1.as_str()) {
                        self.lead = Some(c.1.clone());
                        self.timer = 0.0;
                    }
                    vt = (c.2 + 0.5 * (c.0 - BOT_GAP)).clamp(0.0, BOT_CRUISE);
                    if c.0 < BOT_CLOSE {
                        self.timer += BOT_THINK;
                    }
                    if self.timer >= BOT_FOLLOW {
                        self.state = "overtake";
                        self.timer = 0.0;
                        self.overtakes += 1;
                    }
                }
                None => {
                    self.state = "cruise";
                    self.lead = None;
                }
            },
            "overtake" => {
                lat_t = BOT_PASS_LAT;
                self.timer += BOT_THINK;
                match lead_seen {
                    Some((ds, lv)) if ds > -BOT_PASS_MARGIN && self.timer < 15.0 => {
                        self.lead_ds = ds;
                        vt = (lv + BOT_PASS_EXTRA).max(12.0).min(BOT_CRUISE + 6.0);
                    }
                    _ => {
                        self.state = "return";
                        self.lead = None;
                    }
                }
            }
            "return" => {
                if lat.abs() < 1.0 {
                    self.state = "cruise";
                }
            }
            _ => vt = 0.0,
        }
        let look = (6.0 + 0.6 * v).clamp(8.0, 30.0);
        let (q, d) = self.at(s + look);
        let target = q + Vec2::new(d.y, -d.x) * lat_t;
        let to = target - p;
        let e = wrap(to.y.atan2(to.x) - fwd.y.atan2(fwd.x));
        let (mut gas, mut brake, mut left, mut right) = (v < vt - 0.3, v > vt + 1.5, e > BOT_DEAD, e < -BOT_DEAD);
        if gas && v < 0.5 {
            self.stuck += BOT_THINK;
        } else {
            self.stuck = 0.0;
        }
        if self.stuck > 3.0 {
            self.stuck = 0.0;
            self.reverse = 2.0;
            self.stucks += 1;
        }
        if self.reverse > 0.0 {
            self.reverse -= BOT_THINK;
            (gas, brake) = (false, true);
            std::mem::swap(&mut left, &mut right);
        }
        self.keys = [gas, brake, left, right];
        self.keys
    }

    pub(crate) fn note(&self) -> String {
        format!("{}:{}", self.state, self.lead.as_deref().unwrap_or("-"))
    }
}

/// The drag bot (both games: `tools/parity/trafbot.py`): throttle from the
/// green, up-shifts at fixed times, and a lane change (a tap of left, else
/// right) when a car stands within 50 m ahead in the player's lane.
pub(crate) const DRAG_SHIFTS: [f32; 5] = [1.5, 3.2, 5.3, 8.0, 11.5];

/// `TRAFH_RAM`: the drag bot's lane change into a rival (as trafbot.py's `drag_ram_tap`).
const RAM_EVERY: f32 = 4.0;
const RAM_FROM: f32 = 2.0;

/// `TRAFH_SHOVE`: the race clock of each shove (as trafbot.py's SHOVE_AT).
const SHOVE_AT: [f32; 4] = [8.0, 14.0, 20.0, 26.0];

/// The player's place and velocity for shove `k` beside a rival at `at`
/// moving at `v`, heading `hd` (its velocity's direction): 1.7 m to its right (k even) or left (k
/// odd), the rival's velocity plus 3 m/s towards it (trafbot.shove_place).
fn shove_place(at: Vec3, v: Vec3, hd: f32, k: usize) -> (Vec3, Vec3) {
    let right = Vec3::new(hd.sin(), -hd.cos(), 0.0);
    let side = if k % 2 == 0 { right } else { -right };
    (at + side * 1.7, v - side * 3.0)
}

/// Some(true): tap left, Some(false): right, None: no rival beside (within
/// 6 m along and 1.8–6 m across; the nearest across).
fn drag_ram_tap(p: Vec2, fwd: Vec2, rivals: &[Vec2]) -> Option<bool> {
    let right = Vec2::new(fwd.y, -fwd.x);
    rivals
        .iter()
        .map(|&c| ((c - p).dot(fwd), (c - p).dot(right)))
        .filter(|&(ds, dl)| ds.abs() < 6.0 && (1.8..6.0).contains(&dl.abs()))
        .min_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .map(|(_, dl)| dl < 0.0)
}

pub(crate) fn drag_lane_tap(p: Vec2, fwd: Vec2, cars: &[(String, Vec2, Vec2)]) -> Option<bool> {
    let right = Vec2::new(fwd.y, -fwd.x);
    let mut blocked = false;
    let (mut left_busy, mut right_busy) = (false, false);
    for (_, cp, _) in cars {
        let r = *cp - p;
        let (ds, dl) = (r.dot(fwd), r.dot(right));
        if ds > 0.0 && ds < 50.0 && dl.abs() < 1.8 {
            blocked = true;
        }
        if ds > -10.0 && ds < 30.0 {
            if (-5.5..-1.8).contains(&dl) {
                left_busy = true;
            }
            if (1.8..5.5).contains(&dl) {
                right_busy = true;
            }
        }
    }
    if !blocked {
        return None;
    }
    if !left_busy {
        Some(true)
    } else if !right_busy {
        Some(false)
    } else {
        None
    }
}

fn env_or<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn seeds() -> Vec<u64> {
    let s = std::env::var("TRAFH_SEEDS").unwrap_or_else(|_| "0".into());
    if let Some((a, b)) = s.split_once('-') {
        (a.trim().parse().unwrap()..=b.trim().parse().unwrap()).collect()
    } else {
        s.split(',').filter_map(|x| x.trim().parse().ok()).collect()
    }
}

fn traffic_cars(t: &Traffic) -> Vec<(String, Vec2, Vec2)> {
    t.cars
        .iter()
        .enumerate()
        .filter(|(_, c)| c.lane.is_some())
        .map(|(i, c)| (i.to_string(), c.state.position.truncate(), c.state.velocity.truncate()))
        .collect()
}

fn set_keys(game: &mut Game, k: [bool; 4]) {
    game.platform.keys.clear();
    for (on, code) in k.into_iter().zip([KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight])
    {
        if on {
            game.platform.keys.insert(code);
        }
    }
}

fn tap(game: &mut Game, code: KeyCode) {
    game.platform.keys.insert(code);
    game.key(code, &mut || {}).unwrap();
}

#[test]
#[ignore = "harness: the traffic parity harness's sessions (docs/traffic-harness.md)"]
fn traffic_harness() {
    crate::tests::testkit::game();
    let out = std::path::PathBuf::from(std::env::var("TRAFH_OUT").expect("TRAFH_OUT=<dir>"));
    std::fs::create_dir_all(&out).unwrap();
    let scen = std::env::var("TRAFH_SCEN").unwrap_or_else(|_| "stand".into());
    let secs: f32 = env_or("TRAFH_SECONDS", 120.0);
    let every: u64 = env_or("TRAFH_EVERY", 2);
    let at: Vec<f32> = std::env::var("TRAFH_AT")
        .unwrap_or_else(|_| "462.4,-46.2,10,-20.2".into())
        .split(',')
        .filter_map(|x| x.trim().parse().ok())
        .collect();
    for seed in seeds() {
        // SAFETY: the test thread alone reads the knob at the world build.
        unsafe {
            if seed > 0 {
                std::env::set_var("NFSU2_TRAFFIC_SEED", seed.to_string());
            } else {
                std::env::remove_var("NFSU2_TRAFFIC_SEED");
            }
        }
        let mut game = Game::new().unwrap();
        // A seed other than 0 is also a boot of its own: the shared generator from another
        // clock reading (the original seeds it from the QPC at boot, 0x57ECC0), so the pool's
        // models differ from seed to seed as from boot to boot (`TRAFH_BOOT=0`: keep it).
        if seed > 0 && std::env::var("TRAFH_BOOT").map_or(true, |v| v != "0") {
            game.rng = nfsu2::opponents::GameRng::boot(seed.wrapping_mul(0x9E37_79B9));
        }
        let mut profile = Profile::new("Trafh");
        profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        game.profile = Some(profile);
        let path = out.join(format!("ours-{}-s{seed}.tsv", scen.replace(':', "-")));
        let wall = std::time::Instant::now();
        let frames = (secs * 60.0) as u64;
        if let Some(mode) = scen.strip_prefix("qr:") {
            let qr = match mode {
                "sprint" => screens::QrMode::Sprint,
                "drag" => screens::QrMode::Drag,
                "drift" => screens::QrMode::Drift,
                "url" => screens::QrMode::Url,
                "streetx" => screens::QrMode::StreetX,
                _ => screens::QrMode::Circuit,
            };
            let ty = nfsu2::frontend::quick_race::race_type(qr);
            game.settings.qr[ty].traffic = env_or("TRAFH_LEVEL", 3);
            let driver = std::env::var("TRAFH_DRIVER")
                .unwrap_or_else(|_| if mode == "drag" { "drag" } else { "throttle" }.into());
            let driver_meta = match (std::env::var("TRAFH_RAM").is_ok_and(|v| v == "1"), std::env::var("TRAFH_SHOVE").is_ok_and(|v| v == "1")) {
                (_, true) => format!("{driver}-shove"),
                (true, _) if driver == "drag" => "drag-ram".to_string(),
                _ => driver.clone(),
            };
            game.explore(None).unwrap();
            game.drop_drive();
            game.mode = game.icon_menu(MenuKind::Main);
            game.act(Action::QuickRace, MenuKind::Main).unwrap();
            game.act(Action::QrMode(qr), MenuKind::QrMode).unwrap();
            for _ in 0..3 {
                game.menu_key(Key::Accept).unwrap();
            }
            if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
                game.menu_key(Key::Accept).unwrap();
            }
            let Mode::Race(rc) = &game.mode else { panic!("no race ({})", mode_name(&game.mode)) };
            let track = game.game_data.career.events.get(rc.event).map_or(0, |e| e.track);
            let region = rc.racers.traffic.as_ref().map_or(String::new(), |t| t.region.clone());
            let meta = format!(
                "scenario={scen}\tseed={seed}\tlevel={}\tdriver={driver_meta}\ttrack={track}\troutes=TRACKS/ROUTES{region}/Routes{track}{}.bin",
                game.settings.qr[ty].traffic,
                if rc.reversed { "B" } else { "F" }
            );
            let mut tab = Table::create(&path, &meta);
            let mut live = HashMap::new();
            writeln!(tab.w, "# contacts").unwrap();
            let (mut on, mut shifts, mut last_tap, mut t0) = (false, 0usize, -10.0f32, None::<u64>);
            let ram = std::env::var("TRAFH_RAM").is_ok_and(|v| v == "1");
            let shove = std::env::var("TRAFH_SHOVE").is_ok_and(|v| v == "1");
            let mut shoves = 0usize;
            let mut last_ram = -10.0f32;
            let mut n = 0u64;
            while t0.is_none_or(|t0| n < t0 + frames) && n < frames + 60 * 120 {
                n += 1;
                let Mode::Race(rc) = &game.mode else { break };
                let clock = rc.timing.clock;
                let green = clock >= 0.0;
                if green && t0.is_none() {
                    t0 = Some(n);
                }
                let mut k = [green && driver != "auto", false, false, false];
                if driver == "drag" && green {
                    if shifts < DRAG_SHIFTS.len() && clock >= DRAG_SHIFTS[shifts] {
                        shifts += 1;
                        tap(&mut game, KeyCode::ShiftLeft);
                    }
                    // `TRAFH_RAM=1`: a lane change into a rival beside every 4 s from 2 s.
                    if ram
                        && clock >= RAM_FROM
                        && clock - last_ram >= RAM_EVERY
                        && let (Some(d), Mode::Race(rc)) = (game.sim.drive.as_ref(), &game.mode)
                        && let Some(left) = drag_ram_tap(
                            d.state.position.truncate(),
                            d.state.forward().truncate().normalize_or_zero(),
                            &rc.racers.rivals.iter().map(|r| r.state.position.truncate()).collect::<Vec<_>>(),
                        )
                    {
                        last_ram = clock;
                        last_tap = clock;
                        k[if left { 2 } else { 3 }] = true;
                        tap(&mut game, if left { KeyCode::ArrowLeft } else { KeyCode::ArrowRight });
                    }
                    if let (Some(d), Mode::Race(rc)) = (game.sim.drive.as_ref(), &game.mode)
                        && clock - last_tap > 1.5
                        && let Some(left) = rc.racers.traffic.as_ref().and_then(|t| {
                            drag_lane_tap(
                                d.state.position.truncate(),
                                d.state.forward().truncate().normalize_or_zero(),
                                &traffic_cars(t),
                            )
                        })
                    {
                        last_tap = clock;
                        k[if left { 2 } else { 3 }] = true;
                        tap(&mut game, if left { KeyCode::ArrowLeft } else { KeyCode::ArrowRight });
                    }
                }
                // `TRAFH_SHOVE=1`: the player's car put beside a rival, pushing into it (as
                // trafh_native.py's --shove).
                if shove && green && shoves < SHOVE_AT.len() && clock >= SHOVE_AT[shoves] {
                    let k = shoves;
                    shoves += 1;
                    let picked = if let Mode::Race(rc) = &game.mode {
                        let live: Vec<&Rival> = rc.racers.rivals.iter().filter(|r| !r.state.totaled && r.progress.finished.is_none()).collect();
                        (!live.is_empty()).then(|| {
                            let r = live[k % live.len()];
                            (r.state.position, r.state.velocity, r.state.velocity.y.atan2(r.state.velocity.x))
                        })
                    } else {
                        None
                    };
                    if let (Some((at, v, hd)), Some(d)) = (picked, game.sim.drive.as_mut()) {
                        // The body alone, as place.py writes the original's (position,
                        // orientation, momentum; the drivetrain and the race state kept).
                        let (pos, vel) = shove_place(at, v, hd, k);
                        d.state.position = pos;
                        d.state.orientation = glam::Quat::from_rotation_z(hd);
                        d.state.velocity = vel;
                        d.state.angular_momentum = Vec3::ZERO;
                    }
                }
                set_keys(&mut game, k);
                game.advance_for(1.0 / 60.0).unwrap();
                let Mode::Race(rc) = &mut game.mode else { break };
                if !on && driver == "auto" && rc.timing.clock >= 0.0 {
                    if let (Some(routes), Some(d)) = (rc.racers.ai_routes.as_ref(), game.sim.drive.as_ref()) {
                        rc.autopilot = Some(Autopilot::new(d, routes, d.car.car_type.handling.race.drift()));
                        on = true;
                    }
                }
                let Some(t0) = t0 else { continue };
                if (n - t0) % every != 0 {
                    continue;
                }
                let t = (n - t0) as f32 / 60.0;
                if let Some(tr) = rc.racers.traffic.as_ref() {
                    tab.traffic(t, tr);
                }
                // A race's cars meet in its own collision field.
                tab.contacts(t, &rc.racers.cars, &mut live);
                tab.rivals(t, &rc.racers.rivals, &rc.racers.cars);
                if let Some(d) = game.sim.drive.as_ref() {
                    tab.player(t, d, driver.clone());
                }
            }
            tab.w.flush().unwrap();
        } else {
            let start = (Vec3::new(at[0], at[1], at[2]), at.get(3).copied().unwrap_or(0.0).to_radians());
            game.explore(Some(start)).unwrap();
            let mut bot = None;
            if scen == "bot" {
                let t = game.sim.traffic.as_ref().unwrap();
                let route = bot_route(&t.routes, start.0.truncate(), start.1, 12000.0);
                assert!(route.len() > 10, "no route from {at:?}");
                let mut f = std::fs::File::create(out.join("route.tsv")).unwrap();
                for p in &route {
                    writeln!(f, "{:.2}\t{:.2}", p.x, p.y).unwrap();
                }
                // The player put on the route's start, along it.
                let d = (route[1] - route[0]).normalize_or_zero();
                game.explore(Some((route[0].extend(start.0.z), d.y.atan2(d.x)))).unwrap();
                bot = Some(Bot::new(route));
            }
            let region = game.sim.traffic.as_ref().map_or(String::new(), |t| t.region.clone());
            let meta = format!(
                "scenario={scen}\tseed={seed}\tlevel={}\tdriver={}\troutes=TRACKS/ROUTES{region}/RoutesFreeRoam.bin",
                game.sim.traffic.as_ref().map_or(0, |t| t.level),
                if bot.is_some() { "bot" } else { "stand" }
            );
            let mut tab = Table::create(&path, &meta);
            let mut live = HashMap::new();
            // The contact census is on (an empty census still counts as zero records).
            writeln!(tab.w, "# contacts").unwrap();
            for n in 1..=frames {
                let t = n as f32 / 60.0;
                if let (Some(b), Some(d), Some(tr)) = (bot.as_mut(), game.sim.drive.as_ref(), game.sim.traffic.as_ref())
                {
                    let f = (d.state.orientation * Vec3::X).truncate().normalize_or_zero();
                    let k = b.think(t, d.state.position.truncate(), f, d.state.speed(), &traffic_cars(tr));
                    set_keys(&mut game, k);
                } else {
                    game.platform.keys.clear();
                }
                game.advance_for(1.0 / 60.0).unwrap();
                if n % every != 0 {
                    continue;
                }
                if let Some(tr) = game.sim.traffic.as_ref() {
                    tab.traffic(t, tr);
                    tab.contacts(t, &tr.field, &mut live);
                }
                if let Some(e) = game.sim.encounters.as_ref() {
                    tab.racers(t, e);
                }
                if let Some(d) = game.sim.drive.as_ref() {
                    tab.player(t, d, bot.as_ref().map_or("stand".into(), |b| b.note()));
                }
            }
            tab.w.flush().unwrap();
            if let Some(b) = &bot {
                println!("TRAFH bot overtakes {} stuck {}", b.overtakes, b.stucks);
            }
        }
        println!("TRAFH wrote {} ({:.1} s)", path.display(), wall.elapsed().as_secs_f32());
    }
    unsafe { std::env::remove_var("NFSU2_TRAFFIC_SEED") };
}

//! Style points and drift scoring (see docs/formats.md, "Drift scoring").
//!
//! The game keeps a style manager per human racer (racer +0x1D4, built by
//! 0x56EBF0 in 0x5F7A10): one "trick" per entry of the trick table (GLOBALB
//! chunk 0x34A07, 39 entries of 0x30 bytes) whose mode mask matches the
//! race. In a drift race a drift manager (racer +0x1D8, 0x56C900) turns the
//! car's slide into a points rate each frame (0x56ED50) and feeds the first
//! trick, "DRIFT"; the style manager's update (0x56FD20 → 0x56E930) runs
//! each trick's state machine, banking a drift's points when it ends.

use crate::formats::bundle::{chunks, read_f32, read_u32, walk};

/// One entry of the trick table (chunk 0x34A07).
#[derive(Clone, Debug, PartialEq)]
pub struct TrickDef {
    /// Its name, a language label ("DRIFT", "Good Drift!" …).
    pub name: u32,
    pub index: u8,
    /// +0x06: points accumulate while the trick is fed (a drift), rather
    /// than once.
    pub continuous: bool,
    /// +0x07: the races the trick exists in (0x56EBF0's mask: 1 any other,
    /// 2 drag, 4 drift, 8 point to point, 0x10 street X, 0x20, 0x40).
    pub mask: u8,
    /// +0x08: how long it must be fed before it counts, s (or, for the
    /// drift tiers, the points they announce at).
    pub min_time: f32,
    /// +0x0C: unfed this long, s, it ends.
    pub timeout: f32,
    /// +0x10: raw points it must pass before it counts.
    pub threshold: f32,
    /// +0x14: the most points one run can make (0: no cap).
    pub cap: f32,
    /// +0x18 / +0x1C: the multiplier's ceiling and its step per completed
    /// run.
    pub max_multiplier: f32,
    pub multiplier_step: f32,
    /// +0x28: points per unit of rate.
    pub points: f32,
    /// +0x2C: the most runs that score (−1: no limit).
    pub max_count: i8,
}

/// The trick table: GLOBALB chunk 0x34A07, its body 16-byte aligned
/// (0x56C4D0 takes `(header + 0x17) & ~0xF`).
pub fn parse_tricks(globalb: &[u8]) -> Vec<TrickDef> {
    let mut out = Vec::new();
    walk(chunks(globalb, 0), &mut |c| {
        if c.id != 0x34A07 {
            return;
        }
        let skip = ((c.offset + 0xF) & !0xF) - c.offset;
        let Some(body) = c.body.get(skip..) else { return };
        out = body
            .chunks_exact(0x30)
            .map(|r| TrickDef {
                name: read_u32(r, 0),
                index: r[4],
                continuous: r[6] != 0,
                mask: r[7],
                min_time: read_f32(r, 0x08),
                timeout: read_f32(r, 0x0C),
                threshold: read_f32(r, 0x10),
                cap: read_f32(r, 0x14),
                max_multiplier: read_f32(r, 0x18),
                multiplier_step: read_f32(r, 0x1C),
                points: read_f32(r, 0x28),
                max_count: r[0x2C] as i8,
            })
            .collect();
    });
    out
}

/// The style manager's mode mask for a race (0x56EBF0).
pub fn mode_mask(race: crate::race::RaceMode, point_to_point: bool) -> u8 {
    use crate::race::RaceMode;
    match race {
        RaceMode::Drift => 4,
        RaceMode::Drag => 2,
        _ if point_to_point => 8,
        RaceMode::StreetX => 0x10,
        _ => 1,
    }
}

/// A trick's state (+0x14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrickState {
    /// Scoring.
    Running,
    /// Fed, not yet past its minimum time and threshold.
    Pending,
    Idle,
    /// Its points went to the total this frame.
    Banked,
    /// Lost (a crash).
    Lost,
}

/// One trick of a style manager.
#[derive(Clone, Debug)]
pub struct Trick {
    pub def: usize,
    pub state: TrickState,
    /// +0x1C: the multiplier its points accumulate with.
    pub multiplier: f32,
    /// +0x20: completed runs.
    pub count: i32,
    /// +0x24: this run's points.
    pub points: f32,
    /// +0x28: the last rate fed.
    pub rate: f32,
    /// +0x2C: banked points.
    pub banked: f32,
    /// +0x30: hold a banked/lost state one more frame.
    pub hold: bool,
    /// +0x34 / +0x38: when it was last fed and when this run started, s.
    pub last_fed: f32,
    pub started: f32,
    /// +0x3C: this run's raw points, without the multiplier.
    pub raw: f32,
    /// +0x40 / +0x44 / +0x50 / +0x54: the longest run, s, the best run's
    /// points, and the largest spans and rates seen.
    pub longest: f32,
    pub best: f32,
    pub widest: f32,
    pub top_rate: f32,
    pub fed_from: f32,
    pub fed_to: f32,
    /// +0x18: the trick's points count in the total.
    pub counts: bool,
}

impl Trick {
    fn new(def: usize) -> Trick {
        // 0x56D660 from the constructor: idle, multiplier 1.
        Trick {
            def,
            state: TrickState::Idle,
            multiplier: 1.0,
            count: 0,
            points: 0.0,
            rate: 1.0,
            banked: 0.0,
            hold: false,
            last_fed: 0.0,
            started: 0.0,
            raw: 0.0,
            longest: 0.0,
            best: 0.0,
            widest: 0.0,
            top_rate: 0.0,
            fed_from: 0.0,
            fed_to: 0.0,
            counts: true,
        }
    }

    /// 0x56D510: a new state; leaving Banked counts the run and steps the
    /// multiplier; entering it keeps the records.
    fn set_state(&mut self, d: &TrickDef, state: TrickState, now: f32) {
        if self.state == state {
            return;
        }
        if self.state == TrickState::Banked {
            self.step(d);
        }
        if state == TrickState::Banked {
            self.longest = self.longest.max(now - self.started);
            self.best = self.best.max(self.points);
            self.widest = self.widest.max(self.fed_to - self.fed_from);
        }
        self.state = state;
    }

    fn step(&mut self, d: &TrickDef) {
        self.count += 1;
        if self.multiplier < d.max_multiplier {
            self.multiplier += d.multiplier_step;
        }
    }

    /// 0x56C750: this run's points after `dt` more of the last rate: the
    /// rate × points × multiplier added (capped by +0x14 in its sign), and
    /// the raw points (rate × points, at most the new total) added to +0x3C.
    fn accumulate(&mut self, d: &TrickDef, dt: f32) -> f32 {
        let per = d.points * self.rate;
        let mut total = self.points + per * self.multiplier * dt;
        if (d.cap > 0.0 && total > d.cap) || (d.cap < 0.0 && total < d.cap) {
            total = d.cap;
        }
        self.raw += (per * dt).min(total);
        total
    }

    /// 0x56D630 / the banking in 0x56E930: the run's points, truncated, go
    /// to the bank.
    fn bank(&mut self, d: &TrickDef, hold: bool, now: f32) {
        self.hold = hold;
        self.banked += self.points.trunc();
        self.set_state(d, TrickState::Banked, now);
    }

    /// 0x56D5D0: the run fails. A pending run just goes idle; otherwise it
    /// is lost (a banked one counted first).
    fn lose(&mut self, d: &TrickDef, hold: bool) {
        if self.state == TrickState::Pending {
            self.points = 0.0;
            self.raw = 0.0;
            self.hold = false;
            self.state = TrickState::Idle;
            return;
        }
        self.hold = hold;
        if self.state != TrickState::Lost {
            if self.state == TrickState::Banked {
                self.step(d);
            }
            self.state = TrickState::Lost;
        }
    }

    /// 0x56D6D0: back to idle unless a hold keeps the result a frame.
    fn reset(&mut self, d: &TrickDef) {
        if self.hold {
            self.hold = false;
            return;
        }
        self.points = 0.0;
        self.raw = 0.0;
        self.started = 0.0;
        if self.state != TrickState::Idle {
            if self.state == TrickState::Banked {
                self.step(d);
            }
            self.state = TrickState::Idle;
        }
    }
}

/// A racer's style manager (0x56EBF0): its tricks and the total.
#[derive(Clone, Debug)]
pub struct StyleManager {
    pub defs: Vec<TrickDef>,
    /// One per table entry; `None` where the entry's mask misses the race.
    pub tricks: Vec<Option<Trick>>,
    /// +0x9C: the banked points of every counting trick, truncated each.
    pub total: f32,
    /// The tricks 0x56FD20 reported to the HUD in its last update (see
    /// [`TrickReport`]).
    pub reports: Vec<TrickReport>,
}

/// A trick 0x56FD20 hands the HUD (0x56FE1C → 0x4DA6F0 → 0x4C88E0): each
/// update, before the banked and lost tricks reset, every trick (slot
/// order; trick +0x10 is 1 for every built trick, 0x56ECE4) that is
/// running, banked or lost (+0x14 0, 3, 4) and not held (+0x30 = 0), the
/// trick object itself linked into the list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrickReport {
    /// Trick +0x0C, the table index (0x56ECC1).
    pub index: usize,
    /// The table entry's name label (+0) and continuous byte (+6).
    pub name: u32,
    pub continuous: bool,
    pub state: TrickState,
    /// +0x24 this run's points and +0x1C the multiplier.
    pub points: f32,
    pub multiplier: f32,
}

impl StyleManager {
    pub fn new(defs: Vec<TrickDef>, mask: u8) -> StyleManager {
        let tricks = defs.iter().enumerate().map(|(i, d)| (d.mask & mask != 0).then(|| Trick::new(i))).collect();
        StyleManager { defs, tricks, total: 0.0, reports: Vec::new() }
    }

    pub fn trick(&self, i: usize) -> Option<&Trick> {
        self.tricks.get(i).and_then(Option::as_ref)
    }

    /// 0x56D5D0(1) on trick `i` when it is running or pending (the misc
    /// detectors' losses: 0x56F5E7, 0x56F958).
    pub fn lose(&mut self, i: usize) {
        let Some(Some(t)) = self.tricks.get_mut(i) else { return };
        if matches!(t.state, TrickState::Running | TrickState::Pending) {
            let d = &self.defs[t.def];
            t.lose(d, true);
        }
    }

    /// 0x56EAC0: feed trick `i` with a rate at `now`. Unless it has scored
    /// its most runs or is banked/lost: a continuous trick goes from idle
    /// to pending; a one-off trick whose points pass its threshold banks
    /// at once.
    pub fn feed(&mut self, i: usize, value: f32, now: f32) {
        let Some(Some(t)) = self.tricks.get_mut(i) else { return };
        let d = &self.defs[t.def];
        if !(d.max_count < 0 || t.count < d.max_count as i32) || matches!(t.state, TrickState::Banked | TrickState::Lost) {
            return;
        }
        t.rate = value;
        t.last_fed = now;
        t.fed_to = 0.0;
        t.top_rate = t.top_rate.max(value);
        if !d.continuous {
            let total = t.accumulate(d, 1.0);
            if (d.points < 0.0 && total < d.threshold) || (d.points > 0.0 && d.threshold < total) {
                t.points = total;
                t.set_state(d, TrickState::Running, now);
                t.bank(d, false, now);
            }
        } else if t.state == TrickState::Idle {
            t.fed_from = 0.0;
            t.started = now;
            t.set_state(d, TrickState::Pending, now);
        }
    }

    /// 0x56FD20: the total, then each trick: a banked or lost one resets
    /// to idle (or is held a frame), then its own update (0x56E930).
    pub fn update(&mut self, dt: f32, now: f32) {
        self.total = self
            .tricks
            .iter()
            .flatten()
            .filter(|t| t.counts)
            .map(|t| t.banked.trunc())
            .sum();
        // Kept until the HUD takes them (the clone may update more often
        // than it draws); a bound keeps them from piling up unread.
        let reports = self.tricks.iter().flatten().filter(|t| matches!(t.state, TrickState::Running | TrickState::Banked | TrickState::Lost) && !t.hold).map(|t| {
            let d = &self.defs[t.def];
            TrickReport { index: t.def, name: d.name, continuous: d.continuous, state: t.state, points: t.points, multiplier: t.multiplier }
        });
        self.reports.extend(reports);
        if self.reports.len() > 256 {
            self.reports.drain(..self.reports.len() - 256);
        }
        let defs = &self.defs;
        for t in self.tricks.iter_mut().flatten() {
            let d = &defs[t.def];
            if matches!(t.state, TrickState::Banked | TrickState::Lost) {
                t.reset(d);
            }
            Self::trick_update(t, d, dt, now);
        }
    }

    /// 0x56E930: a trick unfed longer than its timeout banks if running,
    /// fails if pending; while fed it accumulates; a pending trick past
    /// its minimum time whose raw points pass its threshold starts running.
    fn trick_update(t: &mut Trick, d: &TrickDef, dt: f32, now: f32) {
        let since = now - t.last_fed;
        if since > d.timeout {
            match t.state {
                TrickState::Running => t.bank(d, false, now),
                TrickState::Pending => {
                    t.lose(d, false);
                    t.reset(d);
                }
                _ => {}
            }
        }
        if matches!(t.state, TrickState::Running | TrickState::Pending) && since < dt + dt {
            t.points = t.accumulate(d, dt);
        }
        if t.state == TrickState::Pending && now - t.started > d.min_time && ((d.points < 0.0 && t.raw < d.threshold) || (d.points > 0.0 && d.threshold < t.raw)) {
            t.count += 1;
            t.state = TrickState::Running;
        }
    }

    /// 0x56D720 at a lap's end: running tricks bank (held a frame),
    /// pending ones fail.
    pub fn bank_running(&mut self, now: f32) {
        let defs = &self.defs;
        for t in self.tricks.iter_mut().flatten() {
            let d = &defs[t.def];
            match t.state {
                TrickState::Running => t.bank(d, true, now),
                TrickState::Pending => t.lose(d, true),
                _ => {}
            }
        }
    }
}

/// The drift trick (entry 0, "DRIFT") and the drift tiers' entries
/// (0x79DAD0: 8..14, "Good Drift!" … "Drift King!").
pub const DRIFT: usize = 0;
pub const CHAINED: usize = 1;
const TIERS: [usize; 7] = [8, 9, 10, 11, 12, 13, 14];

/// Speeds a drift must keep, m/s, by performance class (0x79DA68 to start
/// a drift; 0x79DA58 while one runs or the car counter-steers).
const START_SPEED: [f32; 4] = [11.111112, 9.722222, 8.333334, 6.944445];
const KEEP_SPEED: [f32; 4] = [13.888889, 12.5, 11.111112, 8.333334];
/// The multiplier levels' speeds, m/s (0x79DA78: [0] the 5 km/h margin,
/// then levels 1..5).
static LEVEL_SPEED: crate::exe_tables::Table<[f32; 6]> = crate::exe_tables::Table::new(|t| &t.drift_level_speed);
/// Points the current drift needs for the next level, by class and level
/// (0x79DA90).
static LEVEL_POINTS: crate::exe_tables::Table<[[i32; 4]; 4]> = crate::exe_tables::Table::new(|t| &t.drift_level_points);
/// The AI's drift points multiplier by difficulty (0x79DAF0).
pub const AI_DIFFICULTY: [f32; 3] = [1.0, 1.25, 1.5];

/// 0x56E0B0's closeness of one other car at `distance` m: 1 at 1 m
/// (0x7FA70C) or nearer, falling linearly to 0 at 20 m (0x7FA710) and
/// beyond.
pub fn closeness(distance: f32) -> f32 {
    1.0 - (distance.clamp(1.0, 20.0) - 1.0) / 19.0
}

/// What the drift manager reads of its racer each frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct DriftInput {
    /// Speed, m/s (car +0x430), and forward speed (+0x42C).
    pub speed: f32,
    pub forward_speed: f32,
    /// Slip angle, radians, positive with the nose left of the travel
    /// (0x59FA20).
    pub slip: f32,
    /// Yaw rate, rad/s (car +0x118).
    pub yaw_rate: f32,
    /// The race done, 0..1 (racer info +0x14).
    pub progress: f32,
    /// The car is touching something (collision +0x120 ≥ 1).
    pub touching: bool,
    /// Wheels on a surface other than road (car +0x394.. ≠ 1).
    pub off_road_wheels: u32,
    /// The car's top speed, m/s (0x5B5940).
    pub top_speed: f32,
    /// The racer goes the wrong way (racer +0x248) — the player only.
    pub wrong_way: bool,
    /// The last hit (MiscStyleManager, racer +0x1DC: +0x2C when, +0x30
    /// its strength, and whether it costs a drift: kind +0x34 = 3 or the
    /// +0x38 flag): seconds since it, strength, costs.
    pub hit: Option<(f32, f32, bool)>,
    /// Σ closeness over the world list's cars past the racers (0x890080
    /// +0x18 .. +0x10: the traffic, not the rivals) whose +0x28 is set:
    /// [`closeness`] of each 3D distance (0x43CE10 between the cars'
    /// +0x60); 0 with no traffic.
    pub closeness: f32,
    /// The racer is the human (racer +0x18).
    pub human: bool,
}

/// A racer's drift manager (0x56C900 / 0x56D7E0).
#[derive(Clone, Debug)]
pub struct DriftManager {
    /// +0x0C: the car's performance class (0x5B5340).
    pub class: usize,
    /// +0x1C: the multiplier level, 1..5.
    pub level: u32,
    /// +0x20: the current drift's points for the next level (0 at the top).
    pub next_level: i32,
    /// +0x24: when the level last dropped, s.
    dropped: Option<f32>,
    /// +0x28: since when the car runs backwards or the wrong way, s.
    reversing: Option<f32>,
    /// +0x2C: the wrong-way countdown (4 − t after the first second), or −1.
    pub countdown: f32,
    /// +0x30: the furthest progress yet.
    progress: f32,
    /// +0x38: the AI difficulty's multiplier (1 for the human).
    pub difficulty: f32,
    /// +0x15: a hit already cost this drift.
    hit_flag: bool,
    /// +0x16 / +0x17: the level is falling / may not rise.
    pub falling: bool,
    pub held: bool,
    /// +0x18: the off-road factor (0x56CA90).
    pub surface: f32,
    /// +0x40: the drift tiers announced in the current drift.
    tier: usize,
    /// +0x44: the drift's widest slip, signed.
    widest: f32,
    /// +0x3C: the proximity factor (0x56E0B0).
    pub near: f32,
}

impl DriftManager {
    pub fn new(class: usize, difficulty: f32) -> DriftManager {
        DriftManager {
            class: class.min(3),
            level: 1,
            next_level: LEVEL_POINTS[class.min(3)][0],
            dropped: None,
            reversing: None,
            countdown: -1.0,
            progress: 0.0,
            difficulty,
            hit_flag: false,
            falling: false,
            held: false,
            surface: 1.0,
            tier: 0,
            widest: 0.0,
            near: 1.0,
        }
    }

    /// 0x56ED50: one frame. Returns a tier's name to announce (0x4AC120).
    pub fn update(&mut self, style: &mut StyleManager, input: &DriftInput, now: f32) -> Option<u32> {
        let announce = self.tiers(style, input);
        self.wrong_way(input, now);
        let angle = self.angle_points(style, input);
        let level = self.next_level_for(style, input, now);
        self.surface = Self::surface_factor(input);
        let hit = self.hit(input);
        self.near = self.proximity(style, input);
        let rate = self.difficulty * self.surface * self.near * angle;
        if hit > 0.2 && self.hit_flag {
            if let Some(Some(t)) = style.tricks.get_mut(DRIFT)
                && matches!(t.state, TrickState::Running | TrickState::Pending)
            {
                let d = &style.defs[t.def];
                t.lose(d, true);
            }
            self.level = 1;
            return announce;
        }
        if rate > 0.0 {
            if let Some(Some(t)) = style.tricks.get_mut(DRIFT) {
                t.multiplier = level as f32;
            }
            style.feed(DRIFT, rate, now);
            if style.trick(DRIFT).is_some_and(|t| t.state == TrickState::Running) && input.slip * self.widest < 0.0 && input.slip.abs() > 0.4 * 0.666_666_7 {
                self.widest = input.slip;
            }
        }
        self.level = level;
        announce
    }

    /// 0x56CCE0: in a running drift, the next tier once its points pass
    /// the tier entry's +0x08 (the human hears it), and the widest slip;
    /// otherwise the tiers and slip reset and the chain's multiplier
    /// returns to 1.
    fn tiers(&mut self, style: &mut StyleManager, input: &DriftInput) -> Option<u32> {
        let running = style.trick(DRIFT).map(|t| (t.state, t.points));
        match running {
            Some((TrickState::Running, points)) => {
                let mut said = None;
                if let Some(&e) = TIERS.get(self.tier)
                    && style.defs.get(e).is_some_and(|d| d.min_time < points)
                {
                    self.tier += 1;
                    if input.human {
                        said = Some(style.defs[e].name);
                    }
                }
                let s = input.slip;
                if self.widest == 0.0 || (s * self.widest > 0.0 && ((s < 0.0 && s < self.widest) || (s > 0.0 && self.widest < s))) {
                    self.widest = s;
                }
                said
            }
            _ => {
                self.widest = 0.0;
                self.tier = 0;
                if let Some(Some(t)) = style.tricks.get_mut(CHAINED) {
                    t.multiplier = 1.0;
                }
                None
            }
        }
    }

    /// The popup's bonus colour (0x4C7EA0): the off-road factor over 1.
    pub fn bonus(&self) -> bool {
        self.surface > 1.0 || self.near > 1.0
    }

    /// 0x56DEF0: backwards (or the human the wrong way) starts a timer;
    /// after a second the countdown shows 4 − t, after four it restarts.
    fn wrong_way(&mut self, input: &DriftInput, now: f32) {
        self.countdown = -1.0;
        if (input.human && input.wrong_way) || input.forward_speed < 0.0 {
            let since = *self.reversing.get_or_insert(now);
            let t = now - since;
            if t > 4.0 {
                self.reversing = None;
            }
            if t > 1.0 {
                self.countdown = 4.0 - t;
            }
        } else {
            self.reversing = None;
        }
    }

    /// 0x56C950: the drift's rate — (speed / 8.33)² × |slip| when nothing
    /// is touched, the race has not gone backwards, the slip is over 5° and
    /// the speed over the class's minimum (the higher one while a drift
    /// runs or the car counter-steers).
    fn angle_points(&mut self, style: &StyleManager, input: &DriftInput) -> f32 {
        if input.touching {
            return 0.0;
        }
        let mut points = 0.0;
        if self.progress <= input.progress {
            let counter = input.yaw_rate.abs() > 0.1 && input.slip * input.yaw_rate < 0.0;
            let keep = style.trick(DRIFT).is_some_and(|t| t.state != TrickState::Running) || counter;
            let min = if keep { KEEP_SPEED[self.class] } else { START_SPEED[self.class] };
            if self.reversing.is_none() && input.slip.abs() > 5.0f32.to_radians() && min < input.speed {
                let v = input.speed / 8.333_334;
                points = v * v * input.slip.abs();
            }
        }
        self.progress = self.progress.max(input.progress);
        points
    }

    /// 0x56CB60: the multiplier level. Too slow for the level (or going
    /// backwards) drops it a level at once and again every 1.5 s; fast
    /// enough for the next, with the running drift's points past the
    /// level's mark, raises it.
    fn next_level_for(&mut self, style: &StyleManager, input: &DriftInput, now: f32) -> u32 {
        self.falling = false;
        self.held = false;
        let mut level = self.level;
        let l = self.level as usize;
        if l > 1 && (self.reversing.is_some() || input.speed < LEVEL_SPEED[0] + LEVEL_SPEED[l.min(5)]) {
            self.falling = true;
            let drop = match self.dropped {
                Some(at) => now - at > 1.5,
                None => true,
            };
            if drop {
                level = self.level - 1;
                self.dropped = Some(now);
            }
        } else if let Some(at) = self.dropped {
            self.held = true;
            if now - at > 1.5 {
                self.dropped = None;
            }
        } else if l < 4 && input.speed < LEVEL_SPEED[(l + 1).min(5)] - LEVEL_SPEED[0] {
            self.held = true;
        } else if let Some(t) = style.trick(DRIFT)
            && t.state == TrickState::Running
            && l < 5
            && (self.next_level as f32) < t.points
        {
            level = self.level + 1;
        }
        self.next_level = if level < 5 { LEVEL_POINTS[self.class][(level as usize).clamp(1, 4) - 1] } else { 0 };
        level
    }

    /// 0x56E0B0: while a drift runs, 1 plus, for each other car, its
    /// closeness (1 at 1 m to 0 at 20 m) × this car's speed between 10 and
    /// 160 km/h (0..1).
    fn proximity(&self, style: &StyleManager, input: &DriftInput) -> f32 {
        if style.trick(DRIFT).is_none_or(|t| t.state != TrickState::Running) {
            return 1.0;
        }
        let speed = (input.speed.clamp(2.777_778, 44.444_447) - 2.777_778) / (44.444_447 - 2.777_778);
        1.0 + input.closeness * speed
    }

    /// 0x56CA90: with two or more wheels off the road, their count × the
    /// speed over a quarter of the top speed (at most 1); else 1.
    fn surface_factor(input: &DriftInput) -> f32 {
        if input.off_road_wheels <= 1 {
            return 1.0;
        }
        let f = input.speed / (input.top_speed * 0.25);
        if f <= 0.0 { 0.0 } else { input.off_road_wheels as f32 * f.min(1.0) }
    }

    /// 0x56DCB0: a hit in the last half second gives its strength once;
    /// one of the costly kinds raises the flag that makes it cost the drift.
    fn hit(&mut self, input: &DriftInput) -> f32 {
        match input.hit {
            Some((since, strength, costs)) if since < 0.5 => {
                if self.hit_flag {
                    return 0.0;
                }
                self.hit_flag = costs;
                strength
            }
            _ => {
                self.hit_flag = false;
                0.0
            }
        }
    }
}

/// The drift race's standings (the list 0x864630 of 0x20-byte nodes at
/// 0x864640 + 0x20·i, built at the race start by 0x56D860 from
/// 0x5F7A10, refreshed each frame by 0x56DDE0 from 0x6076E0 (at 0x608C7A); verified).
/// Node: +0x08 racer number, +0x0C the car's class (0x5B5340), +0x10 the
/// score (f32), +0x14 a flag byte (1 for the local human and for every AI
/// racer, 0 for a downhill track's virtual rivals), +0x18 the name, +0x1C
/// the car record.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardEntry {
    /// The clone's racer: 0 the player, i + 1 rival i; `None` for a
    /// virtual rival of a downhill drift track.
    pub racer: Option<usize>,
    pub name: String,
    pub car: String,
    /// +0x10: the racer's banked DRIFT points (style manager trick 0,
    /// +0x2C, read by 0x56DDE0), or a virtual rival's fixed score.
    pub score: f32,
    /// +0x14: the leader board's SetPlayerColor rows (0x4AAC50 / 0x4AADC0
    /// test the row's copy of it, 0x4C6EC0) and the results' highlighted
    /// rows (0x4BF140).
    pub flag: bool,
}

/// The downhill drift tracks (0x104E–0x1053): offline, 0x56D860 puts no
/// AI racer in the list but three virtual rivals with fixed scores.
pub fn downhill(track: u16) -> bool {
    (0x104E..=0x1053).contains(&track)
}

/// The virtual rivals' scores (table 0x7FA5C0, 6 rows of 13 dwords: the
/// track number, then 12 f32 by class × 3 + difficulty, the class the
/// lowest 0x5B5340 of the humans): the first rival the table's, the
/// second × 0.95, the third × 0.9 (racer numbers 100, 101, 102).
pub fn virtual_scores(track: u16, class: usize, difficulty: usize) -> Option<[f32; 3]> {
    static TABLE: crate::exe_tables::Table<[(u16, [f32; 12]); 6]> = crate::exe_tables::Table::new(|t| &t.drift_virtual_scores);
    let (_, row) = TABLE.iter().find(|(t, _)| *t == track)?;
    let s = row[class.min(3) * 3 + difficulty.min(2)];
    Some([s, s * 0.95, s * 0.9])
}

/// 0x56DDE0's sort (List_SortPass 0x43E650, stable, with 0x56CDE0: a
/// stays before b when a's score is at least b's): the highest score
/// first, ties in list order.
pub fn sort_board(board: &mut [BoardEntry]) {
    board.sort_by(|a, b| b.score.total_cmp(&a.score));
}

/// A racer's place in the drift standings (0x56DDA0: its node's index + 1
/// by racer number, 0 when absent).
pub fn board_place(board: &[BoardEntry], racer: usize) -> Option<usize> {
    board.iter().position(|e| e.racer == Some(racer)).map(|i| i + 1)
}

/// The winning margin of a drift event (0x513FE0 with the event type +0x0F
/// = 5): the first node's score over the second's − 1, × 100; 100 with
/// no runner-up score.
pub fn board_margin(board: &[BoardEntry]) -> f32 {
    match (board.first(), board.get(1)) {
        (Some(a), Some(b)) if b.score > 0.0 => (a.score / b.score - 1.0) * 100.0,
        _ => 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drift_def() -> TrickDef {
        TrickDef {
            name: 0xA46A_AEA8,
            index: 0,
            continuous: true,
            mask: 4,
            min_time: 0.1,
            timeout: 0.3,
            threshold: 50.0,
            cap: 0.0,
            max_multiplier: 5.0,
            multiplier_step: 0.0,
            points: 120.0,
            max_count: -1,
        }
    }

    #[test]
    fn a_drift_runs_then_banks_when_it_ends() {
        let mut s = StyleManager::new(vec![drift_def()], 4);
        let dt = 1.0 / 60.0;
        let mut now = 0.0;
        for _ in 0..120 {
            s.feed(DRIFT, 1.0, now);
            s.update(dt, now);
            now += dt;
        }
        let t = s.trick(DRIFT).unwrap();
        assert_eq!(t.state, TrickState::Running);
        // About 2 s at 120 points a second.
        assert!((t.points - 240.0).abs() < 10.0, "{}", t.points);
        // Unfed past 0.3 s it banks; the next frame it is idle again.
        for _ in 0..30 {
            s.update(dt, now);
            now += dt;
        }
        let t = s.trick(DRIFT).unwrap();
        assert_eq!(t.state, TrickState::Idle);
        assert!(t.banked > 200.0);
        s.update(dt, now);
        assert_eq!(s.total, s.trick(DRIFT).unwrap().banked.trunc());
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_rate_grows_with_speed_and_slip() {
        let mut s = StyleManager::new(vec![drift_def()], 4);
        let mut m = DriftManager::new(3, 1.0);
        let input = DriftInput { speed: 25.0, forward_speed: 24.0, slip: 0.5, top_speed: 60.0, human: true, ..Default::default() };
        m.update(&mut s, &input, 0.0);
        // (25 / 8.333)² × 0.5 = 4.5.
        assert!((s.trick(DRIFT).unwrap().rate - 4.5).abs() < 0.01);
        assert_eq!(s.trick(DRIFT).unwrap().state, TrickState::Pending);
    }
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn traffic_close_by_raises_a_running_drift() {
        assert_eq!(closeness(0.5), 1.0);
        assert_eq!(closeness(20.0), 0.0);
        assert!((closeness(10.5) - 0.5).abs() < 1e-6);
        let mut s = StyleManager::new(vec![drift_def()], 4);
        let m = DriftManager::new(3, 1.0);
        // Pending: no bonus.
        let input = DriftInput { speed: 44.444_447, closeness: 1.5, ..Default::default() };
        assert_eq!(m.proximity(&s, &input), 1.0);
        if let Some(Some(t)) = s.tricks.get_mut(DRIFT) {
            t.state = TrickState::Running;
        }
        // Running at 160 km/h or more: 1 + Σ closeness.
        assert!((m.proximity(&s, &input) - 2.5).abs() < 1e-5);
        // The racer's own speed weighs it: none at 10 km/h.
        assert_eq!(m.proximity(&s, &DriftInput { speed: 2.7, ..input }), 1.0);
    }

    fn entry(racer: Option<usize>, score: f32) -> BoardEntry {
        BoardEntry { racer, name: String::new(), car: String::new(), score, flag: racer.is_some() }
    }

    #[test]
    fn the_board_places_by_points_not_by_finish() {
        // The player (0) drifted most but finished last: first on the board.
        let mut b = vec![entry(Some(1), 900.0), entry(Some(2), 1200.0), entry(Some(0), 5000.0), entry(Some(3), 1200.0)];
        sort_board(&mut b);
        assert_eq!(b.iter().map(|e| e.racer.unwrap()).collect::<Vec<_>>(), [0, 2, 3, 1]);
        assert_eq!(board_place(&b, 0), Some(1));
        assert_eq!(board_place(&b, 1), Some(4));
        assert!((board_margin(&b) - (5000.0 / 1200.0 - 1.0) * 100.0).abs() < 1e-3);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn downhill_rivals_have_fixed_scores() {
        assert!(downhill(4174) && downhill(4179) && !downhill(4301));
        let s = virtual_scores(4175, 1, 2).unwrap();
        assert_eq!(s, [220000.0, 220000.0 * 0.95, 220000.0 * 0.9]);
        assert_eq!(virtual_scores(4301, 0, 0), None);
    }
}

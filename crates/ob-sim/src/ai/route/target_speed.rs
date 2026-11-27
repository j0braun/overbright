use super::*;

/// What the catch-up needs from the race (0x4172D0 / 0x4084A0).
#[derive(Clone, Copy, Debug)]
pub struct CatchUp {
    /// Metres the AI is ahead of the human (negative behind).
    pub gap: f32,
    pub own_speed: f32,
    pub human_speed: f32,
    pub laps: u32,
    /// The race done, 0..1.
    pub progress: f32,
    pub skill: usize,
    /// 0x89E7A8 is 2 or 3.
    pub alt: bool,
    /// Race seconds (the catch-up waits the row's +0x580 seconds).
    pub race_time: f32,
    /// A street-X race (its own curves +0x2C0 / +0x318).
    pub street_x: bool,
    /// The human's line-speed term (0x41757B..0x417676): with the option
    /// on, the human's mean ratio of speed to racing-line speed and place.
    pub line: Option<LineSpeed>,
    /// The human's bonus scale 0x89E7FC[player index] (0x4174EB, read
    /// outside street X or with the tuning byte +0x421 set; else 1): the
    /// Quick Race set-up 0x53F160 stores tuning +0x94C + 4·(4·difficulty +
    /// the human car's performance class) at 0x53F306 / 0x53F331, the
    /// career set-ups 1.0 (0x5267C2, 0x5268A8, 0x526A28).
    pub scale: f32,
}

impl Default for CatchUp {
    fn default() -> CatchUp {
        CatchUp {
            gap: 0.0,
            own_speed: 0.0,
            human_speed: 0.0,
            laps: 0,
            progress: 0.0,
            skill: 0,
            alt: false,
            race_time: 0.0,
            street_x: false,
            line: None,
            scale: 1.0,
        }
    }
}

/// The human player's line-speed record for the catch-up (player +0xA0,
/// the mean 0x579F30(10) the player update 0x602660 pushes once a second
/// at 0x602842: car +0x430 over the racing-line speed of its road node's
/// control point, byte +0x2C).
#[derive(Clone, Copy, Debug, Default)]
pub struct LineSpeed {
    /// The mean (player +0xA0 → +0x0C, read by 0x5EB210).
    pub ratio: f32,
    /// The human's place (racer status +0x0A, 1-based), the index into the
    /// tuning pairs +0x49C / +0x4CC.
    pub place: usize,
}

/// One opponent's target speed state (controller +0x700 / +0x704 /
/// +0x72C / +0x770).
#[derive(Clone, Debug)]
pub struct SpeedTarget {
    pub(super) skill: usize,
    /// The controller's skill row (vtable +0x78, racer info +0x901) was
    /// negative: no jitter (0x41C6E2), row 0 for the factors (0x41C849).
    pub(super) human: bool,
    pub(super) jitter: f32,
    pub(super) jitter_at: Option<f64>,
    pub(super) catch_up: f32,
    pub(super) catch_up_at: Option<f64>,
}

impl SpeedTarget {
    pub fn new(skill: usize) -> SpeedTarget {
        SpeedTarget { skill, human: false, jitter: 0.0, jitter_at: None, catch_up: 1.0, catch_up_at: None }
    }

    /// A human racer's (its racer info +0x901 is −2, 0x6373F0 called with
    /// 0xFFFFFFFE at 0x526028 / 0x526B41 / 0x5324CF) once an AI racer
    /// controller drives it: row max(−2, 0) = 0 for the speed factor
    /// (0x41C849..0x41C85C) and no jitter (0x41C6E0: a negative row skips
    /// it).
    pub fn human() -> SpeedTarget {
        SpeedTarget { human: true, ..SpeedTarget::new(0) }
    }

    /// The skill row (controller slot +0x78).
    pub fn skill(&self) -> usize {
        self.skill
    }

    /// The catch-up multiplier last worked out (controller +0x770); the
    /// car's grip bonus is it at least 1 (car +0x468).
    pub fn catch_up(&self) -> f32 {
        self.catch_up
    }

    /// 0x41C5E0, race state 7: no distance jitter or skill speed factor.
    /// All seven stage entries at 0x78357C are .84 for place 1; 0x783598
    /// contains 1 for every other place. Career outruns disable catch-up,
    /// but the unused jitter still updates the racer RNG and its timer.
    pub fn update_outrun(
        &mut self,
        tuning: &AiTuning,
        route: &RacerRoute,
        rng: &mut GameRng,
        clock: f64,
        leader: bool,
        _dt: f32,
    ) -> f32 {
        self.refresh(tuning, rng, clock, None);
        let v = route.speed_at(route.distance());
        let v = if v.abs() < 0.001 { 30.0 } else { v };
        let factor = if leader { 0.84 } else { 1.0 };
        v * factor * self.catch_up
    }

    /// The speed for the driver (0x41C5E0, offline racer): the route's
    /// speed at the car's distance plus an offset of ±row[+0x584] m redrawn
    /// every +0xAD4 s, times the row's factor (+0x538) and the catch-up
    /// (30 m/s when the route reads 0). `rng` is the world's generator
    /// (0x41C71E: 0x5EA510 with ECX = [0x890080], world +0xF8), shared by
    /// every racer in turn, **verified** (the pinned circuit's world
    /// generator steps exactly once per rival on the first racing frame);
    /// `clock` race seconds; `catch_up` None when the option is off.
    pub fn update(
        &mut self,
        tuning: &AiTuning,
        route: &RacerRoute,
        rng: &mut GameRng,
        clock: f64,
        catch_up: Option<CatchUp>,
        _dt: f32,
    ) -> f32 {
        self.refresh(tuning, rng, clock, catch_up);
        let v = route.speed_at(route.distance() + self.jitter);
        let v = if v.abs() < 0.001 { 30.0 } else { v };
        v * tuning.speed_factor(self.skill) * self.catch_up
    }

    /// Shared timer work precedes the native race-state branch.
    fn refresh(&mut self, tuning: &AiTuning, rng: &mut GameRng, clock: f64, catch_up: Option<CatchUp>) {
        if !self.human
            && self.skill < 10
            // 0x41C6EB: (sim tick − +0x700) × 0.00025 (0x4026E0) strictly
            // above the period (`fcomp; test ah,0x41; jne` skips equal),
            // on whole ticks (the clone's clock is a sum of float frame
            // steps): observed, the pinned circuit's second offset draws
            // fall on the frame after 5 s, not on it.
            && self.jitter_at.is_none_or(|t| ((clock - t) * 4000.0).round() / 4000.0 > f64::from(tuning.jitter_period()))
        {
            let j = tuning.jitter(self.skill);
            self.jitter = rng.unit(j + j) - j;
            self.jitter_at = Some(clock);
        }
        if self.catch_up_at.is_none_or(|t| clock - t >= f64::from(tuning.catch_up_period())) {
            self.catch_up_at = Some(clock);
            self.catch_up = catch_up.map_or(1.0, |c| catch_up_multiplier(tuning, &c));
        }
    }
}

/// The catch-up multiplier (0x4172D0 with 0x4084A0): after the row's
/// +0x580 seconds, a bonus from the gap — curve k = alt + 2·(laps ≥ 4) at
/// skill 0, k + 4 at skill 9, lerped by skill — plus the speed term
/// clamp(−0.005·(v_ai − v_human) / (0.01·gap² + 1), ±0.05); faded over the
/// race by the curve at +0x504.
pub fn catch_up_multiplier(t: &AiTuning, c: &CatchUp) -> f32 {
    // 0x4084A0 clamps x only at 0 (0x4085EE); the cap at 9 is the clone's own.
    let skill = c.skill.min(9);
    let mut bonus = 0.0;
    if c.race_time >= t.catch_up_hold(skill) && c.gap < t.f(0xADC) {
        let (a, b) = if c.street_x {
            (t.curve(0x2C0, 11, c.gap), t.curve(0x318, 11, c.gap))
        } else {
            let k = usize::from(c.alt) + 2 * usize::from(c.laps >= 4);
            (t.curve(k * 0x58, 11, c.gap), t.curve((k + 4) * 0x58, 11, c.gap))
        };
        // 0x4085DA (0x576C70(0, k, 9.0, k + 4)): the k curve is the value at
        // x = 0 and the k + 4 curve at x = 9, where x = max(0, the controller's
        // vtable +0x78 result) (0x4085E3). `skill` is taken as that x; the
        // provider is not resolved. k is evaluated last, 0x4085CB, from the
        // object set at 0x408587; k + 4 at 0x4085B8 from the object at 0x408593.
        bonus = a + (b - a) * skill as f32 / 9.0;
        let speed = ((c.own_speed - c.human_speed) * -0.005 / (0.01 * c.gap * c.gap + 1.0)).clamp(-0.05, 0.05);
        bonus += speed;
    }
    let scale = if !c.street_x || t.data[0x421] != 0 { c.scale } else { 1.0 };
    let mut m = 1.0 + scale * bonus;
    // 0x417531..0x417676: the line-speed term, with the option on, once the
    // human is picked (+0x748, only in the block above, so after the hold),
    // outside street X or with the tuning byte +0x422 set: the human's ratio
    // v picks the pair at +0x4CC (v < 1) or +0x49C (else) + 8 × place, an
    // evenly spaced curve over skill 0..9 (0x413390(pair, 2, 0, 9.0), read at
    // the skill by 0x5764E0) gives f, and the multiplier becomes
    // m × max(1 + (v − 1)·f, row +0x53C, street X +0x544) (0x401050 = max).
    let picked = c.race_time >= t.catch_up_hold(skill);
    if let Some(l) = c.line.filter(|_| picked && (!c.street_x || t.data[0x422] != 0)) {
        let at = if l.ratio < 1.0 { 0x4CC } else { 0x49C } + 8 * l.place.min(5);
        let (a, b) = (t.f(at), t.f(at + 4));
        let f = a + (b - a) * (skill as f32 / 9.0).clamp(0.0, 1.0);
        let floor = t.row(skill, if c.street_x { 0x544 } else { 0x53C });
        m *= (1.0 + (l.ratio - 1.0) * f).max(floor);
    }
    // 0x41767C..0x41771A: faded by the curve +0x504 over the race (laps
    // done / laps), outside street X or with the tuning byte +0x423 set.
    if !c.street_x || t.data[0x423] != 0 {
        m = (m - 1.0) * t.curve(0x504, 4, c.progress) + 1.0;
    }
    m
}

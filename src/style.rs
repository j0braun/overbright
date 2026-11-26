//! Style points outside the drift scorer (verified unless noted; see
//! docs/formats.md, "Style points and the nitrous refill").
//!
//! Every local human racer gets a style manager (0x5FC3A6 → 0x56EBF0,
//! racer +0x1D4) in every race and in free roam, with the trick-table
//! entries whose mask matches the mode (0x56EBF0: drift 4, drag 2, point
//! to point 8, street X 0x10, else 1 — free roam too, observed). Each frame
//! the racer update 0x6016B0 runs the misc style manager's detectors
//! (0x56FF40), the style manager's update (0x56FD20) and, with the Race
//! Breaker flag 0x89E7E7 set, turns the change of the total into nitrous
//! (0x601737 → 0x592650).

use crate::drift::{StyleManager, TrickState};
use crate::nitrous::Tank;

/// The trick table's entries the misc style manager feeds (GLOBALB chunk
/// 0x34A07; the slot is the entry's index, style manager + 4 × slot).
pub const SPRAY_SHOW: usize = 15;
pub const TRAFFIC_HIT: usize = 16;
pub const LEAD_LAP: usize = 17;
pub const CLEAN_SECTION: usize = 18;
pub const CLEAN_LAP: usize = 19;
pub const SHORTCUT: usize = 20;
pub const AVOIDED_ACCIDENT: usize = 21;
/// "360".
pub const SPIN: usize = 22;
pub const BEST_LAP: usize = 23;
pub const HEAD_START: usize = 24;
pub const J_TURN: usize = 25;
pub const BURNOUT: usize = 26;
pub const CLEARED_ACCIDENT: usize = 27;
/// "GAINED 1ST" … "GAINED 4TH" (32 down to 29; 28, "GAINED 5TH", is
/// never fed: 0x56F830 needs a place under 5).
pub const GAINED_1ST: usize = 32;
/// The hydraulics' tricks (0x56F9C0: style manager +0x84 and +0x98).
pub const BOUNCE: usize = 33;
pub const HYDROS: usize = 38;
pub const POWER_SLIDE: usize = 34;
pub const HANG_TIME: usize = 35;
pub const DRAFTING: usize = 36;
pub const REVERSE: usize = 37;

/// The reverse speed REVERSE needs, m/s (0x79DB7C: 40 mph).
const REVERSE_SPEED: f32 = 17.881_09;
/// The speed under which a nitrous shot is a SPRAY SHOW (0x5ECDC0:
/// |car +0x430| × 2.237 under 5 mph, or the gas off).
const SPRAY_MPH: f32 = 5.0;
/// "No hit yet" (0x79DC20): misc +0x2C is 0 or 0x7FFFFFFF.
const NO_HIT: f32 = 99_999.9;

/// The nitrous tank a change of `points` in the style total is worth ([`crate::nitrous::refill_fraction`]).
pub use crate::nitrous::refill_fraction;

/// A push that passed the contact callback's gate (0x56FB30; see
/// `CarState::style_contact`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// The record's summed pushes (+0x60).
    pub strength: f32,
    /// Car against car (0x592C70), and either car traffic (owner kind 3,
    /// car +0x14 → +4).
    pub car: bool,
    pub traffic: bool,
}

/// One of the car's pairs with another car (the pair list 0x86B490 as
/// 0x56F180 walks it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NearPair {
    /// The pair's identity (when it was made; the list's order).
    pub id: u64,
    /// The hull distance its last detection found (pair +0x10).
    pub distance: Option<f32>,
    /// It has a contact record (pair +0x244).
    pub contact: bool,
    /// It goes: no record and the boxes apart (0x592E70).
    pub parting: bool,
    /// Neither car ghosted (0x5EBFB0, the timer +0x974 / +0x978) nor
    /// hidden (0x5EC070, +0x998).
    pub fit: bool,
    /// |v − v_other| (the bodies' vt+0x0C, 0x4011B0, 0x440F70), m/s.
    pub relative_speed: f32,
    /// The cars' distance (+0x60, 0x401150) and how far this car is above
    /// the other (+0x68 difference), m.
    pub apart: f32,
    pub above: f32,
}

/// The misc style manager (0x56CE00, racer +0x1DC): the detectors' state.
#[derive(Clone, Debug)]
pub struct Misc {
    /// +0x08 / +0x09: no hit over 10 since the last section / lap
    /// (1 from the constructor; a hit over 10 clears them and +0x0A, whose
    /// reader is **not traced**).
    clean_section: bool,
    clean_lap: bool,
    /// +0x10: the best place reached (0 until the first look).
    best_place: u32,
    /// +0x14 / +0x18: the 360's half-turns and the heading they count
    /// from, degrees.
    spin: u8,
    spin_from: f32,
    /// +0x1C: the J-turn's stage.
    j_turn: u8,
    /// +0x1D / +0x1E / +0x20: the near miss's stage, whether it is a jump
    /// over (CLEARED ACCIDENT), and its pair.
    near: u8,
    cleared: bool,
    near_pair: Option<u64>,
    /// +0x2C / +0x30: when the last hit was (s on the style clock) and its
    /// strength.
    hit: Option<(f32, f32)>,
    /// The racer stats' burnout (racer +0x1A0 +0x28, −1 from 0x5EDE20 /
    /// 0x5F73E0): −1 waiting, 0 spinning, else the distance at its end.
    burnout: f32,
    /// The distance driven while the race is on (car +0x410: += dt × |v|
    /// for 0 < |v| < 1000, 0x604874), m.
    driven: f32,
    /// The kind-0xB route triggers already scored (their +0x1A word, set
    /// by 0x56F970; 0x5D59D0 clears every trigger's from the 0x1600-byte
    /// object's ctor 0x5FF4A0, which World_Start 0x60A690 builds on every
    /// track load, TrackLoader::LoadHandler step 6 at 0x57F962, after
    /// step 3 has loaded the Paths file: once per race or free-roam load,
    /// as the clone makes a new `Misc`).
    shortcuts: Vec<usize>,
    /// The lap element's +0x65 (0x4AA4B5): a lap was finished before.
    lap_seen: bool,
}

impl Default for Misc {
    fn default() -> Misc {
        Misc { clean_section: true, clean_lap: true, best_place: 0, spin: 0, spin_from: 0.0, j_turn: 0, near: 0, cleared: false, near_pair: None, hit: None, burnout: -1.0, driven: 0.0, shortcuts: Vec::new(), lap_seen: false }
    }
}

/// What the detectors read of the player's racer and car for the frame.
#[derive(Clone, Debug)]
pub struct Detect {
    /// The forward speed (car +0x42C) and |velocity| (car +0x430), m/s.
    pub forward_speed: f32,
    pub speed: f32,
    /// A nitrous shot started this frame on the player's key (0x6034E6).
    pub shot_started: bool,
    /// The input block's gas (car +0x30 → +0x20C), 0x5ECDC0's other term.
    pub gas: f32,
    /// What the nitrous key handler read (**observed** under
    /// Wine: 0x6034C6 runs before the frame's input write 0x5EB924, so
    /// 0x5ECDC0 sees the previous world update's gas and car +0x430 = |v|):
    /// `CarState::nitrous_key`. `None` falls back to `gas` / `speed`.
    pub key: Option<(f32, f32)>,
    /// Race status 4 (0x890118 → +0): the race is on.
    pub racing: bool,
    /// A race (race type 0x89E7B0 non-zero; free roam is 0).
    pub race: bool,
    /// The race clock (race status +0x14 × 0.00025), s.
    pub race_time: f32,
    /// The racers (0x89E7C0 + 0x89E7C4 + 0x89E7C8 + 0x89E7CC).
    pub racers: u32,
    /// The racer's place (+0x0A, 1 the leader).
    pub place: u32,
    /// The racer is done (0x5EE260: status +0x10 set in a race).
    pub done: bool,
    /// This frame the racer passed its furthest line (+0x0E) / finished a
    /// new lap (+0x0D) (0x5F7C40).
    pub new_section: bool,
    pub new_lap: bool,
    /// Wheels down (car +0x424), the up vector's z (+0xB8), a live contact
    /// record (car +0x3C → +0x18).
    pub wheels_down: usize,
    pub up_z: f32,
    pub touching: bool,
    /// The height over the ground (0x56F10D: car +0x68 − the centre of
    /// mass's z (0x5ED410 → +8) − the ground under car +0x60, 0x5E7E80), m.
    pub height: f32,
    /// The slip angle (0x59FA20), radians; the yaw rate (car +0x118),
    /// rad/s; the heading (u16 car +0x40C × 360 / 65536), degrees.
    pub slip: f32,
    pub yaw_rate: f32,
    pub heading: f32,
    /// The draft (car +0x4DC, [`draft`]).
    pub draft: f32,
    /// The car is ghosted (0x5EBFB0).
    pub ghosted: bool,
    /// The mean slip of the wheels 0x592880 counts (car +0x3B4 + 8i).
    pub wheel_slip: f32,
    /// The frame's time step, s.
    pub dt: f32,
    /// The contacts since the last frame and the car's pairs.
    pub hits: Vec<Hit>,
    pub near: Vec<NearPair>,
    /// The first kind-0xB route trigger (0x88F298, 0x5D9C00(car +0x8E0,
    /// 0xB, 0)) holding the car's road-locator point (car +0x8D8 +0x08;
    /// the clone takes the car's x, y), by record order.
    pub shortcut: Option<usize>,
    /// BEST LAP's test (the lap element 0x4AA1B0, only while its mask is
    /// on: racing with the lap info option): a lap finished this frame
    /// (racer +0x0D) with the time it took (0x5EE2A0, racer +0x44 + 4·i)
    /// and the best lap (0x5EE2C0, +0x6C + 4·i), s.
    pub lap_done: Option<(f32, f32)>,
    /// The hydraulics: this frame's kicks (car +0x408 + i set), the
    /// corners moving (0x5ED480 state 2 or 3) and a bounce input on
    /// (0x5ED2A0).
    pub hydro_kicks: usize,
    pub hydro_moving: usize,
    pub bounce_input: bool,
}

impl Default for Detect {
    fn default() -> Detect {
        Detect {
            forward_speed: 0.0,
            speed: 0.0,
            shot_started: false,
            gas: 0.0,
            key: None,
            racing: false,
            race: false,
            race_time: 0.0,
            racers: 1,
            place: 1,
            done: false,
            new_section: false,
            new_lap: false,
            wheels_down: 4,
            up_z: 1.0,
            touching: false,
            height: 0.0,
            slip: 0.0,
            yaw_rate: 0.0,
            heading: 0.0,
            draft: 0.0,
            ghosted: false,
            wheel_slip: 0.0,
            dt: 0.0,
            hits: Vec::new(),
            shortcut: None,
            lap_done: None,
            near: Vec::new(),
            hydro_kicks: 0,
            hydro_moving: 0,
            bounce_input: false,
        }
    }
}

/// The draft car +0x4DC (0x5EBD90, every car each frame from 0x609F00):
/// the sum over the other cars (the car list 0x890080) of what each gives
/// (0x5EBC30), at most 0.9 (0x7A55C4; the sum stops once it reaches it).
/// `others`: each other car's position (+0x60) and velocity (+0x70).
pub fn draft(me: glam::Vec3, others: impl IntoIterator<Item = (glam::Vec3, glam::Vec3)>) -> f32 {
    let mut sum = 0.0f32;
    for (at, v) in others {
        sum += draft_from(me, at, v);
        if sum >= 0.9 {
            break;
        }
    }
    sum.min(0.9)
}

/// 0x5EBC30: the other car moving faster than 1 m/s (0x8023D8) and within
/// 60 m (0x8023EC); c = its direction of travel · (it − me) / distance;
/// past 0.95 (0x7A55C0), a = (c − 0.95) / 0.05; its draft's length L =
/// a × the speed curve 0x89CF30 (from 0x8023D0: (0, 0), (1, 0), (5, 10),
/// (150, 60) — m/s to m); the draft is the curve 0x8023F0 at
/// clamp(distance / L, 0, 1): {1, 0.9, 0.75, 0.55, 0.3, 0} in fifths.
fn draft_from(me: glam::Vec3, at: glam::Vec3, v: glam::Vec3) -> f32 {
    let speed = v.length();
    if speed <= 1.0 {
        return 0.0;
    }
    let gap = at - me;
    let d = gap.length();
    if d < 1e-5 || d > 60.0 {
        return 0.0;
    }
    let c = v.normalize_or_zero().dot(gap) / d;
    let a = (c - 0.95) / (1.0 - 0.95);
    if a <= 0.0 {
        return 0.0;
    }
    let length = lerp_points(&[(0.0, 0.0), (1.0, 0.0), (5.0, 10.0), (150.0, 60.0)], speed) * a;
    if length <= 1e-5 {
        return 0.0;
    }
    let t = (d / length).clamp(0.0, 1.0) * 5.0;
    const FALL: [f32; 6] = [1.0, 0.9, 0.75, 0.55, 0.3, 0.0];
    let i = t.trunc() as usize;
    if t <= 0.0 {
        return FALL[0];
    }
    if i >= FALL.len() - 1 {
        return FALL[FALL.len() - 1];
    }
    let f = t - i as f32;
    (1.0 - f) * FALL[i] + f * FALL[i + 1]
}

/// 0x5767A0: a piecewise-linear curve through (x, y) points, flat past
/// its ends.
fn lerp_points(points: &[(f32, f32)], x: f32) -> f32 {
    if x <= points[0].0 {
        return points[0].1;
    }
    let last = points[points.len() - 1];
    if x >= last.0 {
        return last.1;
    }
    for w in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x >= x0 && x <= x1 {
            let span = x1 - x0;
            return if span.abs() < 1e-6 { y0 } else { y0 + (y1 - y0) * (x - x0) / span };
        }
    }
    points[0].1
}

/// 0x56D030: the angle between two headings, degrees (at most 180).
fn turned(a: f32, b: f32) -> f32 {
    let d = (a - b).abs();
    d.min(360.0 - d)
}

impl Misc {
    /// Seconds since the last hit (0x56CFE0), 99999.9 with none.
    fn since_hit(&self, now: f32) -> f32 {
        self.hit.map_or(NO_HIT, |(t, _)| now - t)
    }

    /// A trick's multiplier back to 1 (+0x1C), the combo broken.
    fn reset_multiplier(style: &mut StyleManager, i: usize) {
        if let Some(Some(t)) = style.tricks.get_mut(i) {
            t.multiplier = 1.0;
        }
    }

    /// 0x56FB30 for a hit past its gate: over 10 the clean section, clean
    /// lap and +0x0A flags clear; a car hit with traffic in it feeds
    /// TRAFFIC HIT at min(1, strength / 40) (0x79DB30) unless the racer is
    /// done, and any car hit sets AVOIDED ACCIDENT's multiplier back to 1;
    /// then the hit's time and strength are kept.
    fn contact(&mut self, style: &mut StyleManager, h: &Hit, done: bool, now: f32) {
        if h.strength > 10.0 {
            self.clean_section = false;
            self.clean_lap = false;
        }
        if h.car {
            if h.traffic && !done {
                style.feed(TRAFFIC_HIT, (h.strength / 40.0).min(1.0), now);
            }
            Self::reset_multiplier(style, AVOIDED_ACCIDENT);
        }
        self.hit = Some((now, h.strength));
    }

    /// 0x56EEF0: a new section (+0x0E) scores CLEAN SECTION when no hit
    /// over 10 came since the last, else the flag is set again and the
    /// trick's multiplier goes back to 1; a new lap (+0x0D) the same for
    /// CLEAN LAP, and LEAD LAP when the racer leads (+0x0A = 1; in drift
    /// the points board, not ported: drift has neither trick).
    fn sections(&mut self, style: &mut StyleManager, d: &Detect, now: f32) {
        if d.new_section {
            if self.clean_section {
                style.feed(CLEAN_SECTION, 1.0, now);
            } else {
                self.clean_section = true;
                Self::reset_multiplier(style, CLEAN_SECTION);
            }
        }
        if d.new_lap {
            if self.clean_lap {
                style.feed(CLEAN_LAP, 1.0, now);
            } else {
                self.clean_lap = true;
                Self::reset_multiplier(style, CLEAN_LAP);
            }
            if d.place == 1 {
                style.feed(LEAD_LAP, 1.0, now);
            } else {
                Self::reset_multiplier(style, LEAD_LAP);
            }
        }
    }

    /// 0x56F030: HANG TIME while the race is on, the racer not done, no
    /// hydraulics bounce input (0x5ED2A0), all
    /// four wheels up, no live contact, more than 0.75 m (0x79DB48) over
    /// the ground; a run that is not yet scoring also needs the car upright
    /// (up.z over 0.8, 0x7850EC) and no hit in the last 0.3 s.
    fn hang_time(&self, style: &mut StyleManager, d: &Detect, now: f32) {
        let running = style.trick(HANG_TIME).is_some_and(|t| t.state == TrickState::Running);
        let since = self.since_hit(now);
        let ok = d.racing && !d.done && !d.bounce_input && d.wheels_down == 0 && !d.touching && (d.up_z > 0.8 || running) && (since > 0.3 || running);
        if ok && d.height > 0.75 {
            style.feed(HANG_TIME, 1.0, now);
        }
    }

    /// 0x56F4D0: POWER SLIDE with |slip| over 5° (0x79DB3C), faster than
    /// 40 mph (0x79DB40), all four wheels down and no live contact, at
    /// min(1, speed × |slip| / 20) (0x79DB44); a hit over 5 (0x79DB38) in
    /// the last 0.1 s (0x79DB28) loses a run under way instead.
    fn power_slide(&self, style: &mut StyleManager, d: &Detect, now: f32) {
        let slip = d.slip.abs();
        let mut rate = 1.0;
        let mut ok = false;
        if slip > 5f32.to_radians() && d.speed > 17.881_1 && d.wheels_down == 4 {
            rate = (d.speed * slip / 20.0).min(1.0);
            ok = !d.touching;
        }
        if self.since_hit(now) < 0.1 && self.hit.is_some_and(|(_, s)| s > 5.0) {
            style.lose(POWER_SLIDE);
            return;
        }
        if ok {
            style.feed(POWER_SLIDE, rate, now);
        }
    }

    /// 0x56F630: the 360. With all four wheels down, a yaw rate over 2
    /// rad/s (0x79DB4C) starts it from the heading; it goes on while the
    /// yaw rate stays over 1 (0x79DB50), and each time the heading has
    /// turned 170° (0x79DC3C) from the last mark it counts a half-turn
    /// and marks again; the second half-turn scores "360".
    fn spin(&mut self, style: &mut StyleManager, d: &Detect, now: f32) {
        let yaw = d.yaw_rate.abs();
        if d.wheels_down != 4 || (self.spin == 0 && yaw <= 2.0) || (self.spin > 0 && yaw <= 1.0) {
            self.spin = 0;
        } else if self.spin == 0 {
            self.spin = 1;
            self.spin_from = d.heading;
        } else if self.spin <= 2 && turned(d.heading, self.spin_from) >= 170.0 {
            self.spin += 1;
            self.spin_from = d.heading;
        }
        if self.spin == 3 {
            style.feed(SPIN, 1.0, now);
            self.spin = 0;
        }
    }

    /// 0x56F180: the near miss. While the car is not ghosted, faster than
    /// 30 mph (0x79DB64) and its last hit more than 0.2 s (0x79DB68) ago,
    /// it looks for the first of its pairs (list order) that stays and
    /// whose cars are both fit: on the ground, one at most 1.5 m away
    /// (pair +0x10, 0x79DB5C) closing or parting faster than 80 mph
    /// (0x79DB60); with all four wheels up, one whose car is within 3 m
    /// (0x79DB54) and more than 1 m (0x79DB58) below (a jump over). Then
    /// it watches that pair: a contact ends it; the pair going, or its
    /// distance past 1.875 m (1.5 × 1.25, 0x7862E8), scores AVOIDED
    /// ACCIDENT, or CLEARED ACCIDENT for a jump over.
    fn near_miss(&mut self, style: &mut StyleManager, d: &Detect, now: f32) {
        if d.ghosted || d.speed <= 13.410_8 || self.since_hit(now) <= 0.2 {
            self.near_pair = None;
            self.near = 0;
        } else if self.near == 0 {
            self.cleared = false;
            self.near_pair = None;
            for p in d.near.iter().filter(|p| !p.parting && p.fit) {
                if d.wheels_down == 0 {
                    if p.apart < 3.0 && p.above > 1.0 {
                        self.cleared = true;
                        self.near_pair = Some(p.id);
                        break;
                    }
                } else if p.distance.is_some_and(|x| x < 1.5) && p.relative_speed > 35.762_2 {
                    self.near_pair = Some(p.id);
                    break;
                }
            }
            if self.near_pair.is_some() {
                self.near = 1;
            }
        } else if self.near == 1 {
            match d.near.iter().find(|p| Some(p.id) == self.near_pair) {
                None => self.near = 2,
                Some(p) if p.contact => self.near = 0,
                Some(p) if p.parting || p.distance.is_some_and(|x| x > 1.5 * 1.25) => self.near = 2,
                Some(_) => {}
            }
        }
        if self.near == 2 {
            style.feed(if self.cleared { CLEARED_ACCIDENT } else { AVOIDED_ACCIDENT }, 1.0, now);
            self.near = 0;
            self.cleared = false;
            self.near_pair = None;
        }
    }

    /// 0x56F730: HEAD START (a race on with more than one racer, between
    /// 1 and 1.5 s (0x79DB70 / 0x79DB74) in, leading, faster than 5 mph
    /// (0x79DB6C)); past 1.5 s each new best place under 5 scores GAINED
    /// 1ST … 4TH; DRAFTING faster than 20 m/s (0x79DB88) with a draft over
    /// 0.25 (0x79DB8C), at the draft; REVERSE below −40 mph; a hit over 3
    /// (0x79DB78) less than 0.3 s ago loses a REVERSE under way (0x56D5D0).
    fn place(&mut self, style: &mut StyleManager, d: &Detect, now: f32) {
        if d.race && d.racing && d.racers > 1 && d.race_time > 1.0 && d.race_time < 1.5 && d.place == 1 && d.speed > 2.235_136 {
            style.feed(HEAD_START, 1.0, now);
        }
        if d.race_time > 1.5 {
            if self.best_place == 0 {
                self.best_place = d.place;
            }
            if self.best_place > 1 && d.place < self.best_place && d.place < 5 {
                self.best_place = d.place;
                style.feed(GAINED_1ST + 1 - d.place as usize, 1.0, now);
            }
        }
        if d.speed > 20.0 && d.draft > 0.25 {
            style.feed(DRAFTING, d.draft, now);
        }
        if d.forward_speed < -REVERSE_SPEED {
            style.feed(REVERSE, 1.0, now);
        }
        if self.since_hit(now) < 0.3 && self.hit.is_some_and(|(_, s)| s > 3.0) {
            style.lose(REVERSE);
        }
    }

    /// 0x56FA70: the J-turn. With all four wheels down and no hit in the
    /// last 0.1 s: reversing at 30 mph (0x79DB80) or more starts it; then,
    /// never under 20 mph (0x79DB84), going forward faster than 20 mph
    /// scores J-TURN.
    fn j_turn(&mut self, style: &mut StyleManager, d: &Detect, now: f32) {
        if d.wheels_down != 4 || self.since_hit(now) <= 0.1 {
            self.j_turn = 0;
            return;
        }
        match self.j_turn {
            0 if d.forward_speed <= -13.410_8 => self.j_turn = 1,
            1 if d.speed < 8.940_55 => self.j_turn = 0,
            1 if d.forward_speed > 8.940_55 => self.j_turn = 2,
            _ => {}
        }
        if self.j_turn == 2 {
            style.feed(J_TURN, 1.0, now);
            self.j_turn = 0;
        }
    }

    /// The racer stats' burnout (0x5F7420 at 0x5F771B, every racer each
    /// frame, while the race is on and not race type 7): in the first 5 s
    /// (0x7843E8) a mean wheel slip over 0.5 (0x7A569C) starts it; once
    /// the slip drops under 0.5 it ends, keeping the distance driven, and
    /// feeds BURNOUT at that distance (10 points a metre, over 50).
    fn burnout(&mut self, style: &mut StyleManager, d: &Detect, now: f32) {
        if d.racing && d.speed > 0.0 && d.speed < 1000.0 {
            self.driven += d.dt * d.speed;
        }
        if !d.racing || (self.burnout != 0.0 && self.burnout != -1.0) {
            return;
        }
        if self.burnout == -1.0 && d.race_time < 5.0 {
            if d.wheel_slip > 0.5 {
                self.burnout = 0.0;
            }
            return;
        }
        if self.burnout == 0.0 && d.wheel_slip < 0.5 {
            self.burnout = self.driven;
            style.feed(BURNOUT, self.driven, now);
        }
    }
}

/// One frame of the misc style manager: the contacts the physics gave
/// (0x56FB30), then the detectors 0x56FF40 runs (offline always:
/// 0x5F0100 is false) — 0x56EEF0, 0x56F030, 0x56F4D0, 0x56F630, 0x56F180,
/// 0x56F730, 0x56FA70 — and the racer stats' burnout (0x5F7420, which
/// Racer_Update 0x6016B0 runs first). SPRAY SHOW (the player's nitrous
/// event, 0x6034C6 → 0x60352D): a shot started while 0x5ECDC0 gives the
/// purge — under 5 mph, or with the gas (input +0x20C) at most 0.001
/// (**verified** 2026-11-04; the clone reads this frame's gas where the
// prov: inferred style.detect
/// key handler reads the last frame's, **inferred** equivalent). SHORTCUT (0x56F970) and BEST
/// LAP (0x4AA4AA, the lap element's) too; last BOUNCE and HYDROS
/// (0x56F9C0, the hydraulics).
pub fn detect(misc: &mut Misc, style: &mut StyleManager, d: &Detect, now: f32) {
    for h in &d.hits {
        misc.contact(style, h, d.done, now);
    }
    misc.burnout(style, d, now);
    misc.sections(style, d, now);
    misc.hang_time(style, d, now);
    misc.power_slide(style, d, now);
    misc.spin(style, d, now);
    misc.near_miss(style, d, now);
    misc.place(style, d, now);
    misc.shortcut(style, d, now);
    misc.j_turn(style, d, now);
    misc.best_lap(style, d, now);
    let (gas, speed) = d.key.unwrap_or((d.gas, d.speed));
    if d.shot_started && (gas.abs() <= 0.001 || speed * 2.237 < SPRAY_MPH) {
        style.feed(SPRAY_SHOW, 1.0, now);
    }
    hydraulics(style, d, now);
}

/// 0x56F9C0, the detectors' last (0x56FF9F): BOUNCE at rate 1 when two or
/// more wheels were kicked this frame; HYDROS at 0.25 per moving corner
/// while no bounce input is on.
fn hydraulics(style: &mut StyleManager, d: &Detect, now: f32) {
    if d.hydro_kicks > 1 {
        style.feed(BOUNCE, 1.0, now);
    }
    if d.hydro_moving > 0 && !d.bounce_input {
        style.feed(HYDROS, d.hydro_moving as f32 * 0.25, now);
    }
}

impl Misc {
    /// 0x56F970 (SHORTCUT): the first kind-0xB route trigger holding the
    /// car, not scored yet (+0x1A = 0), is marked and feeds SHORTCUT at
    /// rate 1 (200 points) when the style manager has it (+0x50, its
    /// +0x10 set). The trigger found first decides: an overlapping one
    /// behind it is not looked at.
    fn shortcut(&mut self, style: &mut StyleManager, d: &Detect, now: f32) {
        let Some(i) = d.shortcut else { return };
        if self.shortcuts.contains(&i) {
            return;
        }
        self.shortcuts.push(i);
        style.feed(SHORTCUT, 1.0, now);
    }

    /// BEST LAP (0x4AA444..0x4AA4B5 in the lap element's update 0x4AA1B0,
    /// for the player's racer): on a frame with a new lap (+0x0D) and a
    /// lap time set (0x5EE280 neither 0 nor 0x7FFFFFFF), the lap's time no
    /// longer than the best and a lap finished before (+0x65) feed slot
    /// 0x17 at rate 1 (0x56ED20; 500 points); then +0x65 = 1. (With race
    /// type 0x89E7B0 = 2 and 0x89E818 neither 0 nor 0x7FFFFFFF, 0x4AA3B4
    /// takes another branch: element +0x2C shows the lap clock (0x5EE280)
    /// minus 0x865188 plus 0x89E818 and jumps to 0x4AA5E1, past BEST LAP.
    /// Only 0x5400A8 can store 2 there, from 0x7FAA28, which 0x5802F0
    /// reads from the 0x8650CC record/playback stream; every other writer
    /// stores 0, 1, 8 or 0x5F0190's 0/1/3/5. The clone makes no type 2;
    // prov: open style.best_lap
    /// what 0x7FAA28 holds in play is **open**.)
    fn best_lap(&mut self, style: &mut StyleManager, d: &Detect, now: f32) {
        let Some((last, best)) = d.lap_done else { return };
        if last <= best && self.lap_seen {
            style.feed(BEST_LAP, 1.0, now);
        }
        self.lap_seen = true;
    }
}

/// The refill (0x601737, Race Breaker on): the total's change since the
/// last frame (racer +0x194, set to the total at the racer's set-up,
/// 0x5FC46A) goes into the tank (0x592650). Negative changes (TRAFFIC HIT,
/// WALL IMPACT) take nitrous away.
pub fn refill(style: &StyleManager, base: &mut f32, tank: Option<&mut Tank>) {
    let delta = style.total - *base;
    *base = style.total;
    if let Some(t) = tank {
        t.refill(refill_fraction(delta));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::{StyleManager, TrickDef};

    fn defs() -> Vec<TrickDef> {
        let def = |index: u8, mask: u8, points: f32, max_count: i8| TrickDef {
            name: 0,
            index,
            continuous: false,
            mask,
            min_time: 0.0,
            timeout: 0.0,
            threshold: 0.0,
            cap: 0.0,
            max_multiplier: 1.0,
            multiplier_step: 1.0,
            points,
            max_count,
        };
        (0..39u8)
            .map(|i| match i as usize {
                SPRAY_SHOW => def(i, 0x49, 100.0, 1),
                TRAFFIC_HIT => TrickDef { threshold: -30.0, ..def(i, 0x49, -50.0, -1) },
                AVOIDED_ACCIDENT => TrickDef { max_multiplier: 5.0, ..def(i, 0x4B, 100.0, -1) },
                CLEARED_ACCIDENT | SPIN | J_TURN => def(i, 0x49, 400.0, -1),
                BURNOUT => TrickDef { threshold: 50.0, ..def(i, 0x49, 10.0, -1) },
                _ => def(i, 0, 0.0, -1),
            })
            .collect()
    }

    fn step(misc: &mut Misc, s: &mut StyleManager, d: &Detect, now: f32) {
        detect(misc, s, d, now);
        s.update(0.016, now);
    }

    #[test]
    fn the_draft_falls_off_behind_a_car() {
        use glam::Vec3;
        // Right behind a car at 30 m/s: L = 10 + 50 × 25 / 145 = 18.6 m.
        let ahead = (Vec3::new(5.0, 0.0, 0.0), Vec3::new(30.0, 0.0, 0.0));
        let d = draft(Vec3::ZERO, [ahead]);
        let t: f32 = 5.0 / (10.0 + 50.0 * 25.0 / 145.0) * 5.0;
        let f = t - t.trunc();
        let want = [1.0, 0.9, 0.75, 0.55, 0.3, 0.0][t as usize] * (1.0 - f) + [1.0, 0.9, 0.75, 0.55, 0.3, 0.0][t as usize + 1] * f;
        assert!((d - want).abs() < 1e-5, "{d} {want}");
        // Beside it, or it standing, gives nothing; two cars cap at 0.9.
        assert_eq!(draft(Vec3::ZERO, [(Vec3::new(0.0, 5.0, 0.0), Vec3::new(30.0, 0.0, 0.0))]), 0.0);
        assert_eq!(draft(Vec3::ZERO, [(Vec3::new(5.0, 0.0, 0.0), Vec3::ZERO)]), 0.0);
        assert_eq!(draft(Vec3::ZERO, [(Vec3::new(0.5, 0.0, 0.0), Vec3::new(30.0, 0.0, 0.0)); 3]), 0.9);
    }

    #[test]
    fn a_traffic_hit_costs_points_and_breaks_the_near_miss_combo() {
        let mut s = StyleManager::new(defs(), 1);
        let mut misc = Misc::default();
        s.tricks[AVOIDED_ACCIDENT].as_mut().unwrap().multiplier = 3.0;
        // Strength 20: rate 0.5, −25 is not under −30; 40: −50 banks.
        let hit = |strength| Detect { hits: vec![Hit { strength, car: true, traffic: true }], ..Default::default() };
        step(&mut misc, &mut s, &hit(20.0), 0.0);
        assert_eq!(s.total, 0.0);
        assert_eq!(s.trick(AVOIDED_ACCIDENT).unwrap().multiplier, 1.0);
        step(&mut misc, &mut s, &hit(40.0), 0.1);
        step(&mut misc, &mut s, &Detect::default(), 0.2);
        assert_eq!(s.total, -50.0);
    }

    #[test]
    fn a_near_miss_scores_when_the_pair_parts_untouched() {
        let mut s = StyleManager::new(defs(), 1);
        let mut misc = Misc::default();
        let pair = |distance: f32, contact| NearPair { id: 7, distance: Some(distance), contact, parting: false, fit: true, relative_speed: 40.0, apart: 4.0, above: 0.0 };
        let at = |p: Vec<NearPair>| Detect { speed: 20.0, near: p, ..Default::default() };
        step(&mut misc, &mut s, &at(vec![pair(1.0, false)]), 0.0);
        step(&mut misc, &mut s, &at(vec![pair(1.2, false)]), 0.016);
        assert_eq!(s.total, 0.0);
        step(&mut misc, &mut s, &at(vec![pair(2.0, false)]), 0.032);
        step(&mut misc, &mut s, &at(vec![]), 0.048);
        assert_eq!(s.total, 100.0);
        // A touch on the watched pair ends it without points.
        step(&mut misc, &mut s, &at(vec![pair(1.0, false)]), 1.0);
        step(&mut misc, &mut s, &at(vec![pair(0.0, true)]), 1.016);
        step(&mut misc, &mut s, &at(vec![]), 1.032);
        assert_eq!(s.total, 100.0);
    }

    #[test]
    fn spins_and_j_turns_need_the_wheels_down() {
        let mut s = StyleManager::new(defs(), 1);
        let mut misc = Misc::default();
        for (k, h) in [0.0, 90.0, 175.0, 270.0, 350.0].into_iter().enumerate() {
            step(&mut misc, &mut s, &Detect { yaw_rate: 3.0, heading: h, ..Default::default() }, k as f32 * 0.1);
        }
        assert_eq!(s.total, 400.0);
        let j = |forward_speed: f32| Detect { forward_speed, speed: forward_speed.abs(), ..Default::default() };
        for (k, v) in [-15.0, -10.0, 9.0, 12.0].into_iter().enumerate() {
            step(&mut misc, &mut s, &j(v), 1.0 + k as f32 * 0.1);
        }
        step(&mut misc, &mut s, &Detect::default(), 2.0);
        assert_eq!(s.total, 800.0);
    }

    #[test]
    fn a_burnout_scores_the_distance_driven_when_it_ends() {
        let mut s = StyleManager::new(defs(), 1);
        let mut misc = Misc::default();
        let b = |wheel_slip, race_time, dt| Detect { racing: true, race: true, wheel_slip, race_time, speed: 10.0, dt, ..Default::default() };
        step(&mut misc, &mut s, &b(3.0, 0.5, 0.0), 0.0);
        step(&mut misc, &mut s, &b(3.0, 6.0, 0.8), 0.1);
        assert_eq!(s.total, 0.0);
        step(&mut misc, &mut s, &b(0.2, 6.1, 0.1), 0.2);
        step(&mut misc, &mut s, &Detect::default(), 0.3);
        assert_eq!(s.total, 90.0);
    }

    #[test]
    fn a_spray_show_scores_once_and_fills_the_tank() {
        let mut s = StyleManager::new(defs(), 1);
        let mut base = s.total;
        let mut tank = Tank::new(4.0, 0);
        tank.left = 0;
        let d = Detect { shot_started: true, ..Default::default() };
        let mut misc = Misc::default();
        detect(&mut misc, &mut s, &d, 0.0);
        s.update(0.016, 0.016);
        refill(&s, &mut base, Some(&mut tank));
        assert_eq!(s.total, 100.0);
        assert_eq!(tank.left, 1067);
        detect(&mut misc, &mut s, &d, 1.0);
        s.update(0.016, 1.016);
        s.update(0.016, 1.032);
        refill(&s, &mut base, Some(&mut tank));
        assert_eq!((s.total, tank.left), (100.0, 1067));
    }

    /// 0x5ECDC0 at the shot's start (0x60350F): moving at 20 m/s with the
    /// gas on is no SPRAY SHOW; with the gas off it is one.
    #[test]
    fn a_spray_show_needs_the_purge_test() {
        for (gas, scored) in [(1.0, 0.0), (0.0, 100.0)] {
            let mut s = StyleManager::new(defs(), 1);
            let d = Detect { shot_started: true, speed: 20.0, gas, ..Default::default() };
            detect(&mut Misc::default(), &mut s, &d, 0.0);
            s.update(0.016, 0.016);
            assert_eq!(s.total, scored, "gas {gas}");
        }
    }

    /// Observed under Wine: the key handler 0x6034C6 runs
    /// before the frame's input write 0x5EB924, so 0x5ECDC0 tests the
    /// previous world update's gas and |v|: gas and throttle pressed in
    /// the same frame (gas 1 now, 0 before) is a SPRAY SHOW; a car that
    /// was standing (|v| 0 before) is one even at 20 m/s now.
    #[test]
    fn the_nitrous_key_reads_the_previous_update() {
        for (key, scored) in [((0.0, 20.0), 100.0), ((1.0, 20.0), 0.0), ((1.0, 0.0), 100.0)] {
            let mut s = StyleManager::new(defs(), 1);
            let d = Detect { shot_started: true, speed: 20.0, gas: 1.0, key: Some(key), ..Default::default() };
            detect(&mut Misc::default(), &mut s, &d, 0.0);
            s.update(0.016, 0.016);
            assert_eq!(s.total, scored, "key {key:?}");
        }
    }
}

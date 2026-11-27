use super::script::{SMS_LOC_EVENTS_OUT_THERE, SMS_LOC_GO_TO_GARAGE};

/// 0x7993C8: the outrun lines' second key, by the start's pick.
const OUTRUN_LINES: [u32; 5] = [1, 2, 4, 8, 0x200];

/// Rachel's hints (career +0x887C = 0x8635BC, 0x190 bytes, saved with the
/// career): eight timed slots {ticks, kind, value} at +0xA8 posted by
/// 0x5111F0 (the first slot whose kind is 0; none free drops the post),
/// counted down by the free-roam driving time (0x551080 → 0x54BD40, from
/// 0x551330 only while the D-Day script is not running and the driving
/// time is above 0), one firing at a time when due and Rachel is free
/// (0x53E0C0). Most kinds are Rachel's voice only (speech bank
/// UG2_Speech, `audio::speech::lines`).
#[derive(Clone, Debug, PartialEq)]
pub struct Hints {
    /// +1: set with the counter at 3 when DDAY_EVENT_A is over (0x524340).
    pub dday_over: bool,
    /// +4: Rachel's complaints about stage-0 races (kind 1), 0..3.
    pub complaints: u8,
    /// +8: kind 5's line said (once).
    pub stage1_line: bool,
    /// +9: the garage hint posted (kind 4, once).
    pub garage: bool,
    /// +10: the events hint posted (kinds 6 and 10, once).
    pub events: bool,
    /// +0xB / +0xC: UI_SponsorPopup's lines (0x525520, once) — +0xC also
    /// gates 0x551080's kind 0xF.
    pub sponsor: bool,
    pub sponsor_said: bool,
    /// +0xD + n: the DVD rating flags (set by 0x525400 with ftol(milestone
    /// +0x3C) and by 0x551080's kind 0xF).
    pub ratings: [bool; 11],
    /// +0x18: a DVD hint (kind 0xD) pending; cleared when it fires.
    pub dvd: bool,
    /// +0x1A: the outrun line's pick (0x511000: Rand_Int(5), an index into
    /// {1, 2, 4, 8, 0x200} at 0x7993C8), which the end's line reuses.
    pub outrun_pick: u8,
    /// +0x19: the rating 0x551080 posts kind 0xF for, written (as career
    /// +0x8895) only by 0x5331D0: ftol of the highest rating among the
    /// stage's DVD covers not reached yet, else 0 (`Profile::offer_covers`).
    pub rating_due: u8,
    /// +0x28: set by 0x5243E0(2) (from 0x566EEE in a shop's purchase
    /// flow), cleared by 0x551080 on the next driving step.
    pub fitted: bool,
    /// +0x2C (count +0xA4, at most 30): the covers Rachel has spoken of
    /// (0x525400; 0x5112F0 looks one up).
    pub covers: Vec<u32>,
    /// +0xA5: the stage the next kind-12 line names (0x528330).
    pub stage_line: u8,
    /// +0xA6: the showcase's line said (0x525860, once).
    pub showcase: bool,
    /// +0xA8: the slots {ticks, kind, value}.
    pub slots: [(i32, u32, u32); 8],
    /// +0x110 / +0x114: the driving time in ticks last step, and the sum.
    pub(super) drive_ticks: Option<i32>,
    pub drive_sum: u32,
    /// +0x11C: the front-end clock 0x8651A8's ticks summed over the calls
    // prov: inferred frontend.dday.holds_input
    /// of 0x551080 (+0x118 the last reading; **inferred** across a load:
    /// the first call after it adds nothing).
    pub fe_clock: u32,
    /// +0x120 + 4 · kind: when each front-end line (0x5243E0) last played,
    /// on `fe_clock`; 0 never.
    pub fe_last: [u32; 28],
    /// 0x8641F8 (a session global, not saved): set by 0x5243E0(0x11) and,
    /// while a shop is visited, recomputed as "the shop is a garage"
    /// (kind 0); while set, kinds 1, 2 and 0x12 become 0x11.
    pub garage_fe: bool,
    /// +0x1C / +0x20: the neighbourhood line's zone (the player's +0x9B8)
    /// and the driving time (+0x114) it last spoke or was first seen at.
    pub zone: u32,
    pub zone_time: u32,
    /// The object's other bytes as loaded (+0x108 / +0x10C the sim clock,
    /// …).
    pub raw: Vec<u8>,
}

impl Default for Hints {
    fn default() -> Hints {
        Hints {
            dday_over: false,
            outrun_pick: 0,
            complaints: 0,
            stage1_line: false,
            garage: false,
            events: false,
            sponsor: false,
            sponsor_said: false,
            ratings: [false; 11],
            dvd: false,
            rating_due: 0,
            fitted: false,
            covers: Vec::new(),
            stage_line: 0,
            showcase: false,
            slots: [(0, 0, 0); 8],
            drive_ticks: None,
            drive_sum: 0,
            fe_clock: 0,
            fe_last: [0; 28],
            garage_fe: false,
            zone: 0,
            zone_time: 0,
            raw: vec![0; HINTS_LEN],
        }
    }
}

/// The object's size in the career block.
pub const HINTS_LEN: usize = 0x190;

/// Stage 0's races (their hashes, +8): 0x5242D0 compares these.
pub const STAGE0_RACES: [u32; 3] = [0xCA20_7583, 0xCA20_7584, 0x1BCB_7AB0];

/// ftol(1.0 (0x7993BC) × 4000 + 0.5): a second of driving.
pub(super) const SECOND: i32 = 4000;

/// 0x511240's table 0x7FA3F0 (20 × {cover, Career_Magazine value}; other
/// covers 0x10).
static MAGAZINE_LINES: crate::exe_tables::Table<[(u32, u32); 20]> = crate::exe_tables::Table::new(|t| &t.magazine_lines);

/// 0x511260's table 0x7FA490 (10 × {cover, Career_Dvd value}; other
/// covers 1).
static DVD_LINES: crate::exe_tables::Table<[(u32, u32); 10]> = crate::exe_tables::Table::new(|t| &t.dvd_lines);

/// 0x53E0C0 case 15's table 0x7FA4DC (Career_Dvd value by rating).
static RATING_LINES: crate::exe_tables::Table<[u32; 11]> = crate::exe_tables::Table::new(|t| &t.rating_lines);

/// What a fired hint does besides Rachel's line (0x53E0C0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fire {
    /// A message into the inbox (0x5338A0).
    Message(u32),
    /// Kind 2: career +0x873F (0x86347F) set, the marker table and the
    /// HUD rebuilt (0x533320, 0x4F92A0): stage 0's races appear.
    Reveal,
    /// Kind 1: a complaint (the counter +4 0 → 1 → 2 → 3); the third sets
    /// career +0x873E (0x86347E) and rebuilds markers and HUD: stage 0's
    /// races close.
    Complain { close: bool },
}

/// A fired hint: its effect and Rachel's line (a speech switch tuple,
/// `audio::speech::lines`), each optional.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fired {
    pub fire: Option<Fire>,
    pub line: Option<Vec<u32>>,
}

use crate::audio::speech::lines;

impl Hints {
    /// 0x5111F0: the first free slot (kind 0) takes the post.
    fn post(&mut self, ticks: i32, kind: u32, value: u32) {
        if let Some(s) = self.slots.iter_mut().find(|s| s.1 == 0) {
            *s = (ticks, kind, value);
        }
    }

    fn queued(&self, kind: u32) -> bool {
        self.slots.iter().any(|s| s.1 == kind)
    }

    /// Pending (ticks, kind) — for tests and the selftest.
    pub fn pending(&self) -> Vec<(i32, u32)> {
        self.slots.iter().filter(|s| s.1 != 0).map(|s| (s.0, s.1)).collect()
    }

    /// 0x525860, from UI_ShowCase_Preview's INIT (0x4CF452): once
    /// (+0xA6), Career (0x10000, 0x20) on the speech stream (0x79B870;
    /// no stream stop first, no post).
    pub fn showcase_line(&mut self) -> Option<Vec<u32>> {
        if self.showcase {
            return None;
        }
        self.showcase = true;
        Some(lines::career(0x1_0000, 0x20))
    }

    /// 0x525660's first part (career stage ≥ 2, 0x55117F; each driving
    /// step after the count): the player's zone `zone` (+0x9B8; 0 none:
    /// nothing). The first one seen is kept with the driving time; a
    /// change is kept and, when Rachel is free (0x5112B0) and at least
    /// ftol(0x7993C0 (300 s) × 4000 + 0.5) ticks were driven since
    /// +0x20 (signed), returns true: the caller looks for the place to
    /// name (`Profile::neighbourhood_line`).
    pub fn zone_changed(&mut self, zone: u32, free: bool) -> bool {
        if zone == 0 {
            return false;
        }
        if self.zone == 0 {
            self.zone = zone;
            self.zone_time = self.drive_sum;
        }
        if zone == self.zone {
            return false;
        }
        self.zone = zone;
        free && self.drive_sum.wrapping_sub(self.zone_time) as i32 >= 1_200_000
    }

    /// 0x5255E0 / 0x525620, from UI_ChooseUniquePart's Accept (0x561730
    /// on 0xD05FC3A3, after the part is fitted 0x55C240 / 0x528A00 or the
    /// performance unique added 0x521440): kind 8 (a part) or 9 (a
    /// performance unique, +0x151) in ftol(0x7993BC 1.0 × 4000 + 0.5).
    pub fn unique_taken(&mut self, perf: bool) {
        self.post(SECOND, if perf { 9 } else { 8 }, 0);
    }

    /// 0x5243E0(kind), the front-end lines (career only, 0x850078 = 1;
    /// the caller checks that). `shop`: the visited shop (0x863444 /
    /// 0x86344C) as (kind +0x50, hash +0x38). 0x8641F8 (`garage_fe`) is set
    /// by kind 0x11 and, in a shop, recomputed as its kind = 0; kind 1 in
    /// Megalow (0x7FC8D0D0) becomes 0; with 0x8641F8 set kinds 1, 2 and
    /// 0x12 become 0x11. Kind 4 and 0x11 take their once-paths
    /// (`car_lot`, `crib`) while their flags (+9 / +0xA) are clear; kind 2
    /// sets +0x28. Then a kind whose last time +0x120 + 4·kind is set (not
    /// 0 or 0x7FFFFFFF) and no more than ftol(3600 (0x7993C4) × 4000 +
    /// 0.5) ticks of +0x11C ago says nothing; else its line (the speech
    /// stream stopped first, 0x45B490) and the time is kept. Lines
    /// (switch 0x524FD4): FE_Screen (0x510F30 via 0x79AD08) with b 0x20 —
    /// 0 (0x400000) and 1 (0x800000) when the kind's time is unset, else
    /// (1) (0x401820), 2 8, 3 0x10, 4 0x20, 5 0x40, 6 0x80, 7 0x100, 8
    /// 0x400, 9 0x800, 10 0x2000, 11 0x4000, 12 0x8000, 13 0x10000, 14
    /// 0x20000, 15 0x40000, 16 0x80000, 18 0x2000000; 17 (0x11) none;
    /// 19–27 FE_OptionScreen (0x510F50 via 0x79AD0C) 1, 2, 4, …, 0x100
    /// with b 0x10.
    pub fn fe_line(&mut self, kind: u32, shop: Option<(u8, u32)>) -> Option<Vec<u32>> {
        if kind == 0x11 {
            self.garage_fe = true;
        }
        let mut k = kind;
        if let Some((shop_kind, hash)) = shop {
            self.garage_fe = shop_kind == 0;
            if hash == 0x7FC8_D0D0 && kind == 1 {
                k = 0;
            }
        }
        if self.garage_fe && matches!(k, 1 | 2 | 0x12) {
            k = 0x11;
        }
        match k {
            4 if !self.garage => return self.car_lot(),
            0x11 if !self.events => return self.crib(),
            2 => self.fitted = true,
            _ => {}
        }
        let k = k as usize;
        let last = *self.fe_last.get(k)?;
        let set = last != 0 && last != 0x7FFF_FFFF;
        if set && (self.fe_clock.wrapping_sub(last) as i32) <= 3600 * 4000 {
            return None;
        }
        let line = match k {
            0 => Some(lines::fe_screen(if set { 1 } else { 0x40_0000 }, 0x20)),
            1 => Some(lines::fe_screen(if set { 1 } else { 0x80_0000 }, 0x20)),
            2 => Some(lines::fe_screen(8, 0x20)),
            3..=16 => Some(lines::fe_screen(
                [
                    0x10, 0x20, 0x40, 0x80, 0x100, 0x400, 0x800, 0x2000, 0x4000, 0x8000, 0x10000, 0x20000, 0x40000,
                    0x80000,
                ][k - 3],
                0x20,
            )),
            18 => Some(lines::fe_screen(0x200_0000, 0x20)),
            19..=27 => Some(lines::fe_option(1 << (k - 19), 0x10)),
            _ => None,
        };
        self.fe_last[k] = self.fe_clock;
        line
    }

    /// 0x5243E0(4): the car lot's INIT (0x4ED9E0 on 0x35F8620B, 0x4EDACB),
    /// in a career (0x850078 = 1): once (+9), Rachel's FE_Screen line
    /// (0x510F30: 0x1000000, 0x20) and kind 4 in a second of driving.
    pub fn car_lot(&mut self) -> Option<Vec<u32>> {
        if self.garage {
            return None;
        }
        self.garage = true;
        self.post(SECOND, 4, 0);
        Some(lines::fe_screen(0x100_0000, 0x20))
    }

    /// 0x5243E0(0x11): UI_CareerCrib's INIT (0x4AF3A0, 0x4AF502): once
    /// (+10), the FE_Screen line (0x100000, 0x10), kind 6 in a second and
    /// kind 10 in 30 s (0x1D4C0 ticks). (It also sets 0x8641F8, which
    /// turns the shops' kinds 1, 2 and 0x12 into this one; not ported with
    /// them.)
    pub fn crib(&mut self) -> Option<Vec<u32>> {
        if self.events {
            return None;
        }
        self.events = true;
        self.post(SECOND, 6, 0);
        self.post(120_000, 10, 0);
        Some(lines::fe_screen(0x10_0000, 0x10))
    }

    /// 0x525520 (UI_SponsorPopup's INIT, 0x4C13DF): once (+0xB); the first
    /// time (+0xC clear) the Career line 0x8000000 and kind 0xB in a
    /// second, else the line 0x10.
    pub fn sponsor_popup(&mut self) -> Option<Vec<u32>> {
        if self.sponsor {
            return None;
        }
        self.sponsor = true;
        if !self.sponsor_said {
            self.sponsor_said = true;
            self.post(SECOND, 0xB, 0);
            Some(lines::career(0x800_0000, 0x20))
        } else {
            Some(lines::career(0x10, 0x20))
        }
    }

    /// 0x525400(milestone), from the magazine triggers of kinds 3 / 4
    /// (0x533B50, 0x533AA0) and a DVD offered anew (0x5331D0). A magazine
    /// (+0x24 1 or 3) not yet in `covers`: its cover (+0x28) is added and
    /// kind 0xE posted in a second with the cover. A DVD (2 or 4) whose
    /// cover is not in `covers`, while `fitted`: added, kind 0xD posted in
    /// a second with the cover, `dvd` set and the rating flag +0xD +
    /// ftol(+0x3C) set.
    pub fn milestone(&mut self, kind: u8, cover: u32, rating: f32) {
        match kind {
            1 | 3 if !self.covers.contains(&cover) => {
                if self.covers.len() < 30 {
                    self.covers.push(cover);
                }
                self.post(SECOND, 0xE, cover);
            }
            2 | 4 if !self.covers.contains(&cover) && self.fitted => {
                if self.covers.len() < 30 {
                    self.covers.push(cover);
                }
                self.post(SECOND, 0xD, cover);
                self.dvd = true;
                if let Some(f) = self.ratings.get_mut(rating as usize) {
                    *f = true;
                }
            }
            _ => {}
        }
    }

    /// 0x524250, from 0x5410F0 as DDAY_EVENT_A becomes the current event
    /// (the career start 0x495A30, the movie's D-Day start 0x54AF90):
    /// kind 2 in 60000 ticks (15 s of driving) and kind 3 in 160000, each
    /// unless one is queued.
    pub fn dday_a(&mut self) {
        if !self.queued(2) {
            self.post(60_000, 2, 0);
        }
        if !self.queued(3) {
            self.post(160_000, 3, 0);
        }
    }

    /// 0x5242D0(event, won), from the win routine (0x54152D) and the loss
    /// path (0x54C9C0 → 0x528510 → 0x52855A): a stage-0 race over posts a
    /// complaint (kind 1) in a second unless one is queued; DDAY_EVENT_A
    /// (0xDD60E402) over sets the counter to 3 and +1. Then, for a won
    /// race that is not scripted (event +0x34 ≠ 0): kind 5 in a second
    /// while +8 is clear and the race is a stage-1 one (+0x37 = 1), and
    /// kind 7 in a second when it is its stage's final event (stage record
    /// [+0x37] → +0x28) and the stage is not 5.
    pub fn race_over(&mut self, event: u32, won: bool, scripted: bool, stage: u8, stage_final: bool) {
        if STAGE0_RACES.contains(&event) {
            if !self.queued(1) {
                self.post(SECOND, 1, 0);
            }
        } else if event == crate::frontend::career::DDAY_EVENT_A {
            self.complaints = 3;
            self.dday_over = true;
        }
        if won && !scripted {
            if !self.stage1_line && stage == 1 {
                self.post(SECOND, 5, 0);
            }
            if stage_final && stage != 5 {
                self.post(SECOND, 7, 0);
            }
        }
    }

    /// 0x528330(k), the stage end of the current stage: unless a kind-12
    /// line is pending (+0xA5 set), kind 12 in 15 s (0xEA60 ticks); +0xA5
    /// = the new stage.
    pub fn stage_up(&mut self, new_stage: u8) {
        if self.stage_line == 0 {
            self.post(60_000, 12, 0);
        }
        self.stage_line = new_stage;
    }

    /// 0x551080 each free-roam frame (the D-Day script not running) with
    /// the driving time `drive` (s, after this frame's step) and the
    /// front-end clock's step `real_dt` (s); `free` is 0x525A00 (Rachel
    /// may speak). Above 0 s of driving: the driving ticks' step counts
    /// the slots down (0x54BD40) and the soonest due one fires (0x53E0C0).
    pub fn tick(&mut self, drive: f32, real_dt: f32, free: bool) -> Option<Fired> {
        // +0x118 / +0x11C: the front-end clock, whatever the driving time.
        self.fe_clock = self.fe_clock.wrapping_add((real_dt.max(0.0) * 4000.0).round() as u32);
        if drive <= 0.0 {
            return None;
        }
        // +0x110 / +0x114: ftol(drive × 4000 + 0.5), its step the delta.
        let now = (drive * 4000.0 + 0.5) as i32;
        let step = now - self.drive_ticks.unwrap_or(now);
        self.drive_ticks = Some(now);
        self.drive_sum = self.drive_sum.wrapping_add(step as u32);
        let fired = self.count_down(step, free);
        // 0x551080 after the count: kind 0xF for the rating +0x19 names,
        // with +0x28 and +0xC set, no DVD pending and its flag clear.
        if self.fitted && self.sponsor_said && !self.dvd && self.rating_due != 0 {
            let n = self.rating_due as usize;
            if !self.ratings.get(n).copied().unwrap_or(true) {
                self.post(SECOND, 0xF, n as u32);
                self.ratings[n] = true;
                self.rating_due = 0;
            }
        }
        self.fitted = false;
        fired
    }

    /// 0x54BD40(step): every live slot counts down; the best is slot 0 at
    /// first, then any live slot below the best so far (strictly); it
    /// fires when live, at or below 0 and Rachel is free.
    fn count_down(&mut self, step: i32, free: bool) -> Option<Fired> {
        let mut best = 0;
        for i in 0..8 {
            if self.slots[i].1 != 0 {
                self.slots[i].0 -= step;
                if self.slots[i].0 < self.slots[best].0 {
                    best = i;
                }
            }
        }
        let (ticks, kind, value) = self.slots[best];
        if kind == 0 || ticks > 0 || !free {
            return None;
        }
        self.slots[best] = (0, 0, 0);
        Some(self.fire(kind, value))
    }

    /// 0x53E0C0(kind, value): Rachel's line for the kind, and what else it
    /// does. Lines (Career = (1, a, b) unless named): 1 Time (4, 1 / 4 /
    /// 8 by the complaint), 2 Intro_Events (0x10, 0x400000, 8), 3
    /// Intro_Events (0x10, 0x8000, 1), 4 (0x2000, 0x20), 5 (1, 0x20) once
    /// (+8), 6 (0x1000000, 0x20), 7 (0x40, 0x20), 8 (0x80, 0x20), 9
    /// (0x100, 0x20), 10 (0x800000, 0x10), 11 (0x20000, 0x20), 12
    /// Intro_Area (0x20, {1, 2, 4, 4}[stage − 2], 0x20) for stages 2–5
    /// (+0xA5 then 0), 13 Career_Dvd (0x511260(value), 1) (+0x18 then 0),
    /// 14 Career_Magazine (0x511240(value), 1), 15 Career_Dvd
    /// (0x7FA4DC[value], 4).
    fn fire(&mut self, kind: u32, value: u32) -> Fired {
        let career = |a| Some(lines::career(a, 0x20));
        match kind {
            1 => {
                let (line, close) = match self.complaints {
                    0 => (1, false),
                    1 => (4, false),
                    2 => (8, true),
                    _ => return Fired::default(),
                };
                self.complaints += 1;
                Fired { fire: Some(Fire::Complain { close }), line: Some(lines::time(line)) }
            }
            2 => Fired { fire: Some(Fire::Reveal), line: Some(lines::intro_events(0x40_0000, 8)) },
            3 => Fired { fire: None, line: Some(lines::intro_events(0x8000, 1)) },
            4 => Fired { fire: Some(Fire::Message(SMS_LOC_GO_TO_GARAGE)), line: career(0x2000) },
            5 if !self.stage1_line => {
                self.stage1_line = true;
                Fired { fire: None, line: career(1) }
            }
            6 => Fired { fire: Some(Fire::Message(SMS_LOC_EVENTS_OUT_THERE)), line: career(0x100_0000) },
            7 => Fired { fire: None, line: career(0x40) },
            8 => Fired { fire: None, line: career(0x80) },
            9 => Fired { fire: None, line: career(0x100) },
            10 => Fired { fire: None, line: Some(lines::career(0x80_0000, 0x10)) },
            11 => Fired { fire: None, line: career(0x2_0000) },
            12 => {
                let line = match self.stage_line {
                    2 => Some(1),
                    3 => Some(2),
                    4 | 5 => Some(4),
                    _ => None,
                };
                self.stage_line = 0;
                Fired { fire: None, line: line.map(|a| lines::intro_area(a, 0x20)) }
            }
            13 => {
                self.dvd = false;
                let v = DVD_LINES.iter().find(|e| e.0 == value).map_or(1, |e| e.1);
                Fired { fire: None, line: Some(lines::dvd(v, 1)) }
            }
            14 => {
                let v = MAGAZINE_LINES.iter().find(|e| e.0 == value).map_or(0x10, |e| e.1);
                Fired { fire: None, line: Some(lines::magazine(v, 1)) }
            }
            15 => Fired { fire: None, line: RATING_LINES.get((value & 0xFF) as usize).map(|&v| lines::dvd(v, 4)) },
            _ => Fired::default(),
        }
    }

    /// Rachel's line at an outrun's start (0x607190 → 0x532E40 → 0x511000
    /// on this object; verified, the disassembly): with +1 set a random
    /// one, Outrun (8, 1, b) with b = {1, 2, 4, 8, 0x200}[Rand_Int(5)],
    /// the pick kept at +0x1A; else the first, (8, 0x20, 0x200). The draw
    /// (`rand`, 0x43C1C0) is made whether or not there is a speech object.
    pub fn outrun_start(&mut self, rand: &mut dyn FnMut(u32) -> u32) -> Vec<u32> {
        if self.dday_over {
            self.outrun_pick = rand(5) as u8;
            lines::outrun(1, OUTRUN_LINES[usize::from(self.outrun_pick).min(4)])
        } else {
            lines::outrun(0x20, 0x200)
        }
    }

    /// Rachel's line at an outrun's end (0x541330 → 0x5110C0(won) on this
    /// object, after 0x45B490 stops the speech; verified, the
    /// disassembly): with +1 set Outrun (8, a, b) with b the start's pick
    /// and a by Rand_Int(2) (drawn always): won 8 / 16, lost 4 / 2 (for 0 /
    /// not 0); else (8, 0x40 won / 0x80 lost, 0x200) and +1 is set.
    pub fn outrun_end(&mut self, won: bool, rand: &mut dyn FnMut(u32) -> u32) -> Vec<u32> {
        if self.dday_over {
            let r = rand(2) != 0;
            let a = match (won, r) {
                (true, false) => 8,
                (true, true) => 16,
                (false, false) => 4,
                (false, true) => 2,
            };
            lines::outrun(a, OUTRUN_LINES[usize::from(self.outrun_pick).min(4)])
        } else {
            self.dday_over = true;
            lines::outrun(if won { 0x40 } else { 0x80 }, 0x200)
        }
    }

    /// The object's bytes (career +0x887C): the traced fields over the
    /// loaded ones.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = self.raw.clone();
        b.resize(HINTS_LEN, 0);
        b[1] = self.dday_over as u8;
        b[4..8].copy_from_slice(&(self.complaints as u32).to_le_bytes());
        b[8] = self.stage1_line as u8;
        b[9] = self.garage as u8;
        b[0xA] = self.events as u8;
        b[0xB] = self.sponsor as u8;
        b[0xC] = self.sponsor_said as u8;
        for (i, &f) in self.ratings.iter().enumerate() {
            b[0xD + i] = f as u8;
        }
        b[0x18] = self.dvd as u8;
        b[0x19] = self.rating_due;
        b[0x1A] = self.outrun_pick;
        b[0x28] = self.fitted as u8;
        for i in 0..30 {
            let v = self.covers.get(i).copied().unwrap_or(0);
            b[0x2C + 4 * i..0x30 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
        b[0xA4] = self.covers.len().min(30) as u8;
        b[0xA5] = self.stage_line;
        b[0xA6] = self.showcase as u8;
        for (i, s) in self.slots.iter().enumerate() {
            let o = 0xA8 + 12 * i;
            b[o..o + 4].copy_from_slice(&s.0.to_le_bytes());
            b[o + 4..o + 8].copy_from_slice(&s.1.to_le_bytes());
            b[o + 8..o + 12].copy_from_slice(&s.2.to_le_bytes());
        }
        b[0x1C..0x20].copy_from_slice(&self.zone.to_le_bytes());
        b[0x20..0x24].copy_from_slice(&self.zone_time.to_le_bytes());
        b[0x110..0x114].copy_from_slice(&self.drive_ticks.unwrap_or(0).to_le_bytes());
        b[0x114..0x118].copy_from_slice(&self.drive_sum.to_le_bytes());
        b[0x11C..0x120].copy_from_slice(&self.fe_clock.to_le_bytes());
        for (i, t) in self.fe_last.iter().enumerate() {
            b[0x120 + 4 * i..0x124 + 4 * i].copy_from_slice(&t.to_le_bytes());
        }
        b
    }

    /// From the career's bytes. The driving-time reading +0x110 goes with
    /// the saved driving time (career +0x8764); 0 or 0x7FFFFFFF is none
    /// (0x551080 then takes a reading and counts nothing).
    pub fn from_bytes(b: &[u8]) -> Hints {
        let mut raw = b.to_vec();
        raw.resize(HINTS_LEN, 0);
        let b = &raw;
        let u32_at = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let n = (b[0xA4] as usize).min(30);
        Hints {
            dday_over: b[1] != 0,
            outrun_pick: b[0x1A],
            zone: u32_at(0x1C),
            zone_time: u32_at(0x20),
            complaints: u32_at(4).min(3) as u8,
            stage1_line: b[8] != 0,
            garage: b[9] != 0,
            events: b[0xA] != 0,
            sponsor: b[0xB] != 0,
            sponsor_said: b[0xC] != 0,
            ratings: std::array::from_fn(|i| b[0xD + i] != 0),
            dvd: b[0x18] != 0,
            rating_due: b[0x19],
            fitted: b[0x28] != 0,
            covers: (0..n).map(|i| u32_at(0x2C + 4 * i)).collect(),
            stage_line: b[0xA5],
            showcase: b[0xA6] != 0,
            slots: std::array::from_fn(|i| {
                (u32_at(0xA8 + 12 * i) as i32, u32_at(0xAC + 12 * i), u32_at(0xB0 + 12 * i))
            }),
            drive_ticks: Some(u32_at(0x110) as i32).filter(|&t| t != 0 && t != 0x7FFF_FFFF),
            drive_sum: u32_at(0x114),
            fe_clock: u32_at(0x11C),
            fe_last: std::array::from_fn(|i| u32_at(0x120 + 4 * i)),
            garage_fe: false,
            raw: raw.clone(),
        }
    }
}

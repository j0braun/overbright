/// The messages the intro sends (bin hashes of their names, 0x43DB50).
pub const SMS_INSTRUCTION: u32 = 0x9C58_9393;
pub const SMS_LOC_GO_TO_GARAGE: u32 = 0x51B3_0CEC;
pub const SMS_LOC_EVENTS_OUT_THERE: u32 = 0x8391_4C51;

/// The D-Day script (0x861E7C: +0 state, +4 the state's start tick, +8 a
/// message read, +9 the SMS screens closed). 0 is idle and 9 done; the
/// career reset (0x532A43) zeroes it, the city start (0x5404FE) sets 9
/// once the intro movie has played this session (0x86347C).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Script {
    pub state: u8,
    pub since: f32,
    /// +8: set by reading any message while the script runs (0x514DC0).
    pub read: bool,
    /// +9: set as the SMS screens close while it runs (0x497730 →
    /// 0x5024F0, flow event 2 in screen state 0xD).
    pub closed: bool,
}

/// What a step asks of the game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    None,
    /// State 1: the HUD prompt kind 0x14 with value 2 (0x4CA670(2) →
    /// 0x4C9170(0x14, 2)).
    Prompt,
    /// State 3: SMS_INSTRUCTION into the inbox (0x5338A0, once) and the
    /// SMS screens open (0x529710 → state 0xD).
    Instruction,
    /// State 6 → 7: the marker table rebuilt (0x533320) and the GPS to
    /// DDAY_EVENT_A's goal (0x532D90).
    GpsToA,
    /// State 8 → 9: driving input back (0x605C80(2)), the markers rebuilt,
    /// SMS_INSTRUCTION if still missing, the GPS to DDAY_EVENT_A again.
    Done,
}

impl Script {
    /// 0x502420: neither idle nor done.
    pub fn running(&self) -> bool {
        !matches!(self.state, 0 | 9)
    }

    /// 0x513BC0 → 0x5023F0, on flow event 1 (0x60B3B0) in a career: with no
    /// race (0x89E7B0 = 0) and no car of the player's own yet (career
    /// +0x873D, set when a car slot becomes current, 0x500BD0), an idle
    /// script starts.
    pub fn begin(&mut self) {
        if self.state == 0 {
            self.state = 1;
        }
    }

    /// 0x5342A0, run each free-roam frame instead of the driving-time
    /// count (0x5513AD). The waits are 2 s (2.0 < Δ × 0.00025; 8000 < Δ
    /// ticks); states 3–6 also wait for Rachel to be free (`free`, the
    /// speech test 0x525A00: true without a speech object, else ready and
    /// not busy) and speak as they go on (Career lines, 0x510ED0 through
    /// the helpers): 3 → 5 0x525910 (0x800, 0x20), 5 → 6 0x525960 (2,
    /// 0x20), 6 → 7 0x5259B0 (0x10000000, 0x20). State 7's own test of
    /// 0x525A00 (0x534465) is not used: 7 goes to 8 at once.
    pub fn step(&mut self, now: f32, free: bool) -> (Step, Option<Vec<u32>>) {
        use crate::audio::speech::lines;
        let after = |since: f32| now - since > 2000.0;
        let step = match self.state {
            1 => {
                self.since = now;
                self.state = 2;
                Step::Prompt
            }
            2 if after(self.since) => {
                self.state = 3;
                Step::None
            }
            // State 3 goes straight to 5 (4 is never entered).
            3 if free => {
                self.state = 5;
                self.since = now;
                return (Step::Instruction, Some(lines::career(0x800, 0x20)));
            }
            4 if after(self.since) && free => {
                self.state = 5;
                Step::None
            }
            5 if after(self.since) && free && self.read && self.closed => {
                self.state = 6;
                self.since = now;
                return (Step::None, Some(lines::career(2, 0x20)));
            }
            6 if after(self.since) && free => {
                self.state = 7;
                self.since = now;
                return (Step::GpsToA, Some(lines::career(0x1000_0000, 0x20)));
            }
            7 => {
                self.state = 8;
                self.since = now;
                Step::None
            }
            8 => {
                self.state = 9;
                Step::Done
            }
            _ => Step::None,
        };
        (step, None)
    }

    /// 0x514DC0: reading an unread message while the script runs.
    pub fn on_read(&mut self) {
        if self.running() {
            self.read = true;
        }
    }

    /// 0x497730: the SMS screens closing while the script runs.
    pub fn on_sms_closed(&mut self) {
        if self.running() {
            self.closed = true;
        }
    }

    /// 0x6000A0's script branch (0x60031E): in a career while the script
    /// runs the HUD features are only the script's (from 0x40000, which
    /// 0x4CA090 masks off): state 2 the engage prompt's high bit 2
    /// (0x502440); above 2 that bit too, except in state 5 once a message
    /// is read (0x502460); from 6 the minimap (low 1, 0x502490); above 8
    /// low 0x60 (0x5024B0, never while running).
    pub fn hud_features(&self) -> Option<(u32, u32)> {
        if !self.running() {
            return None;
        }
        let s = self.state;
        let (mut lo, mut hi) = (0, 0);
        if s == 2 {
            hi = 2;
        }
        if s > 2 && !(s == 5 && self.read) {
            hi |= 2;
        }
        if s >= 6 {
            lo |= 1;
        }
        if s > 8 {
            lo |= 0x60;
        }
        Some((lo, hi))
    }

    /// 0x4CA6D0's marker test while the script runs (0x4CA74D): nothing
    /// before state 7 (0x5024D0), then only car lot icons (type 11).
    pub fn shows_marker(&self, car_lot: bool) -> bool {
        self.state >= 7 && car_lot
    }

    /// 0x5EA6F2: while the script runs the player's input mode is 0xA
    /// (0x605C80: only pause 0xBD, skip 0xBE and Read SMS 0xC2 bound), so
    /// the car takes no driving input; 0xBE skips (0x534290 → 0x534210)
    /// only with 0x864FA4 set, which nothing writes.
    pub fn holds_input(&self) -> bool {
        self.running()
    }
}

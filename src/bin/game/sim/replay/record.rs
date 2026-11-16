use crate::*;

/// A replay (the game's ReplayManager 0x8650B4, built by 0x57A870): the
/// race's state at the green light (REPLAY_RACE_SNAPSHOT 0x7A0834), the
/// periodic LAP_SNAPSHOTs (0x57D070) the control bar skips between, and
/// the player's inputs each frame after. The game records its joylog
/// channels; the clone records its `Controls` and the frame's dt, plus the
/// keys that change the car between frames (inferred stand-ins).
pub(crate) struct Recording {
    pub(crate) start: Snapshot,
    pub(crate) frames: Vec<ReplayFrame>,
    /// A shift (up?) or reset key since the last frame.
    pub(crate) shift: Option<bool>,
    pub(crate) reset: bool,
    /// The LAP_SNAPSHOTs in time order (the manager's list +0x98), the
    /// interval between them (+0xC4) and the time recorded so far (the
    /// tick 0x865188 less the start +0x84, here the frames' dt summed).
    pub(crate) laps: Vec<LapSnapshot>,
    pub(crate) interval: f32,
    pub(crate) elapsed: f32,
    /// The INSTANT_REPLAY snapshots, oldest first (manager +0xB8 of them,
    /// at most +0xA8 = 2), the 60 Hz counter 0x7FB720 with its carried
    /// fraction (0x865154), and the count the next is due at (+0x74).
    pub(crate) instants: Vec<LapSnapshot>,
    pub(crate) ticks: u32,
    pub(crate) carry: f32,
    pub(crate) next_instant: u32,
}

/// A LAP_SNAPSHOT: the race's state at the start of frame `frame`, `time`
/// seconds after the green light (snapshot +0x28 / 4000).
pub(crate) struct LapSnapshot {
    pub(crate) time: f32,
    pub(crate) frame: usize,
    pub(crate) state: Snapshot,
}

/// LAP_SNAPSHOTs at most (manager +0xB0, 5 from 0x57AAE0), and their first
/// interval, s (+0xC4, 15.0 from 0x57A870).
pub(crate) const LAP_SNAPSHOTS: usize = 5;

pub(crate) const LAP_INTERVAL: f32 = 15.0;

/// INSTANT_REPLAY snapshots kept (+0xA8, 2 from 0x57AAE0) and the 0x7FB720
/// ticks between them (0x5806C0: 300, 5 s at 60 Hz).
pub(crate) const INSTANT_REPLAYS: usize = 2;

pub(crate) const INSTANT_TICKS: u32 = 300;

/// A marker index at or above this is `instants[m - INSTANT_MARK]`.
pub(crate) const INSTANT_MARK: usize = LAP_SNAPSHOTS;

impl Recording {
    pub(crate) fn new(d: &Drive, race: &Race) -> Recording {
        Recording {
            start: Snapshot::take(d, race),
            frames: Vec::new(),
            shift: None,
            reset: false,
            laps: Vec::new(),
            interval: LAP_INTERVAL,
            elapsed: 0.0,
            instants: Vec::new(),
            ticks: 0,
            carry: 0.0,
            next_instant: 0,
        }
    }

    /// 0x57A0F0, each frame's time step: 0x7FB720 counts whole 60ths of a
    /// second, the fraction carried to the next frame (0x865154; the
    /// truncation of 0x75BED4 is inferred).
    pub(crate) fn tick(&mut self, dt: f32) {
        let t = self.carry + dt;
        let n = (t * 60.0) as u32;
        self.ticks += n;
        self.carry = t - n as f32 / 60.0;
    }

    /// 0x5806C0, each recorded frame before the LAP_SNAPSHOT (0x5810B9 then
    /// 0x5810C7): once 0x7FB720 reaches +0x74 (0 in a new manager, 0x57A8F2;
    /// one per race, 0x60A6D4) it moves on 300 and, with two held, the
    /// oldest INSTANT_REPLAY goes, then a new one is taken. The oldest
    /// would become a HIGHLIGHT_SNAPSHOT when its score (+0x1D8, added by
    /// 0x578EA0 to every held INSTANT_REPLAY: CarCollision, CloseCars,
    /// CarInAir, CarSlidingSideways, Speed, ...) is above 0 and beats the
    /// lowest held one, but the highlight limit +0xB4 is 0 (0x57A942,
    /// 0x57AB76: no other writer), so 0x5806C0 always drops it and the
    /// scores never matter; the clone keeps none.
    pub(crate) fn instant_replay(&mut self, d: &Drive, race: &Race) {
        if self.ticks < self.next_instant {
            return;
        }
        self.next_instant = self.ticks + INSTANT_TICKS;
        if self.instants.len() >= INSTANT_REPLAYS {
            self.instants.remove(0);
        }
        self.instants.push(LapSnapshot {
            time: self.elapsed,
            frame: self.frames.len(),
            state: Snapshot::take(d, race),
        });
    }

    /// The snapshot a marker names: a LAP_SNAPSHOT or an INSTANT_REPLAY.
    pub(crate) fn mark(&self, m: usize) -> &LapSnapshot {
        if m >= INSTANT_MARK { &self.instants[m - INSTANT_MARK] } else { &self.laps[m] }
    }

    /// 0x57D070, each recorded frame before the simulation: once the time
    /// since the start reaches count × interval, a LAP_SNAPSHOT; with the
    /// list full the interval doubles and every second snapshot (the 2nd,
    /// 4th, ...) goes, so those kept stay on the new interval.
    pub(crate) fn lap_snapshot(&mut self, d: &Drive, race: &Race) {
        let due = |r: &Recording| r.laps.len() as f32 * r.interval <= r.elapsed;
        if !due(self) {
            return;
        }
        while self.laps.len() == LAP_SNAPSHOTS {
            self.interval *= 2.0;
            let mut i = 0;
            self.laps.retain(|_| {
                i += 1;
                i % 2 == 1
            });
            if self.laps.len() >= LAP_SNAPSHOTS || !due(self) {
                return;
            }
        }
        self.laps.push(LapSnapshot { time: self.elapsed, frame: self.frames.len(), state: Snapshot::take(d, race) });
    }

    /// The control bar's markers (0x4C0AA0 over the snapshot list, in the
    /// order taken: 0x57C090 appends, and in a frame the INSTANT_REPLAY
    /// comes before the LAP_SNAPSHOT): the first INSTANT_REPLAY and every
    /// LAP_SNAPSHOT except, once there is a marker, those under 20 s
    /// before the recording's end.
    pub(crate) fn markers(&self) -> Vec<usize> {
        let mut list: Vec<(usize, bool, usize)> =
            self.laps.iter().enumerate().map(|(i, l)| (l.frame, true, i)).collect();
        list.extend(self.instants.iter().enumerate().map(|(i, l)| (l.frame, false, INSTANT_MARK + i)));
        list.sort();
        let mut out = Vec::new();
        let mut instant = false;
        for (_, lap, m) in list {
            if lap {
                if !out.is_empty() && self.elapsed - self.mark(m).time < 20.0 {
                    continue;
                }
            } else if std::mem::replace(&mut instant, true) {
                continue;
            }
            out.push(m);
        }
        out
    }

    /// The time from the start to the start of frame `at`, s.
    pub(crate) fn time_at(&self, at: usize) -> f32 {
        self.frames[..at.min(self.frames.len())].iter().map(|f| f.dt).sum()
    }
}

/// One recorded race frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReplayFrame {
    pub(crate) controls: Controls,
    pub(crate) dt: f32,
    pub(crate) shift: Option<bool>,
    pub(crate) reset: bool,
}

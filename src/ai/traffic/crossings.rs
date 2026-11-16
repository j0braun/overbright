use super::*;

/// How far before a road's crossing point the other road's near edge lies
/// along the direction of travel (0x410800: the other road's point moved
/// out by its half widths, −left and +right, each projected on the
/// direction; minus the smaller). 0 for a road not of the intersection.
pub(super) fn stop_back(f: &RouteFile, x: usize, road: Option<usize>, backward: bool) -> f32 {
    let i = &f.intersections[x];
    let (Some(road), Some((r, c))) = (road, road.and_then(|r| f.crossing_point(i, r))) else { return 0.0 };
    let Some((or, oc)) = f.crossing_point(i, 1 - road) else { return 0.0 };
    let dir = f.direction(r, c, backward);
    let at = f.records[r].points[c].position.truncate();
    let hw = f.records[or].points[oc].half_width;
    let left = (f.point(or, oc, false, 0.0, -hw[0]).truncate() - at).dot(dir);
    let right = (f.point(or, oc, false, 0.0, hw[1]).truncate() - at).dot(dir);
    -left.min(right)
}

/// The line a car waits at: 12 m before the crossing road's edge
/// (0x412170). The car's own lane-object +0x0C, also subtracted there, is
/// not traced (taken as 0).
pub(super) const STOP_LINE: f32 = 12.0;

/// Within this far of its line a car waiting at an intersection goes at
/// most `CROSSING_CRAWL` (0x425150).
pub(super) const CROSSING_SLOW: f32 = 30.0;

pub(super) const CROSSING_CRAWL: f32 = 6.0;

/// The intersection action's look for a car ahead (0x402ED0 with 30 m)
/// and the distance it stops within (15 m).
pub(super) const AHEAD_LOOK: f32 = 30.0;

pub(super) const AHEAD_STOP: f32 = 15.0;

/// A road keeps the right of way while one of its cars is within this of
/// its line (0x409110).
pub(super) const HOLD: f32 = 12.0;

/// The second road may go when the first's nearest car is at least this
/// many seconds from its line (0x4109A0 → 0x408F80).
pub(super) const GAP_TIME: f32 = 3.0;

/// A traffic car's slot at an intersection (an "IncomingTrafficCar":
/// +0x14 + 16·(car id − 0x32): used, road, distance, time).
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub(super) road: usize,
    pub(super) dist: f32,
    pub(super) time: f32,
}

/// The intersections' run-time state (chunk 0x34123's entries, written in
/// place): per intersection the road holding the right of way (+0x10) and
/// a slot per traffic car.
#[derive(Clone, Debug, Default)]
pub struct Crossings {
    pub(super) phase: Vec<Option<usize>>,
    pub(super) slots: Vec<Vec<Option<Slot>>>,
}

impl Crossings {
    pub fn new(f: &RouteFile, cars: usize) -> Crossings {
        Crossings { phase: vec![None; f.intersections.len()], slots: vec![vec![None; cars]; f.intersections.len()] }
    }

    /// 0x413800: a car's think first drops its slot everywhere (the per-road
    /// nearest car is then found again, 0x408FB0; the clone takes the
    /// nearest when asked).
    pub fn leave(&mut self, car: usize) {
        for s in &mut self.slots {
            if let Some(slot) = s.get_mut(car) {
                *slot = None;
            }
        }
    }

    /// Each frame (the placer's 0x4111C0 → 0x409110): a road keeps the
    /// right of way only while one of its cars is within `HOLD` of its
    /// line.
    pub fn tick(&mut self) {
        for (phase, slots) in self.phase.iter_mut().zip(&self.slots) {
            if let Some(r) = *phase {
                if !slots.iter().flatten().any(|s| s.road == r && s.dist < HOLD) {
                    *phase = None;
                }
            }
        }
    }

    /// The time to its line of a road's nearest registered car (0x408F80;
    /// 99999 none).
    pub(super) fn nearest(&self, x: usize, road: usize) -> f32 {
        self.slots[x].iter().flatten().filter(|s| s.road == road).map(|s| s.time).fold(99_999.0, f32::min)
    }

    /// 0x4109A0: whether a road may go: kinds 1 and 2 only; the road
    /// holding the right of way may, the other may not; with neither, road
    /// 0 may and road 1 when road 0's nearest car is `GAP_TIME` or more
    /// from its line.
    pub(super) fn may_go(&self, x: usize, kind: u32, road: usize) -> bool {
        if !(1..=2).contains(&kind) {
            return false;
        }
        match self.phase[x] {
            Some(r) => r == road,
            None => road == 0 || self.nearest(x, 0) >= GAP_TIME,
        }
    }

    /// 0x410A10: a road going takes the right of way unless the other has it.
    pub(super) fn claim(&mut self, x: usize, road: usize) {
        if self.phase[x] != Some(1 - road) {
            self.phase[x] = Some(road);
        }
    }

    /// 0x410A50: two traffic cars registered at an intersection on
    /// different roads, both before their lines' far side (distance ≥ 0).
    pub(super) fn crossing(&self, x: usize, a: usize, b: usize) -> bool {
        match (self.slots[x].get(a).copied().flatten(), self.slots[x].get(b).copied().flatten()) {
            (Some(p), Some(q)) => p.dist >= 0.0 && q.dist >= 0.0 && p.road != q.road,
            _ => false,
        }
    }

    /// Whether a road of an intersection holds the right of way.
    pub fn holds(&self, x: usize) -> Option<usize> {
        self.phase.get(x).copied().flatten()
    }
}

/// Whether a traffic car thinks on this frame (0x429D90 for a traffic car,
/// driver class 3): the frame loop's counter 0x7FB724 (+1 per simulated
/// frame in 0x57A0F0, called once from the loop at 0x5814FA; not the 60 Hz
/// tick 0x7FB720) modulo twice the traffic list's size must be odd, and
/// half of it is the car's slot, so the cars take turns, one every other
/// frame.
pub fn think_slot(update: u64, cars: usize) -> Option<usize> {
    let m = update % (2 * cars.max(1) as u64);
    (m & 1 == 1).then_some((m >> 1) as usize)
}

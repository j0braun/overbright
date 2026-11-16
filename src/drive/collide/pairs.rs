use crate::drive::*;

/// A contact record lives this long after its last impulse (0x592BE0,
/// 0x7FBEE0); a body with a live record is in contact (body +0x18).
pub(in crate::drive) const RECORD_LIFE: f32 = 0.3;

/// The collision pair list's order (0x86B490): pairs are appended at its
/// tail as they are made (0x5A0350), track polygons' and cars' alike, and
/// contacts are gathered in that order, so of two equal times the older
/// pair's wins. One counter for every pair, as the list is one.
pub(in crate::drive) static PAIRS_MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn next_pair() -> u64 {
    PAIRS_MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

pub(in crate::drive) static RECORDS_MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A new contact record's identity (its sound object's +0x24).
pub(crate) fn next_record() -> u64 {
    RECORDS_MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

/// The sliding speed a push hands the record (0x5AF0F0 → 0x5A9630's third
/// argument, record +0x40): the contact's relative velocity less its part
/// along the normal (0x43D0C0), its length — the same tangent the friction
/// ramps in by below 1 m/s.
pub fn slide_speed(v: Vec3, normal: Vec3) -> f32 {
    (v - normal * v.dot(normal)).length()
}

/// A track pair's contact record (0x5A9440): seconds since it was made
/// (witness +0x08) and since its last push (+0x0C, 0x5A9630), the point
/// of that push (+0x30), and whether the polygon is upright (|n.z| < 0.1
/// on the polygon's normal, body +0x60 → +0x78: 0x5A0170).
#[derive(Clone, Copy, Debug)]
pub struct ContactRecord {
    pub age: f32,
    pub life: f32,
    pub point: Vec3,
    pub upright: bool,
    /// |n.z| < 0.7: a wall-like polygon (the clone's own reading, for its
    /// checks and probes; no native test reads it).
    pub wall: bool,
    /// |n.z| > 0.7 (0x5A01C0: either side a track polygon, class 3, whose
    /// normal's z, polygon +0x60 → +0x78, is over 0.7 at 0x7844D8): the
    /// body rests on a floor-like polygon (0x5ED230's contact test).
    pub floor: bool,
    /// The record's sound object has had its first push (0x46A1F0, +0x18).
    pub sounded: bool,
    /// The sliding speed of its last push (+0x40, the push's third
    /// argument: [`slide_speed`]).
    pub slide: f32,
    /// Which record this is, for its sound object ([`next_record`]).
    pub id: u64,
    /// The sum of its pushes' |J| (+0x60, 0x5A9689).
    pub impulse: f32,
    /// Its sound's kind (+0x124) and the direction of that first push
    /// (+0x40, from record +0x70), set by its first update whether or not
    /// it plays, for the wall repeat filter (kind 9 before).
    pub sound: (usize, Vec3),
}

/// What a live record touches, for the AI's contact tests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Touching {
    /// A track polygon, upright or not (0x5A0170).
    Track { upright: bool },
    /// Another car (0x592C70: both bodies of type 0) and that car's speed
    /// (car +0x430, m/s).
    Car { speed: f32 },
}

/// One of the car's live contact records in the pair list's order (car
/// +0x3C, walked by 0x5A0830): seconds since it was made and since its
/// last push, that push's point (world) and what it touches.
#[derive(Clone, Copy, Debug)]
pub struct ContactNote {
    pub life: f32,
    pub age: f32,
    pub point: Vec3,
    pub touching: Touching,
    /// A track polygon with |n.z| < 0.7 (the clone's own wall reading).
    pub wall: bool,
    /// A track polygon with |n.z| > 0.7 (0x5A01C0); never a car.
    pub floor: bool,
    /// The record's summed pushes |J| (+0x60, 0x5A9689), which the
    /// traffic's stuck test reads (0x408DE0: over 30 the car stops).
    pub impulse: f32,
}
/// One car in a [`step_cars`] frame.
pub struct Racer<'a> {
    pub car: &'a Car,
    pub state: &'a mut CarState,
    pub input: Input,
}

/// A pair of cars the collision step keeps between frames (0x5A0350):
/// the distance query's remembered vertices and the pair's contact record.
#[derive(Clone, Default)]
pub(in crate::drive) struct CarPair {
    pub(in crate::drive) cache: Option<narrow::PairCache>,
    /// Seconds since the record was made or pushed; whether it has had a
    /// crash (record +0x10).
    pub(in crate::drive) record: Option<(f32, bool)>,
    /// Seconds since the record was made (witness +0x08) and its last
    /// push's point (+0x30).
    pub(in crate::drive) life: f32,
    pub(in crate::drive) point: Vec3,
    /// The record's sound object has had its first push (0x46A1F0).
    pub(in crate::drive) sounded: bool,
    /// The record's identity ([`next_record`]) and its last push's sliding
    /// speed (+0x40).
    pub(in crate::drive) id: u64,
    pub(in crate::drive) slide: f32,
    /// The record's summed pushes (+0x60) and the hull distance the
    /// pair's last detection found (pair +0x10, 0x5B3E05; None before
    /// its first).
    pub(in crate::drive) impulse: f32,
    pub(in crate::drive) distance: Option<f32>,
    /// When the pair was made: the game appends pairs to its list
    /// (0x5A0350 at the tail 0x86B494) and gathers contacts in list order,
    /// so an older pair's contact wins a tie ([`next_pair`]).
    pub(in crate::drive) made: u64,
}

/// What the world collision step keeps between frames for a set of cars:
/// their pairs (by index into the slice, which must keep its order) and
/// the world's random numbers (0x890080 +0xF8, seeded per race from the
/// game's generator, 0x5FF557).
#[derive(Clone, Default)]
pub struct CarField {
    pub(in crate::drive) pairs: crate::fxhash::FxHashMap<(usize, usize), CarPair>,
    pub rng: crate::opponents::GameRng,
    /// Who sat at each index of the last slice (see [`CarField::bind`]).
    ids: Vec<u64>,
}

impl CarField {
    pub fn new(seed: u32) -> CarField {
        CarField { pairs: Default::default(), rng: crate::opponents::GameRng(seed), ids: Vec::new() }
    }

    /// Names the cars of the coming slice (`ids`, one per index, each
    /// car's own identity) and moves the kept pairs to their cars' new
    /// indices, dropping those of cars no longer in it. The game's pairs
    /// belong to the bodies (0x5A0350); the clone keys them by slice
    /// index, which shifts whenever a traffic car is placed or parked.
    pub fn bind(&mut self, ids: &[u64]) {
        if self.ids.as_slice() == ids {
            return;
        }
        let at = |id: u64| ids.iter().position(|&x| x == id);
        let old = std::mem::take(&mut self.pairs);
        for ((a, b), p) in old {
            if let (Some(&ia), Some(&ib)) = (self.ids.get(a), self.ids.get(b))
                && let (Some(na), Some(nb)) = (at(ia), at(ib))
            {
                self.pairs.insert((na, nb), p);
            }
        }
        self.ids = ids.to_vec();
    }

    pub(in crate::drive) fn has_record(&self, i: usize) -> bool {
        self.pairs.iter().any(|(&(a, b), p)| (a == i || b == i) && p.record.is_some())
    }

    /// Whether car `i` (its index in the [`step_cars`] slice) has a live
    /// contact record with another car.
    pub fn touching(&self, i: usize) -> bool {
        self.has_record(i)
    }

    /// The identities ([`CarField::bind`]) of the cars the car named `id`
    /// has a live contact record with (its body's record list, car +0x3C).
    pub fn partners(&self, id: u64) -> Vec<u64> {
        let mut out: Vec<u64> = Vec::new();
        for (&(a, b), p) in &self.pairs {
            if p.record.is_none() {
                continue;
            }
            let (ia, ib) = (self.ids.get(a).copied(), self.ids.get(b).copied());
            if ia == Some(id)
                && let Some(o) = ib
            {
                out.push(o);
            } else if ib == Some(id)
                && let Some(o) = ia
            {
                out.push(o);
            }
        }
        out.sort_unstable();
        out
    }
}

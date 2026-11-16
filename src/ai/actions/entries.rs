use super::*;

/// An object the racer's road entries can hold: a car (racer, human or
/// traffic) with its state this frame.
#[derive(Clone, Copy, Debug)]
pub struct RoadCar {
    /// Stable identity (the action's target, +0x218).
    pub id: usize,
    pub position: Vec3,
    pub velocity: Vec3,
    pub forward: Vec2,
    /// Acceleration along its velocity (0x60BE40 → 0x5EC340; m/s²).
    pub accel: f32,
    /// 5EC3B0: retained world acceleration along the full3D forward axis.
    pub forward_accel: f32,
    /// Half length and half width (shape extents, 0x5AACE0).
    pub half: Vec2,
    /// A racer (0x60BEB0: object +0x1C → +0x1C set): the human or an AI
    /// racer, not traffic.
    pub racer: bool,
    /// The human (0x614890: +0x1C → +0x14 → +4 = 1).
    pub human: bool,
    /// Traffic (0x6148D0: +0x1C → +0x14 → +4 = 3, the controller type).
    pub traffic: bool,
    /// Ghosted (0x60BF80 → 0x5EBFB0: car +0x974 / +0x978 still running).
    pub ghosted: bool,
    /// The world acceleration (car +0x80, the object's +0x14 that 0x420B80
    /// steps a traffic car's path with).
    pub acceleration: Vec3,
    /// The path the car drives (car +0x30 → +0x10, at most 20 points; a
    /// racer's last driven path), which 0x4275E0 sweeps it along.
    pub path: CarPath,
}

/// A driver's path (car +0x30 → +0x10: a count byte, then up to 20 points
/// of 16 bytes), kept by value so that [`RoadCar`] stays `Copy`.
#[derive(Clone, Copy, Debug, Default)]
pub struct CarPath {
    pub n: u8,
    pub p: [Vec2; PATH_MAX],
}

impl CarPath {
    pub fn from_points(points: &[Vec3]) -> CarPath {
        let mut c = CarPath::default();
        for q in points.iter().take(PATH_MAX) {
            c.p[c.n as usize] = q.truncate();
            c.n += 1;
        }
        c
    }

    pub fn points(&self) -> &[Vec2] {
        &self.p[..self.n as usize]
    }

    #[cfg(test)]
    pub(super) fn with_y(mut self, y: f32) -> CarPath {
        for q in &mut self.p[..self.n as usize] {
            q.y = y;
        }
        self
    }
}

/// The most points a driver's path holds (0x404CD0 refuses a 21st,
/// count byte > 0x13; 0x428CE0 sweeps only paths of 1…20 points).
pub const PATH_MAX: usize = 20;

/// One road entry (controller +0x08 + 0x30k).
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub car: RoadCar,
    /// +0x10: s along the racer's list; 0x60BFB0(lane 0): off the line.
    pub at: Located,
    /// +0x00 / +0x04: the object's velocity less the car's, across and
    /// along the car's direction (0x6148F0: both velocities turned by
    /// 0x4000 − the heading of 0x40ADD0's direction, 0x5BAE70). The clone
    /// takes the list's direction for 0x40ADD0's and +0x00 positive to
    // prov: inferred ai.actions.module
    /// the right, the side `Located::lateral` grows to (**inferred**).
    pub rel_lateral: f32,
    pub closing: f32,
    /// +0x08 / +0x0C: half extents across and along the travel (k = cos²
    /// of the heading difference, see Road entries).
    pub across: f32,
    pub along: f32,
}

/// The lane record +0x30C: lane, offset, rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub lane: i32,
    pub offset: f32,
    pub rate: f32,
}

impl Plan {
    pub fn plain(rate: f32) -> Plan {
        Plan { lane: 0, offset: 0.0, rate }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Type 6, priority 0, no end (0x4235A0).
    JustDrive,
    /// Type 0, 5 s (0x423610).
    Pass,
    /// Type 4, no time limit (0x4238C0).
    Block,
    /// IJustGotHit: type 0xB, priority 400, no time limit (0x423CC0,
    /// vtable 0x7857B4).
    GotHit,
    /// AvoidAvoidable: type 1, priority 300, no time limit (0x4243B0,
    /// vtable 0x785858).
    Avoid,
    /// AccidentRecovery: type 0xD, priority 500, no time limit (0x4241C0,
    /// vtable 0x7857E8).
    Recover,
    /// "Drive at offset": type 0x13, priority 0, no end (0x426F20 →
    /// 0x4234A0(0x13, 0, −1), vtable 0x785A18; slots 6–8 0x403170 false),
    /// AIGoalOffsetPursuit's default action (`pursuit`).
    OffsetPursuit,
}

impl Kind {
    pub fn type_id(self) -> i32 {
        match self {
            Kind::JustDrive => 6,
            Kind::Pass => 0,
            Kind::Block => 4,
            Kind::GotHit => 0xB,
            Kind::Avoid => 1,
            Kind::Recover => 0xD,
            Kind::OffsetPursuit => 0x13,
        }
    }
}

/// What the action hands the controller besides the path: the target
/// speed's cap (+0x1F0; None with +0x1FC set, the racing-line speed),
/// whether the pedals may be used (+0x1FD → driver +0x23C) and a reset
/// request (0x5EBF70: car +0x96C).
#[derive(Clone, Debug)]
pub struct Steer {
    pub path: Vec<Vec3>,
    pub cap: Option<f32>,
    pub pedals: bool,
    pub reset: bool,
}

/// What the racer's think needs besides its route.
pub struct Scene<'a> {
    pub routes: &'a AiRoutes,
    pub me: RoadCar,
    pub others: &'a [RoadCar],
    /// The race clock (0 at the green light).
    pub clock: f32,
    /// JustDrive's plain-line rate (0.2 in Street X, else 0.05).
    pub rate: f32,
    pub street_x: bool,
    pub drift: bool,
    /// A drag race (0x89E7D8 / 0x890108).
    pub drag: bool,
    /// 0x5EF000: the start hold (racer numbers 100…105).
    pub held: bool,
    /// The tuning table row for Block (0x81CB60 +0x934 + 8·(racer
    /// +0x902)).
    pub block_row: usize,
    /// The racers whose bodies have a live contact record with this car
    /// (0x402AE0 walks the body's records, car +0x3C; racers only, its
    /// flag 1), and whether it has any live car contact at all.
    pub hits: &'a [usize],
    pub touching: bool,
    /// Wheels on the ground (+0x424), the up axis's z, the speed along
    /// the heading (+0x42C).
    pub wheels_down: usize,
    pub up_z: f32,
    pub forward_speed: f32,
    /// The skill row's odds of taking a pass or an avoid (0x408280: the
    /// AI tuning +0x530 + 0x68·row).
    pub odds: f32,
}

/// The racer controller's reach (0x404910, ECX = car +0x2C): max(125 m
/// (0x7E8E60), vt+0x60 × car +0x430), vt+0x60 = 0x408240 = 3.0 s for the
/// racer controller (vtable 0x785370; 0x7E8E0C / 0x783670 = 3.0, 0x783B40
/// = 2.0, max of the three), **verified**. The speed is signed: reversing
/// gives 125 m.
pub fn reach(speed: f32) -> f32 {
    REACH.max(REACH_TIME * speed)
}

/// The road entries (0x6148F0): the car's own and up to 15 others, each
// prov: open ai.actions.entries
/// taken when its 3D distance is at most the reach ([`reach`], 0x614944;
/// `fcomp; test ah,0x41; jnp` takes equal) or 0x403A40 finds
/// it reached in 3 s at ±|Δv| (negative when the car's forward meets the
/// other's direction head on). Sorted by the signed squared distance in
/// (s, offset) from the car, largest first (ahead first, the farthest
/// ahead leading; behind after, the nearest first). +0x08 / +0x0C: the
/// other's half width and length mixed by k = cos² of its heading less
/// the car's (+0x40C): across = k·w + (1 − k)·l, along = (1 − k)·w + k·l.
/// **Not ported**: the race-coordinate window (|Δ| ≤ 3 × reach, 0x5D5CE0)
/// and the object filter (0x5EC070, car +0x550).
pub fn entries(route: &RacerRoute, scene: &Scene, accel: f32) -> (Vec<Entry>, Option<usize>) {
    let mut list: Vec<Entry> = Vec::new();
    let me = scene.me;
    // Every object, the car itself too, is filed by 0x419C10 → 0x419B20
    // (`locate_object`: the nearest list point, s extrapolated off the
    // list's span).
    let Some(mine) = route.locate_object(scene.routes, me.position) else { return (Vec::new(), None) };
    let dir = mine.direction;
    let across_dir = Vec2::new(dir.y, -dir.x);
    let mut push = |c: &RoadCar, at: Located| {
        let dv = c.velocity.truncate() - me.velocity.truncate();
        let cos = c.forward.dot(me.forward).clamp(-1.0, 1.0);
        let k = cos * cos;
        list.push(Entry {
            car: *c,
            at,
            rel_lateral: dv.dot(across_dir),
            closing: dv.dot(dir),
            across: k * c.half.y + (1.0 - k) * c.half.x,
            along: (1.0 - k) * c.half.y + k * c.half.x,
        });
    };
    push(&me, mine);
    let reach = reach(scene.forward_speed);
    let mut taken = 0;
    for c in scene.others.iter().filter(|c| c.id != me.id) {
        if taken >= ENTRIES {
            break;
        }
        let d = c.position.distance(me.position);
        let dv = (c.velocity - me.velocity).length();
        let closing = if me.forward.dot(c.forward) < 0.0 { -dv } else { dv };
        let a = if me.forward.dot(c.forward) > 0.0 { accel - c.accel } else { accel + c.accel };
        if !(d <= reach || d < -((3.0 * a * 0.5 + closing) * 3.0)) {
            continue;
        }
        let Some(at) = route.locate_object(scene.routes, c.position) else { continue };
        push(c, at);
        taken += 1;
    }
    let key = |e: &Entry| {
        let ds = e.at.s - mine.s;
        let q = ds * ds + (e.at.lateral - mine.lateral).powi(2);
        if ds < 0.0 { -q } else { q }
    };
    list.sort_by(|a, b| key(b).total_cmp(&key(a)));
    let own = list.iter().position(|e| e.car.id == me.id);
    (list, own)
}

/// 0x60C000: the entry's s less the car's.
pub(super) fn gap(list: &[Entry], own: usize, e: &Entry) -> f32 {
    e.at.s - list[own].at.s
}

/// 0x403A40(target, car, gap, closing, …): reached within 3 s (0x7E8E0C)
/// at the closing speed and the cars' accelerations: gap < −3·(1.5·a +
/// closing), a = the car's acceleration along its heading (0x5EC3B0) less
/// the target's (0x60BE40; plus it when the target faces away).
pub(super) fn reachable(me: &RoadCar, own_accel: f32, e: &Entry, gap: f32) -> bool {
    let a = if me.forward.dot(e.car.forward) > 0.0 { own_accel - e.car.accel } else { own_accel + e.car.accel };
    gap < -((3.0 * a * 0.5 + e.closing) * 3.0)
}

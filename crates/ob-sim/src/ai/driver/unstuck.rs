use super::*;

/// AIActionGetUnstuck (type 9, vtable 0x785784): the racer backs off
/// whatever holds it and rejoins. The goal's chooser (0x42D0F0, "Win road
/// race" slot +0x38) creates it at priority 500 when the stuck test
/// (0x40E7C0) passes and the last one started over 6 s ago (controller
/// vt+0x94 0x4083A0 on +0x708, 0x783B50); its target speed is −15 m/s for
/// racers (controller type 3, 0x7836D8; else −6) along the racer's path
/// (0x420940).
pub(super) const UNSTUCK_SPEED: f32 = -15.0;
pub(super) const UNSTUCK_COOLDOWN: f64 = 6.0;
/// GetUnstuck's priority (0x40E7C0's 500, the ctor 0x423C30's first
/// argument through 0x4234A0 to +0x200).
pub const UNSTUCK_PRIORITY: i32 = 500;
/// It may not end before 1.3 s (ctor arg from vt+0x98 0x4083E0, its
/// default 0x783B48; +0x210 / +0x214 / +0x218) and ends after 5 s (base
/// timeout +0xC, 0x783628; slot 8 0x403250). vt+0x98 says 1.6 s
/// (0x783B4C) instead when the car is past its road's edge (vt+0x2C
/// 0x40D4F0 > 0) and the race-progress speed's running mean (+0x7A4) is
/// full and under 0.2 m/s (0x7E9574).
pub(super) const UNSTUCK_MIN: f64 = 1.3;
pub(super) const UNSTUCK_MIN_SLOW: f64 = 1.6;
pub(super) const SLOW_PROGRESS: f32 = 0.2;
/// The running mean's samples (0x7E957C) and their spacing (0x7E9578; the
/// timer +0x7A0 starts at construction, 0x4261DD).
pub(super) const PROGRESS_SAMPLES: usize = 5;
pub(super) const PROGRESS_EVERY: f32 = 1.5;
pub(super) const UNSTUCK_TIMEOUT: f64 = 5.0;
/// A contact counts once it is 0.4 s old (0x7836E4) and touched in the
/// last 0.25 s (0x784D28), at a forward speed of 5 m/s or less (0x7836E8).
pub(super) const CONTACT_AGE: f64 = 0.4;
pub(super) const CONTACT_RECENT: f32 = 0.25;
pub(super) const CONTACT_SPEED: f32 = 5.0;
/// Further than this from the route is stuck too (0x7836F4), and nothing
/// starts within 2 s of the car being placed (car +0x9C0, 0x5ED590 against
/// 0x7836F8).
pub(super) const OFF_ROUTE: f32 = 20.0;
pub(super) const SETTLE: f32 = 2.0;
/// The road-edge test (0x40DF50) counts towards a start under this speed
/// (a racer, 0x7836EC).
pub(super) const EDGE_SPEED: f32 = 25.0;
/// Facing the path this well ends it (0x7836E0, the cosine).
pub(super) const FACING: f32 = 0.9;

/// A point in the car's frame of travel (0x591FD0 with the frame
/// 0x591DF0): the velocity's direction, reversed when the forward speed
/// (+0x42C) is negative, or the heading when |v|² ≤ 1; the point's
/// distance right of it and along it from the car's position.
pub fn travel_frame(car: &CarState, p: Vec3) -> (f32, f32) {
    let v = car.velocity.truncate();
    let d = if v.length_squared() > 1.0 {
        let d = v.normalize();
        if car.speed() < 0.0 { -d } else { d }
    } else {
        car.forward().truncate().normalize_or_zero()
    };
    let r = (p - car.position).truncate();
    (r.dot(Vec2::new(d.y, -d.x)), r.dot(d))
}

/// Where a contact touches the car (0x402A40, its record's point +0x30 in
/// the frame of travel): within 95% of the half width it is the front or
/// the rear, by the sign along the way of travel; else a side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactSide {
    Front,
    Rear,
    Side,
}

/// The racer's road where it is (the route's current control point, car
/// +0x2C → +0xC, travelled backward when +0x10 is clear): the point, its
/// own tangent, the tangent of travel (0x401AA0 backward), the left and
/// right half widths (+0x14 / +0x16).
#[derive(Clone, Copy, Debug)]
pub struct RoadEdge {
    pub position: Vec2,
    pub tangent: Vec2,
    pub travel: Vec2,
    pub half_width: [f32; 2],
    pub backward: bool,
}

/// 0x40DF50(car, check): is the car running off its road? 0 no; 1 yes; 2
/// moving forward but turned more than 70° from the road (cos ≤ 0.342).
/// Under 1 m/s: the point 2 m ahead along the heading is past an edge
/// (by the point's own tangent) and further out than the car. Else the
/// car and the point 0.1 s ahead along its velocity, to the right of the
/// way of travel: the time to the right edge at that lateral rate (only
/// that side is timed, as built: moving left reads 99999 s), 1 s or less,
/// or either side already past its edge and getting worse, is a yes —
/// then forward speed > 0 gives 2 or 0 by the heading, else 1. With
/// `check` a yes is taken back when `look` (the controller's point, vt
/// +0x80) lies within 60° of the velocity (the heading under 1 m/s,
/// 0x4036E0 with 0.5).
pub fn leaving_road(car: &CarState, edge: &RoadEdge, look: Option<Vec3>, check: bool) -> u8 {
    let at = car.position.truncate();
    let heading = car.forward().truncate().normalize_or_zero();
    let side = |p: Vec2, t: Vec2| (p - edge.position).dot(Vec2::new(t.y, -t.x));
    let speed = car.velocity.length();
    if speed <= 1.0 {
        let (l0, l1) = (side(at, edge.tangent), side(at + heading * 2.0, edge.tangent));
        let (left, right) = (edge.half_width[0], edge.half_width[1]);
        let out_right = l1 > right && l1 > l0;
        let out_left = l1 < -left && l1 < l0;
        if !out_right && !out_left {
            return 0;
        }
    } else {
        let q = at + car.velocity.truncate() * 0.1;
        let (l, lq) = (side(at, edge.travel), side(q, edge.travel));
        let (left, right) = if edge.backward {
            (edge.half_width[1], edge.half_width[0])
        } else {
            (edge.half_width[0], edge.half_width[1])
        };
        let (a, aq, b, bq) = (right - l, right - lq, left + l, left + lq);
        let rate = (bq - b) / 0.1;
        let t = if rate > 0.001 && a > 0.0 && b > 0.0 { a / rate } else { 99999.0 };
        if t > 1.0 && !(a < 0.0 && aq < a) && !(b < 0.0 && bq < b) {
            return 0;
        }
        // The forward test reads car +0x42C (0x40E142 against 0.0 at
        // 0x7D7D3C), the speed the car's mover exports, not the body's
        // velocity along the heading: a car on the BeltMover (mode 4) shows
        // the belt's speed there whatever way its placed body slid this
        // frame.
        if car.exported_forward_speed() > 0.0 {
            return if heading.dot(edge.tangent.normalize_or_zero()) <= 0.342 { 2 } else { 0 };
        }
    }
    if check && let Some(p) = look {
        let dir = if speed < 1.0 { heading } else { car.velocity.truncate().normalize_or_zero() };
        if dir.dot((p.truncate() - at).normalize_or_zero()) > 0.5 {
            return 0;
        }
    }
    1
}

/// How far the car is beyond its road's edge (0x401C10 on 0x40CC40): its
/// offset l right of the way of travel off the route point, then l − the
/// right half width when l > 0, else −l − the left one (the halves swap
/// travelled backward, 0x401BC0); negative on the road.
pub fn past_edge(car: &CarState, edge: &RoadEdge) -> f32 {
    let t = edge.travel;
    let l = (car.position.truncate() - edge.position).dot(Vec2::new(t.y, -t.x));
    let (left, right) =
        if edge.backward { (edge.half_width[1], edge.half_width[0]) } else { (edge.half_width[0], edge.half_width[1]) };
    if l > 0.0 { l - right } else { -l - left }
}

/// A racer's GetUnstuck action and what it remembers between them.
#[derive(Clone, Debug, Default)]
pub struct Unstuck {
    /// When the running one started.
    pub since: Option<f64>,
    /// When the last one started (controller +0x708).
    last: Option<f64>,
    /// Controller +0x7A4 (0x579F30, 5 samples): the race-progress speed,
    /// racer status +0x18 (laps/s) × the road's length (0x88F2B0), pushed
    /// (0x576980) every 1.5 s at 0x42C357; +0x7A0 the time to the next.
    progress: Vec<f32>,
    next_sample: Option<f32>,
    /// The running one's minimum run (vt+0x98 at its construction).
    min: f64,
    /// Whether the running one's path has been planned (its begin
    /// 0x420940: 0x41D160(0, 0) drops the driven path, so its first lane
    /// path is planned from the car).
    planned: bool,
    /// The race clock of the car's last placement on the road by a reset
    /// (car +0x9C0, stamped by every exit of 0x5F52D0, reached from
    /// Car_PlaceOnRoad 0x5FBEE0 for the AI resets); None: placed on the grid
    /// only, which the original stamps long before the green (observed
    /// 8.88 s before it in circuit_s1_pinned), so its 2 s have passed by
    /// the green.
    pub placed: Option<f32>,
}

/// GetUnstuck's lane path (its update 0x423E20 → 0x420D40 on the lane
/// record +0x30C): the record's rate +0x08 = 0.25 per metre (0x783718,
/// 0x423E5F / 0x423FDE) and its offset +0x04 = 0 (0x7D7D3C) while the gated
/// block before it (0x423E95 … 0x423FD2) is skipped, against JustDrive's
/// 0.05 (0x7E8E64): the path leaves the car's offset for the line five
/// times as fast.
pub const UNSTUCK_LANE_RATE: f32 = 0.25;

/// The class of a contact record's point (0x402A40).
pub fn contact_side(car: &CarState, handling: &Handling, p: Vec3) -> ContactSide {
    let (right, along) = travel_frame(car, p);
    if right.abs() < 0.95 * handling.dimensions[1] * 0.5 {
        if along > 0.0 { ContactSide::Front } else { ContactSide::Rear }
    } else {
        ContactSide::Side
    }
}

impl Unstuck {
    /// While one runs: whether this is its first lane path (planned from
    /// the car, 0x420940's 0x41D160(0, 0)); later frames keep the path as
    /// 0x420D40 does.
    pub fn first_path(&mut self) -> bool {
        self.since.is_some() && !std::mem::replace(&mut self.planned, true)
    }

    /// One think (0x42ABD0's action pass): track the contact, end a running
    /// action — timed out (slot 8), backed into something (slot 7
    /// 0x40E9F0: a rear contact held at low speed) or facing the path again
    /// with nothing held in front (slot 6 0x414440) — or start one when the
    /// racer is held (a front or side contact) or far off its route
    /// (0x40E7C0). The reverse target speed while it runs.
    /// `edge` is its road for 0x40DF50 and the off-route test;
    /// `since_placed` the seconds since the car was put on the grid or
    /// reset (car +0x9C0, stamped by 0x5F52D0).
    /// `toward` is the route's facing point (0x4143F0,
    /// [`RacerRoute::facing_point`](crate::ai::route::RacerRoute::facing_point));
    /// `look` the driver's look-ahead point (driver vt+0x80 0x405BC0:
    /// +0x224, last frame's, as the controllers think before the drivers).
    /// `progress` is this frame's race-progress speed (m/s) when the feed
    /// runs (0x42C2F6: a coordinate race 0x88F2BE with a race on,
    /// 0x89E7B0 neither 0 nor 7), with the frame time.
    /// `running` is the priority of the action the controller runs (−1
    /// for none): the chooser (0x42D0F0) proposes GetUnstuck only while it
    /// is ≤ 500, stamping controller +0x708 (the 6 s cooldown) as it does,
    /// and the manager (0x429FB2: `cmp [new +0x200], ebp; jg`) takes the
    /// proposal only above it, so an AccidentRecovery (priority 500) is
    /// never displaced and runs on to its reset.
    /// `held`: 0x40E7C0's early −1 returns — the start hold 0x5EF000
    /// (racer numbers 100…105, 12 − 0.5·(n − 100) s; under Wine a rival
    /// teleported against the pocket's wall at 7 s pivoted out with no
    /// GetUnstuck, one put there at 13 s backed off at once) and 0x60BF00
    /// (two wheels or fewer down).
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        car: &CarState,
        handling: &Handling,
        toward: Option<Vec2>,
        look: Option<Vec2>,
        edge: Option<&RoadEdge>,
        since_placed: f32,
        progress: Option<(f32, f32)>,
        clock: f64,
        running: i32,
        held: bool,
    ) -> Option<f32> {
        if let Some((speed, dt)) = progress {
            // 0x577320 counts the timer down; once out it restarts at 1.5 s
            // (0x401840) and the sample is pushed.
            let left = self.next_sample.get_or_insert(PROGRESS_EVERY);
            *left -= dt;
            if *left <= 0.0 {
                *left = PROGRESS_EVERY;
                if self.progress.len() == PROGRESS_SAMPLES {
                    self.progress.remove(0);
                }
                self.progress.push(speed);
            }
        }
        // The car's records (car +0x3C) by class, made 0.4 s ago or more
        // (+0x08), with the car's speed (+0x430) at most 5 m/s (0x4010C0
        // within 0.001 for the start and slot 7, strictly under for slot 6);
        // the start also wants a push in the last 0.25 s (+0x0C).
        let v = car.velocity.length();
        let slow = v < CONTACT_SPEED || (v - CONTACT_SPEED).abs() <= 0.001;
        let record = |sides: &[ContactSide], recent: bool| {
            car.contacts.iter().any(|n| {
                sides.contains(&contact_side(car, handling, n.point))
                    && f64::from(n.life) >= CONTACT_AGE
                    && (!recent || n.age < CONTACT_RECENT)
            })
        };
        let look = look.map(|p| p.extend(car.position.z));
        let leaving = |e: &RoadEdge| leaving_road(car, e, look, true);
        let facing_at = toward;
        if let Some(start) = self.since {
            let age = clock - start;
            let backed = slow && record(&[ContactSide::Rear], false);
            let blocked = v < CONTACT_SPEED && record(&[ContactSide::Front], false);
            let done = age > UNSTUCK_TIMEOUT
                || (age >= self.min
                    && (backed
                        || edge.is_some_and(|e| leaving(e) == 2)
                        || (since_placed > SETTLE && facing(car, facing_at) && !blocked)));
            if !done {
                return Some(UNSTUCK_SPEED);
            }
            self.since = None;
            return None;
        }
        let ready = since_placed > SETTLE && self.last.is_none_or(|t| clock - t > UNSTUCK_COOLDOWN);
        // In a drift or Street X race the car-state block (the off-route
        // distance and the road-edge test) is skipped and only the contacts
        // count: 0x40E7C0 goes straight to its contact loop (0x40E8E5) under
        // 0x89E7D9 / 0x890104 / 0x89E7E3 / 0x89010C.
        let car_state = !(handling.race.drift() || handling.race.street_x());
        let off_route = car_state && edge.is_some_and(|e| past_edge(car, e) > OFF_ROUTE);
        let off_road = car_state && car.velocity.length() < EDGE_SPEED && edge.is_some_and(|e| leaving(e) == 1);
        let pinned = slow && record(&[ContactSide::Front, ContactSide::Side], true);
        if running <= UNSTUCK_PRIORITY && ready && !held && (pinned || off_route || off_road) {
            self.last = Some(clock);
            if running >= UNSTUCK_PRIORITY {
                // Proposed and deleted (the priority is not above).
                return None;
            }
            let crawling = self.progress.len() == PROGRESS_SAMPLES
                && self.progress.iter().sum::<f32>() / (PROGRESS_SAMPLES as f32) < SLOW_PROGRESS;
            self.min =
                if crawling && edge.is_some_and(|e| past_edge(car, e) > 0.0) { UNSTUCK_MIN_SLOW } else { UNSTUCK_MIN };
            self.since = Some(clock);
            self.planned = false;
            self.last = Some(clock);
            return Some(UNSTUCK_SPEED);
        }
        None
    }
}

/// The car points at its route (0x414440 with 0x4143F0): the heading
/// against the way to the facing point, 2D, cosine 0.9 or more.
pub(super) fn facing(car: &CarState, point: Option<Vec2>) -> bool {
    let at = car.position.truncate();
    let Some(p) = point else { return false };
    (p - at).normalize_or_zero().dot(car.forward().truncate().normalize_or_zero()) >= FACING
}

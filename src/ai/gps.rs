//! The GPS route search: the router 0x81CBC0 (0x40C440 start, 0x4268A0
//! update from the world update 0x609D32, 0x4131F0 clear) and its A*
//! over the route file's race records (0x422B90 the start nodes, 0x422F90
//! the search), the route a driver keeps (+0x3E4 / +0x3F0, filled by the
//! router's callback 0x41EA30) and the walk along it (0x41AF90).
//!
//! See docs/formats.md, "GPS", for what is verified and what is inferred.

use glam::{Vec2, Vec3};

use crate::formats::ai_routes::{Junction, RouteFile};

/// A control point: record index and point index.
pub type Cp = (usize, usize);

/// The A* lists' caps (0x423222: open < 0x9F; 0x423267: closed < 0x4FF).
const OPEN_MAX: usize = 0x9F;
const CLOSED_MAX: usize = 0x4FF;
/// A node's recorded junction decisions (+0x38, while +0xB9 < 0x3F).
const PATH_MAX: usize = 0x3F;
/// 0x420270 gives up after 10000 m (0x784500) of one straight walk.
const EXTEND_MAX: f32 = 10000.0;
/// No point (0x47C34F80): a dead end's estimate.
const NO_ESTIMATE: f32 = 100000.0;
// prov: choice ai.gps.FRAME_WORK
/// The work each frame's search may do. The original stops it on time:
/// 0x43E5C0 (the milliseconds since the frame's 0x43BDF0 reading) + 0.4
/// (0x783D50 / 0x783D54) past 1.0 (0x783D4C), so 0.6 ms of real time,
/// which ends the search on a machine-dependent frame. The clone counts
/// work units instead, the same on every machine and run (see
/// [`Router::update`]): this many are what 0.6 ms bought in the median
/// slice on the reference desktop (docs/formats.md, "GPS"; the probe
/// `search_cost_survey`).
pub const FRAME_WORK: u32 = 700_000;
/// The search mode for the player's car (0x413120 / 0x4188C0: owner kind
/// (car +0x14 → +4) 1 → 2, 3 (traffic) → 1, else 0).
pub const MODE_PLAYER: u8 = 2;

/// 0x5D8BC0: a control point usable in the mode (flags bit 11 / 25 / 27).
fn usable(f: &RouteFile, (r, p): Cp, mode: u8) -> bool {
    let flags = f.records[r].points[p].flags;
    match mode {
        0 => flags >> 11 & 1 != 0,
        1 => flags >> 25 & 1 != 0,
        2 => flags >> 27 & 1 != 0,
        _ => false,
    }
}

/// A junction's type in the mode (mode 1 reads the high nibble).
fn kind(j: &Junction, mode: u8) -> u32 {
    if mode == 1 { j.flags >> 4 & 0xF } else { j.flags & 0xF }
}

/// 0x5DCBF0: a link the route must take (2; 8 too in mode 2).
fn forced(j: &Junction, mode: u8) -> bool {
    match mode {
        0 | 1 => kind(j, mode) == 2,
        2 => matches!(j.flags & 0xF, 2 | 8),
        _ => false,
    }
}

/// 0x5DCBA0 / 0x5DCC40: links the route never takes (traffic: 7 (and 8
/// in mode 0), 5 and 10).
fn ignored(j: &Junction, mode: u8) -> bool {
    let k = kind(j, mode);
    let a = match mode {
        0 => matches!(k, 7 | 8),
        1 | 2 => k == 7,
        _ => false,
    };
    a || (mode <= 2 && matches!(k, 5 | 10))
}

/// 0x401CC0: the next control point travelled forward or backward.
fn next(f: &RouteFile, (r, p): Cp, forward: bool) -> Option<Cp> {
    if forward {
        (p + 1 < f.records[r].points.len()).then_some((r, p + 1))
    } else {
        p.checked_sub(1).map(|p| (r, p))
    }
}

/// 0x5D86E0: a junction's target point.
fn target(f: &RouteFile, j: &Junction) -> Option<Cp> {
    let (id, point) = j.target();
    let r = f.record(id)?;
    let p = usize::try_from(point).ok().filter(|&p| p < f.records[r].points.len())?;
    Some((r, p))
}

fn position(f: &RouteFile, (r, p): Cp) -> Vec3 {
    f.records[r].points[p].position
}

/// 0x40CD00: how far a point lies outside the road across (negative
/// inside): right of the point's direction past +0x16, left past +0x14.
fn outside_across(f: &RouteFile, (r, p): Cp, at: Vec2) -> f32 {
    let c = &f.records[r].points[p];
    let d = at - c.position.truncate();
    let l = d.x * c.tangent.y - d.y * c.tangent.x;
    if l > 0.0 { l - c.half_width[1] } else { -(c.half_width[0] + l) }
}

/// 0x40CE10 on (point − cp) · tangent: past the segment ahead, or behind
/// the previous one (its own length for the first point).
fn outside_along(f: &RouteFile, (r, p): Cp, at: Vec2) -> f32 {
    let c = &f.records[r].points[p];
    let a = (at - c.position.truncate()).dot(c.tangent);
    if a > 0.0 {
        a - c.length
    } else {
        let prev = p.checked_sub(1).map_or(c.length, |q| f.records[r].points[q].length);
        -(prev + a)
    }
}

/// 0x413250: the goal lies on this point's stretch of road (across, along)
/// and within 4 m of its height.
fn holds(f: &RouteFile, cp: Cp, goal: Vec3) -> bool {
    outside_across(f, cp, goal.truncate()) < 0.0 && outside_along(f, cp, goal.truncate()) < 0.0 && (position(f, cp).z - goal.z).abs() < 4.0
}

/// A search node (0xC0 bytes from pool 0x81CBB8).
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// +0x00: the start point.
    pub start: Option<Cp>,
    /// +0x04: the point reached.
    pub cp: Option<Cp>,
    /// +0x08: metres travelled; +0x0C: the straight distance left.
    pub g: f32,
    pub h: f32,
    /// +0x10: the goal.
    pub goal: Vec3,
    /// +0x20: up to three successors {point, forward, junction (0xFFFF none)}, count +0xB8.
    succ: Vec<(Cp, bool, u16)>,
    /// +0x38: the junctions passed (low 13 bits) and taken (0x2000).
    pub path: Vec<u16>,
    /// +0xBA: the goal reached.
    pub reached: bool,
    /// +0xBB: the start direction; +0xBC the current one (true forward).
    pub start_forward: bool,
    pub forward: bool,
}

impl Node {
    /// 0x4201D0.
    fn new(f: &RouteFile, cp: Option<Cp>, forward: bool, goal: Vec3, mode: u8) -> Node {
        let mut n = Node { start: cp, cp, g: 0.0, h: 0.0, goal, succ: Vec::new(), path: Vec::new(), reached: false, start_forward: forward, forward };
        n.estimate(f, mode);
        n
    }

    /// 0x420450..: the estimate, then the successors (0x41AE40).
    fn estimate(&mut self, f: &RouteFile, mode: u8) {
        self.h = match self.cp {
            None => NO_ESTIMATE,
            Some(_) if self.reached => 0.0,
            Some(cp) => (position(f, cp).truncate() - self.goal.truncate()).length(),
        };
        self.successors(f, mode);
    }

    /// 0x41AE40.
    fn successors(&mut self, f: &RouteFile, mode: u8) {
        self.succ.clear();
        let Some(cp) = self.cp.filter(|_| !self.reached) else { return };
        let c = &f.records[cp.0].points[cp.1];
        for j in f.junctions_at(c) {
            let jn = &f.junctions[j];
            if jn.direction() == self.forward {
                if jn.flags & 0xF == 3 {
                    return;
                }
                continue;
            }
            let fw = self.forward ^ jn.flips();
            if forced(jn, mode) {
                // 0x40C580 with a forced link: no junction recorded.
                if let Some(t) = target(f, jn) {
                    self.succ.push((t, fw, 0xFFFF));
                }
                return;
            }
            if !ignored(jn, mode)
                && let Some(t) = target(f, jn)
            {
                self.succ.push((t, fw, j as u16));
            }
        }
        if let Some(n) = next(f, cp, self.forward) {
            self.succ.push((n, self.forward, 0xFFFF));
        }
    }

    /// 0x420270: walk on while the road does not branch, then estimate.
    /// The points walked (the search's work, [`Router::update`]).
    fn extend(&mut self, f: &RouteFile, mode: u8) -> u32 {
        let g0 = self.g;
        let mut branch = false;
        let mut walked = 0;
        while let Some(cp) = self.cp {
            if branch {
                break;
            }
            walked += 1;
            if self.g - g0 > EXTEND_MAX {
                self.cp = None;
                break;
            }
            if holds(f, cp, self.goal) {
                self.reached = true;
                break;
            }
            if !usable(f, cp, mode) {
                self.cp = None;
                break;
            }
            let c = &f.records[cp.0].points[cp.1];
            let mut take = None;
            for j in f.junctions_at(c) {
                if take.is_some() || branch {
                    break;
                }
                let jn = &f.junctions[j];
                if jn.direction() == self.forward {
                    continue;
                }
                if jn.flags & 0xF == 3 {
                    self.cp = None;
                    break;
                }
                if forced(jn, mode) {
                    take = Some(*jn);
                } else if !ignored(jn, mode) {
                    branch = true;
                }
            }
            if branch || self.cp.is_none() {
                continue;
            }
            match take {
                Some(jn) => {
                    let t = target(f, &jn);
                    if let Some(t) = t {
                        self.g += cost(f, cp, t, self.forward);
                    }
                    self.cp = t;
                    self.forward ^= jn.flips();
                }
                None => {
                    self.g += f.segment(cp.0, cp.1, !self.forward);
                    self.cp = next(f, cp, self.forward);
                }
            }
        }
        self.estimate(f, mode);
        walked
    }

    /// 0x41AD40: successor `k` from this node, its decisions recorded.
    fn expand(&self, f: &RouteFile, k: usize) -> Node {
        let mut n = self.clone();
        let (cp, fw, _) = self.succ[k];
        n.cp = Some(cp);
        n.forward = fw;
        if let Some(from) = self.cp {
            n.g += cost(f, from, cp, self.forward);
        }
        for &(_, _, j) in &self.succ[..k] {
            if n.path.len() < PATH_MAX {
                n.path.push(j & 0x1FFF);
            }
        }
        let j = self.succ[k].2;
        if j != 0xFFFF && n.path.len() < PATH_MAX {
            n.path.push(j & 0x1FFF | 0x2000);
        }
        n
    }

    /// 0x41AF90's walk from the start along the recorded decisions,
    /// calling `visit` on each point until it returns true or the goal's
    /// stretch is reached (true then, false when the road ends).
    fn walk(&self, f: &RouteFile, mut visit: impl FnMut(Cp) -> bool) -> Option<Cp> {
        let mut forward = self.start_forward;
        let mut cp = self.start?;
        let mut k = 0;
        loop {
            if holds_across_along(f, cp, self.goal) {
                return None;
            }
            if visit(cp) {
                return Some(cp);
            }
            let c = &f.records[cp.0].points[cp.1];
            let mut take = None;
            for j in f.junctions_at(c) {
                let jn = &f.junctions[j];
                if jn.direction() == forward {
                    if jn.flags & 0xF == 3 {
                        break;
                    }
                    continue;
                }
                if ignored(jn, MODE_PLAYER) {
                    continue;
                }
                if forced(jn, MODE_PLAYER) {
                    take = Some(*jn);
                    break;
                }
                if k < self.path.len() && j as u16 == self.path[k] & 0x1FFF {
                    let taken = self.path[k] & 0x2000 != 0;
                    k += 1;
                    if taken {
                        take = Some(*jn);
                        break;
                    }
                }
            }
            match take {
                Some(jn) => {
                    cp = target(f, &jn)?;
                    forward ^= jn.flips();
                }
                None => cp = next(f, cp, forward)?,
            }
        }
    }

    /// 0x41AF90 (mode 2): the route's point whose stretch the car is on,
    /// 5 m beyond the road's edges or ends; none when it is off the route.
    pub fn locate(&self, f: &RouteFile, car: Vec3) -> Option<Cp> {
        let p = car.truncate();
        self.walk(f, |cp| outside_across(f, cp, p) < 5.0 && outside_along(f, cp, p) < 5.0)
    }

    /// The route's points from the start to the goal's stretch, then the
    /// goal (the clone's polyline of the walk above).
    pub fn points(&self, f: &RouteFile) -> Vec<Vec3> {
        let mut out = Vec::new();
        self.walk(f, |cp| {
            out.push(position(f, cp));
            out.len() > 4096
        });
        if !out.is_empty() {
            out.push(self.goal);
        }
        out
    }
}

/// 0x41AF90's goal test (across and along; no height).
fn holds_across_along(f: &RouteFile, cp: Cp, goal: Vec3) -> bool {
    outside_across(f, cp, goal.truncate()) < 0.0 && outside_along(f, cp, goal.truncate()) < 0.0
}

/// 0x40CBC0: (to − from) · from's direction of travel.
fn cost(f: &RouteFile, from: Cp, to: Cp, forward: bool) -> f32 {
    (position(f, to).truncate() - position(f, from).truncate()).dot(f.direction(from.0, from.1, !forward))
}

/// What the router hands its owner when it finishes or is cleared
/// (vt+0x4C, 0x41EA30: the code into driver +0x3E4, the node into +0x3F0).
#[derive(Clone, Debug, PartialEq)]
pub struct Callback {
    /// 1 found, 3 failed, 0 cleared.
    pub code: u8,
    pub node: Option<Node>,
}

/// The router (0x81CBC0).
#[derive(Clone, Debug, Default)]
pub struct Router {
    /// +0x00: 0 idle, 1 start nodes, 2 search, 3 done.
    pub state: u8,
    /// +0x04: 1 running, 2 found, 3 no path, 4 no start; +0x08 the last
    /// finished result.
    pub result: u8,
    pub last: u8,
    start: Vec3,
    dir: Vec2,
    goal: Vec3,
    /// +0x16D4: the mode.
    mode: u8,
    /// +0x40: the race record the start pass is at.
    record: usize,
    open: Vec<Node>,
    closed: Vec<(Option<Cp>, bool, f32)>,
    /// +0x16CC: the start nodes made.
    starts: usize,
    /// +0x17A0 / +0x16E0: the node that reached the goal.
    found: Option<Node>,
    /// +0x17A4: an owner waits for the result.
    owner: bool,
    /// The work done in this frame's call (the clone's budget, not stored
    /// by the original).
    work: u32,
}

impl Router {
    /// 0x40C440: start a search from `start` facing `dir` (car +0x60,
    /// +0x90) to `goal` when idle (state 0).
    pub fn start(&mut self, start: Vec3, dir: Vec2, goal: Vec3, mode: u8) -> bool {
        if self.state != 0 {
            return false;
        }
        *self = Router { start, dir, goal, mode, record: 0, state: 1, result: 1, last: self.last, owner: true, ..Router::default() };
        true
    }

    /// 0x4131F0: drop the search; the owner hears code 0.
    pub fn clear(&mut self) -> Option<Callback> {
        let cb = self.owner.then_some(Callback { code: 0, node: None });
        *self = Router::default();
        cb
    }

    /// 0x4268A0, once a frame: the start pass and the search until
    /// `budget` units of work ([`FRAME_WORK`]) are done; the owner's
    /// callback when it finishes. Like the original's time test, the
    /// budget is checked before each step (a race record of the start
    /// pass, a node of the search), so the step that crosses it completes.
    /// A unit is one visit of an inner loop, which is what the time goes
    /// to: a record, a control point the start pass tests, a point an
    /// extension walks, a node taken, a successor made, an open or closed
    /// entry compared, a comparison of the open list's sort.
    pub fn update(&mut self, f: &RouteFile, budget: u32) -> Option<Callback> {
        self.run(f, &mut |work| {
            #[cfg(test)]
            tests::step_hook();
            work >= budget
        })
    }

    /// [`Router::update`] with `over` asked before each step with the work
    /// done in this call so far: true stops the call's work.
    fn run(&mut self, f: &RouteFile, over: &mut dyn FnMut(u32) -> bool) -> Option<Callback> {
        self.work = 0;
        if self.state == 1 {
            self.start_nodes(f, over);
        }
        if self.state == 2 {
            self.search(f, over);
        }
        if self.state != 3 {
            return None;
        }
        let node = match self.found.take() {
            Some(n) => {
                self.result = 2;
                n
            }
            None => {
                self.result = if self.starts > 0 { 3 } else { 4 };
                Node::new(f, None, false, self.goal, 2)
            }
        };
        self.last = self.result;
        if !self.owner {
            return None;
        }
        let code = match self.result {
            2 => 1,
            3 | 4 => 3,
            _ => 0,
        };
        self.state = 0;
        self.record = usize::MAX;
        self.open.clear();
        self.closed.clear();
        self.starts = 0;
        self.mode = 0;
        Some(Callback { code, node: Some(node) })
    }

    /// 0x422B90: two start nodes (each way) on the first point of each
    /// race record (in id order, 0x88D21C) whose box holds the start,
    /// within 5 m of its stretch and 4 m of its height, stepping
    /// max(1, ⌊distance × 0.02⌋) points.
    fn start_nodes(&mut self, f: &RouteFile, over: &mut dyn FnMut(u32) -> bool) {
        let mut races: Vec<usize> = f.records.iter().enumerate().filter(|(_, r)| r.is_race()).map(|(i, _)| i).collect();
        races.sort_by_key(|&i| f.records[i].id);
        while self.record < races.len() {
            if over(self.work) {
                break;
            }
            self.work += 1;
            let r = races[self.record];
            let rec = &f.records[r];
            let b = rec.bounds;
            if b[0] <= self.start.x && self.start.x <= b[2] && b[1] <= self.start.y && self.start.y <= b[3] {
                let mut k = 0;
                while k < rec.points.len() {
                    self.work += 1;
                    let c = &rec.points[k];
                    let dot = c.tangent.dot(self.dir);
                    if dot.abs() > 0.0 && usable(f, (r, k), self.mode) {
                        let d = self.start.truncate() - c.position.truncate();
                        let along = d.dot(c.tangent);
                        let across = d.x * c.tangent.y - d.y * c.tangent.x;
                        if (-5.0..=c.length + 5.0).contains(&along) && (-c.half_width[0] - 5.0..=c.half_width[1] + 5.0).contains(&across) && (c.position.z - self.start.z).abs() < 4.0 {
                            for fw in [dot > 0.0, dot <= 0.0] {
                                let mut n = Node::new(f, Some((r, k)), fw, self.goal, self.mode);
                                self.work += n.extend(f, self.mode);
                                self.open.push(n);
                            }
                            break;
                        }
                    }
                    k += ((c.position.truncate() - self.start.truncate()).length() * 0.02).max(1.0) as usize;
                }
            }
            self.record += 1;
        }
        if self.record >= races.len() {
            self.work += sort(&mut self.open);
            self.state = 2;
            self.starts = self.open.len();
        }
    }

    /// 0x422F90: the A*, sliced across frames by the budget (the
    /// original: by time).
    fn search(&mut self, f: &RouteFile, over: &mut dyn FnMut(u32) -> bool) {
        loop {
            if self.open.is_empty() || self.found.is_some() || over(self.work) {
                if self.found.is_some() || self.open.is_empty() {
                    self.open.clear();
                    self.closed.clear();
                    self.state = 3;
                }
                return;
            }
            self.work += 1;
            let node = self.open.pop().unwrap();
            if node.reached {
                self.found = Some(node);
                continue;
            }
            for k in 0..node.succ.len() {
                let mut n = node.expand(f, k);
                self.work += 1 + n.extend(f, self.mode);
                let mut dup = false;
                let mut in_open = None;
                for (i, o) in self.open.iter().enumerate() {
                    self.work += 1;
                    if o.cp == n.cp && o.forward == n.forward {
                        in_open = Some(i);
                        dup = n.g >= o.g;
                        if dup {
                            break;
                        }
                    }
                }
                let mut in_closed = None;
                if !dup {
                    for (i, c) in self.closed.iter().enumerate() {
                        self.work += 1;
                        if c.0 == n.cp && c.1 == n.forward {
                            in_closed = Some(i);
                            dup = n.g >= c.2;
                            if dup {
                                break;
                            }
                        }
                    }
                }
                if dup {
                    continue;
                }
                if let Some(i) = in_closed {
                    self.closed.remove(i);
                }
                match in_open {
                    Some(i) => self.open[i] = n,
                    None if self.open.len() < OPEN_MAX => self.open.push(n),
                    None => {}
                }
                self.work += sort(&mut self.open);
            }
            if self.closed.len() < CLOSED_MAX {
                self.closed.push((node.cp, node.forward, node.g));
            }
        }
    }
}

/// 0x40C4D0: bubble the open list into falling g + h (the last is taken).
/// The comparisons made.
fn sort(open: &mut [Node]) -> u32 {
    let n = open.len();
    let mut swapped = true;
    let mut pass = 0;
    let mut work = 0;
    while pass < n && swapped {
        swapped = false;
        for i in 1..n {
            work += 1;
            if open[i - 1].g + open[i - 1].h < open[i].g + open[i].h {
                open.swap(i - 1, i);
                swapped = true;
            }
        }
        pass += 1;
    }
    work
}

/// The player's route follower (driver +0x3E4 state, +0x3F0 node; the
/// part of 0x42A360 the GPS reads).
#[derive(Clone, Debug, Default)]
pub struct Follower {
    /// 0 none, 1 following, 3 the search failed.
    pub state: u8,
    pub node: Option<Node>,
}

impl Follower {
    /// 0x41EA30.
    pub fn hear(&mut self, cb: Callback) {
        self.state = cb.code;
        if let Some(n) = cb.node {
            self.node = Some(n);
        }
    }

    /// 0x42A360 for the player: within 30 m across and 4 m in height of
    /// the goal (+0x400) the route is dropped and the router cleared; in
    /// state 3 (the last search failed) the route point (+0x3E8) is
    /// zeroed, and off the route (0x41AF90 finds no point) likewise; with
    /// no route point a new search from the car to the goal (0x4188C0,
    /// which starts only an idle router). So a failed search ("Unable to
    /// contact satellite": no start node near the car) is tried again
    /// every frame the router is idle, until the car is on a road.
    pub fn update(&mut self, router: &mut Router, f: &RouteFile, car: Vec3, forward: Vec2) {
        let Some(node) = self.node.as_ref() else { return };
        let goal = node.goal;
        if self.state != 0 && (car.truncate() - goal.truncate()).length() < 30.0 && (car.z - goal.z).abs() < 4.0 {
            self.state = 0;
            if let Some(cb) = router.clear() {
                self.hear(cb);
            }
        }
        if self.state == 0 {
            return;
        }
        let on_route = self.state != 3 && self.node.as_ref().and_then(|n| n.locate(f, car)).is_some();
        if on_route {
            self.state = 1;
        } else {
            router.start(car, forward, goal, MODE_PLAYER);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::ai_routes::{ControlPoint, RouteRecord};

    fn point(x: f32, y: f32, junctions: [u16; 2]) -> ControlPoint {
        ControlPoint { position: Vec3::new(x, y, 0.0), coord: 0.0, flags: 1 << 27, length: 10.0, half_width: [5.0, 5.0], tangent: Vec2::X, junctions, bank: 0.0, grade: 0.0, lane_position: [0; 10], lane_speed: [0; 10] }
    }

    /// A straight road of 20 points along +x, 10 m apart.
    fn road() -> RouteFile {
        let points: Vec<_> = (0..20).map(|i| point(i as f32 * 10.0, 0.0, [0xFFFF; 2])).collect();
        let rec = RouteRecord { id: 1, name: String::new(), kind: 0, coord_range: (0.0, 0.0), bounds: [-10.0, -10.0, 200.0, 10.0], flags: 1 << 6, fade: [false; 2], lanes: [false; 10], points };
        let mut f = RouteFile::default();
        f.by_id.insert(1, 0);
        f.records.push(rec);
        f
    }

    thread_local! {
        /// A pause in every step of the search (a loaded or preempted machine).
        static STEP_DELAY: std::cell::Cell<std::time::Duration> = const { std::cell::Cell::new(std::time::Duration::ZERO) };
    }

    pub(super) fn step_hook() {
        let d = STEP_DELAY.get();
        if !d.is_zero() {
            std::thread::sleep(d);
        }
    }

    /// `n` race records of a straight road each, side by side 30 m apart:
    /// the start pass takes a step for each.
    fn roads(n: u16) -> RouteFile {
        let mut f = RouteFile::default();
        for k in 0..n {
            let y = k as f32 * 30.0;
            let points: Vec<_> = (0..20).map(|i| point(i as f32 * 10.0, y, [0xFFFF; 2])).collect();
            let rec = RouteRecord { id: k + 1, name: String::new(), kind: 0, coord_range: (0.0, 0.0), bounds: [-10.0, -10.0, 200.0, y + 10.0], flags: 1 << 6, fade: [false; 2], lanes: [false; 10], points };
            f.by_id.insert(k + 1, k as usize);
            f.records.push(rec);
        }
        f
    }

    /// The tick (1-based) a search from the last road's start to its far
    /// end finishes on, with `budget` and `delay` in each step.
    fn finish_tick(f: &RouteFile, budget: u32, delay: std::time::Duration) -> u32 {
        let y = (f.records.len() - 1) as f32 * 30.0;
        let mut r = Router::default();
        r.start(Vec3::new(5.0, y, 0.0), Vec2::X, Vec3::new(155.0, y, 0.0), MODE_PLAYER);
        STEP_DELAY.set(delay);
        let mut tick = 0;
        let cb = loop {
            tick += 1;
            if let Some(cb) = r.update(f, budget) {
                break cb;
            }
            assert!(tick < 10_000, "no result");
        };
        STEP_DELAY.set(std::time::Duration::ZERO);
        assert_eq!(cb.code, 1);
        tick
    }

    #[test]
    fn the_finishing_tick_does_not_depend_on_the_machine() {
        let f = roads(40);
        // The frame's budget, and one small enough to spread the start
        // pass over several ticks.
        for budget in [FRAME_WORK, 10] {
            let fast = finish_tick(&f, budget, std::time::Duration::ZERO);
            let slow = finish_tick(&f, budget, std::time::Duration::from_micros(200));
            assert_eq!(fast, slow, "budget {budget}: a slow machine finishes the search on another tick");
            assert!(budget == FRAME_WORK || fast > 1, "budget {budget}: the search took one tick");
        }
    }

    fn run(r: &mut Router, f: &RouteFile) -> Callback {
        for _ in 0..1000 {
            if let Some(cb) = r.update(f, u32::MAX) {
                return cb;
            }
        }
        panic!("no result");
    }

    #[test]
    fn a_straight_road_is_found() {
        let f = road();
        let mut r = Router::default();
        assert!(r.start(Vec3::new(5.0, 0.0, 0.0), Vec2::X, Vec3::new(155.0, 1.0, 0.0), MODE_PLAYER));
        let cb = run(&mut r, &f);
        assert_eq!((cb.code, r.result, r.last), (1, 2, 2));
        let n = cb.node.unwrap();
        assert!(n.reached && n.start_forward && n.cp == Some((0, 15)));
        assert_eq!(n.g, 150.0);
        let pts = n.points(&f);
        assert_eq!(pts.len(), 16);
        assert!(n.locate(&f, Vec3::new(52.0, 3.0, 0.0)).is_some());
        assert!(n.locate(&f, Vec3::new(52.0, 30.0, 0.0)).is_none());
    }

    #[test]
    fn off_road_has_no_start() {
        let f = road();
        let mut r = Router::default();
        r.start(Vec3::new(5.0, 50.0, 0.0), Vec2::X, Vec3::new(155.0, 0.0, 0.0), MODE_PLAYER);
        let cb = run(&mut r, &f);
        assert_eq!((cb.code, r.result), (3, 4));
        // The goal off every road: start nodes but no path.
        r.start(Vec3::new(5.0, 0.0, 0.0), Vec2::X, Vec3::new(155.0, 80.0, 0.0), MODE_PLAYER);
        let cb = run(&mut r, &f);
        assert_eq!((cb.code, r.result), (3, 3));
    }

    #[test]
    fn the_follower_searches_again_off_the_route() {
        let f = road();
        let mut r = Router::default();
        r.start(Vec3::new(5.0, 0.0, 0.0), Vec2::X, Vec3::new(155.0, 0.0, 0.0), MODE_PLAYER);
        let mut fo = Follower::default();
        fo.hear(run(&mut r, &f));
        assert_eq!(fo.state, 1);
        fo.update(&mut r, &f, Vec3::new(60.0, 0.0, 0.0), Vec2::X);
        assert_eq!(r.state, 0);
        fo.update(&mut r, &f, Vec3::new(60.0, 40.0, 0.0), Vec2::X);
        assert_eq!((r.state, r.result), (1, 1));
        r.clear();
        fo.update(&mut r, &f, Vec3::new(150.0, 0.0, 0.0), Vec2::X);
        assert_eq!(fo.state, 0);
    }

    #[test]
    fn a_failed_search_is_tried_again() {
        let f = road();
        let mut r = Router::default();
        // Off every road: no start node, "Unable to contact satellite".
        r.start(Vec3::new(5.0, 50.0, 0.0), Vec2::X, Vec3::new(155.0, 0.0, 0.0), MODE_PLAYER);
        let mut fo = Follower::default();
        fo.hear(run(&mut r, &f));
        assert_eq!((fo.state, r.last), (3, 4));
        // Still off the road: searched again, failed again.
        fo.update(&mut r, &f, Vec3::new(5.0, 50.0, 0.0), Vec2::X);
        assert_eq!(r.state, 1);
        fo.hear(run(&mut r, &f));
        assert_eq!(fo.state, 3);
        // On the road: found.
        fo.update(&mut r, &f, Vec3::new(5.0, 1.0, 0.0), Vec2::X);
        fo.hear(run(&mut r, &f));
        assert_eq!((fo.state, r.last), (1, 2));
    }
}

#[cfg(test)]
mod data_tests {
    use super::*;

    /// A search on free roam's routes (NFSU2_ROOT, GPS_FROM / GPS_TO as
    /// "x,y,z,fx,fy" / "x,y,z"). A manual probe: `--ignored`, and it does
    /// nothing without those variables.
    #[test]
    #[ignore = "manual probe: needs GPS_FROM / GPS_TO"]
    fn free_roam_search() {
        let root = crate::testkit::game_root();
        let f = RouteFile::parse(&std::fs::read(root.join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin")).unwrap());
        let v = |k: &str| -> Vec<f32> { std::env::var(k).unwrap().split(',').map(|x| x.parse().unwrap()).collect() };
        let (a, b) = (v("GPS_FROM"), v("GPS_TO"));
        let mut r = Router::default();
        r.start(Vec3::new(a[0], a[1], a[2]), Vec2::new(a[3], a[4]), Vec3::new(b[0], b[1], b[2]), MODE_PLAYER);
        let mut frames = 0;
        let cb = loop {
            frames += 1;
            if let Some(cb) = r.update(&f, FRAME_WORK) {
                break cb;
            }
        };
        eprintln!("frames {frames} code {} result {} starts {}", cb.code, r.result, r.starts);
        if let Some(n) = cb.node {
            eprintln!("g {} path {} points {}", n.g, n.path.len(), n.points(&f).len());
        }
    }

    /// The search's cost over the GPS destinations (every route trigger's
    /// GPS point) from spread start points, on the network of career stage
    /// `GPS_SURVEY_STAGE` (default 1). Per search: the start pass's records
    /// and the search's nodes (each a step), the wall time of each step (the
    /// least over five runs: an unloaded machine), the frames the
    /// original's 0.6 ms slices take on those times, and the frames one run
    /// sliced by the wall clock takes now. A TSV to `GPS_SURVEY_OUT`, and
    /// what each 0.6 ms slice the budget ended bought to `<out>.slices`.
    /// A manual timing probe: `--ignored`.
    #[test]
    #[ignore = "manual timing probe: set GPS_SURVEY_OUT"]
    fn search_cost_survey() {
        use std::fmt::Write as _;
        use std::time::{Duration, Instant};
        let out_path = std::env::var("GPS_SURVEY_OUT").expect("GPS_SURVEY_OUT");
        let stage: u32 = std::env::var("GPS_SURVEY_STAGE").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
        let root = crate::testkit::game_root();
        let dir = root.join("TRACKS/ROUTESL4RA");
        let mut f = RouteFile::parse(&std::fs::read(dir.join("RoutesFreeRoam.bin")).unwrap());
        f.finalize_free_roam_paths(&std::fs::read(dir.join("PathsFreeRoam.bin")).unwrap(), 4000, stage);
        let races = f.records.iter().filter(|r| r.is_race()).count();
        let mut goals: Vec<Vec3> = crate::frontend::career::triggers::RouteTriggers::load(&root).iter().map(|t| t.gps).filter(|g| *g != Vec3::ZERO).collect();
        goals.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
        goals.dedup_by(|a, b| (*a - *b).length() < 1.0);
        // The road under a point: the nearest control point's height and direction.
        let on_road = |at: Vec3| {
            let c = f.records.iter().filter(|r| r.is_race()).flat_map(|r| &r.points).min_by(|a, b| (a.position - at).length().total_cmp(&(b.position - at).length())).unwrap();
            (c.position, c.tangent)
        };
        let mut starts = vec![("intro", (Vec3::new(1695.3, -883.6, 24.1), Vec2::new(0.982, -0.190)))];
        if let Some((p, _)) = crate::frontend::career::triggers::free_roam_start(&root) {
            starts.push(("free roam start", on_road(p)));
        }
        for k in 0..6 {
            starts.push(("trigger", on_road(goals[k * (goals.len() - 1) / 5])));
        }
        // No start node near: the "unable to contact satellite" search.
        starts.push(("off road", (Vec3::new(0.0, 0.0, -500.0), Vec2::X)));
        let mut cases: Vec<(String, Vec3, Vec2, Vec3)> = Vec::new();
        for (name, (p, d)) in &starts {
            for g in &goals {
                cases.push((name.to_string(), *p, *d, *g));
            }
            // A goal on no road: the search runs until the open list is empty.
            cases.push((format!("{name} / no road"), *p, *d, Vec3::new(p.x + 300.0, p.y + 300.0, p.z + 200.0)));
        }
        let mut slices = String::from("start\trecords\tnodes\twork\n");
        let mut tsv = String::from("start\tfrom\tto\tcode\trecords\tnodes\tunloaded_us\tstart_us\tframes_unloaded\tframes_now\twork\tsteps_work\tsteps_us\n");
        // The start pass as one entry, then each node of the search.
        let steps = |v: &[f64], fmt: &str| -> String {
            let n = races.min(v.len());
            let one = |x: f64| if fmt == "{:.0}" { format!("{x:.0}") } else { format!("{x:.2}") };
            std::iter::once(v[..n].iter().sum::<f64>()).chain(v[n..].iter().copied()).map(one).collect::<Vec<_>>().join(",")
        };
        for (name, p, d, g) in &cases {
            // Each step's time, the least over five runs.
            let mut step_us: Vec<f64> = Vec::new();
            let mut work: Vec<u32> = Vec::new();
            let mut code = 0;
            for _ in 0..5 {
                let mut r = Router::default();
                r.start(*p, *d, *g, MODE_PLAYER);
                let (mut at, mut w) = (Vec::new(), Vec::new());
                let t0 = Instant::now();
                let cb = r.run(&f, &mut |done| {
                    at.push(t0.elapsed().as_secs_f64() * 1e6);
                    w.push(done);
                    false
                });
                at.push(t0.elapsed().as_secs_f64() * 1e6);
                w.push(r.work);
                work = w.windows(2).map(|w| w[1] - w[0]).collect();
                code = cb.map_or(0, |c| c.code);
                let d: Vec<f64> = at.windows(2).map(|w| w[1] - w[0]).collect();
                if step_us.is_empty() {
                    step_us = d;
                } else {
                    step_us.iter_mut().zip(&d).for_each(|(m, x)| *m = m.min(*x));
                }
            }
            let records = races.min(step_us.len());
            let nodes = step_us.len() - records;
            // The original's slices on those times: the test before each step
            // ends the frame once 0.6 ms have passed since its start.
            let (mut frames, mut t, mut bought) = (1, 0.0, (0, 0, 0));
            for (i, us) in step_us.iter().enumerate() {
                if t > 600.0 {
                    writeln!(slices, "{name}\t{}\t{}\t{}", bought.0, bought.1, bought.2).unwrap();
                    (frames, t, bought) = (frames + 1, 0.0, (0, 0, 0));
                }
                t += us;
                if i < records { bought.0 += 1 } else { bought.1 += 1 }
                bought.2 += work[i];
            }
            let mut r = Router::default();
            r.start(*p, *d, *g, MODE_PLAYER);
            let mut now = 0;
            loop {
                now += 1;
                let t0 = Instant::now();
                if r.run(&f, &mut |_| t0.elapsed() > Duration::from_micros(600)).is_some() {
                    break;
                }
            }
            let total: f64 = step_us.iter().sum();
            let start: f64 = step_us[..records].iter().sum();
            writeln!(tsv, "{name}\t{:.0},{:.0}\t{:.0},{:.0}\t{code}\t{records}\t{nodes}\t{total:.1}\t{start:.1}\t{frames}\t{now}\t{}\t{}\t{}", p.x, p.y, g.x, g.y, work.iter().sum::<u32>(), steps(&work.iter().map(|&w| f64::from(w)).collect::<Vec<_>>(), "{:.0}"), steps(&step_us, "{:.2}")).unwrap();
        }
        std::fs::write(&out_path, tsv).unwrap();
        std::fs::write(format!("{out_path}.slices"), slices).unwrap();
        eprintln!("{} searches, {} goals, {} starts", cases.len(), goals.len(), starts.len());
    }

    /// The intro's route (career stage 1, the car at the first garage
    /// facing south, the goal the first race): over the network after the
    /// free-roam post-load the original starts backward (north) and the
    /// path is 3436.3 m (observed in the original's memory).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn free_roam_intro_route() {
        let root = crate::testkit::game_root().join("TRACKS/ROUTESL4RA");
        let routes = std::fs::read(root.join("RoutesFreeRoam.bin")).expect("RoutesFreeRoam.bin");
        let paths = std::fs::read(root.join("PathsFreeRoam.bin")).expect("PathsFreeRoam.bin");
        let mut f = RouteFile::parse(&routes);
        f.finalize_free_roam_paths(&paths, 4000, 1);
        let at = Vec2::new(1695.25, -883.64);
        // The car stands on the road: the height of the nearest point.
        let z = f.records.iter().flat_map(|r| &r.points).min_by(|a, b| (a.position.truncate() - at).length().total_cmp(&(b.position.truncate() - at).length())).unwrap().position.z;
        let mut r = Router::default();
        r.start(at.extend(z), Vec2::new(0.98, -0.19).normalize(), Vec3::new(-723.0, -913.0, 17.0), MODE_PLAYER);
        let cb = loop {
            if let Some(cb) = r.update(&f, u32::MAX) {
                break cb;
            }
        };
        let n = cb.node.unwrap();
        assert_eq!(cb.code, 1);
        assert!(!n.start_forward, "starts forward, g {}", n.g);
        assert!((n.g - 3436.3).abs() < 0.5, "g {}", n.g);
    }
}

//! Knock-over props ("smackables", formats in `formats::smackable`; see
//! docs/formats.md, "Knock-over props").
//!
//! Verified: a placed prop starts as an unhit body (`VirginSmokeable`,
//! vtable 0x7A23A8, made by 0x5AD900) with the instance's world box and
//! half its diagonal as radius, and no shape. A body whose box meets it in
//! the sweep (0x5B23C0) wakes it (0x5B3200 → 0x5B13A0): the prop becomes a
//! rigid body (0x5B3250, rigid body 0x5A7F40 on the info's description at
//! +0x20, 0x59B950) at rest where it stood, with the info's mass, inertia
//! and centre of mass and a convex shape — the `ZCS_` volume whose name
//! hash is the info's key (0x5A18D0), else a box of the drawn solid's
//! half size about the centre of mass (0x598770) — and it no longer comes
//! back while the track is loaded. It does not move until hit: while
//! +0x150 (the break impulse) is set vtable +0x5C / +0x60 give zero inverse
//! mass and inertia, so a car bounces off it as off the track; an impulse
//! at least that large (0x5B2080) starts it moving (0x5ADFA0), it takes
//! −(t / |J|)·J and the car's impulse is scaled by t / |J|. Without a break
//! impulse any push starts it. Then it is a two-body contact with the
//! prop's own 1/m and I⁻¹ (no 1.85 t floor, no 0.4), e and μ the larger of
//! the car's and the info's.
//!
//! A moving prop's frame (0x5B1DE0) runs ⌊dt·60⌋ + 1 parts (0x7FC858) of
//! the integrator 0x5AA490 — explicit Euler over the rigid body's state
//! (position, orientation rows, linear and angular momentum, the rest
//! damping) with the car's rigid-body terms (0x59C1F0) — each followed by
//! re-orthonormalising (0x5BB0E0) and the post-step (0x5A8E80): the tidy-up
//! (0x59C600), then a settle test (0x59E440) that lets gravity act, damps
//! the momenta by 0.95, or stops the body and its gravity. A prop that has
//! been still on something for a while stops moving ("No activity",
//! 0x5A8DA0) until hit again. Its contacts with the track are the car's
//! (0x5FFA60: polygon gather, distance query, time of impact, push-out,
//! impulse 0x5AF0F0) with the prop's shape and the larger of the prop's and
//! the polygon's 0.001 for e and μ.
//!
//! The props' contacts with the track, the cars and each other go into the
//! same time-sorted list as the cars' own (`drive::step_world`, 0x5FFA60):
//! the earliest resolved first, both bodies moved back to its moment and the
//! rest of their spans simulated again, at most 20 a frame and 2 per pair.
//! A moving prop sweeps like a car (0x5B23C0): it wakes the unhit props its
//! box meets and pairs with them ([`Props::sweep_moving`]). Every
//! contact takes the pair's last measured closest points (0x594010 reads
//! them as the time of impact's search or the push-out left them).
//!
//! Inferred: where the props' pairs sit among the cars' in the gather
//! (after them; it decides only exact ties). Each record's first push
//! posts the prop's impact sound ([`prop_sound`], `audio::collision`).
//! Not ported: the attached effects and the props' contact particle
//! effects (their surface index is not carried; the particle system is
//! `crate::particles`, docs/formats.md).
mod body;
mod contact;
mod grid;

use self::contact::*;
use body::*;
use grid::*;

use crate::fxhash::FxHashMap as HashMap;

use glam::{Mat3, Mat4, Vec2, Vec3};

use crate::car::Car;
use crate::drive::{ANGULAR_DAMPING, CarState, Pose, REST_DAMPING, Racer, sample, time_of_impact};
use crate::formats::collision::CollisionVolume;
use crate::formats::smackable::{ObjectInfo, ObjectInstance, SceneryGroup};
use crate::ground::GroundQuery;
use crate::narrow;
use crate::world::World;

/// The car's side of the pair (0x5A0CA0 / 0x5A0BF0 against a body; the
/// larger of the two sides wins, 0x5941D0).
const CAR_RESTITUTION: f32 = 0.1;
const CAR_FRICTION: f32 = 0.3;
/// A track polygon's side (vtable 0x7A22B0 +0x70 / +0x74, 0x5AA060): e and
/// μ 0.001, so a prop's own values win.
const TRACK_MATERIAL: f32 = 0.001;
/// A moving prop's step (0x7FC858, 1/60 s): ⌊dt / it⌋ + 1 parts.
const PROP_STEP: f32 = 1.0 / 60.0;
/// The rigid body's gravity (+0x160: 9.8 from 0x7FBEF0 in 0x5A7F40, set
/// again to 9.8 or 0 by the settle test 0x5A8E80).
const GRAVITY: f32 = 9.8;
/// A contact record lives this long after its last push (0x7FBEE0).
const RECORD_LIFE: f32 = 0.3;
/// The settle test (0x59E440) and the stop (0x5A8DA0).
const SETTLE_SPEED: f32 = 2.0;
const REST_SPEED: f32 = 0.8;
const STILL_SPEED: f32 = 0.1;
const SINCE_CAR: f32 = 0.1;
/// A woken prop is lifted this much clear of the ground (0x7844F4), and
/// its draw offset eases out at this rate, m/s (0x7FC85C).
const LIFT_CLEARANCE: f32 = 0.05;
const DRAW_OFFSET_RATE: f32 = 0.2;
const RECORD_QUIET: f32 = 0.025;
const SETTLE_DAMPING: f32 = 0.95;

/// One knock-over prop: what it is and where it stands.
#[derive(Clone, Debug)]
pub struct Prop {
    /// The world placement it moves.
    pub placement: usize,
    /// The `0x34107` table entry it stands in for (instance +4).
    pub scenery: u16,
    pub info: ObjectInfo,
    /// The collision shape about the centre of mass, prop axes: the `ZCS_`
    /// volume less the centre of mass, or the box (0x5B1550).
    pub shape: Vec<Vec3>,
    /// The shape's offset (+0x40 with +0x3E set): the info's centre of
    /// mass for a `ZCS_` volume (0x5B1690 looks the volume up with the
    /// rigid body's +0x1F0; 0x5B5060 writes the body's vtable +0x1C, the
    /// same +0x1F0, before each detection), none for the box (a type-3
    /// shape). The distance query places the shape's fresh supports
    /// R·this off ([`crate::narrow::Raw`]).
    pub offset: Vec3,
    /// The `ZCS_` volume's vertices as stored (the offset not removed),
    /// prop axes: a fresh support's point is one of these through the
    /// body's transform (0x5AFBB0 / 0x5AFF2A); empty for the box.
    pub raw_shape: Vec<Vec3>,
    /// The volume's vertex links (0x5AFBB0's climb); empty for the box.
    pub links: crate::formats::collision::HullLinks,
    /// The body's reach (0x5AE530): the farthest shape point, and the
    /// shape's height above and below the centre of mass plus 0.05 m.
    pub reach: f32,
    pub above: f32,
    pub below: f32,
    pub rest: Mat4,
    /// The unhit body's world box (0x5A0690) and radius (0x5AD900).
    pub world_min: Vec3,
    pub world_max: Vec3,
    pub radius: f32,
}

impl Prop {
    /// Pair instances with infos (by key, as 0x5AD900) and with the scenery
    /// placement they stand in for (`placement_of`: the instance's +4
    /// through the `0x34107` table, 0x5D6440, to a placement and the
    /// table's kind). Kind 1 becomes an unhit body (0x5B0C40); another kind
    /// only when the entry is listed in a scenery group (`0x34108`) that is
    /// on and is not the one named `SMOKEABLE` (0x5B0C40 at 0x5B0D7A skips
    /// that name: it lists every prop, 601 entries in L4RA covering all
    /// 514 placements, all kind 1).
    pub fn pair(
        world: &World,
        infos: &[ObjectInfo],
        instances: &[ObjectInstance],
        volumes: &[CollisionVolume],
        placement_of: impl Fn(u16) -> Option<(usize, u8)>,
        groups: &[SceneryGroup],
    ) -> Vec<Prop> {
        let listed =
            |scenery: u16| groups.iter().any(|g| g.on && g.name != "SMOKEABLE" && g.members.contains(&scenery));
        let mut out = Vec::new();
        for inst in instances {
            let Some(info) = infos.iter().find(|i| i.key == inst.info_key) else { continue };
            let Some((placement, kind)) = placement_of(inst.scenery) else { continue };
            if kind != 1 && !listed(inst.scenery) {
                continue;
            }
            let p = &world.placements[placement];
            let solid = &world.solids[p.solid];
            let com = info.centre_of_mass;
            let volume = volumes.iter().find(|v| v.hash == info.key && !v.vertices.is_empty());
            let offset = if volume.is_some() { com } else { Vec3::ZERO };
            let links = volume.map(|v| v.links.clone()).unwrap_or_default();
            let shape: Vec<Vec3> = match volume {
                Some(v) => v.vertices.iter().map(|q| *q - com).collect(),
                None => {
                    let half = (Vec3::from(solid.bbox_max) - Vec3::from(solid.bbox_min)) * 0.5;
                    (0..8)
                        .map(|i| {
                            half * Vec3::new(
                                if i & 1 == 0 { -1.0 } else { 1.0 },
                                if i & 2 == 0 { -1.0 } else { 1.0 },
                                if i & 4 == 0 { -1.0 } else { 1.0 },
                            )
                        })
                        .collect()
                }
            };
            let reach = shape.iter().fold(0.0f32, |r, q| r.max(q.length()));
            let top = shape.iter().fold(0.0f32, |t, q| t.max(q.z));
            let bottom = shape.iter().fold(0.0f32, |b, q| b.min(q.z));
            out.push(Prop {
                placement,
                scenery: inst.scenery,
                info: info.clone(),
                shape,
                offset,
                raw_shape: volume.map(|v| v.vertices.clone()).unwrap_or_default(),
                links,
                reach,
                above: top + 0.05,
                below: 0.05 - bottom,
                rest: p.transform,
                world_min: inst.bbox_min,
                world_max: inst.bbox_max,
                radius: (inst.bbox_max - inst.bbox_min).length() * 0.5,
            });
        }
        out
    }

    fn mass(&self) -> f32 {
        self.info.mass.max(1e-4)
    }

    /// World inverse inertia R·I⁻¹·Rᵀ: the rigid body's own update
    /// (0x59BC70, 0x5A0D90) turns its body-space one (+0x1B0, the yaw term
    /// × info +0x70, which is 1 for every L4RA kind; +0x158 is set).
    fn inverse_inertia(&self, rot: Mat3) -> Mat3 {
        rot * self.body_inverse_inertia() * rot.transpose()
    }

    /// The body-space inverse inertia as the contact impulse 0x5AF0F0 takes
    /// it: vtable +0x60 (0x5B1C50) returns body +0x1B0 (+0x170 without
    /// +0x158) and 0x43C9C0 applies it to the world-space r × n unturned,
    /// as for a car (0x593A50, `contact::Body::inverse_inertia`). As
    /// observed under Wine: the player's Skyline through a ZCS_FenceH on
    /// S4_H_DRAG_15 took |J| 5.13 (ΔP (−3.121, −1.645, 0.021) × 1.85 / m), the
    /// turned tensor gives 3.36, the unturned one 5.1.
    fn body_inverse_inertia(&self) -> Mat3 {
        if self.info.inertia.determinant().abs() > 1e-12 {
            self.info.inertia.inverse()
        } else {
            Mat3::from_diagonal(Vec3::splat(1e6))
        }
    }
}

/// Every prop's state in a running world.
#[derive(Clone, Default)]
pub struct Props {
    bodies: Vec<Option<Body>>,
    /// This frame: the props that moved, and how many contacts each
    /// prop–polygon and car–prop pair has had (at most 2, 0x802084).
    moved: Vec<usize>,
    track_done: HashMap<(usize, (usize, usize)), usize>,
    car_done: HashMap<(usize, usize), usize>,
    /// The prop–prop pairs (lower index first) made by a moving
    /// prop's sweep, with their distance cache and place in the pair list,
    /// and this frame's contacts per pair.
    pairs: std::collections::BTreeMap<(usize, usize), PropPair>,
    pair_done: HashMap<(usize, usize), usize>,
    /// The car name in each slot at the last contact step. A body's car
    /// pair caches index the hull of the car they were made against, so
    /// a slot whose car changed (a new race, or the cars list rebuilt)
    /// forgets them (see [`Props::sync_cars`]).
    car_names: Vec<String>,
    /// A grid over the unhit props' rest boxes, built at the first
    /// sweep and again whenever `world.smackables` is a different list
    /// (`World::reset_to_loaded` replaces it), shared between clones.
    grid: Option<std::sync::Arc<PropGrid>>,
    /// The woken props (`bodies[k]` is `Some`), ascending; bodies
    /// are only ever woken, never put back to sleep.
    woken: Vec<usize>,
    /// The sweep's candidate list, kept for its allocation.
    scratch: Vec<usize>,
}

/// A prop–prop pair (0x5A0350): the distance query's cache and when it
/// was made.
#[derive(Clone, Debug)]
struct PropPair {
    cache: Option<narrow::PairCache>,
    made: u64,
}

/// A prop's contact found for the frame's sorted list (0x5FFA60): `t`
/// seconds before the end of the span, whether the two overlapped for the
/// whole of it.
#[derive(Clone, Debug)]
pub enum PropEvent {
    Track {
        prop: usize,
        t: f32,
        stuck: bool,
        key: (usize, usize),
        corners: Vec<Vec3>,
        normal: Vec3,
        turn: bool,
        made: u64,
    },
    Car {
        prop: usize,
        car: usize,
        t: f32,
        stuck: bool,
        made: u64,
    },
    /// Two props, `a` < `b`.
    Pair {
        a: usize,
        b: usize,
        t: f32,
        stuck: bool,
        made: u64,
    },
}

impl PropEvent {
    pub fn time(&self) -> f32 {
        match self {
            PropEvent::Track { t, .. } | PropEvent::Car { t, .. } | PropEvent::Pair { t, .. } => *t,
        }
    }

    /// The pair's place in the pair list (`drive::next_pair`).
    pub fn made(&self) -> u64 {
        match self {
            PropEvent::Track { made, .. } | PropEvent::Car { made, .. } | PropEvent::Pair { made, .. } => *made,
        }
    }

    /// Whether this contact goes before `other` in the gather: further
    /// back in time, or of equal times the older pair (0x43BEE0).
    fn before(&self, other: &PropEvent) -> bool {
        self.time() > other.time() || (self.time() == other.time() && self.made() < other.made())
    }
}

/// Whether two woken props' boxes (centre of mass ± reach, x and y) meet.
fn boxes_meet(pa: &Prop, a: &Body, pb: &Prop, b: &Body) -> bool {
    let d = (a.centre.truncate() - b.centre.truncate()).abs();
    d.x <= pa.reach + pb.reach && d.y <= pa.reach + pb.reach
}

/// NFSU2_PROP_LOG's step count (a check aid): the collision steps run so
/// far, one a world tick.
static LOG_STEP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(super) fn log_step() -> u64 {
    LOG_STEP.load(std::sync::atomic::Ordering::Relaxed)
}

/// A body's speed above which a stuck pair of a car and a prop is pushed
/// apart along the car's velocity rather than the line between them
/// (0x5B4300: 55.555557 m/s, 200 km/h).
const PUSH_ALONG_VELOCITY: f32 = 55.555_557;

impl Props {
    pub fn new(world: &World) -> Props {
        Props { bodies: vec![None; world.smackables.len()], ..Props::default() }
    }

    /// Whether these props belong to `world` (an empty set, as
    /// `Props::default()`, takes no part).
    fn live(&self, world: &World) -> bool {
        !self.bodies.is_empty() && self.bodies.len() == world.smackables.len()
    }

    /// How many props have been knocked into motion at some point.
    pub fn knocked(&self) -> usize {
        self.bodies.iter().flatten().filter(|b| b.knocked).count()
    }

    /// Every prop's drawn pose: a woken one's body, else its rest pose,
    /// with whether it has been woken (for a replay restoring an earlier
    /// state of the props).
    pub fn poses(&self, world: &World) -> Vec<(usize, Mat4, bool)> {
        world
            .smackables
            .iter()
            .zip(&self.bodies)
            .map(|(prop, b)| match b {
                Some(b) => (prop.placement, transform(prop, b), true),
                None => (prop.placement, prop.rest, false),
            })
            .collect()
    }

    /// The frame's start, with the cars' (0x6012C0 runs every body's frame
    /// before the collision step): records aged, and each woken prop's
    /// frame (0x5B1DE0) — the start pose and span saved, a moving one
    /// simulated.
    pub(crate) fn begin(&mut self, world: &World, dt: f32) {
        if crate::env_value_os!("NFSU2_PROP_LOG").is_some() {
            LOG_STEP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        self.moved.clear();
        self.track_done.clear();
        self.car_done.clear();
        self.pair_done.clear();
        if !self.live(world) {
            return;
        }
        debug_assert!(
            self.woken.iter().copied().eq(self.bodies.iter().enumerate().filter(|b| b.1.is_some()).map(|b| b.0))
        );
        for &k in &self.woken {
            let prop = &world.smackables[k];
            let Some(b) = self.bodies[k].as_mut() else { continue };
            age_records(b, dt);
            if !b.moving {
                b.start = (b.centre, b.rot, b.velocity);
                b.span = dt;
                continue;
            }
            frame(prop, b, dt);
            self.moved.push(k);
        }
        // The pair list's upkeep (0x592E70): a pair stays while the two
        // boxes meet or a record of it lives.
        let bodies = &self.bodies;
        self.pairs.retain(|&(a, b), _| {
            let (Some(ba), Some(bb)) = (bodies[a].as_ref(), bodies[b].as_ref()) else { return false };
            boxes_meet(&world.smackables[a], ba, &world.smackables[b], bb)
                || ba.records.iter().any(|r| r.0 == Partner::Prop(b))
        });
    }

    /// Forgets the car pair caches of every slot whose car changed since
    /// the last step. A car's hull is its `<CAR>_CV` volume, so the
    /// remembered vertex indices are only meaningful for the car they were
    /// made against; the props outlive a race, and the next race's cars
    /// can put a different model in the same slot.
    fn sync_cars(&mut self, cars: &[Racer]) {
        let n = cars.len().max(self.car_names.len());
        let changed: Vec<usize> = (0..n)
            .filter(|&i| self.car_names.get(i).map(String::as_str) != cars.get(i).map(|c| c.car.name.as_str()))
            .collect();
        if changed.is_empty() {
            return;
        }
        for b in self.bodies.iter_mut().flatten() {
            for &i in &changed {
                b.pairs.remove(&Partner::Car(i));
            }
        }
        self.car_names = cars.iter().map(|c| c.car.name.clone()).collect();
    }

    /// The sweep (0x5B23C0 → 0x5B3200): an unhit prop whose x/y box meets
    /// a car's wakes (0x5B13A0), lifted clear of the ground; the pair is
    /// made with the new body, the one with the smaller box edge first.
    pub(crate) fn sweep(&mut self, world: &World, cars: &[Racer]) {
        if !self.live(world) {
            return;
        }
        if self.grid.as_ref().is_none_or(|g| g.of != PropGrid::key(&world.smackables)) {
            self.grid = Some(std::sync::Arc::new(PropGrid::new(&world.smackables)));
        }
        let grid = std::sync::Arc::clone(self.grid.as_ref().unwrap());
        if crate::env_value_os!("NFSU2_PROP_LOG").is_some()
            && let Some(c) = cars.first()
        {
            let p = c.state.position;
            let (gear, rpm) = c.state.drivetrain.as_ref().map_or((0, 0.0), |t| (t.gear, t.rpm()));
            eprintln!(
                "PROPTICK @{} car {:.3} {:.3} {:.3} speed {:.3} gear {gear} rpm {rpm:.0}",
                log_step(),
                p.x,
                p.y,
                p.z,
                c.state.velocity.length()
            );
        }
        for (i, c) in cars.iter().enumerate() {
            // An unloaded car's body takes no partner (0x593990).
            if !c.state.streamed {
                continue;
            }
            let r = car_radius(c.car);
            // The box about the body's position, the centre of mass (0x5A9DF0).
            let at = CarState::com_at(c.car, &c.state.pose()).truncate();
            // The unhit props the grid puts near the car and every
            // woken one (its box is its body's), in ascending order as the
            // scan of all; a box the grid cannot take scans them all.
            let mut cand = std::mem::take(&mut self.scratch);
            cand.clear();
            if grid.query(at - r, at + r, &mut cand) {
                let bodies = &self.bodies;
                cand.retain(|&k| bodies[k].is_none());
                cand.extend_from_slice(&self.woken);
                cand.sort_unstable();
                cand.dedup();
            } else {
                cand.clear();
                cand.extend(0..world.smackables.len());
            }
            for &k in &cand {
                let prop = &world.smackables[k];
                let (lo, hi) = match &self.bodies[k] {
                    Some(b) => {
                        (b.centre.truncate() - Vec2::splat(prop.reach), b.centre.truncate() + Vec2::splat(prop.reach))
                    }
                    None => (prop.world_min.truncate(), prop.world_max.truncate()),
                };
                if at.x + r < lo.x || hi.x < at.x - r || at.y + r < lo.y || hi.y < at.y - r {
                    continue;
                }
                if self.bodies[k].is_none()
                    && let Err(at) = self.woken.binary_search(&k)
                {
                    if crate::env_value_os!("NFSU2_PROP_LOG").is_some() {
                        let p = prop.rest.w_axis;
                        let (lo, hi) = prop.shape.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), q| (a.min(*q), b.max(*q)));
                        eprintln!(
                            "PROPWAKE {k} {} at {:.2} {:.2} {:.2} shape {} points {lo:.3} .. {hi:.3}",
                            prop.info.name,
                            p.x,
                            p.y,
                            p.z,
                            prop.shape.len()
                        );
                    }
                    self.woken.insert(at, k);
                }
                let b = self.bodies[k].get_or_insert_with(|| {
                    let mut b = wake(prop);
                    lift(world, prop, &mut b);
                    b
                });
                b.car_first.entry(i).or_insert(at.x + r <= hi.x);
            }
            self.scratch = cand;
        }
        self.sweep_moving(world);
    }

    /// The moving props' sweep (0x5FFB10 / 0x5FFC78 → 0x5B23C0 for a
    /// body with +0x12 and +0x19 set, which 0x5ADFA0 sets as the prop
    /// starts moving): every prop whose box meets the moving one's is
    /// paired with it, an unhit one woken first (0x5B3200, lifted clear of
    /// the ground as a car wakes it). The box of a woken prop is its centre
    /// of mass ± its reach.
    pub(crate) fn sweep_moving(&mut self, world: &World) {
        if !self.live(world) {
            return;
        }
        let Some(grid) = self.grid.clone() else { return };
        let movers: Vec<usize> =
            self.woken.iter().copied().filter(|&k| self.bodies[k].as_ref().is_some_and(|b| b.moving)).collect();
        for k in movers {
            let prop = &world.smackables[k];
            let Some(b) = self.bodies[k].as_ref() else { continue };
            let (lo, hi) = (b.centre.truncate() - Vec2::splat(prop.reach), b.centre.truncate() + Vec2::splat(prop.reach));
            let mut cand = std::mem::take(&mut self.scratch);
            cand.clear();
            if grid.query(lo, hi, &mut cand) {
                let bodies = &self.bodies;
                cand.retain(|&j| bodies[j].is_none());
                cand.extend_from_slice(&self.woken);
                cand.sort_unstable();
                cand.dedup();
            } else {
                cand.clear();
                cand.extend(0..world.smackables.len());
            }
            for &j in &cand {
                if j == k {
                    continue;
                }
                let other = &world.smackables[j];
                let (olo, ohi) = match &self.bodies[j] {
                    Some(o) => (o.centre.truncate() - Vec2::splat(other.reach), o.centre.truncate() + Vec2::splat(other.reach)),
                    None => (other.world_min.truncate(), other.world_max.truncate()),
                };
                if hi.x < olo.x || ohi.x < lo.x || hi.y < olo.y || ohi.y < lo.y {
                    continue;
                }
                if self.bodies[j].is_none() {
                    if let Err(at) = self.woken.binary_search(&j) {
                        if crate::env_value_os!("NFSU2_PROP_LOG").is_some() {
                            eprintln!("PROPWAKE {j} {} by prop {k}", other.info.name);
                        }
                        self.woken.insert(at, j);
                    }
                    let mut nb = wake(other);
                    lift(world, other, &mut nb);
                    self.bodies[j] = Some(nb);
                }
                let key = (k.min(j), k.max(j));
                self.pairs.entry(key).or_insert_with(|| PropPair { cache: None, made: crate::drive::next_pair() });
            }
            self.scratch = cand;
        }
    }

    /// The prop pairs' earliest contact (the one furthest back in time,
    /// the first of equals): each moving prop against the track polygons
    /// under it (0x5B5060 with the prop's shape), then each car against
    /// each woken prop whose box it meets (0x5B5060 with both shapes,
    /// 0x5B4BF0: eight halvings over the shorter span, each body blended
    /// over its own).
    pub(crate) fn next(&mut self, world: &World, cars: &[Racer], starts: &[Pose], spans: &[f32]) -> Option<PropEvent> {
        if !self.live(world) {
            return None;
        }
        self.sync_cars(cars);
        let mut best: Option<PropEvent> = None;
        let offer = |e: PropEvent, best: &mut Option<PropEvent>| {
            if best.as_ref().is_none_or(|b| e.before(b)) {
                *best = Some(e);
            }
        };
        for &k in &self.woken {
            let prop = &world.smackables[k];
            let Some(b) = self.bodies[k].as_mut() else { continue };
            if !b.moving {
                continue;
            }
            if let Some(e) = track_event(world, prop, k, b, &self.track_done) {
                offer(e, &mut best);
            }
        }
        // The prop–prop pairs (0x5B5060 makes no contact of two
        // bodies that are both at rest, vtable +0x6C).
        for (&(ka, kb), pair) in self.pairs.iter_mut() {
            if self.pair_done.get(&(ka, kb)).copied().unwrap_or(0) >= crate::contact::MAX_PER_POLYGON {
                continue;
            }
            let (Some(a), Some(b)) = (self.bodies[ka].as_ref(), self.bodies[kb].as_ref()) else { continue };
            if !a.moving && !b.moving {
                continue;
            }
            let (pa, pb) = (&world.smackables[ka], &world.smackables[kb]);
            if !boxes_meet(pa, a, pb, b) {
                continue;
            }
            let d = narrow::distance_at(
                &shape_at(pa, a.centre, a.rot),
                &shape_at(pb, b.centre, b.rot),
                a.centre,
                b.centre,
                [raw_at(pa, a.rot), raw_at(pb, b.rot)],
                &mut pair.cache,
            )
            .0;
            if d > crate::contact::TOUCH {
                continue;
            }
            let (t, stuck) = if d > crate::contact::OVERLAP {
                (0.0, false)
            } else {
                fn blend<'p>(p: &'p Prop, body: &Body) -> impl Fn(f32) -> (Vec<Vec3>, Vec3, narrow::Side<'p>) + use<'p> {
                    let (start, own, centre, rot) = (body.start, body.span, body.centre, body.rot);
                    move |s: f32| {
                        let f = if own > 1e-6 { (s / own).min(1.0) } else { 0.0 };
                        let at = centre.lerp(start.0, f);
                        let turned = orthonormal(lerp_mat(rot, start.1, f));
                        (shape_at(p, at, turned), at, raw_at(p, turned))
                    }
                }
                let (fa, fb) = (blend(pa, a), blend(pb, b));
                pair_time(&fa, &fb, a.span.min(b.span), &mut pair.cache)
            };
            offer(PropEvent::Pair { a: ka, b: kb, t, stuck, made: pair.made }, &mut best);
        }
        for (i, c) in cars.iter().enumerate() {
            // An unloaded car's body takes no partner (0x593990).
            if !c.state.streamed {
                continue;
            }
            let r = car_radius(c.car);
            // The box about the body's position, the centre of mass (0x5A9DF0).
            let at = CarState::com_at(c.car, &c.state.pose()).truncate();
            // Built at the first prop whose box the car meets.
            let mut hulls: Option<(Vec<Vec3>, Vec<Vec3>, Vec3, Vec3, Vec3, Vec3)> = None;
            for &k in &self.woken {
                let prop = &world.smackables[k];
                let Some(b) = self.bodies[k].as_mut() else { continue };
                if self.car_done.get(&(i, k)).copied().unwrap_or(0) >= crate::contact::MAX_PER_POLYGON {
                    continue;
                }
                let (lo, hi) =
                    (b.centre.truncate() - Vec2::splat(prop.reach), b.centre.truncate() + Vec2::splat(prop.reach));
                if at.x + r < lo.x || hi.x < at.x - r || at.y + r < lo.y || hi.y < at.y - r {
                    continue;
                }
                let (end_hull, start_hull, end_com, start_com, end_raw, start_raw) = &*hulls.get_or_insert_with(|| {
                    (
                        CarState::hull_at(c.car, &c.state.pose()),
                        CarState::hull_at(c.car, &starts[i]),
                        CarState::com_at(c.car, &c.state.pose()),
                        CarState::com_at(c.car, &starts[i]),
                        CarState::raw_at(c.car, &c.state.pose()),
                        CarState::raw_at(c.car, &starts[i]),
                    )
                });
                let pair = b.pairs.entry(Partner::Car(i)).or_default();
                // Each query with the bodies' positions (the car's centre of
                // mass, the prop's): its answer stays in the pair (0x5B3D30).
                let d = narrow::distance_at(
                    end_hull,
                    &shape_at(prop, b.centre, b.rot),
                    *end_com,
                    b.centre,
                    [narrow::Side::hull(*end_raw, &c.car.hull_links), raw_at(prop, b.rot)],
                    pair,
                )
                .0;
                if d > crate::contact::TOUCH {
                    continue;
                }
                let (t, stuck) = if d > crate::contact::OVERLAP {
                    (0.0, false)
                } else {
                    let (start, own, centre, brot) = (b.start, b.span, b.centre, b.rot);
                    let car_span = spans[i];
                    let prop_at = |s: f32| {
                        let f = if own > 1e-6 { (s / own).min(1.0) } else { 0.0 };
                        let at = centre.lerp(start.0, f);
                        let turned = orthonormal(lerp_mat(brot, start.1, f));
                        (shape_at(prop, at, turned), at, raw_at(prop, turned))
                    };
                    let car_at = |s: f32| -> (Vec<Vec3>, Vec3, narrow::Side) {
                        let f = if car_span > 1e-6 { (s / car_span).min(1.0) } else { 0.0 };
                        (
                            end_hull.iter().zip(start_hull).map(|(e, s)| *e + (*s - *e) * f).collect(),
                            *end_com + (*start_com - *end_com) * f,
                            narrow::Side::hull(*end_raw + (*start_raw - *end_raw) * f, &c.car.hull_links),
                        )
                    };
                    pair_time(&car_at, &prop_at, car_span.min(own), b.pairs.entry(Partner::Car(i)).or_default())
                };
                let made = *b.made.entry(Partner::Car(i)).or_insert_with(crate::drive::next_pair);
                offer(PropEvent::Car { prop: k, car: i, t, stuck, made }, &mut best);
            }
        }
        best
    }

    /// Resolve the head of the list (0x5B2620): the bodies back to its
    /// moment, pushed apart if they overlapped throughout (0x5B4300), one
    /// impulse (0x5AF0F0). A prop simulates the rest of its span again
    /// here; returns the car that was moved, for the caller to do the same.
    pub(crate) fn resolve(
        &mut self,
        world: &World,
        cars: &mut [Racer],
        starts: &[Pose],
        spans: &[f32],
        e: PropEvent,
        sounds: &mut crate::drive::ContactSounds,
    ) -> Option<usize> {
        match e {
            PropEvent::Track { prop: k, t, stuck, key, corners, normal, turn, .. } => {
                *self.track_done.entry((k, key)).or_default() += 1;
                let prop = &world.smackables[k];
                let b = self.bodies[k].as_mut()?;
                resolve_track(prop, b, t, stuck, key, &corners, normal, turn, sounds);
                self.moved.push(k);
                // The body sweeps again (0x5FFC78).
                self.sweep_moving(world);
                None
            }
            PropEvent::Car { prop: k, car: i, t, stuck, .. } => {
                *self.car_done.entry((i, k)).or_default() += 1;
                let prop = &world.smackables[k];
                let b = self.bodies[k].as_mut()?;
                let c = &mut cars[i];
                let local = glam::Vec3::from(c.car.car_type.handling.centre_of_mass);
                c.state.back_to(&starts[i], t, spans[i], local);
                if b.moving && t > 0.0 {
                    back_to(prop, b, t);
                }
                if car_contact(prop, b, k, i, c.car, c.state, stuck, sounds) {
                    self.moved.push(k);
                }
                // The prop's rest of the span from here (the car's is the
                // caller's).
                if b.moving {
                    b.start = (b.centre, b.rot, b.velocity);
                    b.span = t;
                    simulate(prop, b, t);
                    self.moved.push(k);
                }
                Some(i)
            }
            PropEvent::Pair { a: ka, b: kb, t, stuck, .. } => {
                *self.pair_done.entry((ka, kb)).or_default() += 1;
                let (pa, pb) = (&world.smackables[ka], &world.smackables[kb]);
                let (lo, hi) = self.bodies.split_at_mut(kb);
                let (Some(a), Some(b)) = (lo[ka].as_mut(), hi[0].as_mut()) else { return None };
                for (p, body) in [(pa, &mut *a), (pb, &mut *b)] {
                    if body.moving && t > 0.0 {
                        back_to(p, body, t);
                    }
                }
                let pair = self.pairs.get_mut(&(ka, kb))?;
                prop_pair(pa, a, ka, pb, b, kb, stuck, &mut pair.cache, sounds);
                // Both bodies' rest of the span from here (vtable +0x2C).
                for (k, p, body) in [(ka, pa, a), (kb, pb, b)] {
                    if body.moving {
                        body.start = (body.centre, body.rot, body.velocity);
                        body.span = t;
                        simulate(p, body, t);
                        self.moved.push(k);
                    }
                }
                // The bodies sweep again (0x5FFC78).
                self.sweep_moving(world);
                None
            }
        }
    }

    /// The frame's end: the props that moved and their new transforms.
    pub(crate) fn finish(&mut self, world: &World) -> Vec<(usize, Mat4)> {
        if !self.live(world) {
            return Vec::new();
        }
        let mut moved = std::mem::take(&mut self.moved);
        moved.sort_unstable();
        moved.dedup();
        // NFSU2_PROP_LOG=1 (a check aid, docs/notes/parity/prop-hits.md): each prop
        // that moved this frame, its centre of mass, velocity, spin and
        // whether it still moves.
        if crate::env_value_os!("NFSU2_PROP_LOG").is_some() {
            for &k in &moved {
                if let Some(b) = self.bodies[k].as_ref() {
                    let (c, v, w) = (b.centre, b.velocity, b.spin);
                    eprintln!(
                        "PROP {k} {} @{} pos {:.3} {:.3} {:.3} vel {:.3} {:.3} {:.3} spin {:.3} {:.3} {:.3} up {:.3} moving {}",
                        world.smackables[k].info.name, log_step(), c.x, c.y, c.z, v.x, v.y, v.z, w.x, w.y, w.z, b.rot.z_axis.z, b.moving
                    );
                }
            }
        }
        moved
            .into_iter()
            .filter_map(|k| {
                self.bodies[k].as_ref().map(|b| (world.smackables[k].placement, transform(&world.smackables[k], b)))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::smackable::MATERIALS;

    /// A grid query holds every box a brute-force box test meets
    /// (boxes touching at an edge included), for boxes across cell
    /// borders, outside the grid, huge and not finite.
    #[test]
    fn prop_grid_query_is_a_superset() {
        let mut seed = 0x1234_5678u32;
        let mut rnd = |lo: f32, hi: f32| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            lo + (seed >> 8) as f32 / 16_777_216.0 * (hi - lo)
        };
        let mut boxes = Vec::new();
        for _ in 0..600 {
            let a = Vec2::new(rnd(-900.0, 900.0), rnd(-700.0, 700.0));
            boxes.push((a, a + Vec2::new(rnd(0.0, 40.0), rnd(0.0, 40.0))));
        }
        // Edges exactly on cell borders.
        boxes.push((Vec2::new(-900.0, -700.0), Vec2::new(-868.0, -668.0)));
        let grid = PropGrid::of_boxes((0, 0), &boxes);
        assert!(grid.cells.is_some());
        let meets =
            |q: (Vec2, Vec2), b: (Vec2, Vec2)| !(q.1.x < b.0.x || b.1.x < q.0.x || q.1.y < b.0.y || b.1.y < q.0.y);
        let mut queries: Vec<(Vec2, Vec2)> = Vec::new();
        for _ in 0..3000 {
            let at = Vec2::new(rnd(-1100.0, 1100.0), rnd(-900.0, 900.0));
            let r = rnd(0.5, 12.0);
            queries.push((at - r, at + r));
        }
        // Queries touching boxes exactly at their corners.
        for &(a, b) in boxes.iter().take(50) {
            queries.push((b, b + 3.0));
            queries.push((a - 3.0, a));
        }
        queries.push((Vec2::splat(-1e30), Vec2::splat(1e30)));
        for q in queries {
            let mut out = Vec::new();
            assert!(grid.query(q.0, q.1, &mut out));
            for (k, &b) in boxes.iter().enumerate() {
                if meets(q, b) {
                    assert!(out.contains(&k), "box {k} {b:?} missed by query {q:?}");
                }
            }
        }
        let mut out = Vec::new();
        assert!(!grid.query(Vec2::splat(f32::NAN), Vec2::ZERO, &mut out), "a NaN box scans all");
        assert!(PropGrid::of_boxes((0, 0), &[(Vec2::ZERO, Vec2::splat(f32::INFINITY))]).cells.is_none());
        assert!(
            PropGrid::of_boxes((0, 0), &[(Vec2::ZERO, Vec2::ONE), (Vec2::splat(1e9), Vec2::splat(1e9))])
                .cells
                .is_none()
        );
    }

    fn pylon() -> Prop {
        let info = ObjectInfo {
            name: "ZCS_PylonA".into(),
            key: 0x5087_DB01,
            mass: 0.1,
            inertia: Mat3::from_diagonal(Vec3::new(0.0076, 0.0076, 0.0045)),
            break_impulse: 0.0,
            restitution: 0.005,
            friction: 0.3,
            centre_of_mass: Vec3::new(0.0, 0.0, 0.37),
            material: MATERIALS.iter().position(|m| *m == "TRAFFIC_CONE").unwrap(),
        };
        let half = Vec3::new(0.3, 0.3, 0.4);
        let shape: Vec<Vec3> = (0..8)
            .map(|i| {
                half * Vec3::new(
                    if i & 1 == 0 { -1.0 } else { 1.0 },
                    if i & 2 == 0 { -1.0 } else { 1.0 },
                    if i & 4 == 0 { -1.0 } else { 1.0 },
                )
            })
            .collect();
        Prop {
            placement: 0,
            scenery: 0,
            info,
            shape,
            offset: Vec3::ZERO,
            raw_shape: Vec::new(),
            links: Default::default(),
            reach: half.length(),
            above: 0.45,
            below: 0.45,
            rest: Mat4::from_translation(Vec3::new(10.0, 0.0, 5.0)),
            world_min: Vec3::new(9.7, -0.3, 5.0),
            world_max: Vec3::new(10.3, 0.3, 5.8),
            radius: 0.6,
        }
    }

    /// docs/notes/parity/drag-stage4-parity.md §7.1, observed under Wine: the finished-career save's
    /// Skyline (1.271 t) through a ZCS_FenceH on S4_H_DRAG_15 at frame 268
    /// changed its momentum by (−3.121, −1.645, 0.021), an impulse of
    /// (4.543, 2.394, 0.03) at the 1.85 t scale, 5.11 along the contact
    /// normal. 0x5AF0F0 applies the prop's body-space inverse inertia
    /// (vtable +0x60 = 0x5B1C50 → +0x1B0) to the world r × n unturned; the
    /// turned tensor gave 3.22.
    #[test]
    fn car_push_takes_the_props_unturned_inverse_inertia() {
        let info = ObjectInfo {
            name: "ZCS_FenceH".into(),
            key: 0,
            mass: 0.2,
            inertia: Mat3::from_diagonal(Vec3::new(0.31186244, 1.3186874, 1.0087781)),
            break_impulse: 0.0,
            restitution: 0.005,
            friction: 0.3,
            centre_of_mass: Vec3::new(-0.00048828125, 0.121032715, 2.1285436),
            material: 0,
        };
        let prop = Prop { info, ..pylon() };
        let rot =
            Mat3::from_cols(Vec3::new(0.39309406, -0.91949826, 0.0), Vec3::new(0.91949826, 0.39309406, 0.0), Vec3::Z);
        // The clone's contact at that moment (car side k from prop_contact_terms).
        let (arm, normal, vn, car_k) = (
            Vec3::new(0.37124634, -0.8937988, -1.6209854),
            Vec3::new(0.9273978, 0.37407643, 0.00042619384),
            -43.69442f32,
            0.57054394f32,
        );
        let j = |inv: Mat3| {
            -(1.0 + CAR_RESTITUTION) * vn
                / (car_k + 1.0 / prop.mass() + normal.dot((inv * arm.cross(normal)).cross(arm)))
        };
        let native = 5.109;
        assert!((j(prop.body_inverse_inertia()) - native).abs() < 0.05, "{}", j(prop.body_inverse_inertia()));
        assert!((j(prop.inverse_inertia(rot)) - native).abs() > 1.5);
    }

    #[test]
    fn woken_prop_rests_until_moving_then_falls() {
        let prop = pylon();
        let mut b = wake(&prop);
        assert!((b.centre - Vec3::new(10.0, 0.0, 5.37)).length() < 1e-5);
        // Woken, the tidy-up has queued gravity, but it does not move.
        assert!((b.force.z + 0.1 * 9.8).abs() < 1e-5);
        frame(&prop, &mut b, 1.0 / 60.0);
        assert_eq!(b.centre, Vec3::new(10.0, 0.0, 5.37));
        start_moving(&mut b);
        for _ in 0..60 {
            frame(&prop, &mut b, 1.0 / 60.0);
        }
        // Explicit Euler with momentum: ½·g·t² less the first step, and the
        // rest damping (0 at first) only grows while it falls fast.
        let fall = 5.37 - b.centre.z;
        assert!(fall > 4.0 && fall < 4.95, "{fall}");
        assert_eq!(b.settle, 0);
        assert!((transform(&prop, &b).w_axis.z - (b.centre.z - 0.37)).abs() < 1e-5);
    }

    /// The draw offset eases out at 0.2 m/s while the prop moves unbroken,
    /// and is drawn below the body.
    #[test]
    fn draw_offset_eases_out() {
        let prop = pylon();
        let mut b = wake(&prop);
        b.draw_offset = 0.1;
        frame(&prop, &mut b, 0.25);
        assert_eq!(b.draw_offset, 0.1, "not while at rest");
        start_moving(&mut b);
        frame(&prop, &mut b, 0.25);
        assert!((b.draw_offset - 0.05).abs() < 1e-6);
        assert!((transform(&prop, &b).w_axis.z - (b.centre.z - 0.37 - 0.05)).abs() < 1e-5);
        frame(&prop, &mut b, 0.5);
        assert_eq!(b.draw_offset, 0.0);
    }

    #[test]
    fn slow_body_on_something_settles_and_stops() {
        let prop = pylon();
        let mut b = wake(&prop);
        start_moving(&mut b);
        b.records.push((Partner::Track((0, 0)), 0.0));
        b.momentum = Vec3::new(0.05, 0.0, 0.0);
        post_step(&prop, &mut b);
        // Slow, touching, no car for ages: stopped and weightless.
        assert_eq!(b.settle, 2);
        assert_eq!(b.momentum, Vec3::ZERO);
        assert_eq!(b.force, Vec3::ZERO);
        b.records[0].1 = 0.05;
        simulate(&prop, &mut b, 1.0 / 60.0);
        assert!(!b.moving, "no activity stops it");
    }

    /// Two pylons side by side along x, B `gap` m clear of A, B `dy` m
    /// off to the side; A moving at `v` along x.
    fn pylon_pair(gap: f32, dy: f32, v: f32) -> (Prop, Body, Prop, Body) {
        let pa = pylon();
        let pb = Prop { rest: Mat4::from_translation(Vec3::new(10.6 + gap, dy, 5.0)), ..pylon() };
        let (mut a, b) = (wake(&pa), wake(&pb));
        start_moving(&mut a);
        a.momentum = Vec3::X * v * pa.mass();
        tidy(&pa, &mut a);
        (pa, a, pb, b)
    }

    /// A moving prop meets a resting one (0x5B2620 / 0x5AF0F0 with
    /// two prop bodies): the push runs along A → B, starts B (0x5B2080 →
    /// 0x5ADFA0), takes momentum from A and gives it to B, both with their
    /// own 1/m; the larger e and μ of the two.
    #[test]
    fn a_moving_prop_knocks_a_resting_one() {
        // 1 cm apart: the query's closest points are a clean face pair.
        let (pa, mut a, pb, mut b) = pylon_pair(0.01, 0.0, 5.0);
        let mut cache = None;
        let d = narrow::distance_at(&shape_at(&pa, a.centre, a.rot), &shape_at(&pb, b.centre, b.rot), a.centre, b.centre, narrow::EXACT, &mut cache).0;
        assert!((d - 0.01).abs() < 1e-4, "{d}");
        let before = a.momentum + b.momentum;
        assert!(prop_pair(&pa, &mut a, 0, &pb, &mut b, 1, false, &mut cache, &mut Default::default()));
        assert!(b.moving && b.knocked);
        assert!(b.velocity.x > 0.1 && a.velocity.x < 5.0 - 0.1, "{} {}", a.velocity, b.velocity);
        assert!((a.momentum + b.momentum - before).length() < 1e-4, "momentum kept");
        assert!(a.records.iter().any(|r| r.0 == Partner::Prop(1)) && b.records.iter().any(|r| r.0 == Partner::Prop(0)));
    }

    /// 0x594010 / 0x5B2620 (traced; read under Wine at 200 km/h,
    /// docs/notes/parity/prop-hits.md): the contact's point and normal are the pair's
    /// last measured closest points, not measured again at the contact's
    /// moment — here measured with B off to the side, then B moved in line
    /// without a query: the push still runs along the old diagonal.
    #[test]
    fn the_contact_takes_the_pairs_last_answer() {
        let (pa, mut a, pb, mut b) = pylon_pair(0.01, 0.62, 5.0);
        let mut cache = None;
        let (_, x, y) =
            narrow::distance_at(&shape_at(&pa, a.centre, a.rot), &shape_at(&pb, b.centre, b.rot), a.centre, b.centre, narrow::EXACT, &mut cache);
        let old = (y - x).normalize();
        b.centre.y = 0.0;
        assert!(prop_pair(&pa, &mut a, 0, &pb, &mut b, 1, false, &mut cache, &mut Default::default()));
        let pushed = b.momentum.normalize();
        // The push is the old normal plus friction against the sliding.
        assert!(old.y.abs() > 0.3, "the old points lie on the diagonal: {old}");
        assert!(pushed.dot(old) > 0.9, "{pushed} vs {old}");
    }

    #[test]
    fn orthonormal_follows_the_game() {
        let m = Mat3::from_cols(Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.1, 1.0, 0.0), Vec3::new(0.0, 0.0, 5.0));
        let o = orthonormal(m);
        assert!((o.x_axis - Vec3::X).length() < 1e-6);
        assert!((o.z_axis - Vec3::Z).length() < 1e-6);
        assert!((o.y_axis - Vec3::Y).length() < 1e-6);
    }
}

crate::persist!(Prop { placement, scenery, info, shape, offset, raw_shape, links, reach, above, below, rest, world_min, world_max, radius });

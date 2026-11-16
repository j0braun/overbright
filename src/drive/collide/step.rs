use crate::drive::*;
use crate::fxhash::FxHashMap as HashMap;

/// A resolved contact of a car on the BeltMover (0x5F4970 → the racer
/// controller's 0x402450): it leaves mode 4, so the physics mover runs it
/// for the rest of the frame, and the race step is told
/// ([`CarState::belt_hit`]).
/// Whether a car meets the track in the collision step: not a traffic car
/// on the BeltMover (see `step_world`).
pub(in crate::drive) fn takes_track_contacts(traffic: bool, input: &Input) -> bool {
    !(traffic && matches!(input, Input::Belt(_)))
}

pub(in crate::drive) fn off_belt(c: &mut Racer) {
    if let Input::Belt(controls) = c.input {
        c.input = Input::Ai(controls);
        c.state.belt_hit = true;
    }
}

/// The box test of two bodies (0x592970, no slack): position ± radius in x/y.
pub(in crate::drive) fn boxes_meet(a: Vec3, ra: f32, b: Vec3, rb: f32) -> bool {
    a.x - ra <= b.x + rb && b.x - rb < a.x + ra && a.y - ra <= b.y + rb && b.y - rb < a.y + ra
}

/// One frame for several cars: each car's frame (0x6012C0: the mode
/// update 0x5FC060, then the physics mover 0x5B72E0), then the world
/// collision step (0x5FFA60) over all of them — track polygons and car
/// pairs in one list of contacts, the earliest resolved first, at most 20
/// a frame and 2 per pair. Call it with the same cars in the same order
/// every frame; `CarState::step` is this with one car. No knock-over
/// props take part: [`step_world`] is this with them.
pub fn step_cars(cars: &mut [Racer], field: &mut CarField, world: &World, dt: f32) {
    step_cars_posting(cars, field, world, dt, &mut ContactSounds::default());
}

/// [`step_cars`] posting its contact sounds into `sounds` (a
/// session's queues; `step_cars` drops them).
pub fn step_cars_posting(cars: &mut [Racer], field: &mut CarField, world: &World, dt: f32, sounds: &mut ContactSounds) {
    step_world(cars, field, &mut crate::smackable::Props::default(), world, dt, sounds);
}

/// [`step_cars`] with the knock-over props: their frames (0x5B1DE0) run
/// with the cars', and their contacts with the track and the cars go into
/// the same time-sorted list as the cars' own (0x5FFA60). Returns the
/// placements of the props that moved and their new transforms.
pub fn step_world(
    cars: &mut [Racer],
    field: &mut CarField,
    props: &mut crate::smackable::Props,
    world: &World,
    dt: f32,
    sounds: &mut ContactSounds,
) -> Vec<(usize, glam::Mat4)> {
    step_world_with_post_motion(cars, field, props, world, dt, sounds, |_, _| {})
}

/// Body update 5A9F20 -> 6012C0: mover motion, then queued car recovery,
/// before broadphase/contact processing. The saved collision sweep start
/// remains the pre-motion pose, independently of the outer velocity token.
/// The callback runs once per car's initial motion, never on contact restep.
pub fn step_world_with_post_motion(
    cars: &mut [Racer],
    field: &mut CarField,
    props: &mut crate::smackable::Props,
    world: &World,
    dt: f32,
    sounds: &mut ContactSounds,
    mut post_motion: impl FnMut(usize, &mut Racer),
) -> Vec<(usize, glam::Mat4)> {
    let n = cars.len();
    age_contacts(cars, field, dt);
    let mut starts = motion_frames(cars, field, world, dt, &mut post_motion);
    let mut spans = vec![dt; n];
    props.begin(world, dt);

    // Car pairs (0x592E70, 0x5B23C0): kept while their boxes meet or a
    // record lives and both cars collide with cars; made when the boxes
    // meet, A being the one with the smaller box edge (the body list's
    // order).
    let radii: Vec<f32> = cars.iter().map(|c| CarState::radius(c.car)).collect();
    field.pairs.retain(|&(a, b), p| {
        a < n
            && b < n
            && can_pair(cars, a)
            && can_pair(cars, b)
            && (p.record.is_some() || pair_boxes_meet(cars, &radii, a, b))
    });
    let mut track_cache = sweep_pairs(cars, field, world, &radii, &starts, &spans);
    props.sweep(world, cars);

    let mut track_done: Vec<HashMap<(usize, usize), usize>> = vec![HashMap::default(); n];
    let mut pair_done: HashMap<(usize, usize), usize> = HashMap::default();
    for _ in 0..contact::MAX_EVENTS {
        let Some(found) =
            next_event(cars, field, props, world, &starts, &spans, &track_done, &pair_done, &mut track_cache)
        else {
            break;
        };
        resolve_event(
            found,
            cars,
            field,
            props,
            world,
            &radii,
            &mut starts,
            &mut spans,
            &mut track_done,
            &mut pair_done,
            &mut track_cache,
            sounds,
        );
    }

    expire_records(cars, field);
    contact_notes(cars, field);
    near_misses(cars, field, &radii);
    props.finish(world)
}

/// The frame's ageing of the cars' contact records, ghost timers and
/// pair records.
fn age_contacts(cars: &mut [Racer], field: &mut CarField, dt: f32) {
    for c in cars.iter_mut() {
        for r in c.state.records.values_mut() {
            r.age += dt;
            r.life += dt;
        }
        c.state.ghost_timer = (c.state.ghost_timer - dt).max(0.0);
        if let Some((age, _, _)) = &mut c.state.last_hit {
            *age += dt;
        }
        c.state.hardest_impulse = 0.0;
    }
    for p in field.pairs.values_mut() {
        if let Some(r) = &mut p.record {
            r.0 += dt;
            p.life += dt;
        }
    }
}

/// The draft, then each car's own frame (mode update and mover). Returns
/// the poses the collision sweep starts from.
fn motion_frames(
    cars: &mut [Racer],
    field: &CarField,
    world: &World,
    dt: f32,
    post_motion: &mut impl FnMut(usize, &mut Racer),
) -> Vec<Pose> {
    let n = cars.len();
    // The draft (0x609F00 → 0x5EBD90, each car from every other car in
    // prov: inferred drive.step_world_with_post_motion
    // the list 0x890080; **inferred**: the cars stepped together are that
    // list, and taking it each step rather than once a frame).
    let moving: Vec<(Vec3, Vec3)> = cars.iter().map(|c| (c.state.position, c.state.velocity)).collect();
    for (i, c) in cars.iter_mut().enumerate() {
        c.state.draft =
            crate::style::draft(moving[i].0, moving.iter().enumerate().filter(|&(j, _)| j != i).map(|(_, m)| *m));
    }
    let starts: Vec<Pose> = cars.iter().map(|c| c.state.pose()).collect();
    for i in 0..n {
        let contacts = cars[i].state.in_contact() || field.has_record(i);
        let c = &mut cars[i];
        c.state.frame_dt = dt;
        c.state.restep = false;
        c.state.simulate(c.car, world, c.input, dt, contacts);
        post_motion(i, c);
        c.state.restep = true;
        c.state.since_collision += dt;
    }
    starts
}

// Car pairs need both bodies' +0x190 and both loaded (0x593990), and
// neither held by the race start (`CarState::start_hold`).
fn can_pair(cars: &[Racer], i: usize) -> bool {
    cars[i].state.car_collisions && cars[i].state.streamed && !cars[i].state.start_hold
}

/// A car body's box centre: its position, the centre of mass (0x593B50,
/// 0x5A9DF0).
fn box_at(c: &Racer) -> Vec3 {
    CarState::com_at(c.car, &c.state.pose())
}

fn pair_boxes_meet(cars: &[Racer], radii: &[f32], i: usize, j: usize) -> bool {
    boxes_meet(box_at(&cars[i]), radii[i], box_at(&cars[j]), radii[j])
}

fn high_x(cars: &[Racer], radii: &[f32], i: usize) -> f32 {
    box_at(&cars[i]).x + radii[i]
}

/// One body's sweep (0x5B23C0): car `i` gathers its track polygons
/// (0x5AE940) from `starts[i]`, then pairs with the polygons and with the
/// `partners` its box meets, appended in one order by box high x, so its
/// track and car pairs share one order. The polygons stay in `cache`.
#[allow(clippy::too_many_arguments)]
fn sweep_body(
    cars: &mut [Racer],
    field: &mut CarField,
    world: &World,
    radii: &[f32],
    starts: &[Pose],
    spans: &[f32],
    cache: &mut TrackCache,
    i: usize,
    partners: &[usize],
) {
    let polygons = {
        let c = &mut cars[i];
        cache.at = Some(TrackCacheKey { pose: c.state.pose(), start: starts[i], span: spans[i] });
        &*cache.polygons.insert(c.state.contact_polygons(c.car, world, &starts[i]))
    };
    enum New {
        Track((usize, usize)),
        Car(usize),
    }
    let mut new: Vec<(f32, New)> =
        polygons.iter().filter(|p| !cars[i].state.pair_made.contains_key(&p.0)).map(|p| (p.4, New::Track(p.0))).collect();
    for &j in partners {
        if j != i
            && can_pair(cars, i)
            && can_pair(cars, j)
            && pair_boxes_meet(cars, radii, i, j)
            && !field.pairs.contains_key(&(i, j))
            && !field.pairs.contains_key(&(j, i))
        {
            new.push((high_x(cars, radii, j), New::Car(j)));
        }
    }
    new.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, item) in new {
        match item {
            New::Track(key) => {
                cars[i].state.pair_made.insert(key, next_pair());
            }
            New::Car(j) => {
                field.pairs.insert((i, j), CarPair { made: next_pair(), ..CarPair::default() });
            }
        }
    }
}

/// After a resolution only the event's own bodies sweep again, A then B
/// (0x5FFC78 → 0x5B23C0): each body's sweep, track polygons
/// and car pairs merged by high x as in the first pass, against every
/// other car. The other bodies did not move, so their pairs stand.
#[allow(clippy::too_many_arguments)]
fn resweep(
    cars: &mut [Racer],
    field: &mut CarField,
    world: &World,
    radii: &[f32],
    starts: &[Pose],
    spans: &[f32],
    track_cache: &mut [TrackCache],
    moved: &[usize],
) {
    let everyone: Vec<usize> = (0..cars.len()).collect();
    for &i in moved {
        sweep_body(cars, field, world, radii, starts, spans, &mut track_cache[i], i, &everyone);
    }
}

/// The sweep: each car's track polygons and the car pairs its box makes,
/// in one order. Returns the cars' track caches.
fn sweep_pairs(
    cars: &mut [Racer],
    field: &mut CarField,
    world: &World,
    radii: &[f32],
    starts: &[Pose],
    spans: &[f32],
) -> Vec<TrackCache> {
    let n = cars.len();
    // The sweep (0x5FFA60 → 0x5B23C0 per car body in the body list's
    // order, by box high x): each car gathers its track polygons
    // (0x5AE940), then pairs with the polygons and the later cars its box
    // meets, appended in the list's order, so track and car pairs share
    // one order.
    // Each car's polygons and hulls, kept from this sweep through
    // the contact loop until one of its contacts is resolved.
    let mut track_cache: Vec<TrackCache> = (0..n).map(|_| TrackCache::default()).collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| high_x(cars, radii, i).total_cmp(&high_x(cars, radii, j)));
    for (k, &i) in order.iter().enumerate() {
        let later: Vec<usize> = order[k + 1..].to_vec();
        sweep_body(cars, field, world, radii, starts, spans, &mut track_cache[i], i, &later);
    }
    track_cache
}

/// A contact found for the step's event loop.
enum Found {
    Track(usize, TrackEvent),
    Pair((usize, usize), f32, bool),
    Prop(crate::smackable::PropEvent),
}

/// The next contact to resolve: the latest time wins; of equals the pair
/// earlier in the list.
#[allow(clippy::too_many_arguments)]
fn next_event(
    cars: &mut [Racer],
    field: &mut CarField,
    props: &mut crate::smackable::Props,
    world: &World,
    starts: &[Pose],
    spans: &[f32],
    track_done: &[HashMap<(usize, usize), usize>],
    pair_done: &HashMap<(usize, usize), usize>,
    track_cache: &mut [TrackCache],
) -> Option<Found> {
    let n = cars.len();
    // The latest time wins; of equals the pair earlier in the list.
    let mut best: Option<(f32, u64, Found)> = None;
    let better = |best: &Option<(f32, u64, Found)>, t: f32, made: u64| {
        best.as_ref().is_none_or(|b| t > b.0 || (t == b.0 && made < b.1))
    };
    for i in 0..n {
        let c = &mut cars[i];
        // A traffic car is on the BeltMover only while the world under
        // it is not loaded (0x5F46B0 on car +0x5C0, verified): there is
        // prov: inferred drive.step_world_with_post_motion-2
        // no track there to touch (**inferred**: the clone keeps all of
        // the world's collision, so its belt cars cut through walls and
        // barriers the original never has under them; 4–6 of a
        // race's ~11 permanent stops by 0x408DE0 were such contacts).
        if !takes_track_contacts(c.state.traffic, &c.input) || !c.state.streamed {
            continue;
        }
        if let Some(e) =
            c.state.track_event(c.car, world, &starts[i], spans[i], &track_done[i], &mut track_cache[i])
            && better(&best, e.t, e.made)
        {
            best = Some((e.t, e.made, Found::Track(i, e)));
        }
    }
    // In the pair list's order (`made` is unique per pair).
    let mut keys: Vec<(u64, (usize, usize))> = field.pairs.iter().map(|(k, p)| (p.made, *k)).collect();
    keys.sort_unstable_by_key(|k| k.0);
    for (_, key) in keys {
        if pair_done.get(&key).copied().unwrap_or(0) >= contact::MAX_PER_POLYGON {
            continue;
        }
        let (a, b) = key;
        // A pair made before the hold takes no test while it lasts.
        if cars[a].state.start_hold || cars[b].state.start_hold {
            continue;
        }
        let ends = [
            CarState::hull_at(cars[a].car, &cars[a].state.pose()),
            CarState::hull_at(cars[b].car, &cars[b].state.pose()),
        ];
        let begins = [CarState::hull_at(cars[a].car, &starts[a]), CarState::hull_at(cars[b].car, &starts[b])];
        let hull = |k: usize, f: f32| -> Vec<Vec3> {
            let i = usize::from(k == b);
            ends[i].iter().zip(&begins[i]).map(|(e, s)| *e + (*s - *e) * f).collect()
        };
        let coms = [
            (CarState::com_at(cars[a].car, &cars[a].state.pose()), CarState::com_at(cars[a].car, &starts[a])),
            (CarState::com_at(cars[b].car, &cars[b].state.pose()), CarState::com_at(cars[b].car, &starts[b])),
        ];
        let com = |k: usize, f: f32| -> Vec3 {
            let (e, s) = coms[usize::from(k == b)];
            e + (s - e) * f
        };
        let raws = [
            (CarState::raw_at(cars[a].car, &cars[a].state.pose()), CarState::raw_at(cars[a].car, &starts[a])),
            (CarState::raw_at(cars[b].car, &cars[b].state.pose()), CarState::raw_at(cars[b].car, &starts[b])),
        ];
        let (la, lb) = (&cars[a].car.hull_links, &cars[b].car.hull_links);
        let raw = |fa: f32, fb: f32| -> narrow::Raw {
            let ((ea, sa), (eb, sb)) = (raws[0], raws[1]);
            [narrow::Side::hull(ea + (sa - ea) * fa, la), narrow::Side::hull(eb + (sb - eb) * fb, lb)]
        };
        let pair = field.pairs.get_mut(&key).unwrap();
        let d = narrow::distance_at(&hull(a, 0.0), &hull(b, 0.0), com(a, 0.0), com(b, 0.0), raw(0.0, 0.0), &mut pair.cache).0;
        pair.distance = Some(d);
        if d > contact::TOUCH {
            continue;
        }
        // Ghosted cars only note the touch (0x5B5060: +0x13C).
        let ghost = |k: usize| cars[k].state.ghost || cars[k].state.ghost_timer > 0.0;
        if ghost(a) || ghost(b) {
            continue;
        }
        // Another car counts as a wall for the post-crash grip timer
        // (0x592EE0 is true for cars). **Verified** native (docs/native-fixes/N-PHYS.tsv): the
        // per-pair test 0x5B5060 stamps both car bodies (+0x138 = the sim
        // clock) for every pair its distance query finds touching
        // (pair +0x14) and not ghosted, before any contact is resolved.
        cars[a].state.since_collision = 0.0;
        cars[b].state.since_collision = 0.0;
        let (t, stuck) = if d > contact::OVERLAP {
            (0.0, false)
        } else {
            // 0x5B4BF0: eight halvings over the shorter of the two
            // spans, each car blended over its own.
            let (sa, sb) = (spans[a], spans[b]);
            let (mut lo, mut hi) = (0.0f32, sa.min(sb));
            let mut clear: Option<f32> = None;
            let mut mid = 0.0;
            let mut touched = None;
            for _ in 0..8 {
                mid = (hi - lo) * 0.5 + lo;
                let fa = if sa > 1e-6 { mid / sa } else { 0.0 };
                let fb = if sb > 1e-6 { mid / sb } else { 0.0 };
                let d = narrow::distance_at(&hull(a, fa), &hull(b, fb), com(a, fa), com(b, fb), raw(fa, fb), &mut pair.cache).0;
                if d > contact::OVERLAP {
                    if d <= contact::TOUCH {
                        touched = Some(mid);
                        break;
                    }
                    hi = mid;
                    clear = Some(clear.map_or(mid, |c: f32| c.min(mid)));
                } else {
                    lo = mid;
                }
            }
            match (touched, clear) {
                (Some(m), _) => (m, false),
                (None, Some(c)) if c > 0.0 => (c, false),
                _ => (mid, true),
            }
        };
        let made = field.pairs[&key].made;
        if better(&best, t, made) {
            best = Some((t, made, Found::Pair(key, t, stuck)));
        }
    }
    // The props' pairs (with the track and with the cars) sit in the
    // one pair list by when they were made (0x5A0350 appends every
    // pair at the tail), so of equal times the older pair goes first.
    if let Some(e) = props.next(world, cars, starts, spans)
        && better(&best, e.time(), e.made())
    {
        best = Some((e.time(), e.made(), Found::Prop(e)));
    }
    let (_, _, found) = best?;
    Some(found)
}

/// Resolve a contact (track, car pair or prop), re-step the cars it
/// touched over the rest of their span and sweep again.
#[allow(clippy::too_many_arguments)]
fn resolve_event(
    found: Found,
    cars: &mut [Racer],
    field: &mut CarField,
    props: &mut crate::smackable::Props,
    world: &World,
    radii: &[f32],
    starts: &mut [Pose],
    spans: &mut [f32],
    track_done: &mut [HashMap<(usize, usize), usize>],
    pair_done: &mut HashMap<(usize, usize), usize>,
    track_cache: &mut [TrackCache],
    sounds: &mut ContactSounds,
) {
    match found {
        Found::Track(i, e) => {
            *track_done[i].entry(e.key).or_default() += 1;
            let t = e.t;
            let c = &mut cars[i];
            c.state.resolved += 1;
            // A belt car's track contact is handed back as any other
            // (**verified**): the body filter 0x593990 passes a
            // track partner while [body +0x60] +8 is set (a loaded
            // mode-4 car) and 0x5F46B0 takes no body out of the sweep,
            // so 0x5B2620 → 0x5F4970 → 0x402450 sees it: +0x730 = 2 s
            // and, the world under the car loaded, 0x5F4410's mover at
            // once (0x4024B8..0x4024DA). The clone had kept such a car on
            // its belt, which dragged it through the wall at Sprint 4122
            // backward's hairpin (−1500, 730) into the void
            // (`rivals_keep_the_race_route`, docs/drag-stage4-parity.md
            // §6.7); the earlier stuck-at-barriers trouble that made it
            // keep them no longer shows (unloaded bodies meet nothing).
            off_belt(c);
            track_cache[i] = TrackCache::default();
            c.state.resolve_track(c.car, &starts[i], spans[i], e, sounds);
            c.state.refresh_suspension(c.car, world);
            starts[i] = c.state.pose();
            spans[i] = t;
            if t > 0.0 {
                c.state.simulate(c.car, world, c.input, t, true);
            }
            // The body sweeps again (0x5FFC78 → 0x5B23C0).
            resweep(cars, field, world, radii, starts, spans, track_cache, &[i]);
            props.sweep(world, cars);
        }
        Found::Pair(key, t, stuck) => {
            *pair_done.entry(key).or_default() += 1;
            cars[key.0].state.resolved += 1;
            cars[key.1].state.resolved += 1;
            off_belt(&mut cars[key.0]);
            off_belt(&mut cars[key.1]);
            // 0x5B2620 tells both bodies first (slot +0x48 → 0x5F4970
            // → the controller's slot +0x38): an AI racer the human's
            // car touched takes the physics mover at once (0x402450),
            // for this contact and the rest of the frame.
            for (k, o) in [(key.0, key.1), (key.1, key.0)] {
                if cars[o].state.human && !cars[k].state.human && !cars[k].state.traffic {
                    cars[k].state.human_hit = true;
                    if let Input::Melo(c) = cars[k].input {
                        cars[k].input = Input::Ai(c);
                        cars[k].state.leave_melo();
                    }
                }
            }
            track_cache[key.0] = TrackCache::default();
            track_cache[key.1] = TrackCache::default();
            resolve_cars(cars, field, key, t, stuck, starts, spans, sounds);
            for k in [key.0, key.1] {
                let c = &mut cars[k];
                c.state.refresh_suspension(c.car, world);
            }
            for k in [key.0, key.1] {
                starts[k] = cars[k].state.pose();
                spans[k] = t;
                if t > 0.0 {
                    let c = &mut cars[k];
                    c.state.simulate(c.car, world, c.input, t, true);
                }
            }
            // A, then B (0x5FFC78).
            resweep(cars, field, world, radii, starts, spans, track_cache, &[key.0, key.1]);
            props.sweep(world, cars);
        }
        Found::Prop(e) => {
            let t = e.time();
            if let Some(k) = props.resolve(world, cars, starts, spans, e, sounds) {
                track_cache[k] = TrackCache::default();
                cars[k].state.resolved += 1;
                off_belt(&mut cars[k]);
                let c = &mut cars[k];
                c.state.refresh_suspension(c.car, world);
                starts[k] = cars[k].state.pose();
                spans[k] = t;
                if t > 0.0 {
                    let c = &mut cars[k];
                    c.state.simulate(c.car, world, c.input, t, true);
                }
                resweep(cars, field, world, radii, starts, spans, track_cache, &[k]);
                props.sweep(world, cars);
            }
        }
    }
}

/// Records past their life go (0x592BE0 → 0x5B2280).
fn expire_records(cars: &mut [Racer], field: &mut CarField) {
    for c in cars.iter_mut() {
        c.state.records.retain(|_, r| r.age < RECORD_LIFE - 1e-6);
    }
    for p in field.pairs.values_mut() {
        if p.record.is_some_and(|r| r.0 >= RECORD_LIFE - 1e-6) {
            p.record = None;
        }
    }
}

/// Each car's live records in the pair list's order (0x5A0830).
fn contact_notes(cars: &mut [Racer], field: &CarField) {
    let speeds: Vec<f32> = cars.iter().map(|c| c.state.velocity.length()).collect();
    for (i, c) in cars.iter_mut().enumerate() {
        let st = &mut c.state;
        let mut notes: Vec<(u64, ContactNote)> = st
            .records
            .iter()
            .map(|(k, r)| {
                (
                    st.pair_made.get(k).copied().unwrap_or(0),
                    ContactNote {
                        life: r.life,
                        age: r.age,
                        point: r.point,
                        touching: Touching::Track { upright: r.upright },
                        wall: r.wall,
                        impulse: r.impulse,
                    },
                )
            })
            .collect();
        for (&(a, b), p) in &field.pairs {
            if let (Some(r), true) = (p.record, a == i || b == i) {
                let other = if a == i { b } else { a };
                notes.push((
                    p.made,
                    ContactNote {
                        life: p.life,
                        age: r.0,
                        point: p.point,
                        touching: Touching::Car { speed: speeds[other] },
                        wall: false,
                        impulse: p.impulse,
                    },
                ));
            }
        }
        notes.sort_by_key(|n| n.0);
        st.contacts = notes.into_iter().map(|n| n.1).collect();
    }
}

/// The near-miss detector's view of the pair list (0x56F180), for the
/// cars that have one.
fn near_misses(cars: &mut [Racer], field: &CarField, radii: &[f32]) {
    let n = cars.len();
    let fit = |s: &CarState| !s.ghost && s.ghost_timer <= 0.0;
    for i in 0..n {
        if !cars[i].state.misc_style {
            continue;
        }
        let mut near: Vec<crate::style::NearPair> = field
            .pairs
            .iter()
            .filter(|(k, _)| k.0 == i || k.1 == i)
            .map(|(&(a, b), p)| {
                let o = if a == i { b } else { a };
                let (me, other) = (&*cars[i].state, &*cars[o].state);
                crate::style::NearPair {
                    id: p.made,
                    distance: p.distance,
                    contact: p.record.is_some(),
                    parting: p.record.is_none() && !boxes_meet(box_at(&cars[i]), radii[i], box_at(&cars[o]), radii[o]),
                    fit: fit(me) && fit(other),
                    relative_speed: (me.velocity - other.velocity).length(),
                    apart: me.position.distance(other.position),
                    above: me.position.z - other.position.z,
                }
            })
            .collect();
        near.sort_by_key(|p| p.id);
        cars[i].state.near = near;
    }
}

#[cfg(test)]
mod resweep_tests {
    use super::*;

    /// After a resolution only the event's bodies sweep again,
    /// A then B (0x5FFC78 → 0x5B23C0), each against every other car: B's
    /// new pairs are made before A's when B is the event's first body,
    /// whatever the bodies' box order (the old rule swept all cars by high x).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn moved_bodies_sweep_in_event_order() {
        let car = Car::load(&crate::testkit::game_root(), "240SX").unwrap();
        let world = World::flat(2000.0);
        let mut s0 = CarState::new(Vec3::new(0.0, 0.0, 0.0), 0.0);
        let mut s1 = CarState::new(Vec3::new(4.0, 0.0, 0.0), 0.0);
        let mut s2 = CarState::new(Vec3::new(2.0, 2.0, 0.0), 0.0);
        let mut cars = [
            Racer { car: &car, state: &mut s0, input: Input::Player(Controls::default()) },
            Racer { car: &car, state: &mut s1, input: Input::Player(Controls::default()) },
            Racer { car: &car, state: &mut s2, input: Input::Player(Controls::default()) },
        ];
        let radii: Vec<f32> = cars.iter().map(|c| CarState::radius(c.car)).collect();
        assert!(pair_boxes_meet(&cars, &radii, 0, 2) && pair_boxes_meet(&cars, &radii, 1, 2));
        let starts: Vec<Pose> = cars.iter().map(|c| c.state.pose()).collect();
        let spans = vec![1.0 / 60.0; 3];
        let mut caches: Vec<TrackCache> = (0..3).map(|_| TrackCache::default()).collect();
        let mut field = CarField::default();
        // Car 1 (the larger high x) is the event's first body.
        resweep(&mut cars, &mut field, &world, &radii, &starts, &spans, &mut caches, &[1, 0]);
        let made = |k: (usize, usize)| field.pairs.get(&k).map(|p| p.made);
        let (b, a) = (made((1, 2)).expect("B's pair"), made((0, 1)).or(made((0, 2))).expect("A's pairs"));
        assert!(b < a, "B swept first: {b} {a}");
        // A then B's pair with each other exists once.
        assert!(field.pairs.contains_key(&(1, 0)) ^ field.pairs.contains_key(&(0, 1)));
    }
}

use crate::drive::*;

/// Resolve a car-against-car contact (0x5B2620 with 0x5AF0F0): both cars
/// back to its moment; if they overlapped for the whole span, pushed
/// apart along the line between them (0x5B4300); the contact point is
/// midway between the closest points and the normal runs from A's to B's
/// (0x594010); the impulse, with the crash effects (0x59FB80) first.
pub(in crate::drive) fn resolve_cars(
    cars: &mut [Racer],
    field: &mut CarField,
    key: (usize, usize),
    t: f32,
    stuck: bool,
    starts: &[Pose],
    spans: &[f32],
    sounds: &mut ContactSounds,
) {
    let (a, b) = key;
    for k in [a, b] {
        let f = if spans[k] > 1e-6 { t / spans[k] } else { 0.0 };
        cars[k].state.back_to(&starts[k], f);
    }
    let pair = field.pairs.get_mut(&key).unwrap();
    if stuck {
        push_apart(cars, pair, a, b);
    }
    let (point, normal) = contact_point(cars, pair, a, b);
    // The record is made before the impulse is tried (0x5A9B20).
    let mut record = pair.record.unwrap_or_else(|| {
        pair.life = 0.0;
        pair.impulse = 0.0;
        pair.point = point;
        pair.sounded = false;
        pair.id = next_record();
        (0.0, false)
    });
    pair.record = Some(record);

    let Some((j, slide, mut arm_a, mut arm_b)) = exchange_impulse(cars, a, b, point, normal) else {
        return;
    };
    // The record takes the push (0x5A9630) and the crash check (0x59FB80)
    // runs on each car with the push it gets (A −J, B J).
    record.0 = 0.0;
    pair.point = point;
    pair.slide = slide;
    // **Verified** native (docs/native-fixes/N-PHYS.tsv): 0x5A9630 adds the uncapped |J| to
    // record +0x60 (0x5A9689) and posts the effects with it; only its
    // crash check 0x59FB80 holds |J| to 40 (0x7A1860, at 0x59FBB8).
    let magnitude = j.length().min(40.0);
    post_effects(cars, pair, a, b, point, normal, slide, j, sounds);
    crash_pushes(cars, &mut field.rng, a, b, j, magnitude, point, &mut record);
    pair.record = Some(record);
    // The push's |J| adds to record +0x60 (0x5A9689), then each car with a
    // misc style manager takes the record (0x5A9961 → 0x56FB30).
    pair.impulse += j.length();
    let traffic = cars[a].state.traffic || cars[b].state.traffic;
    for k in [a, b] {
        cars[k].state.style_contact(pair.impulse, pair.life, false, Some(traffic));
    }
    // After a crash on this record, a car with all four wheels down takes
    // the push level with its centre of mass (0x5AF0F0, record +0x10).
    if record.1 {
        for (k, arm) in [(a, &mut arm_a), (b, &mut arm_b)] {
            if cars[k].state.wheels_down() == 4 {
                let up = Mat3::from_quat(cars[k].state.orientation) * Vec3::Z;
                *arm -= up * arm.dot(up);
            }
        }
    }
    let (ca, cb) = (cars[a].car, cars[b].car);
    cars[b].state.take(&cb.car_type.handling, j, arm_b);
    cars[a].state.take(&ca.car_type.handling, -j, arm_a);
}

/// Both cars' hulls where they stand.
fn pair_hulls(cars: &[Racer], a: usize, b: usize) -> (Vec<Vec3>, Vec<Vec3>) {
    (CarState::hull_at(cars[a].car, &cars[a].state.pose()), CarState::hull_at(cars[b].car, &cars[b].state.pose()))
}

/// Both hulls' support offsets where they stand ([`narrow::Raw`]).
fn pair_raw<'a>(cars: &[Racer<'a>], a: usize, b: usize) -> narrow::Raw<'a> {
    [CarState::side_at(cars[a].car, &cars[a].state.pose()), CarState::side_at(cars[b].car, &cars[b].state.pose())]
}

/// 0x5B4300 for two cars: along A → B, 0.005 m then steps growing
/// by 0.01 while under 0.1, the vertical part held to ±0.15; A moves
/// back by it and B on, until clear; at most 20 tries.
fn push_apart(cars: &mut [Racer], pair: &mut CarPair, a: usize, b: usize) {
    let (ha, hb) = pair_hulls(cars, a, b);
    let raw = pair_raw(cars, a, b);
    // A car asleep (its mover's at-rest class 2) is not moved, in the
    // tries or after (vt+0x64, see `track_event`'s push-out).
    let (ka, kb) = ((cars[a].state.rest != 2) as u8 as f32, (cars[b].state.rest != 2) as u8 as f32);
    // Between the bodies' positions (vt+0x34 → 0x593290 from vt+0x08 =
    // 0x593B50: car +0x60, the centre of mass), not the car origins.
    let dir = (CarState::com_at(cars[b].car, &cars[b].state.pose())
        - CarState::com_at(cars[a].car, &cars[a].state.pose()))
    .normalize_or_zero();
    let (mut dist, mut step, mut offset) = (0.0f32, 0.005f32, Vec3::ZERO);
    for _ in 0..20 {
        dist += step;
        if step < 0.1 {
            step += 0.01;
        }
        offset = dir * dist;
        offset.z = offset.z.clamp(-0.15, 0.15);
        let ma: Vec<Vec3> = ha.iter().map(|p| *p - offset * ka).collect();
        let mb: Vec<Vec3> = hb.iter().map(|p| *p + offset * kb).collect();
        let (ca, cb) = (
            CarState::com_at(cars[a].car, &cars[a].state.pose()),
            CarState::com_at(cars[b].car, &cars[b].state.pose()),
        );
        if narrow::distance_at(&ma, &mb, ca - offset * ka, cb + offset * kb, raw, &mut pair.cache).0 > contact::OVERLAP {
            break;
        }
    }
    cars[a].state.position -= offset * ka;
    cars[b].state.position += offset * kb;
}

/// The contact point midway between the closest points and the normal
/// from A's to B's (0x594010), the points as the pair's last query left
/// them (+0xC0 / +0xD0): 0x5B2620 measures nothing between the time of
/// impact's search or the push-out and the impulse, as for a track pair.
/// **Observed** under Wine (every one of 77 car–car contacts in a rear and
/// a side approach: the point is the midpoint of the pair's closest points
/// of its last logged query; docs/prop-hits.md §7f). A pair with no query
/// yet measures where the cars stand.
fn contact_point(cars: &[Racer], pair: &mut CarPair, a: usize, b: usize) -> (Vec3, Vec3) {
    let (on_a, on_b) = match pair.cache.as_ref().and_then(|c| c.last()) {
        Some((_, x, y)) => (x, y),
        None => {
            let (ha, hb) = pair_hulls(cars, a, b);
            let (ca, cb) = (
                CarState::com_at(cars[a].car, &cars[a].state.pose()),
                CarState::com_at(cars[b].car, &cars[b].state.pose()),
            );
            let (_, x, y) = narrow::distance_at(&ha, &hb, ca, cb, pair_raw(cars, a, b), &mut pair.cache);
            (x, y)
        }
    };
    let point = (on_a + on_b) * 0.5;
    let gap = on_b - on_a;
    let normal = if gap.length() < 1e-6 {
        (cars[b].state.position - cars[a].state.position).normalize_or_zero()
    } else {
        gap.normalize()
    };
    (point, normal)
}

/// A car body's centre of mass in the world.
fn body_com(c: &Racer) -> Vec3 {
    c.state.position + Mat3::from_quat(c.state.orientation) * Vec3::from(c.car.car_type.handling.centre_of_mass)
}

/// A car as the impulse exchange sees it.
fn contact_mover(c: &Racer) -> contact::Mover {
    let h = &c.car.car_type.handling;
    let inertia = inertia_of(h, c.state.ground_inertia);
    contact::Mover {
        velocity: c.state.velocity,
        spin: c.state.spin,
        mass: h.mass,
        // Body-space, not turned (0x593A50, applied at 0x5AF3E1 and
        // 0x5AF468 as in a track contact).
        inverse_inertia: Box::new(move |v| v / inertia),
        crash: c.state.crash,
    }
}

/// The impulse between the two cars at `point` (contact::exchange), with
/// the arms from each centre of mass and the sliding speed; `None` when
/// they part.
fn exchange_impulse(
    cars: &[Racer],
    a: usize,
    b: usize,
    point: Vec3,
    normal: Vec3,
) -> Option<(Vec3, f32, Vec3, Vec3)> {
    let (arm_a, arm_b) = (point - body_com(&cars[a]), point - body_com(&cars[b]));
    let slide = slide_speed(
        contact_mover(&cars[b]).point_velocity(arm_b) - contact_mover(&cars[a]).point_velocity(arm_a),
        normal,
    );
    let j = contact::exchange(
        &contact_mover(&cars[a]),
        &contact_mover(&cars[b]),
        arm_a,
        arm_b,
        normal,
        contact::CAR_RESTITUTION,
        contact::CAR_FRICTION,
    )?;
    Some((j, slide, arm_a, arm_b))
}

/// The push's contact effects, the record's sound on its first push and
/// the scrape's arming.
#[allow(clippy::too_many_arguments)]
fn post_effects(
    cars: &[Racer],
    pair: &mut CarPair,
    a: usize,
    b: usize,
    point: Vec3,
    normal: Vec3,
    slide: f32,
    j: Vec3,
    sounds: &mut ContactSounds,
) {
    // The push's contact effects (0x5DC4D0): two cars are surface 28,
    // `XECI_CAR`, with the faster car's velocity.
    {
        let (va, vb) = (cars[a].state.velocity, cars[b].state.velocity);
        let faster = if va.length() >= vb.length() { va } else { vb };
        crate::particles::post_contact(crate::particles::CAR_SURFACE, point, slide, j.length(), faster, normal);
    }
    // The record's sound (0x5A9630 → 0x4797B0) on its first push.
    if !std::mem::replace(&mut pair.sounded, true) {
        let (sa, sb) = (&cars[a].state, &cars[b].state);
        let kind = crate::audio::collision::car_kind(sa.forward(), sa.position, sb.forward(), sb.position, point);
        sounds.impacts.post(point, kind, j.length());
    }
    // 0x4792D0: the scrape's arming; a car-against-car object never
    // starts FX_Scrape (audio::scrape).
    sounds.pushes.post(crate::audio::scrape::Push {
        record: pair.id,
        class: crate::audio::scrape::Class::Cars,
        slide,
        at: point,
    });
}

/// The crash check (0x59FB80) on each car with the push it gets (A −J,
/// B J): the hardest impulse, totalling, and over 30 the crash mode with
/// its sideways and vertical pushes.
#[allow(clippy::too_many_arguments)]
fn crash_pushes(
    cars: &mut [Racer],
    rng: &mut crate::opponents::GameRng,
    a: usize,
    b: usize,
    j: Vec3,
    magnitude: f32,
    point: Vec3,
    record: &mut (f32, bool),
) {
    for (k, push) in [(a, -j), (b, j)] {
        let c = &mut cars[k];
        c.state.hardest_impulse = c.state.hardest_impulse.max(magnitude);
        c.state.check_totaled(&c.car.car_type.handling, magnitude);
        if magnitude < 30.0 || c.state.crash {
            continue;
        }
        record.1 = true;
        c.state.crash = true;
        c.state.crash_clear = None;
        // The crash camera's test follows in 0x59FB80 (0x5A0095..0x5A0105,
        // `event_camera`).
        c.state.crash_hit = true;
        let h = &c.car.car_type.handling;
        let rot = Mat3::from_quat(c.state.orientation);
        let r = point - body_com(c);
        let d = push.normalize_or_zero();
        let v = c.state.velocity.normalize_or_zero();
        let dot = v.dot(d);
        if dot < -0.97 {
            // Nearly head on: a sideways push across the car's travel.
            let side = (d - v * dot).normalize_or_zero() * -(dot.abs() * 0.2 * magnitude);
            c.state.take(h, Vec3::new(side.x, side.y, 0.0), r);
        }
        // Then a vertical push at the point brought into the car's frame
        // (height dropped): sideways ×1.19 within ±1.19 × half the front
        // track, or head on mirrored front to back half the time (×1.1)
        // and the sideways part ×0.2 past 0.13 m.
        let mut local = rot.transpose() * r;
        local.z = 0.0;
        if dot >= -0.97 {
            let half = (car_track(c.car) * 1.19 * 0.5).abs();
            local.y = (local.y * 1.19).clamp(-half, half);
        } else {
            if rng.below(100) >= 50 {
                local.x = -(local.x * 1.1);
            }
            if local.y.abs() > 0.13 {
                local.y *= 0.2;
            }
        }
        // 1.1 (0x7A1864) offline, 1.2 (0x7A1868) in drag races and
        // online (0x5A0010..0x5A002D: 0x89E7D8 or 0x890108), within ±4.1×
        // (0x7A1844).
        let f = if h.race.drag() { 1.2f32 } else { 1.1f32 };
        let lift = (dot.abs() * 0.1 * magnitude * f).clamp(-4.1 * f, 4.1 * f);
        c.state.take(h, Vec3::new(0.0, 0.0, lift), rot * local);
    }
}

/// The front track width: wheel 0's y less wheel 1's (record +0x124,
/// +0x154; read by 0x59FB80 through car +0x20).
pub(in crate::drive) fn car_track(car: &Car) -> f32 {
    car.car_type.wheels[0].position.y - car.car_type.wheels[1].position.y
}

/// Where the car is and how fast it goes, saved at the start of a span.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pose {
    pub(crate) position: Vec3,
    pub(crate) orientation: Quat,
    pub(crate) velocity: Vec3,
}

/// When, in seconds before the end of the span, the hull first touched the
/// polygon (0x5B4BF0): eight halvings between the end (0) and the start
/// (`span`), keeping the latest time at which they did not overlap. Hull
/// and polygon just touching ends the search early. If they overlapped at
/// every time tried, the last time and `true`: the car must be pushed out.
pub(crate) fn time_of_impact<'a>(
    at: &dyn Fn(f32) -> (Vec<Vec3>, Vec3, narrow::Side<'a>),
    corners: &[Vec3],
    span: f32,
    pair: &mut Option<narrow::PairCache>,
) -> (f32, bool) {
    let (mut lo, mut hi) = (0.0f32, span);
    let mut clear: Option<f32> = None;
    let mut mid = 0.0;
    for _ in 0..8 {
        mid = (hi - lo) * 0.5 + lo;
        let (hull, com, side) = at(mid);
        let d = narrow::distance_at(&hull, corners, com, Vec3::ZERO, [side, narrow::Side::NONE], pair).0;
        if d > contact::OVERLAP {
            if d <= contact::TOUCH {
                return (mid, false);
            }
            hi = mid;
            clear = Some(clear.map_or(mid, |c: f32| c.min(mid)));
        } else {
            lo = mid;
        }
    }
    match clear {
        Some(c) if c > 0.0 => (c, false),
        _ => (mid, true),
    }
}

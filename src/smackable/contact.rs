use super::*;

/// The car's box half size for the sweep: its body's reach (the farthest
/// hull point from the centre of mass, [`Car::body_reach`]), at least 1 m.
pub(super) fn car_radius(car: &Car) -> f32 {
    car.body_reach().radius.max(1.0)
}

/// A moving prop's earliest track contact over its span (0x5B5060 →
/// 0x5B3D30 / 0x5B4BF0 per polygon): the one furthest back in time, the
/// first of equals; a polygon's pair at most twice a frame.
pub(super) fn track_event(
    world: &World,
    prop: &Prop,
    k: usize,
    b: &mut Body,
    done: &HashMap<(usize, (usize, usize)), usize>,
) -> Option<PropEvent> {
    let end = shape_at(prop, b.centre, b.rot);
    let begin = shape_at(prop, b.start.0, b.start.1);
    let span = b.span;
    let (centre, start) = (b.centre, b.start.0);
    let (raw_end, raw_begin) = (raw_at(prop, b.rot), raw_at(prop, b.start.1));
    let at = |t: f32| -> (Vec<Vec3>, Vec3, narrow::Side) {
        let f = if span > 1e-6 { (t / span).min(1.0) } else { 0.0 };
        (
            end.iter().zip(&begin).map(|(e, s)| *e + (*s - *e) * f).collect(),
            centre + (start - centre) * f,
            narrow::Side::hull(raw_end.offset + (raw_begin.offset - raw_end.offset) * f, &prop.links),
        )
    };
    let mut first: Option<PropEvent> = None;
    for (key, corners, normal, turn) in polygons(world, prop, b) {
        if done.get(&(k, key)).copied().unwrap_or(0) >= crate::contact::MAX_PER_POLYGON {
            continue;
        }
        let pair = b.pairs.entry(Partner::Track(key)).or_default();
        let d = narrow::distance_at(&end, &corners, centre, Vec3::ZERO, [raw_end, narrow::Side::NONE], pair).0;
        if d > crate::contact::TOUCH {
            continue;
        }
        let (t, stuck) =
            if d > crate::contact::OVERLAP { (0.0, false) } else { time_of_impact(&at, &corners, span, pair) };
        let made = *b.made.entry(Partner::Track(key)).or_insert_with(crate::drive::next_pair);
        let e = PropEvent::Track { prop: k, t, stuck, key, corners, normal, turn, made };
        if first.as_ref().is_none_or(|f| e.before(f)) {
            first = Some(e);
        }
    }
    first
}

/// A prop's track contact (0x5B2620): back to the moment, pushed out if it
/// overlapped throughout (0x5B4300), one impulse, the rest of the span
/// simulated again.
pub(super) fn resolve_track(
    prop: &Prop,
    b: &mut Body,
    t: f32,
    stuck: bool,
    key: (usize, usize),
    corners: &[Vec3],
    normal: Vec3,
    turn: bool,
    sounds: &mut crate::drive::ContactSounds,
) {
    back_to(prop, b, t);
    // Turned toward the body only when X+0x64 is set (the face's byte 0 is
    // nonzero; the gate at 0x593E34), as for cars (drive.rs).
    let normal = if turn && (b.centre - corners[0]).dot(normal) < 0.0 { -normal } else { normal };
    let partner = Partner::Track(key);
    if normal.z.abs() < 0.1 && !b.walls.contains(&key) {
        b.walls.push(key);
    }
    if stuck {
        let shape = shape_at(prop, b.centre, b.rot);
        let raw = raw_at(prop, b.rot);
        b.centre += crate::contact::push_out(&shape, corners, b.centre, normal, raw, b.pairs.entry(partner).or_default());
    }
    // The pair's last answer as it stands (0x594010 reads pair +0xC0 /
    // +0xD0; see `car_contact`).
    let (on_prop, on_poly) = match b.pairs.get(&partner).and_then(|p| p.as_ref()).and_then(|p| p.last()) {
        Some((_, x, y)) => (x, y),
        None => {
            let shape = shape_at(prop, b.centre, b.rot);
            let raw = [raw_at(prop, b.rot), narrow::Side::NONE];
            let (_, x, y) = narrow::distance_at(&shape, corners, b.centre, Vec3::ZERO, raw, b.pairs.entry(partner).or_default());
            (x, y)
        }
    };
    let point = (on_prop + on_poly) * 0.5;
    if !b.records.iter().any(|r| r.0 == partner) {
        b.records.push((partner, 0.0));
    }
    let arm = point - b.centre;
    let inv = prop.body_inverse_inertia();
    let v = b.velocity + b.spin.cross(arm);
    let vn = v.dot(normal);
    if vn < 0.0 {
        let e = TRACK_MATERIAL.max(prop.info.restitution);
        let mu = TRACK_MATERIAL.max(prop.info.friction);
        let k = 1.0 / prop.mass() + normal.dot((inv * arm.cross(normal)).cross(arm));
        let j = (-(1.0 + e) * vn / k).max(0.3 * prop.mass());
        let impulse = contact_impulse(normal, j, v - normal * vn, mu);
        set_record(b, partner);
        prop_sound(prop, b, partner, point, impulse, 8, sounds);
        take(prop, b, impulse, arm);
    }
    // The rest of the span from here.
    b.start = (b.centre, b.rot, b.velocity);
    b.span = t;
    simulate(prop, b, t);
}

/// A car's contact with a prop at its moment (both bodies already moved
/// back to it). Stuck: pushed apart (0x5B4300) along A → B, or, with a
/// body of the pair faster than 200 km/h, along the car's velocity (A the
/// car) or against it (A the prop); A moves back by the offset and B on,
/// each only when it can move (vtable +0x64: a prop with its break
/// impulse left cannot). Then the impulse (0x5AF0F0, 0x5B2080). Returns
/// whether the prop was pushed.
#[allow(clippy::too_many_arguments)]
pub(super) fn car_contact(prop: &Prop, b: &mut Body, k: usize, i: usize, car: &Car, state: &mut CarState, stuck: bool, sounds: &mut crate::drive::ContactSounds) -> bool {
    let key = Partner::Car(i);
    let rot = Mat3::from_quat(state.orientation);
    let hull = |state: &CarState| -> Vec<Vec3> { car.hull.iter().map(|p| state.position + rot * *p).collect() };
    // Neither body turns here: the offsets stand for the whole contact.
    let raw = [narrow::Side::hull(rot * car.hull_offset(), &car.hull_links), raw_at(prop, b.rot)];
    let fixed = b.break_left > 0.0;
    if stuck {
        let car_first = b.car_first.get(&i).copied().unwrap_or(true);
        // Each body's position at the moment (vt+0x34 → 0x593290 from
        // vt+0x08): the car's is its centre of mass (car +0x60, 0x593B50),
        // not its origin.
        let car_com = CarState::com_at(car, &state.pose());
        let (a, bb) = if car_first { (car_com, b.centre) } else { (b.centre, car_com) };
        let along = if state.velocity.length() >= PUSH_ALONG_VELOCITY {
            if car_first { state.velocity } else { -state.velocity }
        } else {
            bb - a
        };
        let dir = along.normalize_or(Vec3::X);
        // The car's offset, signed so that A moves back and B on.
        let car_sign = if car_first { -1.0 } else { 1.0 };
        let (car_hull, shape) = (hull(state), shape_at(prop, b.centre, b.rot));
        let (mut dist, mut step, mut offset) = (0.0f32, 0.005f32, Vec3::ZERO);
        let pair = b.pairs.entry(key).or_default();
        for _ in 0..20 {
            dist += step;
            if step < 0.1 {
                step += 0.01;
            }
            offset = dir * dist;
            offset.z = offset.z.clamp(-0.15, 0.15);
            let mc: Vec<Vec3> = car_hull.iter().map(|p| *p + offset * car_sign).collect();
            let (mp, at): (Vec<Vec3>, Vec3) = if fixed {
                (shape.clone(), b.centre)
            } else {
                (shape.iter().map(|p| *p - offset * car_sign).collect(), b.centre - offset * car_sign)
            };
            if narrow::distance_at(&mc, &mp, car_com + offset * car_sign, at, raw, pair).0 > crate::contact::OVERLAP {
                break;
            }
        }
        state.position += offset * car_sign;
        if !fixed {
            b.centre -= offset * car_sign;
        }
    }
    // The pair's last answer as it stands (0x594010 reads pair +0xC0 /
    // +0xD0; nothing is measured since the time of impact's search or the
    // push-out, 0x5B2620): at speed the points can lie centimetres apart,
    // from another moment of the search (read under Wine, docs/notes/parity/prop-hits.md).
    let (on_car, on_prop) = match b.pairs.get(&key).and_then(|p| p.as_ref()).and_then(|p| p.last()) {
        Some((_, x, y)) => (x, y),
        None => {
            let (_, x, y) = narrow::distance(&hull(state), &shape_at(prop, b.centre, b.rot), raw, b.pairs.entry(key).or_default());
            (x, y)
        }
    };
    let point = (on_car + on_prop) * 0.5;
    // From the car's point to the prop's (0x594010 runs A's to B's; the
    // impulse is the same either way round).
    // Under 1e-6 apart (0x787594), the line between the bodies' positions
    // (vtable +0x08: the car's centre of mass, the prop's).
    let gap = on_prop - on_car;
    let normal = if gap.length() >= 1e-6 { gap.normalize() } else { b.centre - CarState::com_at(car, &state.pose()) }
        .try_normalize()
        .unwrap_or(Vec3::X);
    if !b.records.iter().any(|r| r.0 == key) {
        b.records.push((key, 0.0));
    }
    let (car_v, car_arm, car_k) = state.prop_contact_terms(car, point, normal);
    let e = CAR_RESTITUTION.max(prop.info.restitution);
    let mu = CAR_FRICTION.max(prop.info.friction);
    let car_inv_mass = (1.0 / car.car_type.handling.mass).min(1.0 / 1.85);
    let arm = point - b.centre;
    let inv = prop.body_inverse_inertia();
    let v = if fixed { Vec3::ZERO } else { b.velocity + b.spin.cross(arm) } - car_v;
    let vn = v.dot(normal);
    if vn >= 0.0 {
        return false;
    }
    let (prop_k, prop_inv_mass) = if fixed {
        (0.0, 0.0)
    } else {
        (1.0 / prop.mass() + normal.dot((inv * arm.cross(normal)).cross(arm)), 1.0 / prop.mass())
    };
    // At least a 0.3 m/s push (0x5AF0F0: 0.3 / the larger 1/m′).
    let j = (-(1.0 + e) * vn / (car_k + prop_k)).max(0.3 / car_inv_mass.max(prop_inv_mass));
    let impulse = contact_impulse(normal, j, v - normal * vn, mu);
    if crate::env_value_os!("NFSU2_PROP_LOG").is_some() {
        eprintln!(
            "PROPCAR @{} {k} {} speed {:.3} stuck {stuck} normal {:.4} {:.4} {:.4} point {:.3} {:.3} {:.3} centre {:.3} {:.3} {:.3} J {:.4} {:.4} {:.4} car {:.3} {:.3} {:.3}",
            super::log_step(), prop.info.name, state.velocity.length(), normal.x, normal.y, normal.z, point.x, point.y, point.z,
            b.centre.x, b.centre.y, b.centre.z, impulse.x, impulse.y, impulse.z, state.position.x, state.position.y, state.position.z
        );
    }
    if crate::env_value_os!("NFSU2_SUBSTEP_DUMP")
        .is_some_and(|n| car.car_type.name.eq_ignore_ascii_case(&n.to_string_lossy()))
    {
        eprintln!(
            "SUBSTEP PROP {} mass {} break_left {} e {e} mu {mu} stuck {stuck} fixed {fixed} moving {} point {point:?} normal {normal:?} vn {vn} j {j} impulse {impulse:?} car_v {car_v:?} prop_v {:?} centre {:?}",
            prop.info.name, prop.info.mass, b.break_left, b.moving, b.velocity, b.centre
        );
    }
    set_record(b, key);
    // 0x5B2080: the larger of |J| and |(c + r) × J| against the break
    // impulse (c the centre-of-mass offset, r the arm from it).
    let mut scale = 1.0;
    if fixed {
        let size = impulse.length().max((arm + b.rot * prop.info.centre_of_mass).cross(impulse).length());
        if size < b.break_left {
            prop_sound(prop, b, key, point, impulse, 7, sounds);
            state.take_push(car, against_wall(b, -impulse), car_arm);
            return false;
        }
        scale = b.break_left / size;
        b.break_left = 0.0;
    }
    if !b.moving {
        start_moving(b);
    }
    prop_sound(prop, b, key, point, impulse * scale, 7, sounds);
    take(prop, b, impulse * scale, arm);
    state.take_push(car, against_wall(b, -impulse * scale), car_arm);
    true
}

/// The car's impulse from a prop that is up against a wall (0x5AF8D8..
/// 0x5AF97B: a car, body type 0, with a type-1 body whose +0x120 — its live
/// wall records — is above 0): its z × 0.25 (0x7A18B0), floored at 0
/// (0x7D7D3C), before it is applied (vtable +0x50 with −J for A, J for B).
pub(super) fn against_wall(b: &Body, impulse: Vec3) -> Vec3 {
    let pressed = b.records.iter().any(|r| matches!(r.0, Partner::Track(k) if b.walls.contains(&k)));
    if pressed { Vec3::new(impulse.x, impulse.y, (impulse.z * 0.25).max(0.0)) } else { impulse }
}

/// A record's first push posts its sound (class 2: 0x46A710, 0x47A550,
/// 0x46A670): kind 7 with a car, 8 against the track; the volume from |J|
/// × the prop's 1/m (vtable +0x5C: 0 while its break impulse is left);
/// it plays unless quiet (under 10 without a car) or another record of
/// the prop is sounding (inferred: sounding while that record lives).
#[allow(clippy::too_many_arguments)]
pub(super) fn prop_sound(prop: &Prop, b: &mut Body, partner: Partner, point: Vec3, impulse: Vec3, kind: usize, sounds: &mut crate::drive::ContactSounds) {
    use crate::audio::collision as sound;
    if b.sounds.contains_key(&partner) {
        return;
    }
    let sounding = b.sounds.values().any(|s| *s);
    let inverse_mass = if b.break_left > 0.0 { 0.0 } else { 1.0 / prop.mass() };
    let volume = sound::volume(impulse.length() * inverse_mass, kind);
    let plays = sound::prop_plays(kind, volume, sounding);
    if crate::env_value_os!("NFSU2_PROP_LOG").is_some() {
        eprintln!(
            "PROPSOUND {} @{} {partner:?} kind {kind} |J| {:.4} volume {volume} plays {plays} at {:.2} {:.2} {:.2}",
            prop.info.name,
            super::log_step(),
            impulse.length(),
            point.x,
            point.y,
            point.z
        );
    }
    if plays {
        // The prop object's start (vtable 0x78BB08 slot 8, 0x46AE00, run
        // by the queue's flush when the object is started): a WATER_DRUM
        // (material 5, info +0xD8) already knocked loose (+0x505) and not
        // yet splashed (+0x50C) splashes, once, at the object's start volume
        // (`collision::splash_level`). The flag is set here as the sound is
        // prov: inferred props.sound.splash_flag
        // posted; natively the start sets it, so a drum whose object the
        // five-object cap drops would splash at a later record (**inferred**
        // equivalent: not modelled).
        let splash = prop.info.material == WATER_DRUM && b.knocked && !b.splashed;
        b.splashed |= splash;
        sounds.impacts.post_impact(sound::Impact {
            at: point,
            kind,
            volume,
            material: prop.info.material,
            crash_camera: false,
            splash,
        });
    }
    b.sounds.insert(partner, plays);
}

/// Normal impulse j plus friction μ·j against the sliding direction, × the
/// sliding speed under 1 m/s, none under 1e-5 (as 0x5AF0F0).
pub(super) fn contact_impulse(normal: Vec3, j: f32, tangent: Vec3, mu: f32) -> Vec3 {
    let slide = tangent.length();
    let f = mu * j * slide.min(1.0);
    normal * j + if slide > 1e-5 { -tangent / slide * f } else { Vec3::ZERO }
}

/// Two moving shapes' time of impact (0x5B4BF0): as [`time_of_impact`]
/// with both blended; each closure gives the shape and the body's position
/// at a time (the pair's dirty test, `narrow::distance_at`: every query
/// leaves its answer in the pair, and the contact takes the last one).
pub(super) fn pair_time<'a, 'b>(
    a: &dyn Fn(f32) -> (Vec<Vec3>, Vec3, narrow::Side<'a>),
    b: &dyn Fn(f32) -> (Vec<Vec3>, Vec3, narrow::Side<'b>),
    span: f32,
    pair: &mut Option<narrow::PairCache>,
) -> (f32, bool) {
    let (mut lo, mut hi) = (0.0f32, span);
    let mut clear: Option<f32> = None;
    let mut mid = 0.0;
    for _ in 0..8 {
        mid = (hi - lo) * 0.5 + lo;
        let ((sa, pa, ra), (sb, pb, rb)) = (a(mid), b(mid));
        let d = narrow::distance_at(&sa, &sb, pa, pb, [ra, rb], pair).0;
        if crate::env_value_os!("NFSU2_PROP_LOG").is_some()
            && let Some((_, x, y)) = pair.as_ref().and_then(|p| p.last())
        {
            eprintln!(
                "PROPTOI @{} t {mid:.6} d {d:.6} a {:.5} {:.5} {:.5} b {:.5} {:.5} {:.5}",
                super::log_step(),
                x.x,
                x.y,
                x.z,
                y.x,
                y.y,
                y.z
            );
        }
        if d > crate::contact::OVERLAP {
            if d <= crate::contact::TOUCH {
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

/// The track polygons a prop may touch (0x5B23C0 → 0x5AE940, the car's
/// gather and pre-test 0x5937C0 with the prop's reach), first 48 in face
/// order.
pub(super) fn polygons(world: &World, prop: &Prop, b: &mut Body) -> Vec<((usize, usize), Vec<Vec3>, Vec3, bool)> {
    let at = b.centre.truncate();
    let radius = prop.reach;
    if world.ground.is_empty() {
        return world
            .collision()
            .triangles_near(at - radius - 0.1, at + radius + 0.1)
            .into_iter()
            .map(|(i, c, n, _)| ((usize::MAX, i), c.to_vec(), n, true))
            .collect();
    }
    let found = world.ground.nearby(&world.visibility, &mut b.ground, at - radius, at + radius, b.velocity.truncate());
    let up = b.rot.z_axis;
    let mut picked = Vec::new();
    for poly in found {
        if picked.len() >= 48 {
            break;
        }
        if at.x + radius < poly.min.x
            || poly.max.x < at.x - radius
            || at.y + radius < poly.min.y
            || poly.max.y < at.y - radius
        {
            continue;
        }
        let c = up.dot(poly.normal).min(1.0);
        let dist = b.centre.dot(poly.normal) - poly.d;
        let height = if c * dist < 0.0 { prop.above } else { prop.below };
        if dist.abs() >= (1.0 - c * c).max(0.0).sqrt() * radius + c.abs() * height {
            continue;
        }
        let inside = world.ground.inside(poly.id, at);
        if poly.distance_xy(at, radius + 0.1, inside) > radius {
            continue;
        }
        picked.push(poly);
    }
    picked.sort_by_key(|p| p.id);
    picked.into_iter().map(|p| (p.id, p.corners[..p.count].to_vec(), p.normal, p.turn)).collect()
}

/// Two props' contact at its moment (both bodies already moved
/// back to it): 0x5B2620 with two prop bodies (vtable 0x7A26A0). Stuck:
/// pushed apart (0x5B4300, A a prop): along B's velocity reversed when B
/// is faster than 200 km/h, else along A → B (the centres of mass), the
/// vertical part held to ±0.15 m, A back and B on, each only when it can
/// move (vtable +0x64 = 0x5B1C90: its break impulse spent). Then 0x5AF0F0:
/// the point midway between the closest points and the normal from A's to
/// B's (0x594010: both bodies have +0x12 set, so neither is a face; under
/// 1e-6 apart the centres' line); e and μ the larger of the two infos'
/// (vtable +0x70 / +0x74 = 0x5B1CC0 / 0x5B1CD0, combined by the larger,
/// 0x5941D0); each body's 1/m and body-space inverse inertia unturned
/// (vtable +0x5C / +0x60, zero while its break impulse is left); j at
/// least 0.3 / the larger 1/m; friction as for the cars. B takes J and A
/// −J (vtable +0x4C = 0x5B2080 starts a resting body, then +0x50). The
/// record's sound object is A's (0x592D90 takes A when it is a prop) and
/// kind 8 (no car in the pair: 0x4598B0 gives 7 only with +0x128 bit
/// 0x10).
#[allow(clippy::too_many_arguments)]
pub(super) fn prop_pair(
    pa: &Prop,
    a: &mut Body,
    ka: usize,
    pb: &Prop,
    b: &mut Body,
    kb: usize,
    stuck: bool,
    cache: &mut Option<narrow::PairCache>,
    sounds: &mut crate::drive::ContactSounds,
) -> bool {
    let (fixed_a, fixed_b) = (a.break_left > 0.0, b.break_left > 0.0);
    if stuck {
        let along = if b.velocity.length() >= PUSH_ALONG_VELOCITY { -b.velocity } else { b.centre - a.centre };
        let dir = along.normalize_or(Vec3::X);
        let (sa, sb) = (shape_at(pa, a.centre, a.rot), shape_at(pb, b.centre, b.rot));
        let raw = [raw_at(pa, a.rot), raw_at(pb, b.rot)];
        let (ma_on, mb_on) = ((!fixed_a) as u8 as f32, (!fixed_b) as u8 as f32);
        let (mut dist, mut step, mut offset) = (0.0f32, 0.005f32, Vec3::ZERO);
        for _ in 0..20 {
            dist += step;
            if step < 0.1 {
                step += 0.01;
            }
            offset = dir * dist;
            offset.z = offset.z.clamp(-0.15, 0.15);
            let ma: Vec<Vec3> = sa.iter().map(|p| *p - offset * ma_on).collect();
            let mb: Vec<Vec3> = sb.iter().map(|p| *p + offset * mb_on).collect();
            if narrow::distance_at(&ma, &mb, a.centre - offset * ma_on, b.centre + offset * mb_on, raw, cache).0
                > crate::contact::OVERLAP
            {
                break;
            }
        }
        a.centre -= offset * ma_on;
        b.centre += offset * mb_on;
    }
    // The pair's last answer as it stands (0x594010, see `car_contact`).
    let (on_a, on_b) = match cache.as_ref().and_then(|c| c.last()) {
        Some((_, x, y)) => (x, y),
        None => {
            let raw = [raw_at(pa, a.rot), raw_at(pb, b.rot)];
            let (_, x, y) = narrow::distance(&shape_at(pa, a.centre, a.rot), &shape_at(pb, b.centre, b.rot), raw, cache);
            (x, y)
        }
    };
    let point = (on_a + on_b) * 0.5;
    let normal = (on_b - on_a)
        .try_normalize()
        .filter(|_| (on_b - on_a).length() >= 1e-6)
        .or((b.centre - a.centre).try_normalize())
        .unwrap_or(Vec3::X);
    let (ra, rb) = (point - a.centre, point - b.centre);
    if !a.records.iter().any(|r| r.0 == Partner::Prop(kb)) {
        a.records.push((Partner::Prop(kb), 0.0));
    }
    if !b.records.iter().any(|r| r.0 == Partner::Prop(ka)) {
        b.records.push((Partner::Prop(ka), 0.0));
    }
    let point_v = |body: &Body, r: Vec3| body.velocity + body.spin.cross(r);
    let v = point_v(b, rb) - point_v(a, ra);
    let vn = v.dot(normal);
    if crate::env_value_os!("NFSU2_PROP_LOG").is_some() {
        eprintln!(
            "PROPPAIR @{} {ka} {kb} stuck {stuck} vn {vn:.4} normal {:.4} {:.4} {:.4} point {:.3} {:.3} {:.3}",
            super::log_step(),
            normal.x,
            normal.y,
            normal.z,
            point.x,
            point.y,
            point.z
        );
    }
    if vn >= 0.0 {
        return false;
    }
    let side = |p: &Prop, fixed: bool, r: Vec3| -> (f32, f32) {
        if fixed {
            (0.0, 0.0)
        } else {
            let inv = p.body_inverse_inertia();
            (1.0 / p.mass(), normal.dot((inv * r.cross(normal)).cross(r)))
        }
    };
    let ((ia, ta), (ib, tb)) = (side(pa, fixed_a, ra), side(pb, fixed_b, rb));
    if ia == 0.0 && ib == 0.0 {
        return false;
    }
    let e = pa.info.restitution.max(pb.info.restitution);
    let mu = pa.info.friction.max(pb.info.friction);
    let j = (-(1.0 + e) * vn / (ia + ta + ib + tb)).max(0.3 / ia.max(ib));
    let impulse = contact_impulse(normal, j, v - normal * vn, mu);
    // The break test (0x5B2080) for B, then A: a body with its break
    // impulse left keeps it unless the push reaches it; then the push is
    // scaled by what it took.
    let mut scale = 1.0f32;
    for (p, body, r) in [(pb, &mut *b, rb), (pa, &mut *a, ra)] {
        if body.break_left > 0.0 {
            let size = impulse.length().max((r + body.rot * p.info.centre_of_mass).cross(impulse).length());
            if size >= body.break_left {
                scale = scale.min(body.break_left / size);
                body.break_left = 0.0;
                start_moving(body);
            }
        } else if !body.moving {
            start_moving(body);
        }
    }
    set_record(a, Partner::Prop(kb));
    set_record(b, Partner::Prop(ka));
    prop_sound(pa, a, Partner::Prop(kb), point, impulse * scale, 8, sounds);
    take(pb, b, impulse * scale, rb);
    take(pa, a, -impulse * scale, ra);
    true
}

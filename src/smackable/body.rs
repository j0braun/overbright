use super::*;

/// Who a contact record is with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Partner {
    Car(usize),
    Track((usize, usize)),
    /// Another knock-over prop (a moving prop's sweep pairs it
    /// with the props its box meets, 0x5FFB10 → 0x5B23C0).
    Prop(usize),
}

/// A woken prop's rigid body (0x5B3250).
#[derive(Clone, Debug)]
pub(super) struct Body {
    /// Centre of mass (body +0x20) and axes (+0x30, columns here).
    pub(super) centre: Vec3,
    pub(super) rot: Mat3,
    /// Linear and angular momentum (+0x70, +0x80) and the rest damping
    /// (+0x90): the integrated state with position and axes.
    pub(super) momentum: Vec3,
    pub(super) angular_momentum: Vec3,
    pub(super) rest_damping: f32,
    /// P/m and I⁻¹L (+0xA0, +0xC0), set by the tidy-up.
    pub(super) velocity: Vec3,
    pub(super) spin: Vec3,
    /// The force queued for the next step (+0x110: gravity, 0x59C600).
    pub(super) force: Vec3,
    pub(super) gravity: f32,
    /// The prop's +0x150, cleared when it breaks.
    pub(super) break_left: f32,
    /// +0x506 and the settle test's last answer (+0x508).
    pub(super) moving: bool,
    pub(super) settle: u8,
    /// Has moved at some point (the prop's +0x505, set once by 0x5ADFA0).
    pub(super) knocked: bool,
    /// The water drum's splash has played (+0x50C, set by 0x46AE00; nothing
    /// clears it).
    pub(super) splashed: bool,
    /// Live contact records, oldest first, with seconds since the last push.
    pub(super) records: Vec<(Partner, f32)>,
    /// Seconds since a record with a car ended (prop +0x160, 0x59E540).
    pub(super) since_car: f32,
    /// The track faces met that are walls (|n.z| < 0.1): a record with one
    /// counts in the body's +0x120 (0x5A9440 → 0x5A0170 adds 1 when the
    /// pair's polygon, body type 3, has |+0x60 → +0x78| < 0.1; the record's
    /// destructor 0x5A9580 takes it off, floored at 0).
    pub(super) walls: Vec<(usize, usize)>,
    /// The pose at the start of the span (0x593220) and its length (+0x6C;
    /// 5000 when woken, 0x5ADB30).
    pub(super) start: (Vec3, Mat3, Vec3),
    pub(super) span: f32,
    pub(super) ground: GroundQuery,
    pub(super) pairs: HashMap<Partner, Option<narrow::PairCache>>,
    /// Per car pair, whether the car is the pair's A (made with the
    /// smaller box edge first, as the car pairs, 0x592970).
    pub(super) car_first: HashMap<usize, bool>,
    /// When each pair was made: its place in the one pair list
    /// (0x5A0350 appends at the tail, car and prop pairs alike).
    pub(super) made: HashMap<Partner, u64>,
    /// The draw offset (+0x168): how far the body was lifted clear of the
    /// ground when woken, drawn that much lower (0x5ADC7D).
    pub(super) draw_offset: f32,
    /// The live records whose sound object has had its first push, and
    /// whether it started (class 2, `audio::collision`).
    pub(super) sounds: HashMap<Partner, bool>,
}

/// The shape in the world for a pose.
pub(super) fn shape_at(prop: &Prop, centre: Vec3, rot: Mat3) -> Vec<Vec3> {
    // A `ZCS_` volume's points as the original's query rebuilds them (the
    // vertex less the offset, already so in `shape`, then the transform
    // about the centre of mass: narrow::body_point); the box (no offset)
    // as before.
    if prop.offset != Vec3::ZERO {
        return prop.shape.iter().map(|p| narrow::body_point(rot, centre, *p)).collect();
    }
    prop.shape.iter().map(|p| centre + rot * *p).collect()
}

/// The shape's support offset turned into the world for a pose
/// ([`crate::narrow::Raw`]).
pub(super) fn raw_at(prop: &Prop, rot: Mat3) -> narrow::Side<'_> {
    narrow::Side::hull(rot * prop.offset, &prop.links)
}

pub(super) fn lerp_mat(a: Mat3, b: Mat3, f: f32) -> Mat3 {
    Mat3::from_cols(a.x_axis.lerp(b.x_axis, f), a.y_axis.lerp(b.y_axis, f), a.z_axis.lerp(b.z_axis, f))
}

/// 0x5BB0E0: x normalised, z = x × y normalised, y = z × x.
pub(super) fn orthonormal(m: Mat3) -> Mat3 {
    let x = m.x_axis.normalize_or(Vec3::X);
    let z = x.cross(m.y_axis).normalize_or(Vec3::Z);
    Mat3::from_cols(x, z.cross(x), z)
}

/// Woken where it stood (0x5B13A0 → 0x5ADB30): at rest, the span 5000 s,
/// then the post-step once (sim vtable +4, 0x5A8E80).
pub(super) fn wake(prop: &Prop) -> Body {
    let (_, rotation, _) = prop.rest.to_scale_rotation_translation();
    let rot = orthonormal(Mat3::from_quat(rotation.normalize()));
    let centre = prop.rest.w_axis.truncate() + rot * prop.info.centre_of_mass;
    let draw_offset = 0.0;
    let mut b = Body {
        centre,
        rot,
        momentum: Vec3::ZERO,
        angular_momentum: Vec3::ZERO,
        rest_damping: 0.0,
        velocity: Vec3::ZERO,
        spin: Vec3::ZERO,
        force: Vec3::ZERO,
        gravity: GRAVITY,
        break_left: prop.info.break_impulse,
        moving: false,
        settle: 0,
        knocked: false,
        splashed: false,
        records: Vec::new(),
        since_car: f32::MAX,
        walls: Vec::new(),
        start: (centre, rot, Vec3::ZERO),
        span: 5000.0,
        ground: GroundQuery::default(),
        pairs: HashMap::default(),
        car_first: HashMap::default(),
        made: HashMap::default(),
        draw_offset,
        sounds: HashMap::default(),
    };
    post_step(prop, &mut b);
    b
}

/// Before it is placed (0x5B147E) a woken prop is lifted clear of the
/// ground under its placement (0x5E7E80: height h, normal n): lift = h −
/// (z + (lowest + c.z)·n.z) + 0.05 (0x7844F4), z the placement's height,
/// lowest the shape's lowest point about the centre of mass c (0x592A20;
/// the factor read as the normal's z from the call's out-argument:
/// inferred). A positive lift raises the body and is its draw offset.
pub(super) fn lift(world: &World, prop: &Prop, b: &mut Body) {
    if world.ground.is_empty() {
        return;
    }
    let origin = prop.rest.w_axis.truncate();
    let Some((h, n)) = world.collision().ground_at(origin) else { return };
    let Some(lowest) = prop.shape.iter().map(|p| p.z).reduce(f32::min) else { return };
    let lift = h - (origin.z + (lowest + prop.info.centre_of_mass.z) * n.z) + LIFT_CLEARANCE;
    if lift > 0.0 {
        b.centre.z += lift;
        b.start.0 = b.centre;
        b.draw_offset = lift;
    }
}

/// The props' material index of WATER_DRUM (`formats::smackable::MATERIALS`).
pub(super) const WATER_DRUM: usize = 5;

/// 0x5ADFA0.
pub(super) fn start_moving(b: &mut Body) {
    b.moving = true;
    b.knocked = true;
}

/// An impulse at `arm` from the centre of mass (vtable +0x50 → 0x59BC70):
/// P += J, L += r × J, no caps; v and ω follow.
pub(super) fn take(prop: &Prop, b: &mut Body, impulse: Vec3, arm: Vec3) {
    if !b.moving {
        return;
    }
    b.momentum += impulse;
    b.angular_momentum += arm.cross(impulse);
    b.velocity = b.momentum / prop.mass();
    b.spin = (prop.inverse_inertia(b.rot) * b.angular_momentum).clamp_length_max(250.0);
}

/// A record refreshed by a push (0x5A9630).
pub(super) fn set_record(b: &mut Body, key: Partner) {
    match b.records.iter_mut().find(|r| r.0 == key) {
        Some(r) => r.1 = 0.0,
        None => b.records.push((key, 0.0)),
    }
}

/// Records end 0.3 s after their last push (0x592BE0); one with a car
/// marks the time (0x59E540).
pub(super) fn age_records(b: &mut Body, dt: f32) {
    b.since_car += dt;
    for r in &mut b.records {
        r.1 += dt;
    }
    let before = b.records.len();
    let car_ended = b.records.iter().any(|r| r.1 > RECORD_LIFE && matches!(r.0, Partner::Car(_)));
    b.records.retain(|r| r.1 <= RECORD_LIFE);
    let records = &b.records;
    b.sounds.retain(|p, _| records.iter().any(|r| r.0 == *p));
    if car_ended && b.records.len() < before {
        b.since_car = 0.0;
    }
}

/// The prop's frame (0x5B1DE0): the start pose saved (0x593220), then the
/// parts. (The quadratic drag −k·|v|·v before each part never acts: k,
/// prop +0x4F0, is never written and the pool, flags 0xB, hands out zeroed
/// slots.)
pub(super) fn frame(prop: &Prop, b: &mut Body, dt: f32) {
    b.start = (b.centre, b.rot, b.velocity);
    b.span = dt;
    simulate(prop, b, dt);
    // The draw offset eases out at 0.2 m/s (0x7FC85C) while the prop moves
    // with its break impulse spent (0x5B201F).
    if b.moving && b.break_left == 0.0 {
        b.draw_offset = (b.draw_offset - DRAW_OFFSET_RATE * dt).max(0.0);
    }
}

pub(super) fn simulate(prop: &Prop, b: &mut Body, dt: f32) {
    if !b.moving || dt <= 0.0 {
        return;
    }
    let parts = (dt / PROP_STEP) as u32 + 1;
    let h = dt / parts as f32;
    for _ in 0..parts {
        euler(b, h);
        b.rot = orthonormal(b.rot);
        post_step(prop, b);
    }
    // "No activity" (0x5A8DA0): still, on something, and neither a car
    // contact for 0.1 s nor a push from the oldest record for 0.025 s.
    if b.since_car > SINCE_CAR
        && b.records.first().is_some_and(|r| r.1 > RECORD_QUIET)
        && b.velocity.length() < STILL_SPEED
        && b.spin.length() < STILL_SPEED
    {
        b.moving = false;
    }
}

/// One part (0x5AA490 with 0x59C1F0): every derivative from the state as
/// it stands, then state += h·derivative. The prop's rigid body has no top
/// speed and no yaw limit (+0x204, +0x200 are 0), angular damping on
/// (+0x154 = 1) and rest damping on (+0x150, set each frame by 0x5B1DE0).
pub(super) fn euler(b: &mut Body, h: f32) {
    let speed = b.velocity.length();
    let spin = b.spin.length();
    let damping = sample(&ANGULAR_DAMPING, 0.0, 10.0, spin);
    let settle = (speed / 4.0).max(spin / 2.0);
    let rest_rate = if sample(&REST_DAMPING, 0.0, 1.0, settle) <= b.rest_damping { -10.0 } else { 0.1 };
    let dp = b.force - b.momentum * (8.0 * b.rest_damping);
    let dl = -b.angular_momentum * damping - b.angular_momentum * (2.0 * b.rest_damping);
    // d(axis) = ω × axis (0x59C130).
    let w = b.spin;
    let drot = Mat3::from_cols(w.cross(b.rot.x_axis), w.cross(b.rot.y_axis), w.cross(b.rot.z_axis));
    b.centre += b.velocity * h;
    b.rot += drot * h;
    b.momentum += dp * h;
    b.angular_momentum += dl * h;
    b.rest_damping += rest_rate * h;
}

/// The tidy-up (0x59C600): v = P/m (held to 200 m/s only over 0.7 t),
/// ω = I⁻¹L held to 250 rad/s, the rest damping not below 0, forces
/// cleared and gravity −m·g queued.
pub(super) fn tidy(prop: &Prop, b: &mut Body) {
    let m = prop.mass();
    b.velocity = b.momentum / m;
    if b.velocity.length() > 200.0 && m > 0.7 {
        b.momentum *= 200.0 / b.velocity.length();
        b.velocity = b.momentum / m;
    }
    b.spin = (prop.inverse_inertia(b.rot) * b.angular_momentum).clamp_length_max(250.0);
    b.rest_damping = b.rest_damping.max(0.0);
    b.force = if b.gravity > 0.0 { Vec3::new(0.0, 0.0, -m * b.gravity) } else { Vec3::ZERO };
}

/// The post-step (0x5A8E80): the tidy-up, then the settle test (0x59E440):
/// over 2 m/s or 2 rad/s, or with no live record and not settled before,
/// gravity (0); under that, the momenta × 0.95 (1); and under 0.8 with no
/// car contact for 0.1 s, stopped and weightless (2).
pub(super) fn post_step(prop: &Prop, b: &mut Body) {
    tidy(prop, b);
    let (v, w) = (b.velocity.length(), b.spin.length());
    let touching = !b.records.is_empty() || b.settle == 2;
    b.settle = if v <= SETTLE_SPEED && w <= SETTLE_SPEED && touching {
        if v <= REST_SPEED && w <= REST_SPEED && b.since_car >= SINCE_CAR { 2 } else { 1 }
    } else {
        0
    };
    match b.settle {
        0 => b.gravity = GRAVITY,
        1 => {
            b.momentum *= SETTLE_DAMPING;
            b.angular_momentum *= SETTLE_DAMPING;
            b.gravity = GRAVITY;
            tidy(prop, b);
        }
        _ => {
            b.momentum = Vec3::ZERO;
            b.angular_momentum = Vec3::ZERO;
            b.gravity = 0.0;
            tidy(prop, b);
        }
    }
}

/// Back to `t` seconds before the end of the span (0x593640): pose and
/// velocity blended toward the start; the angular momentum stays.
pub(super) fn back_to(prop: &Prop, b: &mut Body, t: f32) {
    let f = if b.span > 1e-6 { (t / b.span).min(1.0) } else { 0.0 };
    b.centre = b.centre.lerp(b.start.0, f);
    b.rot = orthonormal(lerp_mat(b.rot, b.start.1, f));
    b.velocity = b.velocity.lerp(b.start.2, f);
    b.momentum = b.velocity * prop.mass();
}

pub(super) fn transform(prop: &Prop, b: &Body) -> Mat4 {
    let origin = b.centre - b.rot * prop.info.centre_of_mass - Vec3::Z * b.draw_offset;
    Mat4::from_cols(b.rot.x_axis.extend(0.0), b.rot.y_axis.extend(0.0), b.rot.z_axis.extend(0.0), origin.extend(1.0))
}

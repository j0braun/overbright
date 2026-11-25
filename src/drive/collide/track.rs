use crate::drive::*;

impl CarState {
    /// The track polygons the car may touch this step (0x5B23C0 →
    /// 0x5AE940): from the car's polygon cache, those whose bounds meet the
    /// body's box (its position, the centre of mass, ± radius) and that
    /// pass the pre-test 0x5937C0 —
    /// the polygon's plane passes within the body's slab (radius across,
    /// the hull's height above or below along the car's up, 0.05 m added)
    /// and its outline lies within the radius in x/y — the first 48, in face
    /// order. Each with its key, corners, normal, whether it is a wall and
    /// its box's high x (the body list's sort, 0x5B23C0), and whether its
    /// contact turns the normal (X+0x64: the face's byte 0 is nonzero). A
    /// track without collision packs uses its render floors and walls, which
    /// have no face records; they keep the turn.
    pub(in crate::drive) fn contact_polygons(
        &mut self,
        car: &Car,
        world: &World,
        start: &Pose,
    ) -> Vec<((usize, usize), Vec<Vec3>, Vec3, bool, f32, bool)> {
        if world.ground.is_empty() {
            let (end, begin) = (Self::hull_at(car, &self.pose()), Self::hull_at(car, start));
            let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
            for p in end.iter().chain(&begin) {
                lo = lo.min(p.truncate());
                hi = hi.max(p.truncate());
            }
            return world
                .collision()
                .triangles_near(lo - 0.1, hi + 0.1)
                .into_iter()
                .map(|(i, c, n, wall)| {
                    ((usize::MAX, i), c.to_vec(), n, wall, c.iter().fold(f32::MIN, |m, p| m.max(p.x)), true)
                })
                .collect();
        }
        // The body's reach (0x5AE530): radius and heights over the hull,
        // about the centre of mass, the body's position (0x593B50).
        let crate::car::HullReach { radius, top, bottom } = car.body_reach();
        let (above, below) = (top + 0.05, 0.05 - bottom);
        let com = Self::com_at(car, &self.pose());
        let at = com.truncate();
        let polygons = world.ground.nearby(
            &world.visibility,
            &mut self.ground,
            at - radius,
            at + radius,
            self.velocity.truncate(),
        );
        let up = self.orientation * Vec3::Z;
        let mut picked = Vec::new();
        for poly in polygons {
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
            let dist = com.dot(poly.normal) - poly.d;
            let height = if c * dist < 0.0 { above } else { below };
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
        // Pairs end with their track polygon; their place in the pair list
        // stays while the record lives.
        self.pairs.retain(|k, _| picked.iter().any(|p| p.id == *k));
        let records = &self.records;
        self.pair_made.retain(|k, _| records.contains_key(k) || picked.iter().any(|p| p.id == *k));
        picked
            .into_iter()
            .map(|p| (p.id, p.corners[..p.count].to_vec(), p.normal, p.flags & 0x02 != 0, p.max.x, p.turn))
            .collect()
    }

    /// Give the track pairs not yet in the pair list their places, in the
    /// body list's order (by box high x; 0x5B23C0's sweep).
    pub(in crate::drive) fn stamp_track_pairs(
        &mut self,
        polygons: &[((usize, usize), Vec<Vec3>, Vec3, bool, f32, bool)],
    ) {
        let mut new: Vec<_> =
            polygons.iter().filter(|p| !self.pair_made.contains_key(&p.0)).map(|p| (p.4, p.0)).collect();
        new.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, key) in new {
            self.pair_made.insert(key, next_pair());
        }
    }

    pub(crate) fn pose(&self) -> Pose {
        let com = match self.body_com {
            Some((p, q, c)) if p == self.position && q == self.orientation => Some(c),
            _ => None,
        };
        Pose { position: self.position, orientation: self.orientation, velocity: self.velocity, com }
    }

    /// The hull's points in the world for a pose ([`Car::hull_points`]).
    pub(crate) fn hull_at(car: &Car, pose: &Pose) -> Vec<Vec3> {
        car.hull_points(pose.position, Mat3::from_quat(pose.orientation))
    }

    /// The collision body's position for a pose: the centre of mass (the
    /// box of the body, +0x20..+0x2C, is centred on car +0x60), as the
    /// pair's dirty test 0x598630 compares it.
    pub(crate) fn com_at(car: &Car, pose: &Pose) -> Vec3 {
        pose.position + Mat3::from_quat(pose.orientation) * Vec3::from(car.car_type.handling.centre_of_mass)
    }

    /// The offset of the hull's fresh supports in the distance query for a
    /// pose: R·c ([`Car::hull_offset`], [`crate::narrow::Raw`]).
    pub(crate) fn raw_at(car: &Car, pose: &Pose) -> Vec3 {
        Mat3::from_quat(pose.orientation) * car.hull_offset()
    }

    /// The hull's side of a query for a pose: its offset ([`Self::raw_at`])
    /// and its vertex links.
    pub(crate) fn side_at<'a>(car: &'a Car, pose: &Pose) -> narrow::Side<'a> {
        narrow::Side::hull(Self::raw_at(car, pose), &car.hull_links)
    }

    /// The car's earliest track contact over the span just simulated
    /// (0x5B5060 → 0x5B3D30 / 0x5B4BF0 per polygon): the one furthest
    /// back in time, the first of equals in the pair list's order (its
    /// place comes with it). Touching a wall restarts the post-crash grip
    /// timer.
    pub(in crate::drive) fn track_event(
        &mut self,
        car: &Car,
        world: &World,
        start: &Pose,
        span: f32,
        resolved: &crate::fxhash::FxHashMap<(usize, usize), usize>,
        cache: &mut TrackCache,
    ) -> Option<TrackEvent> {
        // The polygons and the hulls depend only on the pose, the
        // span's start and its length, so they stand until a contact of
        // this car is resolved (the caller then clears the cache).
        // `contact_polygons` called again on the same pose returns the
        // same list and changes nothing (its ground query keeps its box,
        // its retains find nothing new to drop).
        let pose = self.pose();
        match &cache.at {
            Some(at) => debug_assert!(at.same(&pose, start, span), "TrackCache used after the car moved"),
            None => cache.at = Some(TrackCacheKey { pose, start: *start, span }),
        }
        if !cache.sorted {
            let mut polygons = match cache.polygons.take() {
                Some(p) => p,
                None => self.contact_polygons(car, world, start),
            };
            self.stamp_track_pairs(&polygons);
            polygons.sort_by_cached_key(|p| self.pair_made[&p.0]);
            cache.polygons = Some(polygons);
            cache.sorted = true;
        }
        let (end, begin) = cache.hulls.get_or_insert_with(|| (Self::hull_at(car, &pose), Self::hull_at(car, start)));
        let (end, begin) = (&*end, &*begin);
        let (com_end, com_begin) = (Self::com_at(car, &pose), Self::com_at(car, start));
        let (raw_end, raw_begin) = (Self::raw_at(car, &pose), Self::raw_at(car, start));
        let at = |t: f32| -> (Vec<Vec3>, Vec3, narrow::Side) {
            let f = if span > 1e-6 { t / span } else { 0.0 };
            (
                end.iter().zip(begin).map(|(e, b)| *e + (*b - *e) * f).collect(),
                com_end + (com_begin - com_end) * f,
                narrow::Side::hull(raw_end + (raw_begin - raw_end) * f, &car.hull_links),
            )
        };
        // The detection at the span's end: the hull in its own frame
        // (`narrow::Body`), so its supports rank and place their vertices as
        // 0x5AFBB0 does. A default box (no offset) has none.
        let offset = car.hull_offset();
        let side_end = narrow::Side::hull(raw_end, &car.hull_links);
        let side_end = if offset == Vec3::ZERO {
            side_end
        } else {
            side_end.with_body(narrow::Body { stored: &car.hull, offset, rot: Mat3::from_quat(pose.orientation), at: com_end })
        };
        let mut first: Option<TrackEvent> = None;
        for (key, corners, normal, wall, _, turn) in cache.polygons.as_ref().unwrap() {
            let (key, normal, wall, turn) = (*key, *normal, *wall, *turn);
            if resolved.get(&key).copied().unwrap_or(0) >= contact::MAX_PER_POLYGON {
                continue;
            }
            let pair = self.pairs.entry(key).or_default();
            let (d, on_hull, on_polygon) = narrow::distance_at(end, corners, com_end, Vec3::ZERO, [side_end, narrow::Side::NONE], pair);
            if d > contact::TOUCH {
                continue;
            }
            if wall {
                // Touching a wall restarts the post-crash grip timer
                // (body +0x140, set at 0x5B5060 through 0x592EE0).
                self.since_collision = 0.0;
                self.wall_contact = Some((on_hull + on_polygon) * 0.5);
            }
            let (t, stuck) = if d > contact::OVERLAP {
                (0.0, false)
            } else {
                time_of_impact(&at, corners, span, self.pairs.entry(key).or_default())
            };
            if first.as_ref().is_none_or(|f| t > f.t) {
                first = Some(TrackEvent {
                    t,
                    stuck,
                    key,
                    corners: corners.clone(),
                    normal,
                    wall,
                    turn,
                    made: self.pair_made[&key],
                });
            }
        }
        first
    }

    /// Resolve a track contact (0x5B2620): back to its moment, pushed out
    /// if it overlapped for the whole span, and the impulse (0x5AF0F0).
    pub(in crate::drive) fn resolve_track(&mut self, car: &Car, start: &Pose, span: f32, e: TrackEvent, sounds: &mut ContactSounds) {
        let h = &car.car_type.handling;
        let TrackEvent { t, stuck, key, corners, normal, wall, turn, .. } = e;
        let end = Self::hull_at(car, &self.pose());
        let begin = Self::hull_at(car, start);
        let f = if span > 1e-6 { t / span } else { 0.0 };
        let mut hull: Vec<Vec3> = end.iter().zip(&begin).map(|(e, b)| *e + (*b - *e) * f).collect();
        let mut at = Self::com_at(car, &self.pose());
        at += (Self::com_at(car, start) - at) * f;
        let raw_end = Self::raw_at(car, &self.pose());
        let raw = narrow::Side::hull(raw_end + (Self::raw_at(car, start) - raw_end) * f, &car.hull_links);
        self.back_to(start, t, span, Vec3::from(h.centre_of_mass));
        let rot = Mat3::from_quat(self.orientation);
        let com = self.com_world(Vec3::from(h.centre_of_mass));
        // The polygon's normal (0x593E00). It is turned toward the car only
        // when X+0x64 is set, that is, the face's byte 0 is nonzero (0x5AA203;
        // the gate at 0x593E34); otherwise the raw normal stays. The turn's A
        // is the mean of the face's corners (C+0x1E0, written by 0x5A1AA0 for
        // a track face; C+0x1D0 is 0). Its test has corners[0]'s sign on a
        // planar face (both points lie on its plane) and differs only on a
        // non-planar one. B stays the car's centre of mass: the native's
        // P+0x100 is still open (formats.md, Contacts → Impulse).
        let face_mean = corners.iter().fold(Vec3::ZERO, |s, c| s + *c) / corners.len() as f32;
        let normal = if turn && (com - face_mean).dot(normal) < 0.0 { -normal } else { normal };
        // The resolution moves a body by the push-out (0x5B2620 at 0x5B26D9 /
        // 0x5B2723) only when its vt+0x64 says it may (the push-out's own
        // tries too, 0x5B4588 / 0x5B45C8): for the car body 0x5AEE90 →
        // 0x5ED200, false while its mover (mode 1 or 3) has the at-rest
        // class 2 (mover vt+0x14, +0x60, the rest test's). A car asleep
        // against a wall therefore stays where it is, its stale overlap
        // and all (round 13).
        if stuck && self.rest != 2 {
            let offset = contact::push_out(&hull, &corners, at, normal, raw, self.pairs.entry(key).or_default());
            self.shift_body(offset, Vec3::from(h.centre_of_mass));
            at += offset;
            for p in &mut hull {
                *p += offset;
            }
        }
        let com = self.com_world(Vec3::from(h.centre_of_mass));
        // The contact point: midway between the pair's closest points as
        // they stand (0x594010 reads pair +0xC0 / +0xD0): 0x5B2620 measures
        // nothing after the time of impact's search or the push-out, so they
        // are that search's last query's — at speed taken at another moment
        // of the search than the one the car is moved back to (read under
        // Wine: 2.4 mm apart along the wall at 120 km/h, docs/notes/parity/prop-hits.md).
        let pair = self.pairs.entry(key).or_default();
        let (on_hull, on_polygon) = match pair.as_ref().and_then(|p| p.last()) {
            Some((_, x, y)) => (x, y),
            None => {
                let (_, x, y) = narrow::distance_at(&hull, &corners, at, Vec3::ZERO, [raw, narrow::Side::NONE], pair);
                (x, y)
            }
        };
        let point = (on_hull + on_polygon) * 0.5;
        // The pair's contact record (0x5A9B20) is made before the impulse
        // is tried; an impulse refreshes it (0x5A9630).
        self.records.entry(key).or_insert(ContactRecord {
            age: 0.0,
            life: 0.0,
            point,
            upright: normal.z.abs() < 0.1,
            wall: normal.z.abs() < 0.7,
            floor: normal.z.abs() > 0.7,
            sounded: false,
            sound: (9, Vec3::ZERO),
            slide: 0.0,
            id: next_record(),
            impulse: 0.0,
        });
        let down = self.wheels_down();
        let (restitution, friction) = contact::materials(normal.z, down);
        let inertia = inertia_of(h, self.ground_inertia);
        let body = contact::Body {
            velocity: self.velocity,
            spin: self.spin,
            mass: h.mass,
            // 0x593A50 hands the impulse the rigid body's body-space
            // inverse inertia (+0x1B0 / +0x170) as it is, and 0x5AF0F0
            // applies it to the world-space r × n (0x5AF3E1): not turned.
            inverse_inertia: Box::new(move |v| v / inertia),
            up: rot * Vec3::Z,
            // A track contact's record never has the crash flag, so the
            // arm is not levelled.
            all_wheels_down: false,
            // 0x593A50 / 0x5A0B10 read the car's crash flag whatever the
            // partner.
            crash: self.crash,
        };
        let slide = slide_speed(self.velocity + self.spin.cross(point - com), normal);
        if crate::env_value_os!("NFSU2_SUBSTEP_DUMP")
            .is_some_and(|n| car.car_type.name.eq_ignore_ascii_case(&n.to_string_lossy()))
        {
            let v = self.velocity + self.spin.cross(point - com);
            eprintln!(
                "SUBSTEP TRACK-IN key {key:?} t {t:.5} span {span:.5} v {:?} w {:?} com {:?} vn {}",
                self.velocity,
                self.spin,
                com,
                v.dot(normal)
            );
        }
        if let Some((linear, angular)) = contact::impulse(&body, point - com, normal, restitution, friction) {
            // Every push's contact effects (0x5A9630 → 0x5DC4D0): the pair's
            // surface against the car's (28). **Inferred**: a wall is
            // SOLID_WALL (20) and a floor ROAD (1); the face's own surface
            // byte is not carried to the contact. Event 6 ramps on the
            // push's sliding speed (record +0x40).
            let face = if wall { 20 } else { 1 };
            crate::particles::post_contact(
                crate::particles::pair_surface(crate::particles::CAR_SURFACE, face),
                point,
                slide,
                linear.length(),
                self.velocity,
                if (com - point).dot(normal) < 0.0 { -normal } else { normal },
            );
            // The push's |J| (0x5A9630's argument: the impulse at 1.85 t,
            // before 0x5A0B10 scales it to the car) adds to record +0x60.
            let push = linear.length() / if h.mass < 1.85 { h.mass / 1.85 } else { 1.0 };
            let mut hit = None;
            if let Some(r) = self.records.get_mut(&key) {
                r.age = 0.0;
                r.point = point;
                r.slide = slide;
                r.impulse += push;
                hit = Some((r.impulse, r.life, r.upright));
                // 0x4814E0 then 0x4792D0: the scrape's arming (audio::scrape).
                sounds.pushes.post(crate::audio::car::pushes::Push {
                    record: r.id,
                    class: crate::audio::car::pushes::Class::Track,
                    slide,
                    at: point,
                });
                // The record's sound (0x5A9630 → 0x4797B0): its first push
                // picks the kind and volume (audio::collision).
                if !r.sounded {
                    use crate::audio::world::impacts as sound;
                    r.sounded = true;
                    // A new hit for the Rumble extra; not the underbody on
                    // a floor-like polygon while the car stands on its
                    // wheels (a kerb or a landing: the bumps' share).
                    let floor = r.floor && down > 0 && (rot * Vec3::Z).z > 0.9;
                    if !floor {
                        self.note_hit(push);
                    }
                    let kind = sound::track_kind(rot * Vec3::X, rot * Vec3::Z, normal);
                    let dir = linear.normalize_or_zero();
                    let volume = sound::tipped(sound::volume(linear.length(), kind), (rot * Vec3::Z).z, down);
                    // 0x46ABA0: not while a started sound along a wall of
                    // this car ran within 0.8 of this push.
                    let live = self.records.iter().filter(|(k, r)| **k != key && r.sounded).map(|(_, r)| r.sound);
                    if !sound::repeats_wall(dir, live) {
                        sounds.impacts.post_impact(sound::Impact {
                            at: point,
                            kind,
                            volume,
                            material: 0,
                            crash_camera: self.crash_camera,
                            splash: false,
                        });
                    }
                    // The first update sets both whether or not it plays.
                    if let Some(r) = self.records.get_mut(&key) {
                        r.sound = (kind, dir);
                    }
                }
            }
            if let Some((impulse, life, upright)) = hit {
                self.style_contact(impulse, life, upright, None);
            }
            let magnitude = push.min(40.0);
            self.hardest_impulse = self.hardest_impulse.max(magnitude);
            self.check_totaled(h, magnitude);
            self.velocity += linear / h.mass;
            self.angular_momentum += angular;
            self.spin_from_momentum(h);
            if crate::env_value_os!("NFSU2_SUBSTEP_DUMP")
                .is_some_and(|n| car.car_type.name.eq_ignore_ascii_case(&n.to_string_lossy()))
            {
                eprintln!(
                    "SUBSTEP TRACK key {key:?} t {t:.5} span {span:.5} stuck {stuck} wall {wall} normal {normal:?} arm {:?} point {point:?} dv {:?} dL {angular:?} v {:?}",
                    point - com,
                    linear / h.mass,
                    self.velocity
                );
            }
        }
    }

    /// A wheel's compression at its rest point `rest` (0x59D260): the
    /// ground under it (the track's height query plus the surface's bump at
    /// the rest point, 0x59D2BC, from the query's own surface) measured
    /// along the car's `up`, plus the radius; −10000 and no normal when
    /// there is no ground or it faces away (n·up < 0.001). Also the
    /// query's surface (1 without ground, 0x5E72F0).
    pub(in crate::drive) fn wheel_compression(
        &mut self,
        world: &World,
        rest: Vec3,
        up: Vec3,
        radius: f32,
    ) -> (f32, Option<Vec3>, u8) {
        let ground = world.ground_for(&mut self.ground, rest);
        let surface = ground.map_or(1, |g| g.2);
        match ground {
            Some((z, n, g)) if n.dot(up) >= 0.001 => {
                let ground = Vec3::new(rest.x, rest.y, z + surface_bump(rest.x, rest.y, g));
                ((ground - rest).dot(n) / n.dot(up) + radius, Some(n), surface)
            }
            _ => (-10000.0, None, surface),
        }
    }

    /// The suspension refresh of a contact's resolution (0x5B2620 calls the
    /// body's vt+0x38, 0x593710, before and after the impulse: the mover's
    /// vt+4, 0x5B04E0, runs every component's update while the sub-step
    /// length 0x7FC800 is 0, as it is outside the movers' sub-step loops,
    /// 0x5AB51A). The suspension update 0x5A87A0 then stores the
    /// compression at the pose the car was moved back to (+0x1C, clamped to
    /// the travel) but leaves its rate (+0x20 is written only when the
    /// sub-step is over 0.0001 s), so the next sub-step's damper sees the
    /// compression change from that pose, not from the end of the span.
    pub(crate) fn refresh_suspension(&mut self, car: &Car, world: &World) {
        let rot = Mat3::from_quat(self.orientation);
        let up = rot * Vec3::Z;
        let h = &car.car_type.handling;
        for i in 0..4 {
            let w = &car.car_type.wheels[i];
            let s = &h.suspension[i / 2];
            let (x, _, _) =
                self.wheel_compression(world, self.position + rot * self.mount(w.position, i), up, w.radius);
            self.wheels[i].compression = x.clamp(s.max_extension, s.max_compression);
        }
        // Measured outside the sub-step loop (0x7FC800 = 0): the rate stays.
        self.last_sub = 0.0;
    }

    /// Back to a moment in the span (0x593640): `t` seconds before the
    /// span's end. The centre of mass is put at the pose 0x593290 gives
    /// (**verified**): start + (end − start) · g with g = (span − t) / span,
    /// the start being the body's saved +0xB0, in f32 in that order
    /// (0x43D0C0: g · d, then + start); the orientation and velocity are
    /// blended toward the start by t / span; the angular momentum stays.
    pub(crate) fn back_to(&mut self, start: &Pose, t: f32, span: f32, com_local: Vec3) {
        let f = if span > 1e-6 { t / span } else { 0.0 };
        let end_com = self.com_world(com_local);
        let start_com = start.com.unwrap_or_else(|| start.position + Mat3::from_quat(start.orientation) * com_local);
        let g = if span > 1e-6 { (span - t) / span } else { 1.0 };
        let d = end_com - start_com;
        let com = Vec3::new(g * d.x + start_com.x, g * d.y + start_com.y, g * d.z + start_com.z);
        self.orientation = self.orientation.lerp(start.orientation, f).normalize();
        self.velocity = self.velocity.lerp(start.velocity, f);
        self.set_com_world(com, com_local);
    }

    /// The style manager's contact callback's gate (0x56FB30, from each
    /// push 0x5A9630 through 0x5A9961): a record whose pushes sum over 3
    /// (record +0x60, 0x79DB34) and that was made at most 0.1 s ago
    /// (record +0x08, 0x79DB28); a push on a track polygon that is not
    /// upright (0x5A0170) while a wheel is down and the car upright (up.z
    /// over 0.9, 0x7850F0) does not count. It notes the hit (misc +0x2C /
    /// +0x30; kind +0x34 = 3 unless car against car, +0x38 a traffic
    /// car's) and hands it to the style detectors. `car`: the other body
    /// is a car, and whether either car is traffic (owner kind 3).
    pub(crate) fn style_contact(&mut self, strength: f32, life: f32, upright: bool, car: Option<bool>) {
        if strength <= 3.0 || life > 0.1 {
            return;
        }
        let up = Mat3::from_quat(self.orientation) * Vec3::Z;
        if !upright && car.is_none() && self.wheels_down() > 0 && up.z > 0.9 {
            return;
        }
        let traffic = car == Some(true);
        self.last_hit = Some((0.0, strength, car.is_none() || traffic));
        if self.misc_style {
            self.style_hits.push(crate::style::Hit { strength, car: car.is_some(), traffic });
        }
    }

    /// The radius of the body's box (0x5AE530): the farthest hull point
    /// from the centre of mass ([`Car::body_reach`]).
    pub(in crate::drive) fn radius(car: &Car) -> f32 {
        car.body_reach().radius
    }
}

/// A car's track contact inputs within one frame's contact step:
/// its polygons (from the pairing sweep, then stamped and in the pair
/// list's order) and its hulls at the span's end and start. Valid while
/// the car's pose, span start and span length stand: cleared whenever one
/// of its contacts is resolved.
#[derive(Default)]
pub(in crate::drive) struct TrackCache {
    pub(in crate::drive) polygons: Option<Vec<((usize, usize), Vec<Vec3>, Vec3, bool, f32, bool)>>,
    sorted: bool,
    hulls: Option<(Vec<Vec3>, Vec<Vec3>)>,
    /// What the cache was made at (checked in debug builds).
    pub(in crate::drive) at: Option<TrackCacheKey>,
}

pub(in crate::drive) struct TrackCacheKey {
    pub(in crate::drive) pose: Pose,
    pub(in crate::drive) start: Pose,
    pub(in crate::drive) span: f32,
}

impl TrackCacheKey {
    /// Bit for bit.
    pub(in crate::drive) fn same(&self, pose: &Pose, start: &Pose, span: f32) -> bool {
        let bits = |p: &Pose| {
            (
                p.position.to_array().map(f32::to_bits),
                p.orientation.to_array().map(f32::to_bits),
                p.velocity.to_array().map(f32::to_bits),
            )
        };
        bits(&self.pose) == bits(pose) && bits(&self.start) == bits(start) && self.span.to_bits() == span.to_bits()
    }
}

/// A track contact found for the car this frame.
pub(in crate::drive) struct TrackEvent {
    pub(in crate::drive) t: f32,
    stuck: bool,
    pub(in crate::drive) key: (usize, usize),
    corners: Vec<Vec3>,
    normal: Vec3,
    wall: bool,
    /// X+0x64 of the contact: whether 0x593E00 turns the normal.
    turn: bool,
    /// The pair's place in the pair list.
    pub(in crate::drive) made: u64,
}

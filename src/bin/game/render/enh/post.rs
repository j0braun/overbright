use crate::*;

/// The drawn cars' motion inputs for the temporal pass: each car's
/// drawn pose, box and wheels, keyed by its slot (the player 0, the
/// companion 1, rivals 100+, encounter cars 200+, traffic 1000+), the
/// nearest first; from the frame's camera and cars as drawn.
pub(crate) fn temporal_cars(
    d: &Drive,
    camera: &Camera,
    poses: &render::snapshot::FramePoses,
    companion: Option<(&Car, &CarState)>,
    race: Option<&Race>,
    encounters: Option<&Encounters>,
    traffic: Option<&Traffic>,
) -> Vec<nfsu2::render::temporal::Car> {
    let car = |key: u64, car: &Car, state: &CarState| {
        let wheels = std::array::from_fn(|i| {
            let w = &car.car_type.wheels[i];
            (w.position, w.radius * 1.1, w.width * 0.5 + 0.05)
        });
        nfsu2::render::temporal::Car::new(key, state.transform(), car.shadow_box, wheels)
    };
    let mut out = vec![car(0, &d.car, poses.player())];
    out.extend(companion.map(|(c, s)| car(1, c, s)));
    out.extend(
        race.into_iter().flat_map(|r| r.racers.rivals.iter().enumerate().map(|(i, r)| car(100 + i as u64, &r.car, poses.car(&r.id, &r.state)))),
    );
    out.extend(encounters.into_iter().flat_map(|e| {
        e.cars.iter().enumerate().filter(|(_, c)| c.active).map(|(i, c)| car(200 + i as u64, &*c.car, poses.car(&c.id, &c.state)))
    }));
    out.extend(traffic.into_iter().flat_map(|t| {
        t.cars.iter().enumerate().filter(|(_, c)| crate::render::frame::traffic_drawn(c)).map(|(i, c)| car(1000 + i as u64, &c.car, poses.car(&c.id, &c.state)))
    }));
    let eye = camera.position;
    out.sort_by(|a, b| {
        a.transform
            .w_axis
            .truncate()
            .distance_squared(eye)
            .total_cmp(&b.transform.w_axis.truncate().distance_squared(eye))
    });
    out
}

/// The drawn cars for the motion blur (docs/notes/graphics/motionblur-dof.md), as
/// [`temporal_cars`] (same keys and boxes) with the player's marked and
/// each wheel's spin (rad/s; a car whose wheels hold none, e.g. traffic,
/// rolls them at its speed).
pub(crate) fn blur_cars(
    d: &Drive,
    camera: &Camera,
    poses: &render::snapshot::FramePoses,
    companion: Option<(&Car, &CarState)>,
    race: Option<&Race>,
    encounters: Option<&Encounters>,
    traffic: Option<&Traffic>,
) -> Vec<nfsu2::render::blur_dof::BlurCar> {
    let mut states: Vec<(u64, &Car, &CarState)> = vec![(0, &d.car, poses.player())];
    states.extend(companion.map(|(c, s)| (1, c, s)));
    states.extend(
        race.into_iter().flat_map(|r| r.racers.rivals.iter().enumerate().map(|(i, r)| (100 + i as u64, &r.car, poses.car(&r.id, &r.state)))),
    );
    states.extend(encounters.into_iter().flat_map(|e| {
        e.cars.iter().enumerate().filter(|(_, c)| c.active).map(|(i, c)| (200 + i as u64, &*c.car, poses.car(&c.id, &c.state)))
    }));
    states.extend(traffic.into_iter().flat_map(|t| {
        t.cars.iter().enumerate().filter(|(_, c)| crate::render::frame::traffic_drawn(c)).map(|(i, c)| (1000 + i as u64, &*c.car, poses.car(&c.id, &c.state)))
    }));
    let spins: HashMap<u64, [f32; 4]> = states
        .iter()
        .map(|(k, car, s)| {
            let mut spin = std::array::from_fn(|i| s.wheels[i].spin);
            if spin.iter().all(|w| *w == 0.0) && s.speed().abs() > 0.5 {
                spin = std::array::from_fn(|i| s.speed() / car.car_type.wheels[i].radius.max(0.1));
            }
            (*k, spin)
        })
        .collect();
    temporal_cars(d, camera, poses, companion, race, encounters, traffic)
        .into_iter()
        .map(|mut c| {
            // The floor higher than the temporal pass's (12 cm over the tyres' contact):
            // the road just around a car keeps the camera's motion.
            let ground = c.wheels.iter().map(|(p, r, _)| p.z - r).fold(f32::INFINITY, f32::min);
            if ground.is_finite() {
                c.lo.z = c.lo.z.max(ground + nfsu2::render::blur_dof::FLOOR);
            }
            nfsu2::render::blur_dof::BlurCar {
                car: c,
                player: c.key == 0,
                spin: spins.get(&c.key).copied().unwrap_or_default(),
            }
        })
        .collect()
}

/// The clone's motion blur (Camera / Per-Object) and depth of field
/// (Cinematic, scenes only) over the world's scene, before the additive
/// layers. `still`: a single shot, its last frame made from the
/// velocities that long before. Nothing (no pass, no allocation) when
/// both are off.
#[allow(clippy::too_many_arguments)]
pub(crate) fn blur_and_dof(
    post: &mut PostFx,
    gpu: &Gpu,
    encoder: &mut wgpu::CommandEncoder,
    scene: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    samples: u32,
    size: (u32, u32),
    d: &Drive,
    camera: &Camera,
    poses: &render::snapshot::FramePoses,
    aspect: f32,
    others: (Option<(&Car, &CarState)>, Option<&Race>, Option<&Encounters>, Option<&Traffic>),
    key: u64,
    mover: u32,
    time: f64,
    still: Option<f64>,
    unjittered: bool,
    opts: &nfsu2::render::RenderOptions,
) {
    use nfsu2::render::blur_dof::{self, BlurDof, DofFrame, Last, MotionFrame, MotionMode};
    // No motion blur in photo mode (the world is paused; the photo
    // camera's own moves are no motion).
    let mode = if blur_dof::photo_dof().is_some() { MotionMode::Original } else { MotionMode::of(opts) };
    let dof = blur_dof::dof_wanted(mover, opts);
    if blur_dof_idle(mode, dof) {
        if let Some(b) = post.blur_dof.as_mut() {
            b.reset();
        }
        return;
    }
    let format = post.scene_format();
    let b = post.blur_dof.get_or_insert_with(|| BlurDof::new(gpu));
    // The passes' pipelines built the first frame they are wanted.
    b.warm(gpu, format, samples, mode != MotionMode::Original, blur_dof::dof_on(opts));
    // (`reversed`: the frame's Reversed-Z.)
    let reversed = opts.reversed_z;
    let (companion, race, encounters, traffic) = others;
    let cars = blur_cars(d, camera, poses, companion, race, encounters, traffic);
    // After FSR 2 the scene is the unjittered display image.
    let view = if unjittered { camera.unjittered() } else { *camera };
    if mode != MotionMode::Original {
        let assumed_last = still.map(|dt| {
            let mut camera = view;
            camera.position -= d.motion.velocity() * dt as f32;
            let by_key = |k: u64| -> Option<&CarState> {
                match k {
                    0 => Some(poses.player()),
                    1 => companion.map(|(_, s)| s),
                    100..200 => race.and_then(|r| r.racers.rivals.get((k - 100) as usize)).map(|r| poses.car(&r.id, &r.state)),
                    200..1000 => encounters.and_then(|e| e.cars.get((k - 200) as usize)).map(|c| poses.car(&c.id, &c.state)),
                    _ => traffic.and_then(|t| t.cars.get((k - 1000) as usize)).map(|c| poses.car(&c.id, &c.state)),
                }
            };
            let cars = cars
                .iter()
                .filter_map(|c| {
                    by_key(c.car.key)
                        .map(|s| (c.car.key, blur_dof::pose_before(c.car.transform, s.velocity, s.spin, dt as f32)))
                })
                .collect();
            Last { key, camera: camera.unjittered(), aspect, cars, time: time - dt }
        });
        // The motion from the ticks (`render::enh::blur_ticks`), in this
        // frame's view and size.
        let tick = poses.blur.clone().map(|mut t| {
            if let Some(l) = t.last.as_mut() {
                l.key = key;
                l.aspect = aspect;
            }
            t
        });
        let frame = MotionFrame { mode, camera: view, aspect, reversed, cars: cars.clone(), key, time, assumed_last, tick };
        b.motion(gpu, encoder, scene, depth, samples, size, format, &frame);
    } else {
        b.reset();
    }
    if dof {
        let boxes: Vec<(glam::Mat4, Vec3, Vec3)> = cars.iter().map(|c| (c.car.transform, c.car.lo, c.car.hi)).collect();
        // The player's car first (the fallback focus).
        let mut ordered = boxes.clone();
        if let Some(p) = cars.iter().position(|c| c.player) {
            ordered.swap(0, p);
        }
        if let Some(i) = blur_dof::focus_car(&view, &ordered) {
            let (t, lo, hi) = ordered[i];
            let focus = blur_dof::photo_band(blur_dof::focus_band(view.position, t, lo, hi));
            b.dof(
                gpu,
                encoder,
                scene,
                depth,
                samples,
                size,
                format,
                &DofFrame { camera: view, aspect, reversed, focus },
            );
        }
    }
}

/// Neither the clone's motion blur (the Original tier) nor its depth of
/// field this frame: [`blur_and_dof`] returns before anything is made or
/// drawn (no pass, no allocation).
pub(crate) fn blur_dof_idle(mode: nfsu2::render::blur_dof::MotionMode, dof: bool) -> bool {
    mode == nfsu2::render::blur_dof::MotionMode::Original && !dof
}

/// What draws the view, for the temporal pass's history: the mode,
/// the camera mover and whether a shop / NIS scene runs. A change is a
/// cut.
pub(crate) fn temporal_key(mode: &Mode, mover: u32, scene: bool) -> u64 {
    let kind: u64 = match mode {
        Mode::Explore => 1,
        Mode::Race(_) => 2,
        Mode::Menu(_) => 3,
        #[allow(unreachable_patterns)]
        _ => 4,
    };
    kind | u64::from(mover) << 8 | u64::from(scene) << 40
}

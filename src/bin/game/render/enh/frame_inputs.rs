use crate::*;

/// The Graphics page's ambient occlusion for a frame of the game's scene
/// drawn from `camera` into `depth` (none with the row off, native).
/// `occluders`: the cars ([`car_occluders`]), whose underglow the
/// occlusion gives way to.
pub(crate) fn ssao_frame(
    camera: &nfsu2::render::camera::Camera,
    aspect: f32,
    depth: &wgpu::TextureView,
    samples: u32,
    fog: Option<nfsu2::fog::Fog>,
    occluders: &[nfsu2::render::lamps::Occluder],
    opts: &nfsu2::render::RenderOptions,
) -> Option<nfsu2::render::ssao::Frame> {
    let strength = opts.ssao;
    (strength > 0.0).then(|| nfsu2::render::ssao::Frame {
        depth: depth.clone(),
        samples,
        near: camera.near,
        far: camera.far,
        reversed: opts.reversed_z,
        scale: camera.projection_scale(aspect),
        fog: fog.map(|f| f.value),
        strength,
        glows: nfsu2::render::ssao::glow_boxes(camera.view(), occluders),
    })
}

/// The Graphics page's screen-space reflections for a frame of the game's
/// scene drawn from `camera` into `depth`, with the road's mask the world
/// drew (none with the row off or no mask, native).
pub(crate) fn ssr_frame(
    camera: &nfsu2::render::camera::Camera,
    aspect: f32,
    depth: &wgpu::TextureView,
    samples: u32,
    mask: Option<nfsu2::render::ssr::Mask>,
    opts: &nfsu2::render::RenderOptions,
) -> Option<nfsu2::render::ssr::Frame> {
    let level = opts.ssr;
    let mask = mask.filter(|_| level != 0)?;
    let forward = camera.forward().normalize();
    let right = forward.cross(camera.up).normalize();
    let up = right.cross(forward);
    Some(nfsu2::render::ssr::Frame {
        mask,
        depth: depth.clone(),
        samples,
        near: camera.near,
        far: camera.far,
        reversed: opts.reversed_z,
        scale: camera.projection_scale(aspect),
        basis: [right, up, forward],
        full: level >= 2,
        rt: None,
    })
}

/// With Modern Lighting's "Modern + RT" the scene is traced before
/// the world draws ([`prepare_modern_rt`]) and reused here (one build a
/// frame).
pub(crate) fn attach_rt(
    ssr: &mut Option<nfsu2::render::ssr::Frame>,
    gpu: &Gpu,
    world: &World,
    region: &str,
    player: (&Car, glam::Mat4),
    eye: Vec3,
    others: &[(&Car, glam::Mat4)],
    opts: &nfsu2::render::RenderOptions,
) {
    if let Some(f) = ssr.as_mut().filter(|_| opts.rt_reflections()) {
        if nfsu2::render::modern_light_bold::wants_rt(opts) {
            f.rt = nfsu2::render::modern_light_bold::frame_rt();
            return;
        }
        let mut cars = Vec::with_capacity(others.len() + 1);
        cars.push(player);
        cars.extend_from_slice(others);
        f.rt = nfsu2::render::rt::frame(gpu, world, region, &cars, eye);
    }
}

/// Modern + RT: this frame's traced scene (the world, the cars where
/// they are) for the world's main view (`modern_light_bold::set_frame_rt`).
pub(crate) fn prepare_modern_rt(
    gpu: &Gpu,
    world: &World,
    region: &str,
    player: (&Car, glam::Mat4),
    eye: Vec3,
    others: &[(&Car, glam::Mat4)],
    opts: &nfsu2::render::RenderOptions,
) {
    if !nfsu2::render::modern_light_bold::wants_rt(opts) {
        return;
    }
    let mut cars = Vec::with_capacity(others.len() + 1);
    cars.push(player);
    cars.extend_from_slice(others);
    nfsu2::render::modern_light_bold::set_frame_rt(nfsu2::render::rt::frame(gpu, world, region, &cars, eye));
}

/// The cars but the player's a ray may hit (as [`car_occluders`]
/// gathers them), the nearest to `eye` first; none unless Ray Traced
/// reflections are on.
pub(crate) fn rt_cars<'a>(
    eye: Vec3,
    poses: &'a render::snapshot::FramePoses,
    companion: Option<(&'a Car, &'a CarState)>,
    race: Option<&'a Race>,
    encounters: Option<&'a Encounters>,
    traffic: Option<&'a Traffic>,
    opts: &nfsu2::render::RenderOptions,
) -> Vec<(&'a Car, glam::Mat4)> {
    if !opts.rt_reflections() && !nfsu2::render::modern_light_bold::wants_rt(opts) {
        return Vec::new();
    }
    let mut out = Vec::new();
    out.extend(companion.map(|(c, s)| (c, s.transform())));
    out.extend(race.into_iter().flat_map(|r| r.racers.rivals.iter().map(|r| (&r.car, poses.car(&r.id, &r.state).transform()))));
    out.extend(
        encounters
            .into_iter()
            .flat_map(|e| e.cars.iter().filter(|c| c.active).map(|c| (&*c.car, poses.car(&c.id, &c.state).transform()))),
    );
    out.extend(
        traffic
            .into_iter()
            .flat_map(|t| t.cars.iter().filter(|c| c.lane.is_some()).map(|c| (&*c.car, poses.car(&c.id, &c.state).transform()))),
    );
    out.sort_by(|a, b| {
        a.1.w_axis.truncate().distance_squared(eye).total_cmp(&b.1.w_axis.truncate().distance_squared(eye))
    });
    out
}

/// Volumetric Fog (docs/atmosphere.md) for a frame of the game's scene
/// drawn from `camera` into `depth`: the one froxel medium at the row's tier
/// (Low, High, Next-Gen: the tier's grid, range and light counts), its lights
/// the nearest light-pack lamps and signs, the Next-Gen car lights' cones (or,
/// without them, the player's headlight) and the rain's density.
#[allow(clippy::too_many_arguments)]
pub(crate) fn volumetric_frame(
    r: &WorldRenderer,
    camera: &Camera,
    aspect: f32,
    depth: &wgpu::TextureView,
    samples: u32,
    fog: Option<nfsu2::fog::Fog>,
    pools: &[HeadlightPool],
    headlight: bool,
    rain: f32,
    opts: &nfsu2::render::RenderOptions,
) -> Option<nfsu2::render::volumetric::Frame> {
    use nfsu2::render::atmosphere as a;
    let tier = a::tier(opts.volumetric_fog)?;
    let camera = camera.unjittered();
    let layers = r.lamp_shadow_layers();
    let lights = a::pick_lights(
        r.lamp_list(),
        &nfsu2::render::modern_light_bold::loaded_emitters(),
        layers.as_ref(),
        camera.position,
        camera.forward(),
        &tier,
    );
    // Next-Gen car lights on: every drawn car's cone from its list (the medium is their air; their own
    // march does not run). Off: the player's headlight.
    let nx6 = opts.car_lights_next_gen;
    let spots: Vec<a::Spot> = if nx6 {
        let (cones, _) = nfsu2::render::car_lights::air_lights(
            &nfsu2::render::car_lights::cars(opts.car_lights_next_gen),
            camera.position,
            camera.forward(),
        );
        a::spots_from_car_lights(&cones, camera.position, layers.as_ref().and_then(|l| l.headlight), tier.spots)
    } else if headlight {
        pools
            .first()
            .and_then(|p| {
                a::headlight_spot(&nfsu2::render::headlight_uniform(Some(p)), layers.as_ref().and_then(|l| l.headlight))
            })
            .into_iter()
            .take(tier.spots)
            .collect()
    } else {
        Vec::new()
    };
    Some(nfsu2::render::volumetric::Frame {
        depth: depth.clone(),
        samples,
        camera,
        aspect,
        reversed: opts.reversed_z,
        next: Box::new(a::NextGen {
            tier,
            fog_color: fog.map_or(glam::Vec3::ZERO, |f| glam::Vec3::from(f.color)),
            rain,
            time: 0.0,
            lights,
            spots,
            spot_scale: if nx6 { a::NX6_SPOT_SCALE } else { 1.0 },
            smoke: None,
            shadows: layers.map(|l| (l.maps, l.views)),
        }),
    })
}

/// Next-Gen car lights (docs/car-lights.md): the headlight cones and
/// lamp halos in the air and the lens streaks for a frame drawn from
/// `camera` into `depth` (None with the tier off or no car lights).
#[allow(clippy::too_many_arguments)]
pub(crate) fn car_air_frame(
    r: &WorldRenderer,
    camera: &Camera,
    aspect: f32,
    depth: &wgpu::TextureView,
    samples: u32,
    fog: Option<nfsu2::fog::Fog>,
    rain: f32,
    time: f32,
    opts: &nfsu2::render::RenderOptions,
) -> Option<nfsu2::render::car_lights::Frame> {
    use nfsu2::render::car_lights as cl;
    if !opts.car_lights_next_gen {
        return None;
    }
    let camera = camera.unjittered();
    let (spots, points) = cl::air_lights(&cl::cars(opts.car_lights_next_gen), camera.position, camera.forward());
    if debug_env().carlights_log {
        eprintln!("car lights: {} cars, {} cones, {} lamps", cl::cars(opts.car_lights_next_gen).len(), spots.len(), points.len());
    }
    if spots.is_empty() && points.is_empty() {
        return None;
    }
    Some(cl::Frame {
        depth: depth.clone(),
        samples,
        camera,
        aspect,
        reversed: opts.reversed_z,
        fog: fog.map(|f| f.value),
        rain,
        time,
        spots,
        points,
        cookie: r.headlight_cookie(),
    })
}

/// Every drawn car's lights for this frame, docs/car-lights.md
/// (`render::car_lights`; nothing with the tier off): the light markers, the
/// headlight colour, the brake flag (car +0x944, held 0.5 s for the racers
/// as `glow_quads` reads it) and the underglow with a drawn `%s_NEON`.
pub(crate) fn publish_car_lights(
    d: &Drive,
    poses: &render::snapshot::FramePoses,
    companion: Option<(&Car, &CarState)>,
    race: Option<&Race>,
    encounters: Option<&Encounters>,
    traffic: Option<&Traffic>,
    seconds: f32,
    opts: &nfsu2::render::RenderOptions,
) {
    if !opts.car_lights_next_gen {
        return;
    }
    let one = |car: &Car, state: &CarState, holds: bool| {
        let neon = (car.neon.lit
            && car
                .texture_for(nfsu2::formats::hash::bin_hash_from(nfsu2::formats::hash::bin_hash(&car.name), "_NEON"))
                .is_some())
        .then(|| nfsu2::shadow::neon_rgb(&car.neon, seconds))
        .flatten();
        nfsu2::render::car_lights::car(
            state.transform(),
            &car.light_flares,
            car.headlight_pool_color,
            state.brake_lights(holds),
            car.shadow_box,
            neon,
        )
    };
    let mut out = vec![one(&d.car, poses.player(), false)];
    out.extend(companion.map(|(c, s)| one(c, s, false)));
    out.extend(race.into_iter().flat_map(|r| r.racers.rivals.iter().map(|r| one(&r.car, poses.car(&r.id, &r.state), true))));
    out.extend(
        encounters.into_iter().flat_map(|e| e.cars.iter().filter(|c| c.active).map(|c| one(&c.car, poses.car(&c.id, &c.state), true))),
    );
    out.extend(
        traffic
            .into_iter()
            .flat_map(|t| t.cars.iter().filter(|c| c.lane.is_some()).map(|c| one(&c.car, poses.car(&c.id, &c.state), false))),
    );
    nfsu2::render::car_lights::publish(out, opts.car_lights_next_gen);
}

/// The Next-Gen atmosphere's frame values from the drive (the clock,
/// the volumetric smoke's medium), after the frame's particles. `clock`:
/// the world clock as drawn (`render::clocks`).
pub(crate) fn atmosphere_drive(v: &mut Option<nfsu2::render::volumetric::Frame>, d: &Drive, clock: f32) {
    if let Some(next) = v.as_mut().map(|f| &mut f.next) {
        next.time = clock;
        next.smoke = d
            .effects
            .cinematic
            .medium()
            .map(|(density, origin, voxel, dims)| nfsu2::render::atmosphere::Smoke { density, origin, voxel, dims });
    }
}

/// Animated Water (docs/water.md): the world's main view leaves its
/// water draws to `render::water` while the row is on; this picks the
/// frame's water and writes its view (the eight light-pack lamps nearest
/// the eye, the car cube unless it is off, the world clock). Off: the
/// native draws, nothing else.
#[allow(clippy::too_many_arguments)]
pub(crate) fn water_prepare(
    gpu: &Gpu,
    water: &mut nfsu2::render::water::Water,
    world: &World,
    r: &mut WorldRenderer,
    camera: &Camera,
    aspect: f32,
    sections: Option<&[u32]>,
    fog: Option<nfsu2::fog::Fog>,
    env: bool,
    race_reversed: bool,
    clock: f32,
    lag: f32,
    rain: f32,
    opts: &nfsu2::render::RenderOptions,
) {
    let on = opts.animated_water;
    r.set_hide_water(on);
    if !on {
        return;
    }
    let eye = camera.position;
    let mut near: Vec<(f32, &nfsu2::render::lamps::Lamp)> = r
        .lamp_list()
        .iter()
        .map(|l| (l.light.position.distance(eye), l))
        .filter(|(d, l)| *d < 120.0 + l.light.outer)
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let lamps = near
        .into_iter()
        .take(8)
        .map(|(_, l)| {
            let g = nfsu2::render::lamps::gpu_lamp(&l.light, -1);
            (Vec3::from_slice(&g.position[..3]), g.position[3], Vec3::from_slice(&g.colour[..3]), g.colour[3])
        })
        .collect();
    let r = &*r;
    let view = nfsu2::render::water::View {
        camera: *camera,
        aspect,
        reversed_z: opts.reversed_z,
        race_reversed,
        sections,
        lod_b: r.main_lod_b(),
        clock,
        lag,
        fog: fog.map(|f| (f.value, f.color)),
        rain,
        lamps,
        env: env.then(|| (r.environment(), opts.enhanced_reflections)),
        min_size: nfsu2::render::main_min_size(opts),
        tier: opts.water_tier,
        modern: opts.modern_lighting > 0,
    };
    water.prepare(gpu, world, &|i| r.placement_hidden(i), &view);
}

/// The cars as boxes for Contact Shadows and Lamp Shadows (clone
/// deviations, docs/lamps-shadows.md), the player's first (the headlight's
/// own car); empty when both are off.
/// `seconds`: the neon's clock ([`nfsu2::shadow::neon_color`]).
pub(crate) fn car_occluders(
    d: &Drive,
    camera: &Camera,
    poses: &render::snapshot::FramePoses,
    companion: Option<(&Car, &CarState)>,
    race: Option<&Race>,
    encounters: Option<&Encounters>,
    traffic: Option<&Traffic>,
    seconds: f32,
    opts: &nfsu2::render::RenderOptions,
) -> Vec<nfsu2::render::lamps::Occluder> {
    let contact = opts.contact_shadows;
    // The underglow's light: for the contact shadow and SSAO.
    let glows = contact || opts.ssao > 0.0;
    if !glows && opts.lamp_shadows == 0 && !nfsu2::render::modern_light::enabled(opts) {
        return Vec::new();
    }
    // How much of the footprint the car's underglow lights (its
    // contact shadow gives way to it; 0 without a drawn `%s_NEON`).
    let glow = |car: &Car| {
        if !car.neon.lit
            || car
                .texture_for(nfsu2::formats::hash::bin_hash_from(nfsu2::formats::hash::bin_hash(&car.name), "_NEON"))
                .is_none()
        {
            return 0.0;
        }
        nfsu2::shadow::neon_color(&car.neon, seconds, 1.0).map_or(0.0, nfsu2::shadow::neon_light)
    };
    let occ = |car: &Car, state: &CarState| nfsu2::render::lamps::Occluder {
        transform: state.transform(),
        min: car.shadow_box.0,
        max: car.shadow_box.1,
        glow: if glows { glow(car) } else { 0.0 },
    };
    let mut out = vec![occ(&d.car, poses.player())];
    out.extend(companion.map(|(c, s)| occ(c, s)));
    out.extend(race.into_iter().flat_map(|r| r.racers.rivals.iter().map(|r| occ(&r.car, poses.car(&r.id, &r.state)))));
    out.extend(encounters.into_iter().flat_map(|e| e.cars.iter().filter(|c| c.active).map(|c| occ(&c.car, poses.car(&c.id, &c.state)))));
    out.extend(
        traffic.into_iter().flat_map(|t| t.cars.iter().filter(|c| c.lane.is_some()).map(|c| occ(&c.car, poses.car(&c.id, &c.state)))),
    );
    // The nearest first (the shaders take at most lamps::MAX_OCCLUDERS).
    let eye = camera.position;
    out[1..].sort_by(|a, b| {
        a.transform
            .w_axis
            .truncate()
            .distance_squared(eye)
            .total_cmp(&b.transform.w_axis.truncate().distance_squared(eye))
    });
    out
}

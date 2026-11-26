use crate::*;

/// Render info +0x364 / +0x37C this frame: the engine and trunk neon's
/// pulsed colours (0x622AA8 / 0x622BF8, only with the slot's part; else
/// the constructor's 0), for the level-3 engine / trunk audio glow.
pub(crate) fn part_glow(car: &Car, seconds: f32) -> [[u8; 4]; 2] {
    [&car.engine_neon, &car.trunk_neon]
        .map(|n| n.as_ref().map_or([0; 4], |(n, _)| nfsu2::shadow::neon_tube_color(n, seconds)))
}

/// The neon tubes of a car (0x615A20 → 0x60D800, from 0x61ACA0 after the
/// other glows): the underglow's, at `seconds` on the neon's clock.
/// The engine and trunk neon too (see the loop below); **not ported**:
/// the cabin neon (+0x800, +0x2CB0), which no shop list fits.
pub(crate) fn neon_tubes(
    view: &nfsu2::shadow::TubeView,
    car: &Car,
    transform: glam::Mat4,
    seconds: f32,
    open: [Option<f32>; 2],
    quads: &mut Vec<nfsu2::particles::Quad>,
) {
    let texture = nfsu2::formats::hash::bin_hash(nfsu2::shadow::NEON_TUBE_TEXTURE);
    let at = transform.w_axis.truncate();
    if !car.neon_tubes.is_empty() {
        let color = nfsu2::shadow::neon_tube_color(&car.neon, seconds);
        nfsu2::shadow::neon_tube_quads(
            view,
            &car.neon_tubes,
            at,
            transform,
            color,
            nfsu2::shadow::NEON_TUBE_WIDTH,
            true,
            1.0,
            texture,
            quads,
        );
    }
    // The engine neon (slot 0x9B) while the hood's channel is not shut
    // and the trunk neon (0x9D) while the trunk's is not (in the front
    // end, 0x8654A4 = 3, also shut, at fade 0: nothing): each at the
    // channel's open fraction (0x434610(channel, 0)), a thin tube in the
    // colour (0.0175 / 0.0225, 0x7A6740 / 0x7A6748), then a wide one
    // (× 10 / × 3.5, 0x7A6600 / 0x7A65F0) at alpha trunc(A · 0.3 / 0.35)
    // (0x7A6604 / 0x7A65F4), neither shortened.
    for (neon, fade, width, wide, alpha) in
        [(&car.engine_neon, open[0], 0.0175f32, 10.0f32, 0.3f32), (&car.trunk_neon, open[1], 0.0225, 3.5, 0.35)]
    {
        let (Some((n, tubes)), Some(fade)) = (neon, fade) else { continue };
        if tubes.is_empty() {
            continue;
        }
        let color = nfsu2::shadow::neon_tube_color(n, seconds);
        nfsu2::shadow::neon_tube_quads(view, tubes, at, transform, color, width, false, fade, texture, quads);
        let glow = [color[0], color[1], color[2], (f32::from(color[3]) * alpha) as i32 as u8];
        nfsu2::shadow::neon_tube_quads(view, tubes, at, transform, glow, wide * width, false, fade, texture, quads);
    }
}

/// The tube view of a camera (camera +0xC4: the field of view as a binary
/// angle).
pub(crate) fn tube_view(camera: &nfsu2::render::Camera, aspect: f32) -> nfsu2::shadow::TubeView {
    let axes = CameraAxes::look(camera.forward(), camera.up);
    let fov = (camera.game_fov.unwrap_or(camera.fov_y) * (65536.0 / std::f32::consts::TAU)) as u32 as u16;
    nfsu2::shadow::TubeView {
        eye: camera.position,
        right: axes.right,
        down: axes.down,
        view_projection: camera.view_projection(aspect),
        aspect,
        fov,
    }
}

/// This frame's glow sprites (0x5CBB10's 0x491860 and 0x61AEA0): the
/// street lamps of the visible sections' flare packs, then the head and
/// brake lights of the player, the race's rivals, career free roam's
/// encounter racers and the traffic (the car render objects 0x890080;
/// 0x61AEA0 takes a car only when 0x60D580 does, which needs +0x550
/// (active) set as World_DrawCars does, so a traffic car that is not
/// drawn shows no glow).
/// `drawn`: the frame's drawn sections (`World::drawn_sections_at` at the
/// camera).
/// `player_hidden`: the view leaves the player's car out
/// (`view_leaves_out_player`), so its neon tubes are not drawn either (the
/// car list 0x8A30F0 the tubes walk lacks it).
#[allow(clippy::too_many_arguments)]
pub(crate) fn glow_quads(
    d: &Drive,
    camera: &Camera,
    poses: &render::snapshot::FramePoses,
    race: Option<&Race>,
    encounters: Option<&Encounters>,
    traffic: Option<&Traffic>,
    aspect: f32,
    seconds: f32,
    drawn: Option<&[u32]>,
    opts: &nfsu2::render::RenderOptions,
    wet_interaction: u8,
    player_hidden: bool,
) -> Vec<nfsu2::particles::Quad> {
    let axes = CameraAxes::look(camera.forward(), camera.up);
    let view = nfsu2::glow::View {
        eye: camera.position,
        right: axes.right,
        down: axes.down,
        forward: axes.forward,
        view_projection: camera.view_projection(aspect),
        scale: nfsu2::render::cull_scale(camera.game_fov.unwrap_or(camera.fov_y)),
        front_end: false,
    };
    let mut quads = Vec::new();
    nfsu2::glow::world_sprites(&view, &d.world.flare_packs, drawn, &mut quads);
    // The brake flag (car +0x944, 0x5F62C0) holds 0.5 s for owner kinds 2
    // and 4 (the racers: rivals and encounter racers), not for the player
    // (1) or the traffic (3).
    let mut car = |car: &Car, state: &CarState, holds: bool, share: f32| {
        nfsu2::glow::car_sprites_faded(
            &view,
            &car.light_flares,
            state.transform(),
            car.headlight_pool_color,
            state.brake_lights(holds),
            share,
            &mut quads,
        );
        // (Wet Road Interaction's and Ambient Detail's lists of
        // these cars are built on the tick now, `Game::enh_fx_tick`.)
    };
    car(&d.car, poses.player(), false, 1.0);
    // The player's underglow lights the spray (as the smoke's `neons`).
    // (Wet Road Interaction as the spray it lights was stepped with.)
    if wet_interaction != 0 {
        // neon_rgb: neon_color's bytes are B, G, R, A (read
        // as R, G, B a yellow neon lit the spray blue).
        let neon = nfsu2::shadow::neon_rgb(&d.car.neon, seconds)
            .filter(|_| {
                d.car
                    .texture_for(nfsu2::formats::hash::bin_hash_from(
                        nfsu2::formats::hash::bin_hash(&d.car.name),
                        "_NEON",
                    ))
                    .is_some()
            })
            .map(|c| (poses.player().transform(), c));
        nfsu2::render::wet_spray::set_neon(neon);
    }
    for r in race.iter().flat_map(|r| &r.racers.rivals) {
        car(&r.car, poses.car(&r.id, &r.state), true, 1.0);
    }
    for c in encounters.iter().flat_map(|e| e.cars.iter().filter(|c| c.active)) {
        car(&c.car, poses.car(&c.id, &c.state), true, 1.0);
    }
    let ring = opts.pop_free && opts.lod_fade;
    for c in traffic.iter().flat_map(|t| t.cars.iter().filter(|c| crate::render::frame::traffic_drawn(c))) {
        // The traffic's flares follow its ring fade.
        let state = poses.car(&c.id, &c.state);
        let share =
            if ring { nfsu2::ai::traffic::ring_presence(state.position.distance(poses.player().position)) } else { 1.0 };
        car(&c.car, state, false, share);
    }
    // 0x494270(view, 0, 0) after 0x61ACF0 (0x5CB61C): the pooled flares,
    // the reflectors lit by the player's car (0x5ED990(0): car 0).
    let t = poses.player().transform();
    nfsu2::glow::pool_sprites(
        &view,
        &d.flare_pools,
        Some((t.w_axis.truncate(), t.x_axis.truncate().normalize_or_zero())),
        &mut quads,
    );
    // 0x61ACA0 (0x5CB629, last): the neon tubes of the cars on the list
    // 0x8A30F0 (**inferred**: every car drawn).
    let tv = tube_view(camera, aspect);
    if !player_hidden {
        neon_tubes(&tv, &d.car, poses.player().transform(), seconds, [None; 2], &mut quads);
    }
    for r in race.iter().flat_map(|r| &r.racers.rivals) {
        neon_tubes(&tv, &r.car, poses.car(&r.id, &r.state).transform(), seconds, [None; 2], &mut quads);
    }
    for c in encounters.iter().flat_map(|e| e.cars.iter().filter(|c| c.active)) {
        neon_tubes(&tv, &c.car, poses.car(&c.id, &c.state).transform(), seconds, [None; 2], &mut quads);
    }
    quads
}

/// The road reflection's flares this frame (0x5CAC9C, with
/// g_LightGlowEnable): the visible sections' street lamps through
/// 0x490920's mode 2, mirrored below the road and streaked by the
/// wetness, from view 1's camera (view 4 shares it).
pub(crate) fn reflection_glows(
    d: &mut Drive,
    camera: &Camera,
    aspect: f32,
    drawn: Option<&[u32]>,
) -> Vec<nfsu2::particles::Quad> {
    let axes = CameraAxes::look(camera.forward(), camera.up);
    let view = nfsu2::glow::View {
        eye: camera.position,
        right: axes.right,
        down: axes.down,
        forward: axes.forward,
        view_projection: camera.view_projection(aspect),
        scale: nfsu2::render::cull_scale(camera.game_fov.unwrap_or(camera.fov_y)),
        front_end: false,
    };
    let mut quads = Vec::new();
    let world = &d.world;
    let mut ground = |p: Vec3| world.ground_for(&mut nfsu2::ground::GroundQuery::default(), p).map(|(z, _, _)| z);
    nfsu2::glow::world_reflection_sprites(
        &view,
        &world.flare_packs,
        drawn,
        d.weather.wet,
        &mut d.flare_mirrors,
        &mut ground,
        &mut quads,
    );
    // 0x5CACB5: then the pooled flares (pool 1), after 0x61ACF0's list.
    nfsu2::glow::pool_reflection_sprites(&view, &d.flare_pools, d.weather.wet, &mut ground, &mut quads);
    quads
}

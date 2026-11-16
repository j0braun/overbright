use crate::*;

/// The main view's car LOD at frame time `now`: [`car_lod`], or with LOD
/// Fade (clone deviation) kept while the size stays within
/// [`nfsu2::qol::CAR_LOD_HYSTERESIS`] of a threshold either way, the
/// switch cross-faded (`CarRenderer::set_lod_faded`); then the airborne
/// rule and the underside test of the car's draw ([`airborne_pick`],
/// [`note_underside`]).
#[allow(clippy::too_many_arguments)]
pub(crate) fn car_lod_main(
    cr: &mut CarRenderer,
    camera: &nfsu2::render::camera::Camera,
    settings: &options::Settings,
    state: &CarState,
    car: &Car,
    draw: LodDraw,
    now: f32,
    opts: &nfsu2::render::RenderOptions,
) {
    let at = state.position;
    let lod = match cr.main_lod() {
        Some(current) if opts.lod_fade && !opts.pop_free && !nfsu2::render::pop_meter::oracle() => {
            let fov = camera.game_fov.unwrap_or(camera.fov_y);
            let k = 256.0 / (fov * 0.5).tan() * opts.car_detail;
            let size = nfsu2::car::screen_size(camera.position, camera.forward(), k, at) as f32;
            let start = if settings.custom[4] == 0 { 1 } else { 0 };
            nfsu2::car::pick_lod_kept(size, start, draw, current, nfsu2::qol::CAR_LOD_HYSTERESIS)
        }
        _ => car_lod(camera, settings, at, draw, opts),
    };
    // The pop meter (NFSU2_POP_STATS): the car's switches and placements
    // in the view, sized by the car's own projection (3 m radius).
    if nfsu2::render::pop_meter::enabled() {
        use nfsu2::render::pop_meter::{Source, event, px};
        let fov = camera.game_fov.unwrap_or(camera.fov_y);
        let k = 256.0 / (fov * 0.5).tan();
        let size = px(nfsu2::car::screen_size(camera.position, camera.forward(), k, at) as f32, opts);
        let secs = if opts.lod_fade { nfsu2::render::RenderOptions::LOD_FADE_SECONDS } else { 0.0 };
        let before = cr.main_lod();
        let moved = cr.meter_at.is_some_and(|p| p.distance(at) > 30.0);
        // The traffic's ring fade (LodDraw::FrontEnd in game is the
        // traffic): a jump seen at neither end is not a pop.
        let ring = draw == LodDraw::FrontEnd && opts.pop_free && opts.lod_fade;
        let share = if ring { nfsu2::ai::traffic::ring_presence(at.distance(camera.position)) } else { 1.0 };
        let seen = cr.meter_share.max(share);
        cr.meter_share = share;
        if let Some(b) = before {
            if moved && b.is_some() && lod.is_some() {
                if seen > 0.0 {
                    event(Source::TrafficRespawn, size, 0.0);
                }
            } else if b != lod
                && seen > 0.0
                && (at - camera.position).normalize_or_zero().dot(camera.forward()) > (fov * 0.6).cos()
            {
                // In the view (a car coming round from behind the camera
                // is not a pop).
                if debug_env().pop_verbose {
                    eprintln!(
                        "pop car {:?} -> {:?} at {:.0} m, {:.0} px, draw {:?}, moved {moved}",
                        b,
                        lod,
                        at.distance(camera.position),
                        size,
                        draw
                    );
                }
                event(if b.is_none() || lod.is_none() { Source::CarCull } else { Source::CarLod }, size, secs);
            }
        }
        cr.meter_at = Some(at);
    }
    let lod = airborne_pick(lod, cr, state);
    cr.set_lod_faded(lod, now, opts.lod_fade);
    note_underside(cr, camera, state, car, lod);
}

/// The airborne rule after the pick (**verified**: 0x620AF0 at
/// 0x6211CB…0x6211F8 in game, 0x6162F0 at 0x616776…0x61679E for the
/// traffic's table): LOD D becomes C while no wheel is down (car +0x424)
/// and the underside was seen on the car's last draw (+0x2EB8).
pub(crate) fn airborne_pick(lod: Option<u8>, cr: &CarRenderer, state: &CarState) -> Option<u8> {
    airborne_pick_seen(lod, cr.underside_seen, state)
}

/// [`airborne_pick`] with the last draw's underside byte given.
pub(crate) fn airborne_pick_seen(lod: Option<u8>, underside_seen: bool, state: &CarState) -> Option<u8> {
    lod.map(|l| nfsu2::car::airborne_lod(l, state.wheels_down() as u32, underside_seen))
}

/// After the pick of a drawn car (0x621321 / 0x616840 → 0x60C330, record
/// 10): whether its underside is visible from the camera, the eye taken
/// into the car's frame (0x43C7C0 / 0x43C9C0) and the four wheel centres of
/// its (upgraded) record. In game only (g_GameState 6). A car the pick
/// leaves out is not drawn and keeps the byte.
pub(crate) fn note_underside(
    cr: &mut CarRenderer,
    camera: &nfsu2::render::camera::Camera,
    state: &CarState,
    car: &Car,
    lod: Option<u8>,
) {
    if lod.is_none() {
        return;
    }
    cr.underside_seen = underside_from(camera.position, state.transform(), car.car_type.wheels.map(|w| w.position));
}

/// 0x60C330's underside test in game for an eye in the world, the car's
/// matrix and its four wheel centres.
pub(crate) fn underside_from(eye: Vec3, car_world: glam::Mat4, wheels: [Vec3; 4]) -> bool {
    nfsu2::car::underside_visible(car_world.inverse().transform_point3(eye), wheels, true)
}

/// The car draw's LOD for a car at `at` seen by `camera` (0x48ACB0's screen
/// size at view +0x0C = 256 / tan(fov / 2), then [`nfsu2::car::pick_lod`]
/// from the Car Geometry Detail's start LOD, 0x5D28E0: Low 1, High 0).
pub(crate) fn car_lod(
    camera: &nfsu2::render::camera::Camera,
    settings: &options::Settings,
    at: Vec3,
    draw: LodDraw,
    opts: &nfsu2::render::RenderOptions,
) -> Option<u8> {
    let fov = camera.game_fov.unwrap_or(camera.fov_y);
    // docs/pop-in.md: World Detail Ultra / Extreme keep the cars'
    // top LOD (the Car Geometry Detail's start) while they are drawn, and
    // draw them down to `qol::DETAIL_FULL_PX` output pixels of radius: no
    // LOD switch, no pop at size 4.
    if nfsu2::render::pop_meter::oracle() {
        return (nfsu2::car::screen_size(camera.position, camera.forward(), 256.0 / (fov * 0.5).tan(), at) > 0)
            .then_some(0);
    }
    if opts.pop_free {
        let w = opts.output_size_or((1280, 960)).0.max(1) as f32;
        let k = 256.0 / (fov * 0.5).tan() * w / 512.0;
        let px = nfsu2::car::screen_size(camera.position, camera.forward(), k, at) as f32;
        let start = if settings.custom[4] == 0 { 1 } else { 0 };
        return (px >= nfsu2::render::RenderOptions::DETAIL_FULL_PX).then_some(start);
    }
    // Car Detail Distance (clone deviation): the size scaled up (1 native).
    let k = 256.0 / (fov * 0.5).tan() * opts.car_detail;
    let size = nfsu2::car::screen_size(camera.position, camera.forward(), k, at);
    let start = if settings.custom[4] == 0 { 1 } else { 0 };
    nfsu2::car::pick_lod(size, start, draw, false)
}

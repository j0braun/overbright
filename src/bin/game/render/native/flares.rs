use crate::*;

/// The flare pools for this frame: 0x48AC00 (the last frame's end:
/// emptied; the blink, stepped by the frame time on the tick
/// (`render::visuals`), is the frame's `blink`), then view 1's scenery
/// cull (0x5B9C30 at the frame's start, 0x5CAB03, before any view draws)
/// adds the light markers of each instance it takes (0x5D6580): in a
/// section visible from the eye, not culled by its flags, at least 17 big
/// on screen (0x5D64E0 / 0x5D66A4), the markers of the solid drawn (LOD A,
/// or B at a low world detail, as the clone's main view). The instance's
/// own frustum box test and visibility-cell bit (0x5D6580's first test) are left to the
// prov: inferred game.fill_flare_pools
/// flares' point tests (**inferred** equivalent); smackable props are
/// drawn by 0x61D160, which adds none.
/// `drawn`: the frame's drawn sections (`World::drawn_sections_at` at the
/// camera); `camera`: the frame's.
pub(crate) fn fill_flare_pools(
    d: &mut Drive,
    blink: bool,
    camera: &Camera,
    world_renderer: Option<&WorldRenderer>,
    lod_b: bool,
    drawn: Option<&[u32]>,
    opts: &nfsu2::render::RenderOptions,
) {
    d.flare_pools.clear();
    d.flare_pools.blink = blink;
    let world = &d.world;
    let Some(visible) = drawn else { return };
    // The light placements in the drawn sections, in the world's order
    // (as testing each one's section against the list).
    let mut lights = std::mem::take(&mut d.flare_lights);
    world.light_placements_in(visible, &mut lights);
    let eye = camera.position;
    let forward = camera.forward();
    let scale = nfsu2::render::cull_scale(camera.game_fov.unwrap_or(camera.fov_y));
    for &i in &lights {
        let p = &world.placements[i];
        // 0x5D674A: an instance without a matrix (flag 0x0100) takes the
        // null-transform path, which adds no flares.
        if nfsu2::render::WorldView::Main.culls(p.flags, d.barriers.reversed) || p.flags & 0x0100 != 0 {
            continue;
        }
        let at = p.transform.w_axis.truncate();
        // LOD Fade (clone deviation): the lamp's glow follows its
        // instance's fade (last frame's; None: not sized, as natively).
        let fade = world_renderer.and_then(|r| r.placement_fade(i, opts));
        match fade {
            Some(f) if f <= 0.0 => continue,
            Some(_) => {}
            // 17 px (0x5D66A4), divided by the Detail Distance option.
            None => {
                if nfsu2::render::scenery_cull_size(at, p.cull_radius, eye, forward, scale)
                    < nfsu2::render::main_min_size(opts)
                {
                    continue;
                }
            }
        }
        let solid = if lod_b { p.mid_solid } else { Some(p.solid) };
        let Some(solid) = solid else { continue };
        d.flare_pools.add_markers_faded(p.transform, &world.solids[solid].light_markers, fade.unwrap_or(1.0));
    }
    d.flare_lights = lights;
}

/// The flag girl's flare glows this frame (0x436050 → 0x4335C0 →
/// 0x433090, into pool 1 with mask 3) while the race start scene's
/// characters play; `frame`: the crowd's frame as drawn.
pub(crate) fn flag_girl_flares(d: &mut Drive, c: &SceneCrowd, frame: f32) {
    for p in c.set.flare_glows(frame / nfsu2::shop_scene::FPS, c.origin, &c.lifts) {
        d.flare_pools.add_flag_girl_flare(p);
    }
}

/// The flag girl's two flare lights (0x433090 into the props object
/// [0x827B28], made at the scene's start by 0x4398A0 → 0x439780 and linked
/// as the light group "Flag Girl Flare Lights" into the list 0x835850 by
/// 0x48E380): flare i's light (props +0x44 + 0x68·i) is a point light
/// (+4 kind 2, +5 2, +7 on) of colour bytes 0x80A991DF (r 0xDF, g 0x91,
/// b 0xA9) at the flare's tip p1, outer radius 3.7, inner 0; the group is
/// switched on (+0x30 = 1) by the first flare and the lights keep their
/// last places until the scene stops (0x43B7A0 → 0x43B1F0 → 0x439930 frees
/// the props). The car lighting (0x48E3D0) gathers them after the packs.
/// With the crowd's frame as drawn.
pub(crate) fn flag_girl_lights(d: &mut Drive, c: Option<(&SceneCrowd, f32)>) {
    let Some((c, frame)) = c else {
        d.world.scene_lights.clear();
        return;
    };
    let flares = c.set.flares(frame / nfsu2::shop_scene::FPS, c.origin, &c.lifts);
    if flares.is_empty() {
        return;
    }
    d.world.scene_lights = flares
        .into_iter()
        .map(|(tip, _)| nfsu2::formats::lights::WorldLight {
            kind: 2,
            sub: 2,
            on: true,
            colour: FLARE_LIGHT_COLOUR,
            position: tip,
            outer: FLARE_LIGHT_RADIUS,
            direction: Vec3::ZERO,
            intensity: 0.0,
            inner: 0.0,
            cone: 14000.0,
        })
        .collect();
}

/// The flare light's colour (0x433090: +0x0C = 0x80A991DF, r g b the low
/// bytes) and radius (+0x1C = 3.7).
pub(crate) const FLARE_LIGHT_COLOUR: [u8; 3] = [0xDF, 0x91, 0xA9];

pub(crate) const FLARE_LIGHT_RADIUS: f32 = 3.7;

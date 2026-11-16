use crate::*;

/// A car's shadow in game (0x6162F0): `%s_SHADOWIG` (0x6381A0, render info
/// +0x33C), the grid laid on the ground through a copy of the car's own
// prov: inferred game.car_shadow_in_game
/// ground-query state (**inferred**: the context 0x6162F0 queries, render
/// info [0] + 0x5C4, is not shown to be the physics' one), and, at
/// `seconds` on the clock, the underglow: the same grid again with
/// `%s_NEON` (+0x344) in the neon's colour (0x622D66).
pub(crate) fn car_shadow_in_game(
    gpu: &Gpu,
    renderer: &mut WorldRenderer,
    world: &World,
    car: &Car,
    state: &CarState,
    seconds: f32,
    eye: Vec3,
    pull: f32,
    opts: &nfsu2::render::RenderOptions,
) -> Vec<(u32, nfsu2::shadow::Shadow)> {
    let mut query = state.ground.clone();
    let mut ground = |p: Vec3| world.ground_for(&mut query, p).map(|(z, n, _)| (z, n));
    let wheels = car.car_type.wheels.map(|w| w.position);
    let Some(s) = nfsu2::shadow::car_shadow(state.transform(), car.shadow_box, wheels, Some(&mut ground)) else {
        return Vec::new();
    };
    // The Graphics page's Shadow Quality (clone deviation): the grid split
    // 2 × 2 (High) or 4 × 4 (Ultra) per cell on the ground.
    let s = match opts.shadow_quality {
        0 => s,
        q => s.refine(if q == 1 { 2 } else { 4 }, &mut ground),
    };
    car_shadow_passes(gpu, renderer, car, s.toward_eye(eye, state.position, pull), "SHADOWIG", seconds, opts)
}

/// The two passes over a car's shadow grid (0x622D3F..0x622F93): the
/// shadow with `%s_<shadow>` when the car has that texture, then the
/// underglow with `%s_NEON` when it has that texture and the neon's alpha
/// is not 0.
pub(crate) fn car_shadow_passes(
    gpu: &Gpu,
    renderer: &mut WorldRenderer,
    car: &Car,
    s: nfsu2::shadow::Shadow,
    shadow: &str,
    seconds: f32,
    opts: &nfsu2::render::RenderOptions,
) -> Vec<(u32, nfsu2::shadow::Shadow)> {
    let mut out = Vec::new();
    // `bin_hash("<car>_<shadow>")` and `bin_hash("<car>_NEON")`, continued
    // from the name's hash (no string built per car per frame).
    use nfsu2::formats::hash::{bin_hash, bin_hash_from};
    let stem = bin_hash_from(bin_hash(&car.name), "_");
    if let Some(t) = car.texture_for(bin_hash_from(stem, shadow)) {
        renderer.add_shadow_texture(gpu, t, opts);
        out.push((t.hash, s.clone()));
    }
    if let (Some(t), Some(c)) =
        (car.texture_for(bin_hash_from(stem, "NEON")), nfsu2::shadow::neon_color(&car.neon, seconds, s.fade))
    {
        renderer.add_shadow_texture(gpu, t, opts);
        out.push((t.hash, s.neon(c)));
    }
    out
}

/// The shadow and underglow textures `car_shadow_passes` would add for
/// `cars`, added at the load instead of in the frame a car first comes on
/// the road (its upload was that frame's longest part). The textures
/// drawn are the same.
/// `opts`: the settings in force at the load.
pub(crate) fn warm_shadow_textures<'a>(gpu: &Gpu, renderer: &mut WorldRenderer, cars: impl Iterator<Item = &'a Car>, opts: &nfsu2::render::RenderOptions) {
    let mut seen = std::collections::HashSet::new();
    for car in cars {
        if !seen.insert(car.name.clone()) {
            continue;
        }
        for kind in ["SHADOWIG", "NEON"] {
            if let Some(t) = car.texture_for(nfsu2::formats::hash::bin_hash(&format!("{}_{kind}", car.name))) {
                renderer.add_shadow_texture(gpu, t, opts);
            }
        }
    }
}

/// Shadows first, then the underglows (rendering orders 2 and 4 of the
/// textures, 0x6388BB / 0x6388EB; **inferred**: the effect 13 lists' order
/// by it is not traced).
pub(crate) fn shadows_then_neons(mut all: Vec<(u32, nfsu2::shadow::Shadow)>) -> Vec<(u32, nfsu2::shadow::Shadow)> {
    all.sort_by_key(|(_, s)| s.color != [0x80, 0x80, 0x80, s.alpha]);
    all
}

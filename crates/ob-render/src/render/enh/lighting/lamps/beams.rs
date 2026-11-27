use super::*;
/// Headlight Beams (docs/notes/graphics/lighting2.md): beams drawn per frame at most
/// (the player's and the nearest other cars').
pub const MAX_BEAMS: usize = 16;
/// Other cars' beams fade out between these distances from the eye.
pub(in crate::render) const BEAM_FADE: (f32, f32) = (110.0, 150.0);

/// A car's headlight beam for world.wgsl's `beam_light`: view 9's projector
/// (`headlight_uniform`, the native one is the player's only) moved to that
/// car, at `fade` of its colour. Every car's lights are on in game: +0x93D
/// is set by the constructor (0x6045B0) and by 0x5F61A0 each update (its
/// flicker branch needs +0x925, which only 0 is ever written to: 0x6045C3,
/// 0x5F61E1, 0x5F62AA), and the player's (owner kind 1) is left as built;
/// under Wine every car read on (docs/notes/graphics/lighting2.md).
pub fn gpu_beam(h: &[[f32; 4]; 5], player: bool, fade: f32) -> GpuLamp {
    GpuLamp {
        position: [h[0][0], h[0][1], h[0][2], if player { 1.0 } else { 0.0 }],
        colour: [h[4][0] * fade, h[4][1] * fade, h[4][2] * fade, h[2][3]],
        direction: [h[3][0], h[3][1], h[3][2], h[1][3]],
        shadow: [h[1][0], h[1][1], h[1][2], 0.0],
    }
}

/// This frame's beams: the first pool's (the player's car) always, then the
/// other cars nearest the eye, faded out by [`BEAM_FADE`], [`MAX_BEAMS`] in all.
pub fn beams(pools: &[crate::render::HeadlightPool], eye: Vec3) -> Vec<GpuLamp> {
    let mut out = Vec::new();
    let Some((player, rest)) = pools.split_first() else { return out };
    out.push(gpu_beam(&crate::render::headlight_uniform(Some(player)), true, 1.0));
    let mut others: Vec<(f32, &crate::render::HeadlightPool)> = rest
        .iter()
        .map(|p| (p.transform.w_axis.truncate().distance(eye), p))
        .filter(|(d, _)| *d < BEAM_FADE.1)
        .collect();
    others.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (d, p) in others.into_iter().take(MAX_BEAMS - 1) {
        let fade = 1.0 - ((d - BEAM_FADE.0) / (BEAM_FADE.1 - BEAM_FADE.0)).clamp(0.0, 1.0);
        out.push(gpu_beam(&crate::render::headlight_uniform(Some(p)), false, fade));
    }
    out
}

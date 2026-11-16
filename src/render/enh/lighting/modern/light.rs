//! Modern Lighting, phase 1 (clone deviation, opt-in through a hidden
//! switch; docs/modern-lighting.md). Nothing here is native: SPEED2.EXE
//! lights the world with its baked vertex colours only (world.efx
//! `tex × v0 × 2`).
//!
//! The street lamps' share of the bake is modelled per pixel as physical
//! point lights (inverse square × cosine, windowed to the authored outer
//! radius) whose intensity and colour were fitted to the bake itself
//! (`examples/modern_light_fit.rs`); that share is then relit: shadowed
//! by Lamp Shadows' layers and the cars, given the per-pixel normal, and a
//! specular lobe. Everything the lamps do not explain (sky, neon, the
//! artists' paint, bounce) stays the bake.

use glam::Vec3;

/// The distance below a lamp at which [`pool`] is 1 (the light packs'
/// street lamps hang about this high over the road; the fit's unit).
pub const POOL_REF: f32 = 9.0;

/// The physical pool of a lamp at `rel` (lamp − surface) on a surface
/// facing `n`: inverse square × cosine, 1 at [`POOL_REF`] straight below,
/// times a smooth window that is exactly 0 at the authored outer radius
/// (1 − (d / outer)⁴)². world.wgsl `ml_pool` is the same function.
pub fn pool(rel: Vec3, n: Vec3, outer: f32) -> f32 {
    pool_soft(rel, n, outer, POOL_SOFT)
}

/// The fitted softening (m²): the lamp's light as from a source of this
/// size, which widens the pool's core the way the bake's is wide
/// (`examples/modern_light_fit.rs` searches it).
pub const POOL_SOFT: f32 = 150.0;

/// [`pool`] with the softening `soft` (m²): (POOL_REF² + soft) / (d² +
/// soft) × cosine × the window.
pub fn pool_soft(rel: Vec3, n: Vec3, outer: f32, soft: f32) -> f32 {
    let d2 = rel.length_squared();
    if outer <= 0.0 || d2 >= outer * outer {
        return 0.0;
    }
    let d = d2.sqrt().max(1e-3);
    let cos = (n.dot(rel) / d).max(0.0);
    let x = d2 / (outer * outer);
    let window = (1.0 - x * x).max(0.0);
    (POOL_REF * POOL_REF + soft) * cos / (d2 + soft.max(1.0)) * window * window
}

/// The hidden switch `NFSU2_MODERN_LIGHTING` (read once): unset, empty or
/// `0` off (the default); `1` or `citycore` the prototype's district (City
/// Core, [`CITY_CORE`]); `all` every lamp of the world. With
/// `NFSU2_MODERN_LIGHTING_BOX=x0,y0,x1,y1` the district is that box.
/// Or the Graphics page's Lighting row (`modern_light_bold::tier`).
pub fn enabled(opts: &crate::render::RenderOptions) -> bool {
    district(opts).is_some()
}

/// The whole world (the Lighting row's district).
pub const ALL: ([f32; 2], [f32; 2]) = ([f32::MIN, f32::MIN], [f32::MAX, f32::MAX]);

/// The district Modern Lighting relights (world x/y, metres): min, max —
/// the whole world with the Lighting row on, else the hidden
/// switch's ([`switch_district`]).
pub fn district(opts: &crate::render::RenderOptions) -> Option<([f32; 2], [f32; 2])> {
    if opts.modern_lighting != 0 {
        return Some(ALL);
    }
    switch_district()
}

/// The hidden switch's district (read once).
pub fn switch_district() -> Option<([f32; 2], [f32; 2])> {
    static D: std::sync::OnceLock<Option<([f32; 2], [f32; 2])>> = std::sync::OnceLock::new();
    *D.get_or_init(|| {
        parse_switch(
            crate::render::debug_env::nfsu2_modern_lighting().as_deref(),
            crate::render::debug_env::nfsu2_modern_lighting_box().as_deref(),
        )
    })
}

/// City Core as the in-game World Map frames it (zoom record 1 of
/// 0x7F42C8: texels 186, 175, size 307 of the 512 map; L4RA's map frame
/// left −3425.43, top 3287.26, size 5633.16): x −1379 … 1999, y −2016 …
/// 1362.
pub const CITY_CORE: ([f32; 2], [f32; 2]) = ([-1379.0, -2016.0], [1999.0, 1362.0]);

/// [`district`] from the switch's and the box's values.
pub fn parse_switch(switch: Option<&str>, boxed: Option<&str>) -> Option<([f32; 2], [f32; 2])> {
    let s = switch?.trim();
    if s.is_empty() || s == "0" {
        return None;
    }
    if let Some(b) = boxed {
        let v: Vec<f32> = b.split(',').filter_map(|f| f.trim().parse().ok()).collect();
        if v.len() == 4 {
            return Some(([v[0].min(v[2]), v[1].min(v[3])], [v[0].max(v[2]), v[1].max(v[3])]));
        }
    }
    match s {
        "all" => Some(ALL),
        _ => Some(CITY_CORE),
    }
}

/// The fitted channel gains on a lamp's authored colour (/255): the bake's
/// lamp share on Bayview's roads is `GAIN × colour / 255 × pool`
/// (`examples/modern_light_fit.rs`, FIT_ROAD=1: per-channel least squares
/// over 187 238 road vertices, r² 0.37 / 0.35 / 0.29; the native weight
/// curve explains r² 0.17). Warm: the bake's pools are sodium-tinted where
/// the packs' colours are grey.
pub const GAIN: [f32; 3] = [0.749, 0.655, 0.445];

/// A lamp's Modern Lighting colour (world.wgsl `Lamp.shadow.yzw`): the
/// authored colour × [`GAIN`] when it stands in `district`, else 0 (the
/// shader leaves it to the bake).
pub fn colour(l: &crate::formats::lights::WorldLight, district: ([f32; 2], [f32; 2])) -> [f32; 3] {
    let (lo, hi) = district;
    let p = l.position;
    if p.x < lo[0] || p.y < lo[1] || p.x > hi[0] || p.y > hi[1] {
        return [0.0; 3];
    }
    let c = l.colour;
    [GAIN[0] * f32::from(c[0]) / 255.0, GAIN[1] * f32::from(c[1]) / 255.0, GAIN[2] * f32::from(c[2]) / 255.0]
}

/// Give the frame's uploaded lamps (`lights`, kept from `lamps` by index
/// `kept`) their Modern Lighting colour in `shadow.yzw` ([`colour`]).
pub fn tag(
    lights: &mut [crate::render::lamps::GpuLamp],
    kept: &[usize],
    lamps: &[crate::render::lamps::Lamp],
    district: ([f32; 2], [f32; 2]),
) {
    for (g, &i) in lights.iter_mut().zip(kept) {
        let c = colour(&lamps[i].light, district);
        g.shadow[1] = c[0];
        g.shadow[2] = c[1];
        g.shadow[3] = c[2];
    }
}

/// `NFSU2_MODERN_LIGHTING_DEBUG=1`: the world shows the modelled lamp share
/// (red) and its relit form (green) instead of its colours; `2` the
/// light the shadows take (red the lamps', green the other lights') and
/// the traced lights' count / 4 (blue).
pub fn debug() -> u8 {
    static D: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *D.get_or_init(|| match crate::render::debug_env::nfsu2_modern_lighting_debug().as_deref() {
        Ok("1") => 1,
        Ok("2") => 2,
        _ => 0,
    })
}

/// Cars farther than this from the eye throw no Modern Lighting shadow
/// (their shadows are a few pixels; each car is a test per pixel).
pub const CAR_REACH: f32 = 45.0;

/// The main view's `lamps.x` while the switch is on: 2 + the cars (the
/// first of `occluders`, nearest the eye first after the player's) within
/// [`CAR_REACH`] of `eye`, + 64 with the debug view (world.wgsl
/// `ml_car_count`).
pub fn frame_x(occluders: &[crate::render::lamps::Occluder], eye: Vec3) -> f32 {
    let n = occluders.iter().take_while(|o| o.transform.w_axis.truncate().distance(eye) <= CAR_REACH).count();
    // The player's car (first) may be farther than a near one only in
    // odd cameras: keep the run of near ones from the start.
    2.0 + n as f32 + 64.0 * f32::from(debug())
}

/// At most this share of the bake (per channel) is taken as lamp light
/// (world.wgsl `ML_SHARE_MAX`).
pub const SHARE_MAX: f32 = 0.85;
/// How much of a lamp's share a full shadow takes (world.wgsl
/// `ML_SHADOW_DEPTH`; the rest stands for its bounce).
pub const SHADOW_DEPTH: f32 = 0.85;
/// The cars' depth in the lamps' light (world.wgsl `ML_CAR_DEPTH`).
pub const CAR_DEPTH: f32 = 0.75;

/// world.wgsl `ml_relight`'s last step: the baked light `bake` with the
/// modelled lamp share `share` taken out and `lit` (the share relit:
/// shading normal, shadows) put back, both limited to [`SHARE_MAX`] of
/// the bake per channel.
pub fn relight(bake: Vec3, share: Vec3, lit: Vec3) -> Vec3 {
    let k = (SHARE_MAX * bake.max(Vec3::ZERO) / share.max(Vec3::splat(1e-6))).min(Vec3::ONE);
    bake - share * k + lit * k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_is_one_below_the_lamp_and_zero_at_the_authored_radius() {
        let below = pool(Vec3::new(0.0, 0.0, POOL_REF), Vec3::Z, 30.0);
        assert!((below - 1.0).abs() < 0.02, "{below}");
        assert_eq!(pool(Vec3::new(30.0, 0.0, 0.5), Vec3::Z, 30.0), 0.0);
        assert_eq!(pool(Vec3::new(0.0, 0.0, -5.0), Vec3::Z, 30.0), 0.0, "a lamp under the surface");
        // Falls off with the distance along the road.
        let mut last = f32::MAX;
        for r in 0..30 {
            let e = pool(Vec3::new(r as f32, 0.0, POOL_REF), Vec3::Z, 30.0);
            assert!(e <= last, "{r}: {e} > {last}");
            last = e;
        }
        // Cosine: a wall facing the lamp across the street takes more than
        // one turned 60° away.
        let rel = Vec3::new(10.0, 0.0, 0.0);
        assert!(pool(rel, Vec3::X, 30.0) > pool(rel, Vec3::new(0.5, 0.866, 0.0), 30.0));
    }

    #[test]
    fn switch_and_district() {
        assert_eq!(parse_switch(None, None), None);
        assert_eq!(parse_switch(Some("0"), None), None);
        assert_eq!(parse_switch(Some(""), None), None);
        assert_eq!(parse_switch(Some("1"), None), Some(CITY_CORE));
        assert_eq!(parse_switch(Some("citycore"), None), Some(CITY_CORE));
        assert_eq!(parse_switch(Some("1"), Some("10,20,-5,4")), Some(([-5.0, 4.0], [10.0, 20.0])));
        assert!(parse_switch(Some("all"), None).unwrap().0[0] < -1e30);
        // City Core's map window from the zoom record and L4RA's map frame.
        let (left, top, size) = (-3425.43f32, 3287.26f32, 5633.16f32);
        let (cx, cy, s) = (186.0f32, 175.0f32, 307.0f32);
        assert!((CITY_CORE.0[0] - (left + cx / 512.0 * size)).abs() < 1.0);
        assert!((CITY_CORE.1[0] - (left + (cx + s) / 512.0 * size)).abs() < 1.0);
        assert!((CITY_CORE.1[1] - (top - cy / 512.0 * size)).abs() < 1.0);
        assert!((CITY_CORE.0[1] - (top - (cy + s) / 512.0 * size)).abs() < 1.0);
    }

    #[test]
    fn colour_is_the_fitted_gain_inside_the_district_only() {
        let mut l = crate::formats::lights::WorldLight {
            kind: 2,
            sub: 2,
            on: true,
            colour: [150, 150, 150],
            position: Vec3::new(0.0, 0.0, 9.0),
            outer: 30.0,
            direction: Vec3::NEG_Z,
            intensity: 1.0,
            inner: 10.0,
            cone: 0.0,
        };
        let c = colour(&l, CITY_CORE);
        assert!((c[0] - 0.749 * 150.0 / 255.0).abs() < 1e-5 && c[0] > c[1] && c[1] > c[2], "{c:?} warm");
        l.position.x = 5000.0;
        assert_eq!(colour(&l, CITY_CORE), [0.0; 3]);
    }

    #[test]
    fn relight_keeps_the_bake_unshadowed_and_never_goes_black() {
        let bake = Vec3::new(0.6, 0.55, 0.4);
        let share = Vec3::new(0.3, 0.27, 0.18);
        assert!((relight(bake, share, share) - bake).abs().max_element() < 1e-6);
        // Fully shadowed (lit = share × (1 − depth)).
        let dark = relight(bake, share, share * (1.0 - SHADOW_DEPTH));
        assert!(dark.cmplt(bake).all() && dark.cmpge(bake * (1.0 - SHARE_MAX)).all(), "{dark}");
        // A share the bake cannot hold is cut to SHARE_MAX of it.
        let small = Vec3::splat(0.1);
        let r = relight(small, Vec3::splat(1.0), Vec3::ZERO);
        assert!((r - small * (1.0 - SHARE_MAX)).abs().max_element() < 1e-6);
    }

    #[test]
    fn shader_matches_the_constants_and_strips_to_the_old_text() {
        let wgsl = crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::World);
        for (name, v) in [
            ("ML_POOL_REF", POOL_REF),
            ("ML_POOL_SOFT", POOL_SOFT),
            ("ML_SHARE_MAX", SHARE_MAX),
            ("ML_SHADOW_DEPTH", SHADOW_DEPTH),
            ("ML_CAR_DEPTH", CAR_DEPTH),
            ("ML_CHARACTER_MARK", crate::render::characters::CHARACTER_MARK),
        ] {
            assert!(wgsl.contains(&format!("const {name}: f32 = {v:?};")), "{name} = {v:?} in world.wgsl");
        }
        // The composer's MODERN sections (core::shader): off, the lamps'
        // module has none of it; on, all of it.
        let off = crate::render::world_source_modern(true, 0);
        assert!(!off.contains("ml_relight") && !off.contains("MODERN") && !off.contains("lamps.x > 1.5"));
        let on = crate::render::world_source_modern(true, 1);
        assert!(on.contains("ml_relight") && on.contains("// BEGIN MODERN"));
        // Every MODERN section sits inside a LAMPS one (the native module
        // never sees one): MODERN without LAMPS adds nothing.
        use crate::render::core::shader::{compose::VariantFlag, registry};
        let native = |extra: &[VariantFlag]| {
            registry::compose(
                registry::ShaderId::World,
                [VariantFlag::TextureBindingPlain].into_iter().chain(extra.iter().copied()).collect(),
            )
            .unwrap()
        };
        assert_eq!(native(&[VariantFlag::Modern]), native(&[]), "a MODERN section outside the LAMPS sections");
    }
}

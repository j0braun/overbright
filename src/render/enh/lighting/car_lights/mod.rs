//! Next-Gen car lights (docs/car-lights.md; a **clone deviation**,
//! opt-in: the Headlight Beams row's third value, `enh.headlight_beams 2`).
//! Nothing here exists in SPEED2.EXE; every look constant is **inferred**
//! (by eye). What the original decides stays its own: which cars have lights
//! (all of them, always on: docs/lighting2.md), where they are (the car
//! models' light markers, `glow::CarFlare`), their colour (render info
//! +0x380 for the headlights) and when the brake lights are on (car +0x944,
//! `CarState::brake_lights`).
//!
//! Per frame the game hands every drawn car's lights over ([`car`],
//! [`publish`]); this module turns them into:
//! * **world glows** (`world_glows`): tail / brake lights as point lights
//!   and the underglow as a soft rectangular area light, after the beams in
//!   world.wgsl's lamp list (`beam_light`, position.w 2 / 3);
//! * **car lamps** (`car_lamps`): the four strongest lights on a car's body
//!   for car.wgsl's Dynamic Lamps slots: other cars' head / tail / brake
//!   lights and underglows, and the car's own underglow bounced off the road
//!   onto its lower body;
//! * **the air** (`CarLightsPass`): every nearby car's headlight cones
//!   marched through the night air with the native HEADLIGHTS cookie (both
//!   lamps of the car), Henyey–Greenstein forward scattering, drifting
//!   density noise, rain and the native fog; tail / brake / head lamps as
//!   closed-form point-light halos; at reduced resolution with a
//!   depth-aware upsample; then
//! * **lens streaks** over each visible lamp (anamorphic streak + soft core,
//!   occlusion read from the scene depth), added on top of the native glow
//!   sprites (kept as they are).

use glam::{Mat4, Vec3};
use std::sync::RwLock;

use crate::render::Gpu;
use crate::render::camera::Camera;
use crate::render::lamps::GpuLamp;

mod model;
pub use model::*;
mod feeds;
pub use feeds::*;
mod air;
pub use air::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::lights::Flare;
    use crate::glow::{CarFlare, LEFT_BRAKELIGHT, LEFT_HEADLIGHT, RIGHT_BRAKELIGHT, RIGHT_HEADLIGHT};

    fn flares() -> Vec<CarFlare> {
        let f = |hash, x: f32, y: f32, dir: Vec3| CarFlare {
            hash,
            flare: Flare { position: Vec3::new(x, y, 0.6), direction: dir, kind: 0, flags: 4 },
        };
        vec![
            f(LEFT_HEADLIGHT, 2.1, 0.7, Vec3::X),
            f(RIGHT_HEADLIGHT, 2.1, -0.7, Vec3::X),
            f(LEFT_BRAKELIGHT, -2.1, 0.7, -Vec3::X),
            f(RIGHT_BRAKELIGHT, -2.1, -0.7, -Vec3::X),
        ]
    }

    fn at(x: f32, braking: bool, neon: Option<Vec3>) -> Car {
        car(
            Mat4::from_translation(Vec3::new(x, 0.0, 0.0)),
            &flares(),
            [230, 230, 220],
            braking,
            (Vec3::new(-2.2, -0.9, 0.0), Vec3::new(2.2, 0.9, 1.3)),
            neon,
        )
    }

    /// The lamps as 0x615640 lights the glow sprites: two heads, two tails
    /// (brighter braking), all in world space.
    #[test]
    fn lights_follow_the_native_markers() {
        let c = at(10.0, false, None);
        assert_eq!(c.lights.len(), 4);
        assert_eq!(c.lights.iter().filter(|l| l.kind == Kind::Head).count(), 2);
        assert!(c.lights.iter().all(|l| (l.position.x - 10.0).abs() > 2.0));
        let tail = c.lights.iter().find(|l| l.kind == Kind::Tail).unwrap();
        let b = at(10.0, true, None);
        let brake = b.lights.iter().find(|l| l.kind == Kind::Brake).unwrap();
        assert!(brake.colour.x > 2.5 * tail.colour.x && tail.facing.x < -0.99);
        // HeadlightColor's clamp: bytes / 128 ≤ 1.
        assert!(c.lights[0].colour.max_element() <= 1.0);
    }

    /// World glows: a rear glow per car (only the braking part with Modern
    /// Lighting's tails), an area light per underglow, the nearest first,
    /// none beyond the reach.
    #[test]
    fn world_glows_per_car() {
        let cars = vec![at(30.0, false, None), at(5.0, true, Some(Vec3::new(0.9, 0.0, 0.7))), at(500.0, true, None)];
        // (the summed budget is `glows_are_bounded_over_cars`')
        let look = Look { road_budget: 0.0, ..LOOK };
        let g = world_glows(&cars, Vec3::ZERO, Vec3::X, false, &look);
        assert_eq!(g.len(), 3);
        assert_eq!(g[0].position[3], GLOW_POINT);
        assert!(g[0].position[0] < 5.0 && g[0].colour[3] == LOOK.brake_reach);
        assert_eq!(g[1].position[3], GLOW_AREA);
        assert!((g[1].direction[3] - 2.2).abs() < 1e-5 && (g[1].shadow[3] - 0.9).abs() < 1e-5);
        assert_eq!(g[2].colour[3], LOOK.tail_reach);
        let m = world_glows(&cars, Vec3::ZERO, Vec3::X, true, &look);
        // Modern Lighting: the tail-only car adds nothing, the braking one
        // its braking part.
        assert_eq!(m.len(), 2);
        assert!(m[0].colour[0] > 0.0 && m[0].colour[0] < g[0].colour[0]);
    }

    /// The cars' road glows summed stay within the budget (a grid
    /// of braking cars), one car's tail lights alone are untouched.
    #[test]
    fn glows_are_bounded_over_cars() {
        let lum = |g: &[GpuLamp]| {
            g.iter().map(|l| l.colour[0] * 0.2126 + l.colour[1] * 0.7152 + l.colour[2] * 0.0722).sum::<f32>()
        };
        let grid: Vec<Car> = (0..8).map(|i| at(4.0 + 3.0 * i as f32, true, None)).collect();
        let g = world_glows(&grid, Vec3::ZERO, Vec3::X, false, &LOOK);
        assert!(g.len() == 8 && lum(&g) <= LOOK.road_budget * 1.0001, "{}", lum(&g));
        let one = world_glows(&[at(10.0, false, None)], Vec3::ZERO, Vec3::X, false, &LOOK);
        let free =
            world_glows(&[at(10.0, false, None)], Vec3::ZERO, Vec3::X, false, &Look { road_budget: 0.0, ..LOOK });
        assert_eq!(one, free);
    }

    /// The halos' veil: lamps at the camera are capped so the
    /// screen mean stays within the budget, a lone far lamp keeps its halo;
    /// the CPU halo is the shader's (same constants).
    #[test]
    fn halo_veil_is_bounded() {
        use crate::render::camera::Camera;
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_lights.wgsl"));
        assert!(
            src.contains(&format!("const CORE: f32 = {CORE};"))
                && src.contains(&format!("const EXTINCTION: f32 = {EXTINCTION}.0;"))
        );
        let cam = Camera::new(Vec3::ZERO, 0.0, 0.0);
        let rot = Mat4::look_to_rh(Vec3::ZERO, cam.forward(), cam.up);
        let clip_to_rel = (cam.projection_depth(4.0 / 3.0, false) * rot).inverse();
        let lamp = |p: Vec3| GpuPoint {
            position: [p.x, p.y, p.z, 0.0],
            colour: [1.0, 1.0, 1.0, 1.0],
            facing: [-cam.forward().x, -cam.forward().y, -cam.forward().z, 0.0],
        };
        let near: Vec<GpuPoint> = (0..12).map(|i| lamp(cam.forward() * (2.0 + i as f32) + Vec3::Z * 0.3)).collect();
        let far = lamp(cam.forward() * 80.0);
        let mut all = near.clone();
        all.push(far);
        let d = LOOK.density;
        let v = halo_veils(&all, clip_to_rel, d, &LOOK);
        assert!(v[..12].iter().any(|s| *s < 0.5) && v[12] == 1.0, "{v:?}");
        assert_eq!(halo_veils(&[far], clip_to_rel, d, &LOOK), vec![1.0]);
        // The capped sum is the budget.
        let scaled: Vec<GpuPoint> = all
            .iter()
            .zip(&v)
            .map(|(p, s)| {
                let mut p = *p;
                p.colour[3] *= s;
                p
            })
            .collect();
        let mut again = halo_veils(&scaled, clip_to_rel, d, &LOOK);
        again.retain(|s| *s < 0.999);
        assert!(again.is_empty(), "{again:?}");
    }

    /// Car lamps: a car in front of another's headlights takes them, the
    /// car behind does not; its own underglow lights its sills; the four
    /// strongest are kept, the native ones included.
    #[test]
    fn cars_take_each_others_lights() {
        let lit = at(0.0, false, None);
        let ahead = car_lamps(std::slice::from_ref(&lit), Vec3::new(12.0, 0.0, 0.5), [[0.0; 4]; 8], &LOOK);
        assert!(ahead[0][3] > 0.0 && ahead[1][0] > 0.0, "{ahead:?}");
        let side = car_lamps(std::slice::from_ref(&lit), Vec3::new(0.0, 15.0, 0.5), [[0.0; 4]; 8], &LOOK);
        assert_eq!(side[0][3], 0.0, "{side:?}");
        let own = at(0.0, false, Some(Vec3::new(0.0, 0.0, 1.0)));
        let o = car_lamps(std::slice::from_ref(&own), Vec3::ZERO, [[0.0; 4]; 8], &LOOK);
        assert!(o[0][3] > 0.0 && o[2][3] > 0.0 && o[1][2] > 0.0 && o[1][0] == 0.0, "{o:?}");
        assert!(o[0][1].abs() > 1.0 && o[2][1].abs() > 1.0, "beside the sills");
        // A street lamp right over the car stays among the four.
        let mut lamps = [[0.0; 4]; 8];
        lamps[0] = [0.0, 0.0, 4.0, 12.0];
        lamps[1] = [0.8, 0.7, 0.5, 0.0];
        let m = car_lamps(&[own.clone(), at(-8.0, true, None)], Vec3::ZERO, lamps, &LOOK);
        assert!((0..4).any(|i| m[i * 2] == lamps[0]));
        assert!((0..4).all(|i| m[i * 2][3] > 0.0));
    }

    /// Air: the nearest cars first, a cone per car with both lamps, every
    /// lamp a point; nothing far away or well behind the eye.
    /// The underglow's road light is a soft spill around the native
    /// pool, inside the look rules' budget (the wash-out in the reported rain frame).
    #[test]
    fn underglow_on_the_road_stays_in_budget() {
        assert!(LOOK.neon_road <= 0.15 && LOOK.neon_reach <= 1.5);
        let c = at(0.0, false, Some(Vec3::new(1.0, 1.0, 0.0)));
        let g = world_glows(&[c], Vec3::new(-6.0, 0.0, 1.5), Vec3::X, false, &LOOK);
        let area = g.iter().find(|l| l.position[3] == GLOW_AREA).unwrap();
        assert!(area.colour[0] <= 0.15 && area.colour[2] == 0.0 && area.colour[3] <= 1.5);
    }

    #[test]
    fn medium_takes_tails_not_heads() {
        // One volume: the froxel medium gets a car's tail / brake lamps (world space, the tail at half,
        // brake whole), never its headlights (their glare is the sprite's, their beam the medium's spot).
        let cars = vec![at(12.0, true, None), at(30.0, false, None)];
        let cam = Camera::new(Vec3::new(0.0, 0.0, 1.0), 0.0, 0.0);
        let (_, points) = air_lights(&cars, cam.position, cam.forward());
        let lamps = medium_lamps_of(&points, &cam, 16.0 / 9.0, false, 1.0);
        let tails = points.iter().filter(|p| p.position[3] > 0.5).count();
        assert_eq!(lamps.len(), tails);
        assert!(lamps.iter().all(|(p, _, f)| p.x > 5.0 && f.x < 0.0), "{lamps:?}");
        for ((_, c, _), p) in lamps.iter().zip(points.iter().filter(|p| p.position[3] > 0.5)) {
            let share = if p.position[3] < 1.5 { 0.5 } else { 1.0 };
            assert!(c.x <= p.colour[0] * share + 1e-6, "bounded by the lamp's own light");
        }
    }

    #[test]
    fn air_lights_nearest_first() {
        let cars = vec![at(60.0, false, None), at(10.0, true, None), at(-80.0, false, None), at(400.0, false, None)];
        let (s, p) = air_lights(&cars, Vec3::ZERO, Vec3::X);
        assert_eq!(s.len(), 2);
        assert!((s[0].position[0] - 12.1).abs() < 1e-4 && (s[0].position[3] - 0.7).abs() < 1e-4);
        assert!(s[0].right[3] > 0.0 && s[0].up[3] > 0.0 && s[0].dir[0] > 0.9);
        assert_eq!(p.len(), 8);
        assert_eq!(p[2].position[3], 2.0);
        assert_eq!(std::mem::size_of::<GpuSpot>(), 80);
        assert_eq!(std::mem::size_of::<GpuPoint>(), 48);
    }

    /// The point lights' air density made on the CPU is the shader's
    /// `air` (0.55..1.45, smooth in space and time), and the shader reads it
    /// from the lamp instead of evaluating it per pixel.
    #[test]
    fn air_at_matches_the_shader_air() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_lights.wgsl"));
        assert!(src.contains("p.colour.a);") && !src.contains("air(c + u.eye.xyz)"));
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for i in 0..2000 {
            let p = Vec3::new(i as f32 * 0.37 - 300.0, (i % 97) as f32 * 1.9 - 80.0, (i % 13) as f32 * 0.6);
            let a = air_at(p, i as f32 * 0.01);
            lo = lo.min(a);
            hi = hi.max(a);
            // Smooth: 1 cm and 1 ms away it moves by far less than its range.
            assert!((air_at(p + Vec3::splat(0.01), i as f32 * 0.01 + 0.001) - a).abs() < 0.01);
        }
        assert!(lo >= 0.55 - 1e-5 && hi <= 1.45 + 1e-5 && hi - lo > 0.4, "{lo} {hi}");
        // Value noise: at integer lattice points (time 0, the second octave aside) the hash itself.
        let a = air_at(Vec3::ZERO, 0.0);
        assert!(a.is_finite());
        // The lattice is the texture's (NOISE³ bytes, the shader's NOISE), periodic.
        assert_eq!(noise_texels().len(), NOISE.pow(3));
        assert!(src.contains(&format!("const NOISE: f32 = {}.0;", NOISE)) && !src.contains("fn hash3"));
    }

    /// The tile lists are conservative: every (pixel, lamp) pair the
    /// shader's rejects let through (h² ≤ HALO², b > a) has the lamp in the
    /// pixel's tile, over random lamps around the eye (in front, beside,
    /// behind, inside the halo) and both depth conventions.
    #[test]
    fn point_tiles_keep_every_lamp_a_pixel_can_see() {
        use crate::render::camera::Camera;
        let mut seed = 0x1234_5678u32;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        for reversed in [false, true] {
            let cam = Camera::new(Vec3::new(10.0, -4.0, 3.0), 0.7, -0.1);
            let aspect = 16.0 / 9.0;
            let low = (240u32, 135u32);
            let rot = Mat4::look_to_rh(Vec3::ZERO, cam.forward(), cam.up);
            let proj = cam.projection_depth(aspect, reversed);
            let to_rel = (proj * rot).inverse();
            let points: Vec<Vec3> = (0..64)
                .map(|i| {
                    let r = if i < 8 { 2.0 + 10.0 * rnd() } else { 3.0 + 80.0 * rnd() };
                    let d = Vec3::new(rnd() * 2.0 - 1.0, rnd() * 2.0 - 1.0, rnd() * 2.0 - 1.0).normalize_or(Vec3::X);
                    // Half of them toward the view.
                    let d = if i % 2 == 0 { (cam.forward() * 1.5 + d).normalize() } else { d };
                    d * r
                })
                .collect();
            let (masks, tiles) = point_tiles(&points, rot, proj, low, HALO);
            let mut checked = 0;
            for y in (0..low.1).step_by(3) {
                for x in (0..low.0).step_by(3) {
                    let (u, v) = ((x as f32 + 0.5) / low.0 as f32, (y as f32 + 0.5) / low.1 as f32);
                    let far = if reversed { 0.0 } else { 1.0 };
                    let h = to_rel * glam::Vec4::new(u * 2.0 - 1.0, 1.0 - v * 2.0, far, 1.0);
                    let dir = (h.truncate() / h.w).normalize();
                    let m = masks[((y / TILE) * tiles.0 + x / TILE) as usize];
                    for (i, c) in points.iter().enumerate() {
                        let t0 = c.dot(dir);
                        let h2 = (c.dot(*c) - t0 * t0).max(0.0);
                        let (a, b) = ((t0 - HALO).max(0.0), t0 + HALO);
                        if h2 <= HALO * HALO && b > a {
                            checked += 1;
                            assert!(
                                m[i / 32] >> (i % 32) & 1 == 1,
                                "lamp {i} at {c} missing in tile ({x}, {y}) reversed {reversed}"
                            );
                        }
                    }
                }
            }
            assert!(checked > 1000, "{checked}");
            // And it culls: on average a tile lists far fewer than all 64.
            let mean =
                masks.iter().map(|m| (m[0].count_ones() + m[1].count_ones()) as f32).sum::<f32>() / masks.len() as f32;
            assert!(mean < 40.0, "{mean}");
        }
    }

    /// An eye standing in a car's beam looking back at the lamps
    /// gets no second aureole from the air (the glow sprite and the lens core
    /// draw it); across the beam and behind it the air is as before; the lens
    /// glare is full within 10 m and falls as 1 / d² beyond.
    #[test]
    fn oncoming_beams_leave_the_aureole_to_the_sprites() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_lights.wgsl"));
        assert!(src.contains("* (1.0 - smoothstep(u.shape.x, u.shape.y, cos_t));"));
        assert!(src.contains("let reach = min(1.0, 200.0 / (100.0 + dist * dist));"));
        let (lo, hi) = LOOK.aureole;
        // Across the beam (90°) and from behind (180°) untouched; 15° off the axis gone.
        assert!(lo > 0.0 && hi < 1.0 && lo < hi);
        assert!(hi < 15f32.to_radians().cos());
        let reach = |d: f32| (200.0 / (100.0 + d * d)).min(1.0);
        assert_eq!(reach(5.0), 1.0);
        assert_eq!(reach(10.0), 1.0);
        assert!((reach(30.0) * 900.0 - 180.0).abs() < 1.0 && (reach(80.0) * 6400.0 - 197.0).abs() < 1.0);
    }

    /// The module validates with single and multisampled depth.
    #[test]
    fn shader_validates() {
        for ms in [false, true] {
            let module = wgpu::naga::front::wgsl::parse_str(&shader_source(ms)).expect("parses");
            wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .expect("valid");
            for e in ["fs_march", "fs_up", "vs_streak", "fs_streak", "vs_full"] {
                assert!(module.entry_points.iter().any(|p| p.name == e), "{e}");
            }
        }
        assert_eq!(std::mem::size_of::<Uniforms>() % 16, 0);
        assert_eq!(divisor(1080), 2);
        assert_eq!(divisor(2160), 3);
        assert_eq!(divisor(4320), 6);
    }
}

//! Light Probes (clone deviation, opt-in; docs/lighting2.md). Nothing
//! here exists in SPEED2.EXE: the original lights a car with a harmonic of
//! its in-game rig and the light-pack lamps near it (0x48E3D0 → 0x4893A0,
//! `lighting.rs`), and the packs hold street lamps only — no neon, no shop
//! signs (docs/lamps-shadows.md). Here:
//!
//! * **Placement**: probes along Bayview's road and race splines (the AI
//!   route files of the world's region, `TRACKS/ROUTES<region>/`
//!   `RoutesFreeRoam.bin` and every race's `Routes<track>F.bin`, each
//!   record's control points), one every
//!   [`SPACING`] m, [`HEIGHT`] m above the road.
//! * **Bake**: a probe is the world as the native renderer draws it into
//!   the car reflection cube (the environment pipelines, the same scenery,
//!   textures and baked vertex colours, emissive signs and neon included),
//!   at [`SIZE`]² a face from the probe, read back and averaged per face
//!   into an ambient cube (six colours). Probes are baked as the car
//!   comes near (nearest first, one at a time, read back a frame later), or
//!   all at once within [`BURST_RADIUS`] when the car arrives where none
//!   is baked yet (a load, a jump): a hitch like a section load, then none.
//! * **Use**: [`CarLighting::in_world`](super::lighting::CarLighting) blends
//!   the nearest probes (inverse distance, within [`REACH`] m) into the car's
//!   harmonic as a second-order term (rgb only: the highlight's strength,
//!   the specular channel, is left as it is). The probe replaces the
//!   ambient light it models instead of adding to it ([`ambient_terms`],
//!   docs/car-scene-specular.md): a lit probe face gives that side of the car its hue at the
//!   car's own brightness (bounded), lifted by at most [`LIFT_CAP`]; a dark
//!   face changes nothing, so the dark gaps and the native contrast stay;
//!   neon and lit signs tint the side of the car facing them.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use glam::{Vec3, Vec4};

use crate::formats::ai_routes::RouteFile;

mod field;
pub use field::*;
mod bake;
pub(in crate::render) use bake::*;

#[cfg(test)]
mod tests {
    #[test]
    fn a_failed_read_back_settles_and_never_spins() {
        use super::{MapFlag, MapState};
        // The map fails after two polls: seen as failed.
        let f = MapFlag::default();
        let mut n = 0;
        assert_eq!(
            f.wait(64, || {
                n += 1;
                if n == 2 {
                    f.set(false);
                }
                true
            }),
            MapState::Failed
        );
        assert_eq!(n, 2);
        // The map succeeds.
        let f = MapFlag::default();
        assert_eq!(
            f.wait(64, || {
                f.set(true);
                true
            }),
            MapState::Ready
        );
        // Never settles: bounded.
        let f = MapFlag::default();
        let mut n = 0;
        assert_eq!(
            f.wait(8, || {
                n += 1;
                true
            }),
            MapState::Failed
        );
        assert_eq!(n, 8);
        // The poll itself fails (device lost).
        let f = MapFlag::default();
        assert_eq!(f.wait(64, || false), MapState::Failed);
    }

    use super::*;

    #[test]
    fn dark_faces_change_nothing_and_lit_ones_are_bounded() {
        let native = Vec3::new(0.30, 0.28, 0.25);
        let lum = |c: Vec3| c.dot(LUMA_WEIGHTS);
        // Dark (any hue): the native ambient exactly.
        assert_eq!(face_ambient(Vec3::splat(0.02), native), native);
        assert_eq!(face_ambient(Vec3::new(0.03, 0.0, 0.03), native), native);
        // A saturated neon face: the hue moves towards it, every channel
        // within HUE_CAP of the luminance, the luminance kept (dim face).
        let neon = Vec3::new(0.35, 0.05, 0.3);
        let a = face_ambient(neon, native);
        assert!(a.x > native.x && a.y < native.y, "{a}");
        assert!((lum(a) - lum(native)).abs() < 0.03 * lum(native), "{a}");
        assert!(((a - native) / lum(native)).abs().max_element() < HUE_CAP + 0.1, "{a}");
        // A very bright face lifts by at most LIFT_CAP.
        let b = face_ambient(Vec3::splat(5.0), native);
        assert!(lum(b) <= lum(native) * (1.0 + LIFT_CAP) + 1e-5 && lum(b) > lum(native), "{b}");
    }

    #[test]
    fn harmonic_terms_reproduce_the_ambient_cube_on_the_axes() {
        let mut cube = [Vec3::ZERO; 6];
        cube[0] = Vec3::new(1.0, 0.0, 1.0); // +x: magenta neon
        let h = harmonic_terms(&cube);
        let px = harmonic_rgb(&h, Vec3::X);
        let nx = harmonic_rgb(&h, -Vec3::X);
        let up = harmonic_rgb(&h, Vec3::Z);
        // Facing it: 0.875 of it; away: 0.125; sideways: none.
        assert!(
            (px - cube[0] * 0.875).length() < 1e-5 && (nx - cube[0] * 0.125).length() < 1e-5 && up.length() < 1e-6,
            "{px} {nx} {up}"
        );
        assert!(h.iter().all(|v| v.w == 0.0), "the highlight's strength stays as it is");
    }

    #[test]
    fn probes_replace_the_ambient_instead_of_adding_to_it() {
        // A car's harmonic: a white fill from above.
        let mut native = [Vec4::ZERO; 10];
        native[0] = Vec4::new(0.3, 0.3, 0.3, 0.2);
        native[9] = Vec4::new(0.1, 0.1, 0.1, 0.0);
        // Under a bright red arcade: the car's light gets redder but its
        // energy stays within LIFT_CAP on every axis.
        let cube = [Vec3::new(2.0, 0.3, 0.4); 6];
        let t = ambient_terms(&cube, &native);
        let lit: [Vec4; 10] = std::array::from_fn(|i| native[i] + t[i]);
        for d in [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::Z] {
            let (n, l) = (harmonic_rgb(&native, d), harmonic_rgb(&lit, d));
            // (per face exact; the quadratic fit leaks an eighth of the
            // opposite face's change, hence the 3 %)
            assert!(l.dot(LUMA_WEIGHTS) <= n.dot(LUMA_WEIGHTS) * (1.0 + LIFT_CAP) * 1.03, "{d} {n} {l}");
            assert!(l.x > n.x && l.x <= n.x + (HUE_CAP + LIFT_CAP) * n.dot(LUMA_WEIGHTS) * 1.2, "{d} {n} {l}");
        }
        // A dark street: nothing.
        let dark = ambient_terms(&[Vec3::splat(0.02); 6], &native);
        assert!(dark.iter().all(|v| *v == Vec4::ZERO));
    }

    #[test]
    fn probes_follow_the_roads_and_blend_the_nearest() {
        let mut f =
            ProbeField::new(vec![Vec3::new(0.0, 0.0, 1.0), Vec3::new(24.0, 0.0, 1.0), Vec3::new(500.0, 0.0, 1.0)]);
        assert!(f.sample(Vec3::ZERO).is_none(), "nothing baked");
        f.baked[0] = Some([Vec3::splat(1.0); 6]);
        f.baked[1] = Some([Vec3::ZERO; 6]);
        let at0 = f.sample(Vec3::new(1.0, 0.0, 1.0)).unwrap();
        let mid = f.sample(Vec3::new(12.0, 0.0, 1.0)).unwrap();
        assert!(at0[0].x > 0.8 && (mid[0].x - 0.5).abs() < 1e-5, "{at0:?} {mid:?}");
        assert!(f.sample(Vec3::new(300.0, 0.0, 1.0)).is_none(), "beyond reach");
        assert_eq!(f.near(Vec3::new(490.0, 0.0, 0.0), 20.0).len(), 1);
    }

    /// Off the roads (no probe in reach, some within the burst
    /// radius) nothing is baked twice; an arrival bursts the unbaked ones.
    #[test]
    fn baked_probes_are_never_baked_again() {
        let mut f = ProbeField::new(vec![
            Vec3::new(50.0, 0.0, 0.0),
            Vec3::new(60.0, 0.0, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(500.0, 0.0, 0.0),
        ]);
        // Off the roads, 50 m from the nearest: one at a time, nearest first.
        let off = Vec3::new(0.0, -45.0, 0.0);
        assert_eq!(f.plan(off), BakePlan::One(2));
        // At a probe with nothing baked: the burst, unbaked only.
        assert_eq!(f.plan(Vec3::ZERO), BakePlan::Burst(vec![2, 0, 1]));
        f.baked[2] = Some([Vec3::ZERO; 6]);
        f.baked[0] = Some([Vec3::ZERO; 6]);
        assert_eq!(f.plan(Vec3::ZERO), BakePlan::One(1));
        f.baked[1] = Some([Vec3::ZERO; 6]);
        assert_eq!(f.plan(Vec3::ZERO), BakePlan::Nothing);
        assert_eq!(f.plan(off), BakePlan::Nothing);
        // Far from every probe: nothing.
        assert_eq!(f.plan(Vec3::new(-2000.0, 0.0, 0.0)), BakePlan::Nothing);
    }

    /// A set-up's switch gives up exactly the baked probes whose bake box
    /// (± far round the probe) holds a switched placement's sphere; the
    /// next plan bakes those near the car at once, then one at a time.
    #[test]
    fn a_switch_gives_up_the_probes_that_see_it() {
        let mut f = ProbeField::new(vec![
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(24.0, 0.0, 1.0),
            Vec3::new(100.0, 0.0, 1.0),
            Vec3::new(300.0, 0.0, 1.0),
        ]);
        for b in &mut f.baked[..3] {
            *b = Some([Vec3::ONE; 6]);
        }
        // Nothing switched: nothing given up.
        assert!(f.release_seeing(&[], 64.0).is_empty());
        assert!(!f.rebake);
        // A sphere at x = 70 (radius 7): probe 0's box reaches 64 (gap 6 <
        // 7), probe 1 and 2 hold it; probe 3 (unbaked) is left.
        assert_eq!(f.release_seeing(&[(Vec3::new(70.0, 0.0, 1.0), 7.0)], 64.0), vec![0, 1, 2]);
        assert!(f.rebake);
        // A sphere just outside a box's corner: kept.
        f.baked[0] = Some([Vec3::ONE; 6]);
        f.rebake = false;
        assert!(f.release_seeing(&[(Vec3::new(-68.0, 68.0, 1.0), 5.0)], 64.0).is_empty());
        // The burst after the switch: the given-up probes within the burst
        // radius, though one in reach is baked.
        f.baked[0] = None;
        f.baked[1] = Some([Vec3::ONE; 6]);
        f.rebake = true;
        assert_eq!(f.plan(Vec3::new(20.0, 0.0, 1.0)), BakePlan::Burst(vec![0]));
        f.rebake = false;
        assert_eq!(f.plan(Vec3::new(20.0, 0.0, 1.0)), BakePlan::One(0));
    }

    #[test]
    fn face_means_and_slots() {
        let row = ROW_BYTES as usize;
        let mut data = vec![0u8; row * SIZE as usize * 6];
        for y in 0..SIZE as usize {
            for x in 0..SIZE as usize {
                data[(2 * SIZE as usize + y) * row + x * 4] = 255;
            }
        }
        let m = face_means(&data, row);
        assert_eq!(m[2], Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(m[0], Vec3::ZERO);
        let slots: Vec<usize> = face_axes().iter().map(|(_, _, f)| slot_of(*f)).collect();
        let mut sorted = slots.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2, 3, 4, 5], "{slots:?}");
    }

    /// Bayview's road network gives probes along every road (asset test).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn bayview_probes_cover_the_roads() {
        let root = crate::testkit::game_root();
        let path = root.join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin");
        let data = std::fs::read(path).unwrap();
        assert!(region_routes(std::path::Path::new(&root), "L4RA").len() > 10, "the races' splines too");
        let routes = RouteFile::parse(&data);
        let f = ProbeField::along_roads(std::slice::from_ref(&routes));
        assert!(f.positions.len() > 500, "{} probes", f.positions.len());
        // Every control point has a probe within a spacing.
        for r in routes.records.iter().take(50) {
            for p in &r.points {
                assert!(!f.near(p.position + Vec3::Z * HEIGHT, SPACING).is_empty());
            }
        }
    }
}

//! Dynamic Lamps, Lamp Shadows and Contact Shadows (clone deviations,
//! opt-in; docs/notes/graphics/lamps-shadows.md). Nothing here exists in SPEED2.EXE: the
//! original lights the world with baked vertex colours only and the cars
//! with a harmonic built from the light packs (0x48E3D0 → 0x4893A0,
//! `lighting.rs`). These options add on top of that, never replacing it:
//!
//! * **Dynamic Lamps**: clustered forward lighting fed by the light packs'
//!   own point (2) and spot (3) lights with their authored colours and
//!   radii and the native weight curve (1 inside the inner radius, falling
//!   by d² to 0 at the outer one; spots over their cone), added to the
//!   world's vertex colour the way the native headlight projector adds to
//!   it (worldheadlight.efx: `tex × (light + v0) × 2`). Past the outer
//!   radius nothing is added, so the gaps between lamps stay as dark as
//!   the baked lighting has them. Cars take the four strongest at their
//!   position, per pixel.
//! * **Lamp Shadows**: a depth-map array for the nearest downward lamps
//!   (static scenery, cached: a lamp's map is drawn once when it gets a
//!   layer) and the player's headlight projector (drawn every frame), plus
//!   the cars as soft ellipsoids; the shadow only takes away the light the
//!   option added (and part of the headlight's), never the baked light.
//! * **Contact Shadows**: a soft box occlusion under each car on the
//!   world's surfaces, before the native shadow texture is blended over it.
//!
//! The frustum is cut into [`TILES_X`] × [`TILES_Y`] screen tiles and
//! [`SLICES`] exponential depth slices; each cluster lists the lights whose
//! sphere may reach it (built on the CPU each frame: a few hundred lights).

use glam::{Mat4, Vec3, Vec4};

use crate::formats::lights::WorldLight;

use crate::render::{FrameParams, FrameUniform, Gpu, PassCull, TextureRef, WorldCore, WorldRenderer, WorldView};
mod records;
pub use records::*;
mod beams;
pub use beams::*;
mod cluster;
pub use cluster::*;
mod fades;
pub use fades::*;
mod shadow_layers;
pub use shadow_layers::*;
mod bindings;
pub(in crate::render) use bindings::*;
mod plan;
mod car_masks;
pub use car_masks::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// The binning as it was (a list per cluster, appended in lamp order,
    /// then each list's head copied while the index list has room).
    fn build_reference(lamps: &[Lamp], view_proj: Mat4, eye: Vec3, forward: Vec3) -> Vec<u32> {
        let planes = super_planes(view_proj);
        let mut cand: Vec<(f32, usize)> = lamps
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                let (p, r) = (l.light.position, l.light.outer);
                let dist = (p - eye).length();
                if dist - r > REACH
                    || (p - eye).dot(forward) < -r
                    || planes.iter().any(|pl| pl.truncate().dot(p) + pl.w < -r)
                {
                    return None;
                }
                Some((dist - r, i))
            })
            .collect();
        cand.sort_by(|a, b| a.0.total_cmp(&b.0));
        cand.truncate(MAX_LIGHTS);
        let mut lists: Vec<Vec<u32>> = vec![Vec::new(); CLUSTERS];
        for (index, (_, i)) in cand.into_iter().enumerate() {
            let l = &lamps[i].light;
            let (x0, x1, y0, y1) = screen_rect(view_proj, l.position, l.outer);
            let z = (l.position - eye).dot(forward);
            for s in slice_of(z - l.outer)..=slice_of(z + l.outer) {
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        lists[((s * TILES_Y + y) * TILES_X + x) as usize].push(index as u32);
                    }
                }
            }
        }
        let cap = list_cap(&lists.iter().map(|l| l.len() as u32).collect::<Vec<_>>());
        let mut words = vec![0u32; CLUSTERS * 2];
        let mut total = 0usize;
        for (c, list) in lists.iter().enumerate() {
            let n = list.len().min(cap).min(MAX_INDICES.saturating_sub(total));
            words[c * 2] = (CLUSTERS * 2 + total) as u32;
            words[c * 2 + 1] = n as u32;
            words.extend_from_slice(&list[..n]);
            total += n;
        }
        words
    }

    /// The counting binning gives the same words as the reference,
    /// with few lamps, many lamps and enough big ones to fill the index
    /// list (its cut).
    #[test]
    fn build_matches_the_reference_binning() {
        let eye = Vec3::new(0.0, 0.0, 2.0);
        let forward = Vec3::X;
        let view_proj = Mat4::perspective_rh(1.2, 16.0 / 9.0, NEAR, FAR) * Mat4::look_to_rh(eye, forward, Vec3::Z);
        let mut seed = 12345u32;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        for (count, big) in [(0usize, 0.0f32), (3, 5.0), (300, 20.0), (1500, 180.0)] {
            let lamps: Vec<Lamp> = (0..count)
                .map(|_| Lamp {
                    section: 1,
                    light: light(
                        Vec3::new(rnd() * 400.0 - 20.0, rnd() * 300.0 - 150.0, rnd() * 20.0),
                        2.0 + rnd() * big,
                    ),
                })
                .collect();
            let c = build(&lamps, &|_| true, view_proj, eye, forward, &|_| -1);
            assert_eq!(c.words, build_reference(&lamps, view_proj, eye, forward), "{count} lamps");
            if count == 1500 {
                // Over the index list, every list cut to one cap (no
                // cluster emptied while another keeps its whole list).
                assert!(
                    c.words.len() <= CLUSTERS * 2 + MAX_INDICES && c.words.len() > CLUSTERS * 2 + MAX_INDICES * 9 / 10,
                    "the index list is full"
                );
                // The last clusters (far slices) keep lights (the old cut
                // emptied them).
                let far: u32 = (CLUSTERS - (TILES_X * TILES_Y) as usize..CLUSTERS).map(|i| c.words[i * 2 + 1]).sum();
                assert!(far > 0);
            }
        }
    }

    /// The lists' cap — none while they fit, else the largest that
    /// fits (the longest lists cut, the short ones kept whole).
    #[test]
    fn list_cap_cuts_the_longest_lists_evenly() {
        assert_eq!(list_cap(&[3, 5, 0]), usize::MAX);
        let mut counts = vec![10u32; CLUSTERS];
        counts[0] = 400_000;
        let cap = list_cap(&counts);
        let kept: usize = counts.iter().map(|&n| (n as usize).min(cap)).sum();
        assert!(kept <= MAX_INDICES && kept + CLUSTERS > MAX_INDICES, "{cap} {kept}");
        assert!(cap >= 10, "short lists kept whole: {cap}");
        let even = vec![200u32; CLUSTERS];
        assert_eq!(list_cap(&even), MAX_INDICES / CLUSTERS);
    }

    fn light(p: Vec3, outer: f32) -> WorldLight {
        WorldLight {
            kind: 2,
            sub: 0,
            on: true,
            colour: [110, 110, 125],
            position: p,
            outer,
            direction: Vec3::ZERO,
            intensity: 1.0,
            inner: 10.0,
            cone: 0.0,
        }
    }

    /// The pool peak is pinned — a baked pool (at or above LAMP_PEAK)
    /// keeps its level exactly, nothing ends above max(bake, LAMP_PEAK),
    /// dark stays dark, and the lamps never darken; and world.wgsl uses the
    /// same constants and form.
    #[test]
    fn pool_peak_is_pinned_to_the_bake() {
        for i in 0..=200 {
            let base = i as f32 / 100.0;
            for j in 0..=40 {
                let lamp = j as f32 / 10.0;
                let out = lamp_term(base, lamp);
                assert!(out >= base - 1e-6);
                assert!(out <= base.max(LAMP_PEAK) + 1e-5, "base {base} lamp {lamp} -> {out}");
                if base >= LAMP_PEAK {
                    assert_eq!(out, base);
                }
            }
        }
        // The pool peak ratio (brightest result over brightest bake) is 1.
        let peak = (0..=200).map(|i| i as f32 / 100.0).map(|b| lamp_term(b, 4.0)).fold(0.0f32, f32::max);
        assert_eq!(peak / 2.0, 1.0);
        // A dark gap (bake 0.02) gains at most 1 + 2 · LAMP_MAX (×1.24).
        assert!(lamp_term(0.02, 4.0) <= 0.02 * 1.24 + 1e-6);
        // Look rule "the bake is the anchor": the other cars' beams are held to the same gap budget.
        assert!(
            crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::World)
                .contains("const BEAM_OTHERS: f32 = 0.3;")
        );
        let wgsl = crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::World);
        assert!(wgsl.contains(&format!("const LAMP_PEAK: f32 = {LAMP_PEAK:?};")));
        assert!(wgsl.contains(&format!("const LAMP_MAX: f32 = {LAMP_MAX:?};")));
        assert!(wgsl.contains("let headroom = clamp(1.0 - base / LAMP_PEAK, 0.0, 1.0);"));
        assert!(wgsl.contains("light += added * light_in * (2.0 * headroom);"));
    }

    /// Headlight Beams — the player's beam is view 9's projector
    /// (`headlight_uniform`) unchanged, the others the same projector moved
    /// to their car, nearest first, faded out by distance; world.wgsl has
    /// the beams only in the lamps' module.
    /// A car headlight's binning sphere holds every point its light
    /// reaches (inside the cone and the reach), and other lights keep theirs.
    #[test]
    fn headlight_bin_sphere_holds_its_cone() {
        use crate::render::modern_light_bold::{HEAD_COS, HEAD_REACH, SUB_HEAD};
        let fwd = (Vec3::new(0.6, 0.8, 0.0) - Vec3::Z * 0.12).normalize();
        let l = WorldLight {
            kind: 3,
            sub: SUB_HEAD,
            on: true,
            colour: [200, 200, 180],
            position: Vec3::new(5.0, -3.0, 0.7),
            outer: HEAD_REACH,
            direction: fwd,
            intensity: 1.0,
            inner: 0.0,
            cone: 2.0 * HEAD_COS.acos(),
        };
        let (c, r) = bin_sphere(&l);
        assert!(r < 0.65 * HEAD_REACH, "{r}");
        let th = cone_threshold(&l);
        let side = fwd.cross(Vec3::Z).normalize();
        let up = side.cross(fwd);
        let mut n = 0;
        for i in 0..=40 {
            let d = HEAD_REACH * i as f32 / 40.0;
            for j in 0..=24 {
                let cos = th + (1.0 - th) * j as f32 / 24.0;
                let sin = (1.0 - cos * cos).max(0.0).sqrt();
                for k in 0..16 {
                    let a = k as f32 * std::f32::consts::TAU / 16.0;
                    let p = l.position + (fwd * cos + (side * a.cos() + up * a.sin()) * sin) * d;
                    assert!((p - c).length() <= r, "{p} outside ({c}, {r})");
                    n += 1;
                }
            }
        }
        assert!(n > 10000);
        let lamp = WorldLight { sub: 0, ..l };
        assert_eq!(bin_sphere(&lamp), (lamp.position, lamp.outer));
    }

    #[test]
    fn beams_are_the_native_projector_on_every_car() {
        let pool = |x: f32| crate::render::HeadlightPool {
            transform: Mat4::from_translation(Vec3::new(x, 0.0, 0.0)),
            color: [200, 100, 64],
        };
        let pools = [pool(0.0), pool(300.0), pool(20.0), pool(130.0)];
        let b = beams(&pools, Vec3::ZERO);
        assert_eq!(b.len(), 3, "the one past the fade is dropped");
        let h = crate::render::headlight_uniform(Some(&pools[0]));
        assert_eq!(b[0], gpu_beam(&h, true, 1.0));
        assert_eq!(b[0].position, [h[0][0], h[0][1], h[0][2], 1.0]);
        assert_eq!((b[0].direction[3], b[0].colour[3]), (h[1][3], h[2][3]), "the projection's scales");
        assert_eq!(b[0].colour[..3], h[4][..3], "HeadlightColor, clamped as c0");
        assert_eq!(b[1].position[0], 20.0, "nearest other first");
        assert_eq!(b[1].colour[0], h[4][0]);
        assert!(b[2].colour[0] < h[4][0] && b[2].colour[0] > 0.0, "fading");
        let many: Vec<_> = (0..40).map(|i| pool(i as f32)).collect();
        assert_eq!(beams(&many, Vec3::ZERO).len(), MAX_BEAMS);
        assert!(beams(&[], Vec3::ZERO).is_empty());
        let native = crate::render::world_source(false);
        let lamps = crate::render::world_source(true);
        assert!(!native.contains("beam_light") && lamps.contains("beam_light"));
    }

    #[test]
    fn native_weight_curve_and_outer_cut() {
        assert_eq!(weight(25.0, 10.0, 30.0), 1.0);
        assert_eq!(weight(900.0, 10.0, 30.0), 0.0);
        assert_eq!(weight(1000.0, 10.0, 30.0), 0.0);
        let w = weight(500.0, 10.0, 30.0);
        assert!((w - (1.0 - 400.0 / 800.0)).abs() < 1e-6);
    }

    #[test]
    fn slices_cover_the_range_monotonically() {
        assert_eq!(slice_of(0.1), 0);
        assert_eq!(slice_of(FAR * 2.0), SLICES - 1);
        let mut last = 0;
        for z in [1.0, 2.0, 5.0, 10.0, 50.0, 100.0, 300.0, 599.0] {
            let s = slice_of(z);
            assert!(s >= last);
            last = s;
        }
    }

    #[test]
    fn a_lamp_ahead_lands_in_the_centre_clusters_and_one_behind_is_dropped() {
        let cam = crate::render::Camera::new(Vec3::ZERO, 0.0, 0.0);
        let vp = cam.view_projection(16.0 / 9.0);
        let lamps = vec![
            Lamp { section: 1, light: light(Vec3::new(0.0, 40.0, 0.0), 5.0) },
            Lamp { section: 1, light: light(Vec3::new(0.0, -80.0, 0.0), 5.0) },
            Lamp { section: 2, light: light(Vec3::new(0.0, 40.0, 0.0), 5.0) },
        ];
        let c = build(&lamps, &|s| s == 1, vp, cam.position, cam.forward(), &|_| -1);
        assert_eq!(c.kept, vec![0]);
        let s = slice_of(40.0);
        let centre = ((s * TILES_Y + TILES_Y / 2) * TILES_X + TILES_X / 2) as usize;
        assert_eq!(c.words[centre * 2 + 1], 1);
        assert_eq!(c.words[c.words[centre * 2] as usize], 0);
        // A cluster far off to the side and near the eye holds nothing.
        assert_eq!(c.words[1], 0);
    }

    #[test]
    fn layers_keep_their_lamps_and_take_new_ones_within_budget() {
        let mut layers = [Some(3), Some(7), None, None];
        let fresh = assign_layers(&mut layers, &[7, 9, 11, 12], 1);
        assert_eq!(layers, [Some(9), Some(7), None, None]);
        assert_eq!(fresh, vec![0]);
        let fresh = assign_layers(&mut layers, &[7, 9, 11, 12], 4);
        assert_eq!(layers, [Some(9), Some(7), Some(11), Some(12)]);
        assert_eq!(fresh, vec![2, 3]);
    }

    #[test]
    fn car_lamps_pick_the_strongest_four() {
        let ls: Vec<WorldLight> = (0..6).map(|i| light(Vec3::new(i as f32 * 5.0, 0.0, 0.0), 30.0)).collect();
        let refs: Vec<&WorldLight> = ls.iter().collect();
        let out = car_lamps(&refs, Vec3::new(100.0, 0.0, 0.0));
        assert_eq!(out, [[0.0; 4]; 8]);
        let out = car_lamps(&refs, Vec3::new(26.0, 0.0, 0.0));
        // The two inside their inner radius (x 20 and 25) come first.
        assert!(out[0][0] == 25.0 || out[0][0] == 20.0);
        assert!(out[2][0] == 25.0 || out[2][0] == 20.0);
        assert!(out[7][3] > 0.0 && out.iter().all(|v| v[3] >= 0.0));
    }

    /// A car's contact shadow gives way to its underglow — full
    /// without one, gone under a full one (centre.w of the GPU record),
    /// and world.wgsl weighs the shadow by it.
    #[test]
    fn contact_shadow_gives_way_to_the_underglow() {
        assert_eq!(contact_share(0.0), 1.0);
        assert_eq!(contact_share(1.0), 0.0);
        assert_eq!(contact_share(2.0), 0.0);
        assert!((contact_share(0.25) - 0.75).abs() < 1e-6);
        let o = |glow| Occluder {
            transform: Mat4::IDENTITY,
            min: Vec3::new(-2.0, -0.9, 0.0),
            max: Vec3::new(2.0, 0.9, 1.4),
            glow,
        };
        assert_eq!(o(0.0).gpu().centre[3], 1.0);
        assert_eq!(o(1.0).gpu().centre[3], 0.0);
        let wgsl = crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::World);
        assert!(wgsl.contains("vis *= 1.0 - frame.lamps.y * footprint * vertical * o.centre.w;"));
    }

    /// Against shadows that looked a bit off under the car: the native
    /// shadow texture over a contact-shadowed ground gives the darker of
    /// the two, not their product, and the contact shadow ends at the
    /// car's box (the native texture alone past it). Mirrors world.wgsl's
    /// `car_shadow_colour`: keep = clamp((1 − a) / vc, 0, 1).
    #[test]
    fn native_and_contact_shadow_take_the_darker() {
        let combined = |a: f32, vc: f32| {
            let keep = ((1.0 - a) / vc.max(1e-3)).clamp(0.0, 1.0);
            vc * keep // the ground (vc) under the decal's (1 − a')
        };
        for (a, vc) in [(0.6, 0.5), (0.3, 0.5), (0.0, 0.5), (0.7, 1.0), (0.9, 0.3)] {
            let want = (1.0f32 - a).min(vc);
            assert!((combined(a, vc) - want).abs() < 1e-6, "a {a} vc {vc}");
        }
        let wgsl = crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::World);
        assert!(wgsl.contains("let keep = clamp((1.0 - c.a) / max(vc, 1e-3), 0.0, 1.0);"));
        assert!(wgsl.contains("const CONTACT_CORE: f32 = 0.75;"));
        assert!(wgsl.contains("/ (h.xy * (1.0 - CONTACT_CORE));"));
        assert!(wgsl.contains("let footprint = 1.0 - smoothstep(0.0, 1.0, length(e));"));
    }
}

#[cfg(test)]
mod pop_fade_tests {
    use super::*;

    /// A section joining the drawn set brings its lamps' light up over
    /// `SECTION_FADE_SECONDS`, one leaving takes it down the same way; a
    /// first frame or an eye jump sets the shares at once.
    #[test]
    fn lamp_shares_follow_the_section_swap() {
        let mut f = PopFades::default();
        let mut vis = vec![true, false, false];
        f.step(&vis, 1.0, Vec3::ZERO);
        assert_eq!((f.share(0), f.share(1)), (1.0, 0.0), "first frame: at once");
        vis = vec![false, true, false];
        let dt = 1.0 / 60.0;
        let mut t = 1.0;
        let frames = (crate::render::RenderOptions::SECTION_FADE_SECONDS / dt).round() as usize;
        for k in 1..=frames + 2 {
            t += dt;
            f.step(&vis, t, Vec3::ZERO);
            if k == frames / 2 {
                assert!(
                    (f.share(1) - 0.5).abs() < 0.05 && (f.share(0) - 0.5).abs() < 0.05,
                    "{} {}",
                    f.share(0),
                    f.share(1)
                );
            }
        }
        assert_eq!((f.share(0), f.share(1)), (0.0, 1.0));
        assert!(f.sections.is_empty(), "settled: nothing tracked");
        // A jump: at once.
        vis = vec![true, false, false];
        f.step(&vis, t + dt, Vec3::new(100.0, 0.0, 0.0));
        assert_eq!((f.share(0), f.share(1)), (1.0, 0.0));
    }
}

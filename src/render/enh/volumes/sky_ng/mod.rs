//! Sky "Next-Gen" (clone deviation, opt-in: the Graphics page's Sky row, `enh.sky 1`; docs/atmosphere.md).
//! Ported from the earlier night-sky prototype (`NFSU2_NIGHT_SKY`) and brought to the next-gen bar:
//! a procedural night sky in the native dome's own palette (statistics of the sky pair's textures, the dome's
//! native shading and fog applied): stars, a moon with a halo, and one drifting cloud layer (lower and heavier in
//! rain) **rendered into a low-resolution sky panorama** (1024 × 256 lat-long, a quarter of its rows a frame),
//! its undersides and the horizon lit by the city's glow. The main view draws it over the dome's pixels (the
//! dome is still drawn first: its depth and alpha, the bleach mask, stay); the road reflection and the cars'
//! cube faces (drawn one by one) draw it where they drew nothing (natively their clear black). Nothing here is
//! in SPEED2.EXE; the dome's traced behaviour is in [`crate::sky`] and `render/mod.rs`.
//! `NFSU2_NIGHT_SKY_COVER=-0.4..0.5` biases the cover, `_RAIN=0..1` forces the rain look, `_TIME=s` offsets
//! the drift clock, `_DEBUG=1` shows the masks (tuning aids, as in the prototype).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use glam::Mat4;

use crate::render::Gpu;

mod palette;
pub use palette::*;
mod gpu;
pub use gpu::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_order_and_alpha() {
        // A 4x16 strip: each row a ramp of greys; the clear (10th pct) <= mean <= lit (90th).
        let (w, h) = (20u32, 16u32);
        let mut px = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let v = (x * 12 + y) as u8;
                px.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let s = main_stats(&px, w, h);
        for r in s {
            assert!(r[0][0] <= r[1][0] && r[1][0] <= r[2][0], "{r:?}");
            assert!((r[1][3] - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn cap_rings_weighted() {
        // A disc with alpha 255 inside radius 0.25 and 0 outside: inner rings opaque, outer transparent.
        let n = 64u32;
        let mut px = Vec::new();
        for y in 0..n {
            for x in 0..n {
                let (u, v) = ((x as f32 + 0.5) / n as f32 - 0.5, (y as f32 + 0.5) / n as f32 - 0.5);
                let a = if (u * u + v * v).sqrt() < 0.25 { 255 } else { 0 };
                px.extend_from_slice(&[100, 120, 140, a]);
            }
        }
        let s = cap_stats(&px, n, n);
        assert!(s[0][1][3] > 0.99);
        assert!(s[ROWS - 1][1][3] < 0.01);
        assert!((s[0][1][2] - 140.0 / 255.0).abs() < 1e-3);
    }

    /// Bayview's sky table: the night pair's mean is darker than the sunrise pair's and its clear statistic
    /// never exceeds its lit one (needs the game).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn bayview_palette() {
        let world = crate::world::World::load(&crate::testkit::game_root(), "L4RA").unwrap();
        let p = Palette::from_world(&world);
        let lum = |c: [f32; 4]| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
        let mean = |s: &Stats| s.iter().map(|r| lum(r[1])).sum::<f32>() / ROWS as f32;
        assert!(mean(&p.main[1]) < mean(&p.main[0]), "night {} sunrise {}", mean(&p.main[1]), mean(&p.main[0]));
        for pair in &p.main {
            for r in pair {
                assert!(lum(r[0]) <= lum(r[1]) + 1e-6 && lum(r[1]) <= lum(r[2]) + 1e-6);
            }
        }
    }

    /// A night sky only — 0 on the sunrise / sunset pairs, 1 on the night pairs, and through a
    /// morph the dome's own SKYMORPHALPHA, every step of the native cycle (crate::sky) included.
    #[test]
    fn night_weight_follows_the_dome() {
        assert_eq!(night_weight(0, None), 0.0);
        assert_eq!(night_weight(2, None), 1.0);
        assert_eq!(night_weight(4, None), 0.0);
        assert_eq!(night_weight(6, None), 1.0);
        assert_eq!(night_weight(2, Some((0, 0.0))), 0.0);
        assert!((night_weight(2, Some((0, 0.25))) - 0.25).abs() < 1e-6);
        assert!((night_weight(4, Some((2, 0.25))) - 0.75).abs() < 1e-6);
        assert_eq!(night_weight(6, Some((4, 1.3))), 1.0);
        let mut tod = crate::sky::TimeOfDay::new(true);
        let mut last = night_weight(tod.sky().set, tod.sky().morph);
        let mut clock = 1.0f32;
        while clock < 700.0 {
            tod.update(clock, 1.0 / 60.0);
            let w = night_weight(tod.sky().set, tod.sky().morph);
            assert!((w - last).abs() < 0.01, "a jump at {clock}: {last} -> {w}");
            last = w;
            clock += 1.0 / 60.0;
        }
    }

    #[test]
    fn off_by_default() {
        assert!(!enabled(&crate::render::RenderOptions::in_force()), "Original by default");
    }

    /// Both modules (main / panorama, reflections) parse and validate; the main one has no reflection entries.
    #[test]
    fn modules_validate() {
        for (src, refl) in [(source(0, 1, false), false), (source(1, 2, true), true)] {
            let module = wgpu::naga::front::wgsl::parse_str(&src).expect("parses");
            wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .expect("valid");
            assert_eq!(module.entry_points.iter().any(|e| e.name == "fs_refl"), refl);
            assert!(module.entry_points.iter().any(|e| e.name == "cs_panorama"));
        }
        assert_eq!(std::mem::size_of::<Uniform>() % 16, 0);
    }
}

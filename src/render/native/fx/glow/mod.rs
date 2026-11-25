//! Light glow sprites (g_LightGlowEnable, 0x870CE4): the soft halos of the
//! street lamps and the cars' head and brake lights. See docs/formats.md
//! "Light glow sprites" for the trace.
//!
//! Every glow is one flare record (0x30 bytes: position +0x10, direction
//! +0x20, type +0x2C, flags +0x2D) drawn by 0x490920 as up to two
//! camera-facing quads, one per layer of its type (0x7F3CC8). In game the
//! view loop draws them after the particles with worldnodepth (effect 13,
//! 0x86EB5C), depth-tested, not depth-written and without alpha writes:
//! into the light-streak buffer when the streaks are on (0x5CBB10), else
//! straight into the back buffer (0x5CB5BC).

use glam::{Mat4, Vec3, Vec4};

use crate::formats::hash::bin_hash;
use crate::formats::lights::{Flare, FlarePack};
use crate::particles::Quad;

mod pools;
mod reflection;
mod sprite;
pub use pools::*;
pub use reflection::*;
pub use sprite::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        let eye = Vec3::ZERO;
        let camera = crate::draw::camera::Camera::looking_at(eye, Vec3::new(0.0, 10.0, 0.0));
        View {
            eye,
            right: Vec3::X,
            down: -Vec3::Z,
            forward: Vec3::Y,
            view_projection: camera.view_projection(1.0),
            scale: 256.0 / (camera.fov_y * 0.5).tan(),
            front_end: false,
        }
    }

    #[test]
    fn street_lamp_is_one_quad_scaled_by_depth() {
        let v = view();
        let flare = Flare { position: Vec3::new(0.0, 50.0, 0.0), direction: -Vec3::Z, kind: 9, flags: 2 };
        let mut out = Vec::new();
        sprites(&v, &flare, Mat4::IDENTITY, 1.0, None, &mut out);
        assert_eq!(out.len(), 1);
        let q = &out[0];
        // 4 m towards the eye.
        let centre = (q.corners[0] + q.corners[2]) * 0.5;
        assert!((centre.y - 46.0).abs() < 1e-3);
        // size 30 × clamp(50 / scale, 0.001, 0.1).
        let s = 30.0 * (50.0 / v.scale).clamp(0.001, 0.1);
        assert!(((q.corners[1] - q.corners[0]).length() - 2.0 * s).abs() < 1e-3);
        assert_eq!(q.rgba, [110, 100, 75, 220]);
        assert_eq!(q.texture, bin_hash("HEADLIGHTGLOW"));
    }

    #[test]
    fn faded_and_culled_by_distance() {
        let v = view();
        let at = |d: f32| Flare { position: Vec3::new(0.0, d, 0.0), direction: -Vec3::Z, kind: 9, flags: 2 };
        let mut out = Vec::new();
        sprites(&v, &at(201.0), Mat4::IDENTITY, 1.0, None, &mut out);
        assert!(out.is_empty());
        sprites(&v, &at(190.0), Mat4::IDENTITY, 1.0, None, &mut out);
        let fade = 1.0 - (190.0f32 * 190.0 - 32000.0) / 8000.0;
        assert_eq!(out[0].rgba[3], (220.0 * fade) as u8);
    }

    #[test]
    fn reflection_is_mirrored_and_streaked() {
        let v = view();
        // A lamp 8 m over the road at z = 0, 50 m ahead.
        let lamp = Flare { position: Vec3::new(0.0, 50.0, 8.0), direction: -Vec3::Z, kind: 9, flags: 2 };
        let h = mirror_height(lamp.position, Some(0.0));
        assert_eq!(h, -8.0);
        assert_eq!(mirror_height(lamp.position, None), MIRROR_NONE);
        assert_eq!(mirror_height(Vec3::new(0.0, 0.0, 0.02), Some(0.0)), MIRROR_NONE);
        let mut out = Vec::new();
        reflection_sprites(&v, &lamp, Mat4::IDENTITY, MIRROR_UNSET, 1.2, &mut out);
        assert!(out.is_empty());
        reflection_sprites(&v, &lamp, Mat4::IDENTITY, h, streak_factor(1.0), &mut out);
        assert_eq!(out.len(), 1);
        let q = &out[0];
        assert_eq!(q.rgba, [110, 100, 75, 255]);
        let s = 30.0 * ((Vec3::new(0.0, 50.0, -8.0) - v.eye).dot(v.forward) / v.scale).clamp(0.001, 0.1);
        assert!(((q.corners[1] - q.corners[0]).length() - 2.0 * s).abs() < 1e-3);
        assert!(((q.corners[3] - q.corners[0]).length() - 2.0 * s * 10.0).abs() < 1e-3);
        // A lamp under 2 m above the road streaks half as far.
        let low = Flare { position: Vec3::new(0.0, 50.0, 1.5), ..lamp };
        out.clear();
        reflection_sprites(&v, &low, Mat4::IDENTITY, mirror_height(low.position, Some(0.0)), 10.0, &mut out);
        let q = &out[0];
        let s = 30.0 * ((Vec3::new(0.0, 50.0, -1.5) - v.eye).dot(v.forward) / v.scale).clamp(0.001, 0.1);
        assert!(((q.corners[3] - q.corners[0]).length() - 2.0 * s * 5.0).abs() < 1e-3);
    }

    #[test]
    fn brake_light_faces_backwards() {
        let v = view();
        // A brake light pointing at the eye (−y) glows; pointing away it
        // has no facing term and no size.
        let towards = Flare { position: Vec3::new(0.0, 10.0, 0.0), direction: -Vec3::Y, kind: 1, flags: 4 };
        let away = Flare { direction: Vec3::Y, ..towards };
        let mut out = Vec::new();
        sprites(&v, &towards, Mat4::IDENTITY, 0.85, None, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rgba[0], (0.85f32 * 128.0) as u8);
        out.clear();
        sprites(&v, &away, Mat4::IDENTITY, 0.85, None, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rgba[3], 0);
    }
}

#[cfg(test)]
mod pool_tests {
    use super::*;

    /// 600 ticks of the blink stepped = 600 frames of the per-frame code
    /// (verbatim below), bit for bit, with the dt of a 30/29/31 tick
    /// pattern, pauses (0) and long frames; drawn = S_{n−1} between ticks.
    #[test]
    fn blink_tick_form_is_the_per_frame_path() {
        let (mut time, mut on) = (0.0f32, false);
        let mut pools = Pools::default();
        let mut blink = FlareBlink::default();
        let mut prev = false;
        for i in 0..600 {
            let dt = [30.0f32, 29.0, 31.0][i % 3] / 1800.0 * if i % 50 < 5 { 0.0 } else { 1.0 }
                + if i % 97 == 0 { 0.2 } else { 0.0 };
            // 0x48AC00 as the per-frame code stepped it.
            time += dt;
            if time > 0.5 {
                time = 0.0;
                on = !on;
            }
            pools.reset(dt);
            assert_eq!((pools.blink_clock().time().to_bits(), pools.blink), (time.to_bits(), on), "frame {i}");
            let cur = blink.step(dt);
            assert_eq!((blink.time().to_bits(), cur), (time.to_bits(), on), "tick {i}");
            assert_eq!(FlareBlink::drawn(cur, cur, 0.0), pools.blink);
            for a in crate::render::native::fx::tick::testkit::ALPHAS {
                assert_eq!(FlareBlink::drawn(prev, cur, a), prev);
            }
            prev = cur;
        }
    }

    #[test]
    fn pools_blink_and_fill_as_0x48ac00_and_0x5d6580() {
        let mut p = Pools::default();
        p.reset(0.3);
        assert!(!p.blink);
        p.reset(0.3);
        assert!(p.blink, "past 0.5 s the blink inverts");
        p.reset(0.3);
        assert!(p.blink, "the time was zeroed");
        let t = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        p.add_markers(t, &[(0, Vec3::new(1.0, 0.0, 0.0)), (3, Vec3::new(0.0, 2.0, 1.0))]);
        assert_eq!(p.flares[0].len(), 1);
        assert_eq!(p.flares[0][0].0.kind, 10);
        assert_eq!(p.flares[0][0].0.flags, 2);
        let (b, mask) = &p.flares[1][0];
        assert_eq!((b.kind, b.flags, *mask), (13, 4, 1));
        assert_eq!(b.direction, Vec3::Y);
        for _ in 0..60 {
            p.add_markers(t, &[(3, Vec3::ZERO)]);
        }
        assert_eq!(p.flares[1].len(), POOL_CAPACITY[1]);
        p.reset(0.0);
        assert!(p.flares[0].is_empty() && p.flares[1].is_empty());
    }
}

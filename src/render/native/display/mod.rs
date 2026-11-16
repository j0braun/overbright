//! The Customize Display values (UI_PC_Customize_Options, 0x870CB8…, in
//! `options::Settings::custom` by row order) that the renderer reads
//! besides the post passes' [`super::postfx::Switches`]. See docs/formats.md
//! "Options", **Custom display values in effect**.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

mod monitor;
mod options;
mod present;
mod sampler;
mod surface;
mod ui_frame;
pub use monitor::*;
pub use options::*;
pub use present::*;
pub use sampler::*;
pub use surface::*;
pub use ui_frame::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_and_aspect() {
        // The native six and the fixed extras, a monitor's ultrawide and
        // duplicate modes merged, in the native order; small modes dropped.
        let list = mode_list(&[(3440, 1440), (1920, 1080), (720, 400), (5120, 1440)]);
        assert_eq!(list[..6], NATIVE_MODES);
        assert!(list.contains(&(3840, 2160)) && list.contains(&(3440, 1440)) && !list.contains(&(720, 400)));
        assert_eq!(list.iter().filter(|&&m| m == (1920, 1080)).count(), 1);
        assert!(list.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(crate::frontend::options::step_mode(&list, (1600, 1200), true), (1920, 1080));
        assert_eq!(crate::frontend::options::step_mode(&list, (5120, 1440), true), (5120, 1440));
        // Wide: 4:3 untouched, 16:9 a centred 4:3 canvas and Hor+.
        set_aspect_mode(AspectMode::Wide);
        assert_eq!(canvas_rect((1280, 960)), (0.0, 0.0, 1280.0, 960.0));
        assert_eq!(canvas_rect((3840, 2160)), (480.0, 0.0, 2880.0, 2160.0));
        assert_eq!(canvas_rect((1280, 1024)), (0.0, 32.0, 1280.0, 960.0));
        assert_eq!(view_aspect(4.0 / 3.0), (0.75, 1.0));
        let (hw, x) = view_aspect(16.0 / 9.0);
        assert!((hw - 0.75).abs() < 1e-6 && (x - 0.75).abs() < 1e-6);
        // Narrower than 4:3 too the 4:3 vertical angle (the sides
        // cropped), not the native Vert− (≈ 161° vertical at 9:16).
        let (hw, x) = view_aspect(9.0 / 16.0);
        assert!((hw - 0.75).abs() < 1e-6 && (x - 64.0 / 27.0).abs() < 1e-5);
        assert_eq!(view_aspect(1.25), (0.75, 4.0 / 3.0 / 1.25));
        // Anchored elements: to the screen's edges at 16:9 and 5:4, to the
        // centred 16:9 safe area wider (the HUD to the edges with Full
        // Width, `ultrawide_reach`); nothing moves at 4:3.
        let v = glam::Vec2::new;
        let f = UiFrame::with_hud((1920, 1080), false);
        assert_eq!(f.reach, (240.0, 0.0));
        assert_eq!(f.to_pixels(v(0.0, 0.0), Place::new(-1, 0)), v(0.0, 0.0));
        assert_eq!(f.to_pixels(v(640.0, 480.0), Place::new(1, 1)), v(1920.0, 1080.0));
        assert_eq!(f.to_pixels(v(320.0, 0.0), Place::CENTRE), v(960.0, 0.0));
        assert_eq!(f.scissor(Place::new(1, 0)), (480, 0, 1440, 1080));
        assert_eq!(UiFrame::with_hud((5120, 1440), false).reach, (320.0, 0.0));
        assert_eq!(UiFrame::with_hud((2560, 1080), false).reach, (240.0, 0.0));
        assert_eq!(UiFrame::new((1280, 1024)).reach, (0.0, 32.0));
        assert_eq!(UiFrame::new((1280, 960)).reach, (0.0, 0.0));
        assert_eq!(Place::new(1, 0).origin(v(500.0, 100.0), v(600.0, 200.0)), v(640.0, 150.0));
        // A stretched strip scales about its middle, its y as anchored.
        let strip = Place { x: 0, y: 1, fill: false, stretch: true, hud: false };
        assert_eq!(strip.origin(v(170.0, 430.0), v(600.0, 480.0)), v(385.0, 480.0));
        set_aspect_mode(AspectMode::Stretched);
        assert_eq!(UiFrame::new((3840, 2160)).reach, (0.0, 0.0));
        assert_eq!(canvas_rect((3840, 2160)), (0.0, 0.0, 3840.0, 2160.0));
        assert_eq!(view_aspect(2.0), (0.5, 1.0));
        set_aspect_mode(AspectMode::Wide);
    }

    /// At 21:9 and 32:9 the front end's side elements and (by
    /// default) the HUD stop at the centred 16:9 safe area; HUD Layout
    /// Full Width takes the HUD to the screen's edges. 4:3, 16:9 (and
    /// 1366 × 768) are as before: every reach the whole screen's.
    #[test]
    fn ultrawide_reach() {
        let side = Place::new(1, 0);
        let hud = Place::hud(1, 0);
        let v = glam::Vec2::new;
        // (size, canvas x, 16:9 reach, full-width reach)
        for (size, cx, safe, full) in [
            ((2560, 1080), 560.0, 240.0, 560.0),
            ((3440, 1440), 760.0, 320.0, 760.0),
            ((3840, 1080), 1200.0, 240.0, 1200.0),
            ((5120, 1440), 1600.0, 320.0, 1600.0),
            ((1920, 1080), 240.0, 240.0, 240.0),
            ((2560, 1440), 320.0, 320.0, 320.0),
            ((1366, 768), 171.0, 171.0, 171.0),
            ((1280, 960), 0.0, 0.0, 0.0),
        ] {
            let (w, h) = (size.0 as f32, size.1 as f32);
            let cw = (h * 4.0 / 3.0).round();
            assert_eq!(side_reach(w, h, cx, cw, Some(SAFE_SPREAD)), safe, "{size:?}");
            assert_eq!(side_reach(w, h, cx, cw, None), full, "{size:?}");
            if aspect_mode() != AspectMode::Wide {
                continue; // another test holds the mode (global)
            }
            let centred = UiFrame::with_hud(size, false);
            let wide = UiFrame::with_hud(size, true);
            assert_eq!(
                (centred.reach.0, centred.hud_reach, wide.reach.0, wide.hud_reach),
                (safe, safe, safe, full),
                "{size:?}"
            );
            // The canvas's right edge, anchored right: at the safe area's
            // right edge, or (HUD, Full Width) the screen's.
            assert_eq!(centred.to_pixels(v(640.0, 0.0), hud).x, cx + cw + safe);
            assert_eq!(wide.to_pixels(v(640.0, 0.0), side), centred.to_pixels(v(640.0, 0.0), side));
            assert_eq!(wide.to_pixels(v(0.0, 0.0), Place::hud(-1, 0)).x, cx - full);
            // The mouse maps back through the same reach.
            for (f, p) in [(&centred, side), (&centred, hud), (&wide, side), (&wide, hud)] {
                let px = f.to_pixels(v(600.0, 30.0), p);
                assert!((crate::frontend::mouse::canvas_point(f, px, p) - v(600.0, 30.0)).length() < 1e-3);
            }
        }
        // Within 16:9 the safe area is the screen: the HUD at the edges.
        assert_eq!(side_reach(1920.0, 1080.0, 240.0, 1440.0, Some(SAFE_SPREAD)), 240.0);
    }

    /// The front-end scene cap: nothing at 21:9 and narrower, a centred
    /// 2.4 : 1 region with fading bars at 32:9.
    #[test]
    fn scene_cap_bars() {
        for (w, h) in [(1280.0, 960.0), (1920.0, 1080.0), (2560.0, 1080.0), (3440.0, 1440.0), (2560.0, 1440.0)] {
            assert_eq!(scene_bars_at(w, h), None, "{w}x{h}");
        }
        assert_eq!(scene_bars_at(3840.0, 1080.0), Some((624.0, 43.0)));
        assert_eq!(scene_bars_at(5120.0, 1440.0), Some((832.0, 58.0)));
        // The open region is 2.4 : 1 and centred.
        let (x, _) = scene_bars_at(5120.0, 1440.0).unwrap();
        assert_eq!((5120.0 - 2.0 * x) / 1440.0, 2.4);
        // The HUD's / menus' 16:9 safe area lies inside it.
        assert!(x <= (5120.0 - 1440.0 * SAFE_SPREAD) * 0.5);
    }

    #[test]
    fn scene_period_is_a_thirtieth() {
        assert_eq!(frame_period(false), frame_cap());
        let p = frame_period(true).unwrap();
        assert!(p >= SCENE_FRAME);
        assert!((SCENE_FRAME.as_secs_f64() - 1.0 / 30.0).abs() < 1e-8);
    }

    #[test]
    fn sleep_margin_is_one_ms_off_windows_and_two_on_windows() {
        // Linux keeps its 1 ms (byte-identical pacing); Windows' sleeps wake
        // later, so it spins the last 2 ms.
        assert_eq!(sleep_margin(false), std::time::Duration::from_millis(1));
        assert_eq!(sleep_margin(true), std::time::Duration::from_millis(2));
        // Well under the shortest period a cap or a scene asks for (1/240 s).
        assert!(sleep_margin(true) < std::time::Duration::from_secs_f64(1.0 / 240.0));
    }

    #[test]
    fn wait_until_ends_on_time() {
        let start = std::time::Instant::now();
        let due = start + std::time::Duration::from_millis(5);
        wait_until(due);
        let late = std::time::Instant::now() - due;
        assert!(std::time::Instant::now() >= due);
        assert!(late < std::time::Duration::from_millis(50), "late by {late:?}");
    }

    #[test]
    fn face_schedule() {
        // Rate 1: one face a frame, in order.
        for f in 0..12 {
            let faces = env_faces(1, f);
            assert_eq!(faces.iter().filter(|&&x| x).count(), 1);
            assert!(faces[(f % 6) as usize]);
        }
        // Rate 2: faces 0 and 4 always, then 1, 2, 3, 5.
        let extra: Vec<usize> =
            (0..4).map(|f| (0..6).find(|&i| i != 0 && i != 4 && env_faces(2, f)[i]).unwrap()).collect();
        assert_eq!(extra, [1, 2, 3, 5]);
        assert!((0..8).all(|f| env_faces(2, f)[0] && env_faces(2, f)[4]));
        assert_eq!(env_faces(3, 7), [true; 6]);
        assert_eq!(env_faces(0, 0), [false; 6]);
    }

    #[test]
    fn from_custom() {
        let top = crate::frontend::options::custom_presets(5);
        let d = Display::from_custom(&top);
        assert_eq!(
            d,
            Display { env_rate: 3, env_world: true, shadows: true, headlight: true, world_lod_b: false, fog: true }
        );
        let low = Display::from_custom(&crate::frontend::options::custom_presets(1));
        assert_eq!(
            low,
            Display { env_rate: 0, env_world: false, shadows: false, headlight: false, world_lod_b: true, fog: false }
        );
    }
}

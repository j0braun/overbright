//! Native visuals on ticks (the original's per-frame visual state;
//! docs/original/uncapped-fps.md "Native visuals on ticks"): the original's
//! per-frame visual effects step once per tick, at the end of
//! `Game::step` ([`Game::visuals_tick`]), and a drawn frame takes their
//! values from the two tick snapshots and α ([`FrameVis`]), writing none of
//! them.
//!
//! - The overbright level 0x876848 (0x5BC110 after the frame's draw at
//!   0x5D2B6A: use, then step): S_n = the level as tick n starts; stepped
//!   on the ticks whose frame runs the screen passes (the world's or the
//!   garage's scene), toward the front end's target unless the world is
//!   drawn.
//! - The nitrous blur level 0x870900 (0x5CC470 steps it, then uses it):
//!   S_n = after the step.
//! - The screen tint (slot 3, 0x5D7800): 0x5DBE90 samples it at
//!   0x5CBAEC, after the screen passes read it at 0x5CB920 (the bleach
//!   amount +0x188 and TINTCOLOR.w +0x1CC) in the same frame function
//!   0x5CAAE0, and again at 0x5D2C31 after the present: frame n draws
//!   the values sampled at frame n − 1's camera. S_n = the sample of the
//!   tick before (the clone sampled and used it in the same frame: one tick
//!   of tint, attributed).
//! - The fog (0x614310 at 0x5CB03D, then the world's draw) and the
//!   horizon fog's visibility (0x5DB910 from 0x5C7480 at 0x5CB81C, then its
//!   grid): sampled and used in the frame, S_n = after the step; each keeps
//!   its own smoothing distance (the fog: 50, the rain's fog: the "Fog
//!   Blend Distance" map; the horizon fog: 20).
//! - The flare blink (0x48AC00 at 0x5CBAF1): stepped by the same
//!   expression as the per-frame code (the f32 difference of the sim clock
//!   since the last step; native: the world dt 0x865150, a native fix of
//!   its own), held at S_{n−1} between ticks.
//!
//! Which ticks step: those whose frame draws what the old per-frame code
//! stepped in (the world's scene: `world_under` without a garage request,
//! or the tuning menus over the dyno; the flare blink with Light Glow on,
//! the horizon fog with its switch, the blur with the native speed blur),
//! with photo mode's view swapped in as the frame swaps it. A still (the
//! shots) is drawn without a tick: [`still_step`] steps it as a frame
//! did before.
use crate::*;
use nfsu2::fog::FogSample;
use nfsu2::glow::FlareBlink;
use nfsu2::horizon_fog::HorizonFog;
use nfsu2::motion_blur::BlurSnapshot;
use nfsu2::render::postfx::OverbrightLevel;
use nfsu2::screen_tint::{ScreenTint, TintSample};

/// The drive's visuals of one tick (part of S_n).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct DriveVis {
    /// The drive's car id (a new world: no blend across two drives).
    pub(crate) key: u64,
    /// Slot 3 as the tick's frame draws it (None: not sampled yet).
    pub(crate) tint: Option<TintSample>,
    /// The fog (None: no fog maps, or not sampled yet).
    pub(crate) fog: Option<FogSample>,
    /// The horizon fog's visibility (None: not sampled yet).
    pub(crate) horizon: Option<f32>,
    /// The blur's level and the camera's velocity.
    pub(crate) blur: BlurSnapshot,
    /// 0x82DAF0.
    pub(crate) blink: bool,
}

/// The drive's tick state of the visuals besides the effects' own
/// (`Drive::tint`, `fog`, `horizon_fog`, `motion`, `flare_pools_clock`).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DriveVisuals {
    /// S_n as the last tick left it.
    pub(crate) out: DriveVis,
    /// The tint sampled at this tick's camera, drawn from the next.
    tint_next: Option<TintSample>,
    /// 0x8358E4 and 0x82DAF0.
    pub(crate) blink: FlareBlink,
}

/// The overbright level's tick state.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Visuals {
    /// 0x876848.
    pub(crate) level: OverbrightLevel,
    /// S_n: the level tick n's frame draws with (before its step).
    pub(crate) drawn: OverbrightLevel,
}

/// One drawn frame's values of the visuals.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameVis {
    pub(crate) tint: ScreenTint,
    pub(crate) fog: Option<nfsu2::fog::Fog>,
    /// The rain tint's colour (truncated, held at S_{n−1} between ticks).
    pub(crate) rain_color: [f32; 3],
    pub(crate) horizon: Option<f32>,
    pub(crate) blur: BlurSnapshot,
    pub(crate) blink: bool,
    pub(crate) overbright: OverbrightLevel,
}

impl FrameVis {
    /// The tick's own values (α = 1, or nothing to blend from).
    pub(crate) fn live(drive: Option<&DriveVis>, overbright: OverbrightLevel) -> FrameVis {
        let d = drive.copied().unwrap_or_default();
        FrameVis {
            tint: TintSample::drawn(
                &d.tint.unwrap_or(TintSample::UNSAMPLED),
                &d.tint.unwrap_or(TintSample::UNSAMPLED),
                0.0,
            ),
            fog: FogSample::drawn(d.fog.as_ref(), d.fog.as_ref(), 0.0),
            rain_color: d.fog.map_or([0.0; 3], |f| f.rain_color),
            horizon: d.horizon,
            blur: d.blur,
            blink: d.blink,
            overbright,
        }
    }

    /// `alpha` of the way from S_{n−1} to S_n (both of the same drive):
    /// the continuous values interpolated (`prev` exactly at α = 0), the
    /// discrete ones (the rain colour, the blink) held at `prev`.
    pub(crate) fn between(
        prev: &DriveVis,
        cur: &DriveVis,
        (ob_prev, ob_cur): (OverbrightLevel, OverbrightLevel),
        alpha: f32,
    ) -> FrameVis {
        let un = TintSample::UNSAMPLED;
        let horizon = match (prev.horizon, cur.horizon) {
            (Some(a), Some(b)) => Some(HorizonFog::drawn(a, b, alpha)),
            (a, b) => a.or(b),
        };
        FrameVis {
            tint: TintSample::drawn(&prev.tint.unwrap_or(un), &cur.tint.unwrap_or(un), alpha),
            fog: FogSample::drawn(prev.fog.as_ref(), cur.fog.as_ref(), alpha),
            rain_color: prev.fog.map_or([0.0; 3], |f| f.rain_color),
            horizon,
            blur: BlurSnapshot::lerp(&prev.blur, &cur.blur, alpha),
            blink: FlareBlink::drawn(prev.blink, cur.blink, alpha),
            overbright: OverbrightLevel::drawn(ob_prev, ob_cur, alpha),
        }
    }
}

/// Whether the motion blur (0x5CC470) is the nitrous one (slot 1 of the
/// view's controller): the nitrous stage 1–3, the speed +0x42C positive
/// and 0x5ECDC0 giving 0 (at least 5 mph with the gas on,
/// `carfx::purging`).
pub(crate) fn nitrous_blur(state: &CarState) -> bool {
    let stage = state.drivetrain.as_ref().map_or(0, |t| t.nitrous);
    let speed = state.speed();
    (1..=3).contains(&stage) && speed > 0.0 && !nfsu2::audio::carfx::purging(stage, speed, state.gas())
}

/// What a world frame steps, and how.
#[derive(Clone, Copy)]
pub(crate) struct Steps {
    /// The flare pools are filled (Light Glow, or a still's ungated glows).
    pub(crate) flares: bool,
    pub(crate) horizon_fog: bool,
    pub(crate) blur: bool,
    /// The tint drawn from the tick before (the tick form) or from this
    /// step (a still: sampled and drawn at once, as before).
    pub(crate) tint_now: bool,
}

/// One tick (or a still's frame) of the drive's visuals at the drive's
/// camera; the result in `d.vis.out`.
pub(crate) fn drive_step(d: &mut Drive, s: Steps) {
    let at = d.camera.position.truncate();
    let weight = d.time_of_day.fog_weight();
    d.vis.out.key = d.car_id.key();
    if s.flares {
        // The flare blink, 0x48AC00 (the per-frame code's dt: the sim clock's f32
        // difference since the last empty).
        let dt = (d.clock - d.flare_pools_clock).max(0.0);
        d.flare_pools_clock = d.clock;
        d.vis.out.blink = d.vis.blink.step(dt);
    }
    // Use, then step (a still: step, then use).
    let sampled = d.tint.step(&d.world, at, weight);
    if s.tint_now {
        d.vis.out.tint = Some(sampled);
    } else {
        d.vis.out.tint = d.vis.tint_next;
    }
    d.vis.tint_next = Some(sampled);
    // The fog.
    d.vis.out.fog = d.fog.step(&d.world, at, weight, d.weather.amount);
    // Step, then use.
    if s.blur {
        d.motion.step(nitrous_blur(&d.state));
    }
    d.vis.out.blur = d.motion.snapshot();
    // The horizon fog's visibility.
    if s.horizon_fog {
        d.vis.out.horizon = Some(d.horizon_fog.step(&d.world, at));
    }
}

/// A still's frame (the shots: drawn without a tick): the visuals stepped
/// once at the drive's camera and drawn at once, as every frame did before
/// the tick form (`gate_glow` false: the flares are always filled).
#[cfg_attr(not(feature = "dev-harness"), allow(dead_code))] // the shots' only
pub(crate) fn still_step(d: &mut Drive, settings: &nfsu2::render::FrameSettings, gate_glow: bool) {
    let switches = settings.switches;
    drive_step(
        d,
        Steps {
            flares: !gate_glow || switches.light_glow,
            horizon_fog: switches.horizon_fog,
            blur: native_speed_blur(switches, || settings.opts.motion_blur),
            tint_now: true,
        },
    );
}

impl Game {
    /// The native visuals' tick, at the end of `Game::step` (after
    /// the cameras and the front end's tick: the camera the tick's frame
    /// draws with). Photo mode's view is swapped in as the frame swaps it.
    pub(crate) fn visuals_tick(&mut self) {
        let swapped = self.photo_begin(None);
        let editor = self.free_camera_begin();
        let world = self.sim.drive.is_some()
            && self.garage_fe.request.is_none()
            && (self.tuning_menu_over_world() || world_under(&self.mode, true));
        let garage = !world
            && self.garage_fe.request.as_ref().is_some_and(|r| {
                self.render.garage.as_ref().is_some_and(|g| g.platform.name == r.platform)
            });
        // Use, then step, on the ticks whose frame runs the screen
        // passes (the world's scene or the garage's).
        self.visuals.drawn = self.visuals.level;
        if world || garage {
            self.visuals.level.step(!world);
        }
        if world && let Some(d) = self.sim.drive.as_mut() {
            // The tick's settings (`Pacing::tick_settings`).
            let ts = self.pacing.tick_settings;
            let switches = ts.switches;
            drive_step(
                d,
                Steps {
                    flares: switches.light_glow,
                    horizon_fog: switches.horizon_fog,
                    blur: native_speed_blur(switches, || ts.motion_blur),
                    tint_now: false,
                },
            );
        }
        if let Some(s) = editor {
            self.free_camera_end(s);
        }
        if let Some(s) = swapped {
            self.photo_end(s);
        }
    }

    /// The frame's visuals `alpha` of the way from S_{n−1} to S_n
    /// (a pure read): the tick's own at α ≥ 1, without S_{n−1}, across a
    /// new drive or a camera cut.
    pub(crate) fn frame_vis(&self, alpha: f32) -> FrameVis {
        let (prev, cur) = (&self.interp.prev, &self.interp.cur);
        let live = || FrameVis::live(self.sim.drive.as_ref().map(|d| &d.vis.out), self.visuals.drawn);
        if !(alpha < 1.0) || prev.tick == 0 {
            return live();
        }
        match (prev.vis.as_ref(), cur.vis.as_ref()) {
            (Some(p), Some(c)) if p.key == c.key && !cur.cut => {
                FrameVis::between(p, c, (prev.overbright, cur.overbright), alpha)
            }
            (_, c) => {
                let mut v = FrameVis::live(c, cur.overbright);
                v.overbright = OverbrightLevel::drawn(prev.overbright, cur.overbright, alpha);
                v
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vis(x: f32, blink: bool) -> DriveVis {
        DriveVis {
            key: 7,
            tint: Some(TintSample { f: [x; 7], grey: x }),
            fog: Some(FogSample { row: [x, 10.0 * x, 1.0, 2.0, 3.0], rain_color: [x; 3] }),
            horizon: Some(x),
            blur: BlurSnapshot { level: x, velocity: Vec3::splat(x) },
            blink,
        }
    }

    /// α = 0 draws S_{n−1} bit for bit; between, every continuous value lies
    /// within the two and the discrete ones are S_{n−1}'s (§1.3).
    #[test]
    fn between_is_prev_at_zero_and_bounded() {
        let (p, c) = (vis(0.25, false), vis(0.75, true));
        let ob = (OverbrightLevel(0.3), OverbrightLevel(0.5));
        let a = FrameVis::between(&p, &c, ob, 0.0);
        let l = FrameVis::live(Some(&p), ob.0);
        assert_eq!(a.tint, l.tint);
        assert_eq!(a.fog.map(|f| (f.value.map(f32::to_bits), f.color.map(f32::to_bits))), l.fog.map(|f| (f.value.map(f32::to_bits), f.color.map(f32::to_bits))));
        assert_eq!(a.horizon.map(f32::to_bits), l.horizon.map(f32::to_bits));
        assert_eq!(a.blur, l.blur);
        assert_eq!(a.overbright, ob.0);
        for k in 1..100 {
            let al = k as f32 / 100.0;
            let m = FrameVis::between(&p, &c, ob, al);
            assert!(m.tint.grey >= 0.25 && m.tint.grey <= 0.75);
            assert!(m.horizon.unwrap() >= 0.25 && m.horizon.unwrap() <= 0.75);
            assert!(m.blur.level >= 0.25 && m.blur.level <= 0.75);
            assert!(m.overbright.0 >= 0.3 && m.overbright.0 <= 0.5);
            assert!(!m.blink, "the blink is held at S_n−1");
            assert_eq!(m.rain_color, [0.25; 3], "the rain colour is held at S_n−1");
        }
    }

    /// Unsampled, the tint is off with the front end's grey (0x5CB8E6).
    #[test]
    fn unsampled_tint_is_off() {
        let v = FrameVis::live(None, OverbrightLevel::default());
        assert!(!v.tint.enabled);
        assert_eq!(v.tint.bleach, 0.0);
        assert_eq!(v.tint.grey, 0.2);
    }
}

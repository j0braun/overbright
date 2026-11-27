//! The sky's time-of-day cycle, as SPEED2.EXE drives it.
//!
//! Traced in the executable:
//!
//! * 0x60FAB0, called every frame from 0x5CBAF6, does nothing unless the
//!   game state (0x8654A4) is 6 (in the world) and the cycle is enabled
//!   (0x8A1C78, set by 0x60F690 on every track load at 0x5FF6AA, and
//!   0x802958 = 1). On its first such frame it fills the sky table at
//!   0x8A07A8 ([`SKY_TABLE`]: four main/cap pairs) and turns the dome on
//!   (0x8A1C98).
//! * The clock it reads, 0x7FB718, is the world clock in seconds: EA's
//!   4000 Hz tick counter 0x865188 × 0.00025 (0x57A1A2), which 0x576D00
//!   resets to 4000 ticks (1.0 s) on every track load (0x5FF54D, in the same
//!   load routine 0x5FF4A0 as 0x5FF6AA).
//! * When `ftol(clock) % 300 == 60` and the cycle is idle (state 0x8A1C8C
//!   = 0), the state becomes 2. Everything past that point is gated by
//!   0x5BF1C0, which is `0x870D14 == 1`.
//! * State 2 advances the table index (0x8A1C94, starting at 0) by 2 mod 8
//!   (and copies the old pair to 0x8A1CA0/0x8A1CA4, which nothing reads).
//!   State 4 accumulates frame time (0x865150) in 0x8A1C9C, sets
//!   `0x8A1C88 = 0.5 + 0.5·min(t/20, 1)` (20 s at 0x802954), and when
//!   t > 20 s goes back to state 0 with 0x8A1C88 = 0 and toggles 0x8A1C90.
//!   (State 1 ramps 0..0.5 and then goes to 2, but nothing ever enters it.)
//! * 0x60FA90 / 0x614310: the fog weight of "Normal Fog" is `1 − 0x8A1C88`
//!   while 0x8A1C90 = 0 and `0x8A1C88` while it is 1; "Night Normal Fog"
//!   gets the rest (see [`crate::fog`]).
//! * The dome (0x60F890) binds `SKY_SUNRISE_A`/`_CAP` through a texture
//!   remap list (0x8A2CAC, set on the dome object 0x8A1DFC by 0x488E20 and
//!   applied / undone around the draw by 0x4905A0 / 0x488E40): to the pair at
//!   the table index when 0x5BF1C0 holds, otherwise to `SKY_NIGHT_A`/`_CAP`.
//!   The gate holds in the shipped game (below), so the dome shows the
//!   pair at the index: sunrise at first.
//! * While the state is 4 and 0x870D14 is set, 0x5C5940 draws the dome with
//!   skymorph.efx (effect 20). 0x5CF687 then binds the previous pair
//!   (index − 2) as SKYMORPHTEXTURE (effect +0x894), sets SKYMORPHALPHA
//!   (+0x844) to `t / 20` (0x60FDF0) and SKYMORPHALPHA2 (+0x86C) to
//!   `ftol(t/20 · 255) << 24`; the shader fades from the old pair to the
//!   new one.
//!
//! 0x870D14 (g_SkyMorphFlag) is the options object's +0x64: the object at
//! 0x870CB0 is built by the static constructor 0x77E550 (`ecx = 0x870CB0`
//! → 0x5B7610), which sets +0x64 = 1 (with +0x00 = 1, +0x04 = 0x78, +0x38,
//! +0x68 = 1, the rest 0); the hardware maximum object 0x8764C8 gets the
//! same, and 0x5BE3FF sets its +0x64 (0x87652C) to 1 again, so the clamp
//! at 0x5BE839 keeps 1. The registry options never touch it. **Verified at
//! run time** (2026-10-29, the original under Wine, memory read in free
//! roam): 0x870D14 = 1; at clock 48 s state 0, index 0, the remap list
//! SUNRISE_A → SUNRISE_A; at 63.3 s state 4, index 2, 0x8A1C88 = 0.582,
//! the list SUNRISE_A → NIGHT_A with its info resolved; at 86 s state 0,
//! 0x8A1C90 = 1, and the screen shows the night dome. So the shipped game
//! runs the whole cycle — sunrise for the first minute, then night, sunset,
//! night, every 300 s — and the apitrace's sunrise dome was simply taken in
//! the first minute. `NFS_SKY_MORPH=0` holds the cycle (a comparison aid).

/// The sky table filled by 0x60FAB0 at 0x8A07A8: main, cap for each step.
pub const SKY_TABLE: [&str; 8] = [
    "SKY_SUNRISE_A",
    "SKY_SUNRISE_A_CAP",
    "SKY_NIGHT_A",
    "SKY_NIGHT_A_CAP",
    "SKY_SUNSET_A",
    "SKY_SUNSET_A_CAP",
    "SKY_NIGHT_A",
    "SKY_NIGHT_A_CAP",
];

/// The pair at table index 0, `SKY_SUNRISE_A` / `_CAP`: the dome until the
/// first fade (and, with the cycle held, for ever).
const SHIPPED: usize = 0;

/// Length of a fade (0x802954).
const FADE_SECONDS: f32 = 20.0;
/// The clock period and the second in it that starts a fade.
const PERIOD: i64 = 300;
const START_AT: i64 = 60;

/// What the renderer binds on the dome this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyFrame {
    /// Table index (even) of the pair drawn on the dome.
    pub set: usize,
    /// While fading: the pair faded from, and SKYMORPHALPHA (0 = old pair).
    pub morph: Option<(usize, f32)>,
}

impl Default for SkyFrame {
    /// Table index 0, the sunrise pair, not fading.
    fn default() -> Self {
        SkyFrame { set: SHIPPED, morph: None }
    }
}

/// The state 0x60FAB0 keeps, as reset by 0x60F750 (on unloading a track,
/// 0x5816A7) and enabled by 0x60F690 (on loading one).
#[derive(Clone, Debug)]
pub struct TimeOfDay {
    /// 0x870D14: the detail switch that lets the cycle run.
    pub morph_detail: bool,
    /// 0x8A1C8C.
    state: u8,
    /// 0x8A1C94.
    index: usize,
    /// 0x8A1C9C: seconds into the fade.
    fade_time: f32,
    /// 0x8A1C88.
    factor: f32,
    /// 0x8A1C90.
    toggle: bool,
}

impl Default for TimeOfDay {
    fn default() -> Self {
        // 0x870D14 = 1 (0x77E550 → 0x5B7610); NFS_SKY_MORPH=0 holds it.
        TimeOfDay::new(std::env::var("NFS_SKY_MORPH").map_or(true, |v| v != "0"))
    }
}

impl TimeOfDay {
    pub fn new(morph_detail: bool) -> TimeOfDay {
        TimeOfDay { morph_detail, state: 0, index: 0, fade_time: 0.0, factor: 0.0, toggle: false }
    }

    /// The state `seconds` after a track load, stepping at 60 fps.
    pub fn after(mut self, seconds: f32) -> TimeOfDay {
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        while t < seconds {
            self.update(1.0 + t, dt);
            t += dt;
        }
        self
    }

    /// One frame of 0x60FAB0. `clock` is the world clock (0x7FB718: 1.0 at
    /// track load, then seconds), `dt` the frame time (0x865150).
    pub fn update(&mut self, clock: f32, dt: f32) {
        if (clock as i64).rem_euclid(PERIOD) == START_AT && self.state == 0 {
            self.state = 2;
        }
        if !self.morph_detail {
            return;
        }
        match self.state {
            1 => {
                self.fade_time += dt;
                self.factor = (self.fade_time / FADE_SECONDS).min(1.0) * 0.5;
                if self.fade_time > FADE_SECONDS {
                    self.fade_time = 0.0;
                    self.state = 2;
                }
            }
            2 => {
                self.index = (self.index + 2) & 7;
                self.state = 4;
            }
            4 => {
                self.fade_time += dt;
                self.factor = (self.fade_time / FADE_SECONDS).min(1.0) * 0.5 + 0.5;
                if self.fade_time > FADE_SECONDS {
                    self.factor = 0.0;
                    self.fade_time = 0.0;
                    self.state = 0;
                    self.toggle = !self.toggle;
                }
            }
            _ => {}
        }
    }

    /// 0x60FA90: the weight of "Normal Fog" (the rest goes to "Night
    /// Normal Fog").
    pub fn fog_weight(&self) -> f32 {
        if self.toggle { self.factor } else { 1.0 - self.factor }
    }

    /// The dome's textures this frame (0x60F890, 0x5C5940, 0x5CF687).
    pub fn sky(&self) -> SkyFrame {
        if !self.morph_detail {
            return SkyFrame::default();
        }
        let morph = (self.state == 4).then(|| ((self.index + 6) & 7, self.fade_time / FADE_SECONDS));
        SkyFrame { set: self.index, morph }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run the cycle at 30 fps from track load to `seconds` on the clock.
    fn run(tod: &mut TimeOfDay, from: f32, seconds: f32) {
        let dt = 1.0 / 30.0;
        let mut clock = from;
        while clock < seconds {
            tod.update(clock, dt);
            clock += dt;
        }
    }

    #[test]
    fn held_cycle_keeps_the_sunrise_dome_with_normal_fog() {
        let mut tod = TimeOfDay::new(false);
        run(&mut tod, 1.0, 1000.0);
        assert_eq!(tod.sky(), SkyFrame { set: 0, morph: None });
        assert_eq!(SKY_TABLE[tod.sky().set], "SKY_SUNRISE_A");
        assert_eq!(tod.fog_weight(), 1.0);
        // Stalled in state 2 after the first minute.
        assert_eq!(tod.state, 2);
    }

    #[test]
    fn cycle_runs_sunrise_night_sunset_night() {
        let mut tod = TimeOfDay::new(true);
        run(&mut tod, 1.0, 59.9);
        assert_eq!(tod.sky(), SkyFrame { set: 0, morph: None });
        assert_eq!(tod.fog_weight(), 1.0);

        // One second into the first fade: night over sunrise, the fog
        // weight jumped to 0.5 and heads for 0.
        run(&mut tod, 59.9, 61.0);
        let sky = tod.sky();
        assert_eq!(sky.set, 2);
        let (from, alpha) = sky.morph.unwrap();
        assert_eq!(from, 0);
        assert!(alpha > 0.0 && alpha < 0.1, "{alpha}");
        assert!((tod.fog_weight() - 0.5).abs() < 0.03, "{}", tod.fog_weight());

        run(&mut tod, 61.0, 100.0);
        assert_eq!(tod.sky(), SkyFrame { set: 2, morph: None });
        assert_eq!(tod.fog_weight(), 0.0);

        // Next fade at 360 s: sunset, fog back to "Normal Fog".
        run(&mut tod, 100.0, 400.0);
        assert_eq!(tod.sky().set, 4);
        assert_eq!(tod.fog_weight(), 1.0);
        run(&mut tod, 400.0, 700.0);
        assert_eq!(tod.sky().set, 6);
        assert_eq!(tod.fog_weight(), 0.0);
        run(&mut tod, 700.0, 1000.0);
        assert_eq!(tod.sky().set, 0);
        assert_eq!(tod.fog_weight(), 1.0);
    }
}

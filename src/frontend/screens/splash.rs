use crate::frontend::fe_ids;
use super::*;

/// UG_LS_Splash (screen 0x4C5500 → 0x4A90F0, handler 0x4C5580), the
/// boot's title after the intro movie. The constructor shows
/// 0xC4DF3FF2, hides 0x43D41F73, 0x13CF446D, 0x8C0BD743 and
/// "LicenseBlurb", and labels "start_click" "Press START button"
/// (0xEEEB248E) then "Press Enter Key" (0x1F182080, the PC's, last);
/// the open time goes to +0x50 (0x8651A8, ticks of 1/4000 s). The
/// package's 0x35F8620B (the screen up) stores the time at +0x4C and
/// sends 0x69696969 to Chyron.fng. Each tick (0xC98356BA): 30 s after
/// opening (× 0.00025 > 30.0) the intro movie comes back (0x4C5080 with
/// UG_LS_IntroFMV.fng); else, 16000 ticks (4 s) after +0x4C and once a
/// session (0x836495), "FadeIn" on 0x02DDF58C and 0x4C46F86A — the
/// constructor runs them at once when 0x836495 is already set. Accept
/// (0x406415E3) or Start (0xB5AF2461) once 0x836495 is set: the START
/// sound (0x51DE70 event 9) and the next screen (0x4C4F50).
///
/// Both timers count the real-time clock 0x8651A8 (**traced**):
/// +0x50 is stamped with it at 0x4A9225, +0x4C at 0x4C55CF; the tick
/// compares (0x8651A8 − +0x50) × 0.00025 > 30.0 (0x4C55F8..0x4C561E) and
/// 0x8651A8 − +0x4C > 0x3E80 as integers (0x4C564E..0x4C5656). The
/// package scripts keep their package-clock times (`opened`, `ready`).
#[derive(Clone, Debug)]
pub struct Splash {
    pub opened: f32,
    /// When "FadeIn" ran (0x836495 set), ms.
    pub ready: Option<f32>,
    /// +0x50: 0x8651A8 at the constructor (signed: the harness's `age`
    /// may move it before 0).
    pub opened_real: i64,
    /// +0x4C: 0x8651A8 when the package's "screen up" message came.
    pub shown_real: Option<i64>,
}

impl Splash {
    /// 30.0 s of 0x8651A8 (0x784478 × 0x784268).
    pub const IDLE_S: f32 = 30.0;
    /// 0x3E80 ticks of 1/4000 s.
    pub const PRESS_TICKS: u64 = 0x3E80;
    /// The package's "screen up" message (+0x4C).
    pub const SHOWN: u32 = fe_ids::DIALOG_READY;

    /// `seen`: 0x836495 is set (the prompt was up before this session);
    /// `real`: 0x8651A8 now.
    pub fn new(now: f32, real: u64, seen: bool) -> Splash {
        Splash { opened: now, ready: seen.then_some(now), opened_real: real as i64, shown_real: None }
    }

    /// The screen as if opened `ms` earlier (both clocks; the harness's
    /// shots and selftest).
    pub fn age(&mut self, ms: f32) {
        self.opened -= ms;
        if let Some(r) = self.ready.as_mut() {
            *r -= ms;
        }
        let ticks = (ms * 4.0).round() as i64;
        self.opened_real -= ticks;
        if let Some(s) = self.shown_real.as_mut() {
            *s -= ticks;
        }
    }

    /// When the package sends [`Self::SHOWN`], ms after opening (0 when
    /// it never does).
    fn shown(assets: &Assets) -> f32 {
        assets.package("UG_LS_Splash").and_then(|p| crate::frontend::host_time(p, Self::SHOWN)).unwrap_or(0.0)
    }

    /// The tick (`now` the package clock in ms, `real` 0x8651A8): true
    /// when the intro movie comes back.
    pub fn tick(&mut self, assets: &Assets, now: f32, real: u64) -> bool {
        let real = real as i64;
        if crate::game_clock::real_secs((real - self.opened_real).max(0) as u64) > f64::from(Self::IDLE_S) {
            return true;
        }
        // The package's "screen up" message: its INIT sends it at script
        // time 0, so +0x4C is the open's 0x8651A8 (**inferred**: that the
        // message is handled within the open's iteration; a later time
        // would add its script time, taken here as real time).
        if self.shown_real.is_none() {
            let at = Self::shown(assets);
            if now - self.opened >= at {
                self.shown_real = Some(self.opened_real + (at.max(0.0) * 4.0).round() as i64);
            }
        }
        if self.ready.is_none() && self.shown_real.is_some_and(|s| real - s > Self::PRESS_TICKS as i64) {
            self.ready = Some(now);
        }
        false
    }

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("UG_LS_Splash") else { return Vec::new() };
        let mut state = ScreenState::default();
        for h in [0x43D4_1F73, 0x13CF_446D, 0x8C0B_D743, fe_hash("LicenseBlurb")] {
            state.hidden.insert(h);
        }
        state.shown.insert(fe_ids::START_CLICK);
        if let Some(text) = assets.strings.get(0x1F18_2080) {
            state.text.insert(fe_hash("start_click"), text.to_string());
        }
        if let Some(at) = self.ready {
            for o in [0x02DD_F58C, 0x4C46_F86A] {
                state.scripts.insert(o, (fe_hash("FadeIn"), at - self.opened));
            }
        }
        layout(assets, package, &state, now - self.opened)
    }
}

//! The engine sound's transients: the `SFXCTL_Shifting` blip
//! machine (update 0x483130 → 0x482D50, states 1–7 at +0x78) and the
//! `SFXCTL_AccelTrans` launch / tip-in machine (update 0x4831A0, states
//! 0–4 at +0xC8). Both run before `SFXCTL_Engine` (0x47C010), which takes
//! their rpm / throttle ramps in place of the sound context's while they
//! are active (0x46F7D0, 0x45C710), the shift's volume bump (+0xE0,
//! 0x45C6C0) and its wobble oscillators (0x45C7A0). See
//! docs/formats.md, "Engine transients".
//!
//! Verified against SPEED2.EXE unless marked; the controllers' update
//! order (Physics, Shifting, AccelTrans, Engine, HybridMotor) is
// prov: inferred audio.transients.module
//! **inferred** from their dependencies.

mod accel;
mod shifting;
mod tables;

pub use crate::audio::util::ramp::*;
pub use accel::AccelTrans;
pub use shifting::{Physics, Shifting};
pub use tables::*;

/// `Sin16` (0x43C2B0), defined with the simulation that also uses it ([`crate::handling::sin16`]).
pub use crate::handling::sin16;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_ends() {
        assert_eq!(curve2(0x7FFF), 0);
        assert!(curve2(0) >= 32700);
        let mut r = Ramp::raw(1000.0, 2000.0, 1.0, 6);
        r.step(0.5);
        assert!((r.value - 1500.0).abs() < 10.0, "{}", r.value);
        r.step(0.6);
        assert!(r.done && r.value == 2000.0);
    }

    fn phys(rpm: f32, gear: i32, prev: i32) -> Physics {
        Physics {
            rpm,
            throttle: 100.0,
            input: 100.0,
            prev_input: 100.0,
            gear,
            prev_gear: prev,
            on: true,
            mph: 40.0,
            up_rpm: 7000.0,
            race_type_8: false,
            scripted: false,
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn upshift_runs_its_states() {
        let rec = SHIFT_RECS[0];
        let mut s = Shifting::default();
        let mut eng = (6000.0, 6500.0);
        s.update(&phys(5000.0, 3, 2), &rec, &mut eng, 100.0, 0.016);
        assert_eq!(s.state, 1);
        assert!(s.swtn_start);
        // The engine rpm put back to the last update's.
        assert_eq!(eng, (6500.0, 6000.0));
        let mut seen = vec![1];
        let mut clunk = false;
        for _ in 0..200 {
            s.update(&phys(5000.0, 3, 3), &rec, &mut eng, 100.0, 0.016);
            clunk |= s.play;
            if *seen.last().unwrap() != s.state {
                seen.push(s.state);
            }
        }
        assert!(clunk);
        assert_eq!(seen, vec![1, 2, 3, 0]);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn downshift_blips() {
        let rec = SHIFT_RECS[4];
        let mut s = Shifting::default();
        let mut eng = (4000.0, 4000.0);
        s.update(&phys(5000.0, 3, 4), &rec, &mut eng, 0.0, 0.016);
        assert_eq!(s.state, 4);
        let mut seen = vec![4];
        let mut top: f32 = 0.0;
        for _ in 0..300 {
            s.update(&phys(5000.0, 3, 3), &rec, &mut eng, 0.0, 0.016);
            top = top.max(s.rpm.value);
            if *seen.last().unwrap() != s.state {
                seen.push(s.state);
            }
        }
        assert_eq!(seen, vec![4, 5, 6, 7, 0]);
        assert!(top > 5500.0, "{top}");
        // Into neutral: nothing.
        let mut n = Shifting::default();
        n.update(&phys(5000.0, 1, 2), &rec, &mut eng, 0.0, 0.016);
        assert_eq!(n.state, 0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn launch_and_tip_in() {
        let rec = LAUNCHES[3];
        let mut a = AccelTrans::default();
        let mut p = phys(1200.0, 2, 2);
        p.mph = 0.0;
        p.on = false;
        p.input = 0.0;
        a.update(&p, 0, false, 1200.0, rec, 0.016);
        p.on = true;
        p.input = 100.0;
        p.prev_input = 0.0;
        a.update(&p, 0, false, 1200.0, rec, 0.016);
        assert_eq!(a.state, 2);
        p.prev_input = 100.0;
        for _ in 0..40 {
            a.update(&p, 0, false, 1200.0, rec, 0.016);
        }
        assert_eq!(a.state, 3);
        // Lifting ends it through state 4.
        p.on = false;
        p.input = 0.0;
        a.update(&p, 0, false, 5000.0, rec, 0.016);
        assert_eq!(a.state, 4);
        for _ in 0..200 {
            a.update(&p, 0, false, 1200.0, rec, 0.016);
        }
        assert_eq!(a.state, 0);
        // A tip-in in third at 4000 rpm, 2 s after the last.
        let mut q = phys(4000.0, 4, 4);
        q.on = true;
        q.prev_input = 0.0;
        a.on = false;
        a.update(&q, 0, false, 4000.0, rec, 0.016);
        assert_eq!(a.state, 1);
        assert!(a.rpm.value > 4900.0 && a.rpm.from == 5000.0, "{}", a.rpm.value);
    }
}

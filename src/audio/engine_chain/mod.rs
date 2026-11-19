//! The player engine's volume chain: SFXCTL_Physics (0x484C50)
//! fills the sound context, SFXCTL_Engine (0x47C010) filters it and runs the
//! rev limiter (0x46F610), SFXCTL_HybridMotor (0x47C3D0) turns it into the
//! layer volumes, and CARSFX_PlayerEngine (0x474030 / 0x473E60) drives one
//! granular voice per recording: the accel GIN (+0x80) and, for hybrid
//! entries, the decel `_DCL` GIN (+0x84). See docs/original/engine-volume-chain.md.
//!
//! The SFXCTL_Shifting blip machine (0x482D50) and the SFXCTL_AccelTrans
//! launch / tip-in machine (0x4831A0) run here too
//! ([`super::transients`]). Pattern uses the Physics +0xD8 scripted input
//! path 0x46FA30; other patterns remain unported. The 0x5ECD80 predicate
//! (the human car mid gear change) is [`EngineInput::shifting`].

mod motor;
mod records;

pub use crate::audio::util::interp::{Interp, Mean4};
pub use motor::*;
pub use records::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn engine_cutoff_follows_the_camera_distance() {
        // 0x46F460: within 6.5 m of the camera (or with none) curve 4's
        // first entry, lin 32655 → 23920, which the original sent as p21
        // at the airport's chase view (under Wine).
        assert_eq!(HybridMotor::engine_cutoff(-1, false), 23920);
        assert_eq!(HybridMotor::engine_cutoff(6, false), 23920);
        assert!(HybridMotor::engine_cutoff(10, false) < 23920);
        let far = HybridMotor::engine_cutoff(40, false);
        assert!(far < 2000 && far >= 725, "{far}");
        // The crash / jump cameras stretch it over 90 m.
        assert!(HybridMotor::engine_cutoff(40, true) > far);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn records_match_the_exe() {
        let r = ENGINES[0];
        assert_eq!((r.gin, r.decel, r.vol_a, r.vol_b, r.cutoff), ("GIN_HondaS2000_Church.gin", 3, 18000, 20000, 300));
        assert_eq!((r.rpm_max, r.rpm_min), (9229.0, 1350.0));
        assert!(!ENGINES[9].hybrid && !ENGINES[38].hybrid);
        let d = DECELS[3];
        assert_eq!((d.gin, d.vol_a, d.vol_d, d.rpm0, d.rpm1), ("GIN_S2000_Church_DCL.gin", 10000, 14500, 3692, 9182));
        for (k, e) in ENGINES.iter().enumerate() {
            assert_eq!(e.gin, format!("GIN_{}.gin", crate::audio::engine::GINS[k]), "{k}");
        }
    }

    /// A motor past its first updates in gear 3: the first update sees
    /// gear 1 as the last (0x484CEE), so gear 3 starts an up-shift; run it
    /// out at idle.
    fn settled(entry: usize) -> HybridMotor {
        let mut h = HybridMotor::new(entry);
        let inp = EngineInput { rpm: 1000.0, idle_rpm: 1000.0, max_rpm: 8000.0, gear: 3, ..Default::default() };
        for _ in 0..600 {
            h.update(&inp, 1.0 / 60.0);
            if h.shift.state == 0 && h.accel.state == 0 {
                return h;
            }
        }
        panic!("the first up-shift never ends");
    }

    fn drive(h: &mut HybridMotor, throttle: f32, rpm: f32, n: usize) {
        let inp = EngineInput { throttle, rpm, idle_rpm: 1000.0, max_rpm: 8000.0, gear: 3, ..Default::default() };
        for _ in 0..n {
            h.update(&inp, 1.0 / 60.0);
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn throttle_crossfades_accel_and_decel() {
        let mut h = settled(0);
        drive(&mut h, 1.0, 5000.0, 60);
        assert_eq!(h.ctx.throttle, 100.0);
        // Steady rpm: B = rec80 +0x3C (0.15), so +0x80 = 100 · 20000 · 0.15 · 0.01.
        assert_eq!(h.out[1..3], [3000, 0], "{:?}", h.out);
        // Rising rpm (|+0xD0| ≥ 150 a frame) lifts B toward +0x34 (0.85).
        let inp =
            |rpm| EngineInput { throttle: 1.0, rpm, idle_rpm: 1000.0, max_rpm: 8000.0, gear: 3, ..Default::default() };
        for k in 0..8 {
            h.update(&inp(2000.0 + 200.0 * k as f32), 1.0 / 60.0);
        }
        assert!(h.out[1] > 10000, "{:?}", h.out);
        // Off the throttle inside the decel band: the decel voice only.
        drive(&mut h, 0.0, 5000.0, 60);
        assert!(h.out[1] == 0 && h.out[2] > 1000, "{:?}", h.out);
        // Off the throttle below the band: both silent.
        drive(&mut h, 0.0, 1000.0, 60);
        assert_eq!(h.rpm_map, 1350.0);
        assert_eq!(&h.out[1..3], &[0, 0]);
        // The rpm map: ctx 1000..10000 onto the record's range.
        drive(&mut h, 1.0, 8000.0, 5);
        assert_eq!(h.ctx.rpm, 10000.0);
        assert_eq!(h.rpm_map, 9229.0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn abk_block_carries_the_layers() {
        let mut h = settled(0);
        drive(&mut h, 1.0, 5000.0, 60);
        let b = h.abk_block(0x7FFF);
        // On throttle at steady rpm: A = 0.85, +0x7C = 18000 · 0.85.
        assert_eq!(h.out[0], 15300);
        assert_eq!((b[0], b[3], b[4], b[5], b[12], b[25]), (0, 1024, ((15300 * 0x7FFF) >> 15) - 0x7FFF, b[4], 1, 0));
        assert_eq!(HybridMotor::new(41).abk_path(), "SOUND/ENGINE/CAR_14_ENG_MB_EE.abk");
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn rev_limiter_ramps_down_and_back() {
        let mut h = settled(0);
        drive(&mut h, 1.0, 7000.0, 30);
        let full = h.out[1];
        // ctx rpm 10000 > 9800: to 0.15 over 450 · 3 ms.
        drive(&mut h, 1.0, 8000.0, 120);
        assert!((h.limiter.value - 0.15).abs() < 1e-6);
        assert!(h.out[1] < full / 4);
        drive(&mut h, 1.0, 7000.0, 30);
        assert_eq!(h.limiter.value, 1.0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn first_update_sees_gear_1_and_input_0_as_the_last() {
        // 0x484CEE: +0xCC = old +0xC8, +0x90 = old +0x8C; the constructor
        // leaves +0xC8 = 1 and +0x8C = 0 (read live under Wine on the first
        // update of three objects).
        let mut h = HybridMotor::new(0);
        let inp = EngineInput { throttle: 0.5, rpm: 3000.0, idle_rpm: 1000.0, max_rpm: 8000.0, gear: 3, ..Default::default() };
        h.update(&inp, 1.0 / 60.0);
        assert_eq!((h.phys.prev_gear, h.phys.prev_input), (1, 0.0));
        h.update(&inp, 1.0 / 60.0);
        assert_eq!((h.phys.prev_gear, h.phys.prev_input), (3, 50.0));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_gear_change_takes_the_decel_gains() {
        // 0x470613 / 0x47024C: while 0x5ECD80 says the human car is
        // changing gear (transmission +0x48, read live under Wine),
        // the Shifting jump table 0x470924 is taken even with its state 0,
        // and state 0 goes to the default 0x4706ED: the rec84 (decel) lerp,
        // not snapped, instead of the AccelTrans on-throttle rec80 lerp.
        let run = |entry: usize| {
            let mut h = settled(entry);
            let mut inp =
                EngineInput { throttle: 1.0, rpm: 5000.0, idle_rpm: 1000.0, max_rpm: 8000.0, gear: 3, ..Default::default() };
            for _ in 0..60 {
                h.update(&inp, 1.0 / 60.0);
            }
            let before = (h.gain_a, h.gain_b);
            let mut plain = h.clone();
            plain.update(&inp, 1.0 / 60.0);
            inp.shifting = true;
            h.update(&inp, 1.0 / 60.0);
            assert_eq!((h.shift.state, h.accel.state, h.d0), (0, 0, 0.0));
            (before, (plain.gain_a, plain.gain_b), (h.gain_a, h.gain_b))
        };
        let step = |from: f32, to: f32| to.clamp(from - 0.2, from + 0.2);
        // Hybrid entry 0 at steady rpm (+0xD0 = 0, so t1 = t2 = 0) and full
        // throttle (the trapezoid mix f = 1): rec80 (a0, b0) without the
        // shift, rec84 (a0, b0) with it, each by the ±0.2 step.
        let (r, d) = (ENGINES[0], DECELS[ENGINES[0].decel]);
        let ((a, b), plain, shift) = run(0);
        assert_eq!(plain, (step(a, r.a0), step(b, r.b0)));
        assert_eq!(shift, (step(a, d.a0), step(b, d.b0)));
        assert_ne!(plain, shift);
        // Single entry 9: the fixed gains instead of the lerp.
        let r = ENGINES[9];
        let ((a, b), plain, shift) = run(9);
        assert_eq!(plain, (step(a, r.a0), step(b, r.b0)));
        assert_eq!(shift, (step(a, r.fixed_a), step(b, r.fixed_b)));
        assert_ne!(plain, shift);
        // Scripted updates (Physics +0xD8) skip the test.
        let mut h = HybridMotor::new(0);
        let mut p = crate::audio::scripted::Pattern::default();
        p.start();
        p.step();
        h.update_scripted(&EngineInput { shifting: true, ..Default::default() }, &p, 1.0 / 60.0);
        assert!(!h.shifting);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn idle_layer_level_is_the_originals() {
        // Read live in the original (free roam, the 206: engine entry 23,
        // standing, engine volume only, master 1): CAR p4 = p5 = −27196 at
        // level0 18257 (p14 2889 = 5186 · level0 >> 15), the GIN bytes 0.
        let mut h = HybridMotor::new(23);
        let inp = EngineInput { throttle: 0.0, rpm: 800.0, idle_rpm: 800.0, max_rpm: 7000.0, gear: 2, ..Default::default() };
        for _ in 0..120 {
            h.update(&inp, 1.0 / 60.0);
        }
        let b = h.abk_block_fx(18257, 5186, 0x7FFF);
        assert_eq!((b[4], b[5], b[14]), (-27196, -27196, 2889));
        assert_eq!(h.voices(18257, 0.0).1, [0, 0]);
    }

    #[test]
    fn interp_shapes() {
        let mut i = Interp::at(0.0);
        i.set(0.0, 1.0, 100, 2);
        i.step(50.0);
        assert!((i.value - 0.25).abs() < 1e-6);
        i.step(60.0);
        assert!(i.done && i.value == 1.0);
    }
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn scripted_physics_matches_native_context_without_gear_refresh_or_launch() {
        let mut h = HybridMotor::new(0);
        let mut input = EngineInput { rpm: 1000.0, idle_rpm: 1000.0, max_rpm: 8000.0, gear: 2, ..Default::default() };
        // Two updates: the first sees the constructor's gear 1 as the last
        // (0x484CEE), the second gear 2, as the capture's +0xCC holds.
        h.update(&input, 1.0 / 60.0);
        h.update(&input, 1.0 / 60.0);
        input.gear = 5; // Native scripted branch retains its prior gear.
        let mut pattern = crate::audio::scripted::Pattern::default();
        pattern.start();
        let fixture =
            &crate::testkit::private("docs/evidence/nis-audio-pattern/natural-physics.bin");
        for (i, row) in fixture.chunks_exact(244).enumerate() {
            pattern.step();
            h.update_scripted(&input, &pattern, 1.0 / 60.0);
            let bits = |offset| u32::from_le_bytes(row[offset..offset + 4].try_into().unwrap());
            assert_eq!(h.ctx.rpm.to_bits(), bits(0x70), "RPM update {i}");
            assert_eq!(h.ctx.throttle.to_bits(), bits(0x84), "throttle update {i}");
            assert_eq!(h.ctx.input.to_bits(), bits(0x8c));
            assert_eq!(h.phys.prev_input.to_bits(), bits(0x90));
            assert_eq!(h.phys.gear as u32, bits(0xc8));
            assert_eq!(h.phys.prev_gear as u32, bits(0xcc));
            assert_eq!(h.ctx.on, row[0xbc] != 0);
            assert_eq!(h.accel.state, 0, "scripted launch must remain suppressed");
        }
        h.update(&input, 1.0 / 60.0);
        assert_eq!(h.phys.gear, 5);
        assert_eq!(h.ctx.rpm, 1000.0);
    }
}

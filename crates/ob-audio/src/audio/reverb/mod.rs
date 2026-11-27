//! The reverb (W8): the sound library's FX graphs (`SOUND\FXEDIT\*.fx`)
//! and SFXObj_Reverb / SFXCTL_Tunnel's choice of preset and wet level.
//!
//! **The graph** (verified, sound7): 0x726DB3 parses a preset for the
//! output's channel count N (0x8B47F9; 2 here): a big-endian file
//! `"FX" 0B 00`, size, then per channel config c = 1..6 a module count
//! (header dword 1 + c) and a link count (dword 7 + c); the config used is
//! the largest c ≤ N with modules (every preset has c = 1 and 2). The
//! config's module records follow those of the configs before it (record
//! sizes 0x818A08 = {4, 8, 4, 4, 8, 8, 4, 8, 4, 4} by kind), then its links
//! of 12 bytes {u32 from, u32 to, u8 from port, u8 to port}: 0x72A26D sets
//! to's input (port 1 → +8, 2 → +0xC) = from and +0x1A = the from port. A
//! record's byte 1 names the output channel it feeds (0x72653C's table
//! 0x8188DD: for c = N = 2, channel k → output slot k; kind 9 holds two
//! channels in its nibbles). 0x7268BF builds the modules (sizes 0x8189E0)
//! with the sample rate sr (0x8B44C6, 44100 = 0xAC44 at 0x724A69, clamped
//! to the driver's range) and file words p2 / p4 / p6 (+2 / +4 / +6):
//!
//! | kind | module | process | parameters |
//! |---|---|---|---|
//! | 0 | input | 0x72A138 | the bus buffer (0x72A164) |
//! | 1 | one-pole low-pass | 0x72AA55 → 0x72D8B0 | a = 2·p4 / sr; g = p6 / 256; y = g(1 − a)·y₁ + g·a·x (0x72AABE) |
//! | 2 | 9-tap FIR low-pass | 0x72A9B3 → 0x72D616 | f = 2·p2 / sr (0x72AA21, 0x72D697 type 2) |
//! | 3 | 9-tap FIR high-pass | 0x72A8C5 → 0x72D616 | f = min(2·p2 / sr, 0.8) (0x72A981, type 3) |
//! | 4 | 9-tap FIR band-pass | 0x72A8C5 → 0x72D616 | f₁ = min(2·p4 / sr, 0.8), f₂ = 2·p6 / sr (0x72A90E, type 4) |
//! | 5 | two-pole resonator | 0x72A7A1 → 0x72A6F8 | f = p2, bandwidth p4, g = p6 / 256 (0x72A817) |
//! | 6 | gain | 0x72A672 | p2 / 256 (0x72A6E4) |
//! | 7 | allpass | 0x72A541 → 0x72A481 | g = (p2 >> 1) / 127, delay p4 ms (0x72A602 → 0x72D567) |
//! | 8 | sum of two inputs | 0x72A3D9 | — |
//! | 9 | splitter (two readers) | 0x72A2F3 | — |
//!
//! Every module pulls its inputs (vtable +0, (module, n, scratch, out,
//! port)); the process hook 0x72655A (called at 0x7294CC after the voices
//! are mixed) points the input at the bus buffer, pulls output slot 1 then
//! slot 2 into a scratch buffer and adds it × the FX's channel gain (+0x68,
//! 1.0 from the load; no writer of +0x68 / +0x80 besides it) into output
//! channel 0 / 1 (0x72C360: dst += src · g), then clears the bus. The bus
//! (one, 0x8180B8 = 1) is fed by every voice with a nonzero send (0x729453:
//! voice +0x38[bus] · its 16-sample block, and that resets the idle count
//! 0x8B47C8 to 0); the hook adds 1 to the idle count and runs only below
//! 3000, so a graph stops 3000 mix calls after the last send (the load sets
//! 3000: a fresh graph waits for a send). The mixer calls the hook once per
//! chunk of at most 256 frames (0x7295A8). Samples are floats in the 16-bit
//! range. The coefficient formulas use the library's own series (cos
//! 0x72A032, sin 0x72A0AE: Taylor to x¹² / x¹³ after subtracting 2π while
//! x > 2π; √(1 + x) 0x72D581: the binomial series to x⁷).
//!
//! **The send** (verified): 0x723BC1(voice, bus, byte) stores byte / 127
//! and 0x729E24 makes the voice's send byte / 127 × its volume (+0x48).
//! Callers: the player engine's two GIN voices (0x473F0E, 0x4740E3,
//! 0x474244: byte = clamp(([0x82B078] · level) >> 23, 0, 127), and the dry
//! "second volume" byte clamp([0x82A998] >> 8, 0, 127), 0x723C41), the
//! interactive music's streams (0x46C470 from 0x73A1A1 / 0x73A26A /
//! 0x73BF6C, **not ported**: which events use it is open) and the event
//! graphs' parameter kind 5 (0x720936; the car banks use none).
//!
//! **The levels** (verified, [`ReverbCtl`]): the preset table 0x7F3778
//! ({wet A, dry A, wet B, dry B} mB, fade-out ms, fade-in ms per preset)
//! gives four targets (0x467FF0 / 32767); wet A drives the engine sends,
//! wet B / dry B the engine ABK block's p14 / p13.

mod control;
mod dsp;
mod graph;

pub use control::*;
pub use dsp::*;
pub use graph::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mixmap::db_to_lin;

    #[test]
    fn series_match_libm() {
        for x in [0.0f32, 0.3, 1.0, 2.0, 3.0] {
            assert!((cos_approx(x) - (x as f64).cos()).abs() < 1e-4, "cos {x}");
            assert!((sin_approx(x) - (x as f64).sin()).abs() < 1e-4, "sin {x}");
        }
        assert!((sqrt1p_approx(-0.25) - 0.75f64.sqrt()).abs() < 1e-4);
    }

    #[test]
    fn allpass_lengths() {
        // 20 ms: 44100 · 1 / 1000 = 44 → 47; 90 ms: 5 → 220 → 223;
        // 100 ms: 6 → 264 → 269; 60 ms: 3 → 132 → 137; 10 ms: 0 → 3.
        assert_eq!(allpass_len(44100, 20), 752);
        assert_eq!(allpass_len(44100, 90), 16 * 223);
        assert_eq!(allpass_len(44100, 100), 16 * 269);
        assert_eq!(allpass_len(44100, 60), 16 * 137);
        assert_eq!(allpass_len(44100, 10), 48);
    }

    #[test]
    fn fir_unit_gain() {
        // Low-pass: DC gain 1 (2Σc0..c3 + c4); high-pass: Nyquist gain 1.
        let lp = fir_design(2, 0.0, 0.25);
        let dc: f32 = 2.0 * lp[..4].iter().sum::<f32>() + lp[4];
        assert!((dc - 1.0).abs() < 1e-6, "{lp:?}");
        let hp = fir_design(3, 0.0317, 0.0);
        let ny: f32 = 2.0 * (hp[0] - hp[1] + hp[2] - hp[3]) + hp[4];
        assert!((ny - 1.0).abs() < 1e-6, "{hp:?}");
    }

    /// A preset of `mods` (stereo config only) and `links`.
    fn fx(mods: &[&[u8]], links: &[(u32, u32, u8, u8)]) -> Vec<u8> {
        let mut b = b"FX\x0b\x00".to_vec();
        let words = [0u32, 1, mods.len() as u32, 0, 0, 0, 0, 0, links.len() as u32, 0, 0, 0, 0];
        for w in words {
            b.extend_from_slice(&w.to_be_bytes());
        }
        b.extend_from_slice(&[0, 0, 0, 0]); // the mono config: one input
        for m in mods {
            b.extend_from_slice(m);
        }
        for &(f, t, pf, pt) in links {
            b.extend_from_slice(&f.to_be_bytes());
            b.extend_from_slice(&t.to_be_bytes());
            b.extend_from_slice(&[pf, pt, 0, 0]);
        }
        b
    }

    fn run(g: &mut Graph, input: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let (mut l, mut r) = (vec![0.0; input.len()], vec![0.0; input.len()]);
        for (k, chunk) in input.chunks(BLOCK).enumerate() {
            let (a, b) = (&mut l[k * BLOCK..k * BLOCK + chunk.len()], &mut r[k * BLOCK..k * BLOCK + chunk.len()]);
            g.process(chunk, true, [a, b]);
        }
        (l, r)
    }

    #[test]
    fn gain_and_one_pole_by_hand() {
        // input → gain 384/256 → slot 1; input → one-pole (5000 Hz, 1.0) → slot 2 via a splitter.
        let f = fx(
            &[&[0, 0, 0, 0], &[9, 0, 0, 0], &[6, 1, 0x01, 0x80], &[1, 2, 0, 0, 0x13, 0x88, 0x01, 0x00]],
            &[(0, 1, 1, 1), (1, 2, 1, 1), (1, 3, 2, 1)],
        );
        let mut g = Graph::parse(&f, 44100).unwrap();
        let (l, r) = run(&mut g, &[1000.0, 0.0, 0.0, 500.0]);
        assert_eq!(l, vec![1500.0, 0.0, 0.0, 750.0]);
        let a = (10000.0f64 / 44100.0) as f32;
        let b = (1.0 - 10000.0f64 / 44100.0) as f32;
        let mut y = 0f32;
        let want: Vec<f32> = [1000.0f32, 0.0, 0.0, 500.0]
            .iter()
            .map(|x| {
                y = y * b + a * x;
                y
            })
            .collect();
        assert_eq!(r, want);
    }

    #[test]
    fn allpass_impulse_by_hand() {
        // An allpass of g = (100 >> 1) / 127, 20 ms (752 samples) on slot 1.
        let f = fx(&[&[0, 0, 0, 0], &[7, 1, 0, 100, 0, 20, 0, 0]], &[(0, 1, 1, 1)]);
        let mut g = Graph::parse(&f, 44100).unwrap();
        let mut x = vec![0.0f32; 3 * 752 + 10];
        x[0] = 1.0;
        let (l, r) = run(&mut g, &x);
        assert!(r.iter().all(|&v| v == 0.0));
        let gg = (50.0f64 * (1.0f32 / 127.0) as f64) as f32;
        // y0 = −g·(1 + guard); y752 = v0 − g·v752 with v752 = g·v0 + guard.
        let v0 = 1.0f32 + GUARD;
        assert_eq!(l[0], -gg * v0);
        assert_eq!(l[752], -gg * (gg * v0 + GUARD) + v0);
        assert!(l[1..752].iter().all(|v| v.abs() < 1e-18));
    }

    #[test]
    fn splitter_alternates_and_idles() {
        let f = fx(&[&[0, 0, 0, 0], &[9, 0x21, 0, 0]], &[(0, 1, 1, 1)]);
        let mut g = Graph::parse(&f, 44100).unwrap();
        let (l, r) = run(&mut g, &[3.0; 4]);
        assert_eq!((l, r), (vec![3.0; 4], vec![3.0; 4]));
        // Idle: a fresh graph waits for a send; 2999 silent chunks run, then it stops.
        let mut g = Graph::parse(&f, 44100).unwrap();
        let (mut a, mut b) = ([0.0f32; 4], [0.0f32; 4]);
        assert!(!g.process(&[0.0; 4], false, [&mut a, &mut b]));
        assert!(g.process(&[0.0; 4], true, [&mut a, &mut b]));
        for _ in 0..2998 {
            assert!(g.process(&[0.0; 4], false, [&mut a, &mut b]));
        }
        assert!(!g.process(&[0.0; 4], false, [&mut a, &mut b]));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn fades_follow_the_table() {
        let mut c = ReverbCtl::default();
        // The game start: preset 8 asked, fade out (140 ms at 5), then in (60 ms).
        c.request(8, 0);
        let mut frames = 0;
        while !c.reload {
            c.update(1.0 / 60.0);
            frames += 1;
            assert!(frames < 100);
        }
        // 140 ms of 60 Hz frames: done on frame 9, switched on the next.
        assert_eq!((frames, c.current), (10, 8));
        for _ in 0..10 {
            c.update(1.0 / 60.0);
        }
        // Hills: −1600 mB → 0x467FF0.
        let want = ((db_to_lin(-1600) as f64 * (1.0f32 / 32767.0) as f64) as f32 as f64 * 32767.0) as i32;
        assert!((c.out[0] - want).abs() <= 1, "{:?} {want}", c.out);
        assert!(c.out[1] >= 0x7FFE);
        // A tunnel: Simple_Tunnel at 0 mB, so the send byte at full level is 127.
        c.frame(Some(5), None, 1.0 / 60.0);
        for _ in 0..40 {
            c.frame(Some(5), None, 1.0 / 60.0);
        }
        assert_eq!(c.current, 5);
        assert_eq!(c.engine_bytes(0x7FFF), (127, 127));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn presets_parse() {
        let root = crate::testkit::game_root();
        for k in 0..12 {
            let g = Graph::load(&root, k, 44100).unwrap_or_else(|| panic!("{}", PRESETS[k]));
            assert!(g.outs.iter().all(Option::is_some), "{}", PRESETS[k]);
        }
    }
}

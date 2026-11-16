//! Authored car-audio pattern state (native SFXCTL_Physics 0x46FA30).
//!
//! A CAR_AUDIO_PATTERN event (command 7, 0x439547 → ctx vtable +0x48 →
//! 0x459550) latches the pattern ID at ctx +0xC4 (pending byte +0xC0).
//! While the car's Physics controller is scripted (+0xD8, see
//! `shop_scene::audio`), each audio update runs 0x46FA30: a pending ID
//! selects one of 15 banks (0x7F0070..0x7F08F8; ID 2..15 its own, any
//! other the default bank 1), then every update advances a quarter of a
//! segment (linear rpm / throttle between {u16 rpm, u8 throttle} records);
//! the last segment's end switches to idle (−500 rpm, −15 throttle a
//! update), clamped to 1000..10000 rpm and 0..100. The result is the sound
//! context's rpm (+0x48) and throttle (+0x44).

#[derive(Clone, Debug)]
pub struct Pattern {
    table: &'static [(u16, u8)],
    segment: Option<usize>,
    substep: u8,
    pub rpm: f32,
    pub throttle: f32,
}

impl Default for Pattern {
    fn default() -> Self {
        Self { table: table(3), segment: None, substep: 0, rpm: 1000.0, throttle: 0.0 }
    }
}

/// The dispatch 0x46FA30 selects by the pending pattern (ctx +0xC4): IDs
/// 2..=15 their own banks, any other ID (1 included) the default bank 1.
pub fn table(id: i32) -> &'static [(u16, u8)] {
    // Banks 1..=15 at 0x7F0070..0x7F08F8 ([`crate::exe_tables::Tables::patterns`]).
    let k = if (2..=15).contains(&id) { id as usize } else { 1 };
    &crate::exe_tables::tables().patterns[k - 1]
}

impl Pattern {
    /// A pending CAR_AUDIO_PATTERN (command 7, handler 0x459550) taken by
    /// 0x46FA30: every ID selects a bank (unknown IDs the default bank 1).
    pub fn start_id(&mut self, id: i32) {
        self.table = table(id);
        self.start();
    }
    /// Restart the selected bank even when the same pattern is active.
    pub fn start(&mut self) {
        self.segment = Some(0);
        self.substep = 0;
        self.rpm = self.table[0].0 as f32;
        self.throttle = self.table[0].1 as f32;
    }

    /// One native audio update (observed 60 Hz), independent of render rate.
    pub fn step(&mut self) {
        if let Some(mut segment) = self.segment {
            self.substep += 1;
            if self.substep == 4 {
                self.substep = 0;
                segment += 1;
                self.segment = (segment + 1 < self.table.len()).then_some(segment);
            }
            // 0x46FD38: final step changes state without evaluating the
            // endpoint; the preceding quarter-step is held this update.
            if self.segment.is_some() {
                let (from, to) = (self.table[segment], self.table[segment + 1]);
                let u = self.substep as f32 / 4.0;
                self.rpm = from.0 as f32 + (to.0 as f32 - from.0 as f32) * u;
                self.throttle = from.1 as f32 + (to.1 as f32 - from.1 as f32) * u;
                self.segment = Some(segment);
            }
        } else {
            self.rpm -= 500.0;
            self.throttle -= 15.0;
        }
        self.rpm = self.rpm.clamp(1000.0, 10000.0);
        self.throttle = self.throttle.clamp(0.0, 100.0);
    }
}

/// Native audio-update cadence. AudioManager_Update (0x483B40) runs the
/// sound contexts only when more than 15 ms (0x787AF4) of real time have
/// passed since its last run (0x43E5C0 on +0xA4), so one update per native
/// frame at the 1/30 s cap a scene holding the cars sets (NIS_SetFrameRate30
/// 0x435F40) and one per two 1/120 s frames (1/60 s) otherwise. Observed
/// under Wine (vanilla, no WidescreenFix): IntroNis08/27 run 30 updates a
/// second up to the camera cut and ~60 after it
/// (`docs/evidence/nis-audio-patterns`).
pub const SCENE_PERIOD: f32 = 1.0 / 30.0;
pub const FREE_PERIOD: f32 = 1.0 / 60.0;

/// Clone-side accumulator turning frame time into native audio updates of
/// the current period, independent of the clone's render rate.
#[derive(Default, Debug)]
pub struct StepClock {
    remainder: f64,
}
impl StepClock {
    pub fn advance(&mut self, dt: f32, period: f32) -> usize {
        if !dt.is_finite() || dt <= 0.0 || period <= 0.0 {
            return 0;
        }
        self.remainder += dt as f64;
        // Whole periods, allowing for the rounding of summed frame times.
        let steps = ((self.remainder + 1e-7) / period as f64).floor() as usize;
        self.remainder = (self.remainder - steps as f64 * period as f64).max(0.0);
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(id: i32, n: usize) -> Vec<(u32, u32)> {
        let mut p = Pattern::default();
        p.start_id(id);
        (0..n)
            .map(|_| {
                p.step();
                (p.rpm.to_bits(), p.throttle.to_bits())
            })
            .collect()
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn duration_follows_native_period_not_render_rate() {
        for period in [SCENE_PERIOD, FREE_PERIOD] {
            let updates = (3.0 / period).round() as usize;
            for fps in [30, 60, 120, 144, 240, 360] {
                let mut pattern = Pattern::default();
                pattern.start_id(4);
                let mut clock = StepClock::default();
                let mut actual = Vec::new();
                for _ in 0..fps * 3 {
                    for _ in 0..clock.advance(1.0 / fps as f32, period) {
                        pattern.step();
                        actual.push((pattern.rpm.to_bits(), pattern.throttle.to_bits()));
                    }
                }
                assert_eq!(actual, run(4, updates), "period {period} FPS {fps}");
            }
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn unknown_ids_take_the_default_bank_and_restart() {
        assert_eq!(run(0, 300), run(1, 300));
        assert_eq!(run(16, 300), run(1, 300));
        assert_eq!(run(-1, 300), run(1, 300));
        let mut pattern = Pattern::default();
        pattern.start_id(4);
        for _ in 0..30 {
            pattern.step();
        }
        pattern.start_id(3);
        pattern.step();
        assert_eq!((pattern.rpm, pattern.throttle), (1282.5, 47.5));
    }

    /// Every bank against the executable's bytes (0x46FA30's switch and the
    /// tables it names), when the game is available.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn banks_match_the_executable() {
        let root = crate::testkit::game_root();
        let exe = std::fs::read(std::path::Path::new(&root).join("SPEED2.EXE")).unwrap();
        let read = |va: usize, n: usize| &exe[va - 0x400000..va - 0x400000 + n];
        let u32_at = |va: usize| u32::from_le_bytes(read(va, 4).try_into().unwrap()) as usize;
        for id in 1..=15 {
            // The jump table at 0x46FF64 indexes ID − 2; ID 1 is the default case.
            let case = if id == 1 { 0x46FC03 } else { u32_at(0x46FF64 + 4 * (id - 2)) };
            let code = read(case, 20);
            assert_eq!(&code[..6], &[0xC7, 0x86, 0xE8, 0, 0, 0], "bank {id}");
            let (address, segments) = (
                u32::from_le_bytes(code[6..10].try_into().unwrap()) as usize,
                u32::from_le_bytes(code[16..20].try_into().unwrap()) as usize,
            );
            let expected: Vec<_> =
                read(address, 4 * (segments + 1)).chunks(4).map(|r| (u16::from_le_bytes([r[0], r[1]]), r[2])).collect();
            assert_eq!(table(id as i32), expected.as_slice(), "bank {id}");
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn natural_shop_exit_matches_every_observed_audio_update() {
        let fixture = &crate::testkit::private("docs/evidence/nis-audio-pattern4/pattern4-physics.bin");
        assert_eq!(fixture.len(), 121 * 244);
        let mut pattern = Pattern::default();
        pattern.start_id(4);
        for (i, bytes) in fixture.chunks_exact(244).enumerate() {
            pattern.step();
            let u = |k| u32::from_le_bytes(bytes[k..k + 4].try_into().unwrap());
            assert_eq!(pattern.rpm.to_bits(), u(0xec), "RPM update {}", i + 1);
            assert_eq!(pattern.throttle.to_bits(), u(0xf0), "throttle update {}", i + 1);
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn natural_shop_entry_matches_every_observed_audio_update() {
        let fixture =
            &crate::testkit::private("docs/evidence/nis-audio-pattern/natural-physics.bin");
        assert_eq!(fixture.len(), 108 * 244);
        let mut pattern = Pattern::default();
        pattern.start();
        for (i, bytes) in fixture.chunks_exact(244).enumerate() {
            pattern.step();
            let u32_at = |k| u32::from_le_bytes(bytes[k..k + 4].try_into().unwrap());
            assert_eq!(u32_at(0), 0x78a008, "captured Physics lifetime");
            assert_eq!(pattern.rpm.to_bits(), u32_at(0xec), "RPM update {}", i + 1);
            assert_eq!(pattern.throttle.to_bits(), u32_at(0xf0), "throttle update {}", i + 1);
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn repeated_command_restarts_pattern_and_idle_does_not_restart_it() {
        let mut pattern = Pattern::default();
        pattern.start();
        for _ in 0..35 {
            pattern.step();
        }
        pattern.start();
        pattern.step();
        assert_eq!((pattern.rpm, pattern.throttle), (1282.5, 47.5));
        for _ in 0..300 {
            pattern.step();
        }
        assert_eq!((pattern.rpm, pattern.throttle), (1000.0, 0.0));
    }
}

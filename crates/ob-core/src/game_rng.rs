//! The game's random numbers (0x43E6B0, 0x43C1C0, 0x43C1F0): the one
//! generator every draw site of the original shares (`opponents`,
//! `rng_sites`, the formats' cover and showcase picks).

/// The game's random numbers (state 0x7EA624, statically 0x12345678).
/// At boot 0x57ECC0 (from the start-up 0x57ED20 chain at 0x57ED5C)
/// reseeds it through 0x43E6B0 with 0x43BDF0() + 0x12345678 —
/// QueryPerformanceCounter (IAT 0x78311C) shifted by 0x8284E4 — unless a
/// replay or online game (0x8650CC / 0x8650D0) supplies the seed. So
/// every boot of the original draws other rivals, names, grid slots and
/// cover choices (**verified** under Wine: three boots, no state within
/// 2·10⁶ steps of 0x12345678 at the main menu). The window seeds with
/// [`GameRng::boot`]; headless runs (shots, tests, flows) keep the static
/// value so they stay reproducible (check aid, not native).
#[derive(Clone, Debug)]
pub struct GameRng(pub u32);

impl Default for GameRng {
    fn default() -> GameRng {
        GameRng(0x1234_5678)
    }
}

impl GameRng {
    /// 0x43E6B0(seed, &state): the state one step past `seed`.
    pub fn seeded(seed: u32) -> GameRng {
        let mut r = GameRng(seed);
        r.advance();
        r
    }

    /// The boot seed (0x57ECC0): a high-resolution clock reading plus
    /// 0x12345678.
    pub fn boot(ticks: u64) -> GameRng {
        GameRng::seeded((ticks as u32).wrapping_add(0x1234_5678))
    }

    fn advance(&mut self) {
        let a = self.0 ^ 0x1D87_2B41;
        let b = (a >> 5) ^ a;
        self.0 = (b << 27) ^ b ^ a;
    }

    /// 0x43C1C0: the state modulo `n`, then a step.
    #[cfg_attr(feature = "dev-harness", track_caller)]
    pub fn below(&mut self, n: u32) -> u32 {
        #[cfg(feature = "dev-harness")]
        draw_log::note(self, 'I', n as f32);
        let r = self.0 % n.max(1);
        self.advance();
        r
    }

    /// 0x43C1F0: the state modulo 0x7FFFFFFF as a fraction of 2³¹, times
    /// `scale`, then a step.
    #[cfg_attr(feature = "dev-harness", track_caller)]
    pub fn unit(&mut self, scale: f32) -> f32 {
        #[cfg(feature = "dev-harness")]
        draw_log::note(self, 'F', scale);
        let r = self.0 % 0x7FFF_FFFF;
        self.advance();
        r as f32 * (1.0 / 2_147_483_648.0) * scale
    }
}

/// Developer harness only (unit RNG-T): with `NFSU2_RNG_LOG=<file>` every draw on a
/// [`GameRng`] appends `seq <TAB> generator address <TAB> I|F <TAB> argument <TAB> state before
/// <TAB> file:line` (the caller, through `#[track_caller]`), so the draws per call site and per
/// generator instance (the shared `Game::rng`, a race's `CarField::rng`, an encounter or
/// traffic stream) can be set beside the original's per-site counts
/// (the maintainer's tool `rngsites.py`). Off (one relaxed load) without the variable.
#[cfg(feature = "dev-harness")]
pub mod draw_log {
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};

    static LOG: OnceLock<Option<Mutex<(u64, std::fs::File)>>> = OnceLock::new();

    #[track_caller]
    pub(super) fn note(rng: &super::GameRng, kind: char, arg: f32) {
        let Some(log) = LOG.get_or_init(|| {
            let path = std::env::var_os("NFSU2_RNG_LOG")?;
            Some(Mutex::new((0, std::fs::File::create(path).ok()?)))
        }) else {
            return;
        };
        let at = std::panic::Location::caller();
        let mut g = log.lock().unwrap_or_else(|e| e.into_inner());
        g.0 += 1;
        let seq = g.0;
        // One unbuffered write a draw: nothing is lost when the process exits.
        let line = format!(
            "{seq}\t{:x}\t{kind}\t{arg}\t{:08x}\t{}:{}\n",
            rng as *const _ as usize,
            rng.0,
            at.file(),
            at.line()
        );
        let _ = g.1.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 0x43E6B0 stores one step of the seed; the boot seed adds 0x12345678
    /// to the clock, so a zero clock gives the static state's successor.
    #[test]
    fn boot_seed_is_one_step_past_the_clock() {
        let mut r = GameRng::default();
        r.advance();
        assert_eq!(GameRng::boot(0).0, r.0);
        assert_ne!(GameRng::boot(1).0, GameRng::boot(2).0);
    }
}

//! The developer harness's hooks in the game's own paths (
//! the `dev-harness` feature). With the feature each hook is
//! `crate::dev::hooks`'s (exactly the inline code it replaced); without it
//! a stub here returns what that code does when no harness run set its
//! state and no check variable is set, so the shipped game runs the same.
#[cfg(feature = "dev-harness")]
pub(crate) use crate::dev::hooks::{
    audio_capture_frame, audio_tick_end, check_tick_dt, frame_trace, headless_size, parity_grid, rain_clock_override, record_state, replay_controls,
    rng_pin, teleport, unknown_flag,
};
#[cfg(not(feature = "dev-harness"))]
pub(crate) use stubs::*;

/// The game's own command line (`unknown_flag`'s message).
pub(crate) const USAGE: &str = "usage: game [--no-boot] [--editor] | --check-game [game folder] | --set-root <game folder> | --verify-cache [track ...] | --prepare | --crash-test | --support-file [folder] | --version | --help";

#[cfg(not(feature = "dev-harness"))]
mod stubs {
    use crate::{Controls, Game};
    use glam::Vec2;

    /// No shot set a rain clock.
    pub(crate) fn rain_clock_override() -> Option<f32> {
        None
    }

    /// No NFSU2_RNG_PIN.
    pub(crate) fn rng_pin(_game: &mut Game, _at: &str) {}

    /// No NFSU2_STATE_HASH sink.
    pub(crate) fn record_state(_game: &Game, _dt: f32) {}

    /// No tick-cadence check (dev-harness and test builds only).
    pub(crate) fn check_tick_dt(_game: &Game, _dt: f32) {}

    /// No NFSU2_PREVIEW_LOG / NFSU2_DEV_SHOP.
    pub(crate) fn teleport(_game: &mut Game) {}

    /// No intro replay holds the controls.
    pub(crate) fn replay_controls(_game: &Game) -> Option<Controls> {
        None
    }

    /// No parity flow placed the player.
    pub(crate) fn parity_grid(_game: &Game) -> Option<Vec2> {
        None
    }

    /// No NFSU2_SHOT_SIZE.
    pub(crate) fn headless_size() -> Option<(u32, u32)> {
        None
    }

    /// No audio capture runs.
    pub(crate) fn audio_capture_frame(_game: &mut Game, _dt: f32) {}

    /// No audio capture renders the tick's frames.
    pub(crate) fn audio_tick_end(_game: &mut Game) {}

    /// No NFSU2_FRAME_TRACE (tick-snapshot measuring aid).
    pub(crate) fn frame_trace(_game: &Game, _poses: &crate::render::snapshot::FramePoses) {}

    /// The game's own flags: --no-boot, --editor, --check-game, --set-root,
    /// --verify-cache, --crash-test; any other flag is unknown.
    pub(crate) fn unknown_flag(args: &[String]) -> Option<&str> {
        const KNOWN: [&str; 7] =
            ["--no-boot", "--editor", "--check-game", "--set-root", "--verify-cache", "--crash-test", "--prepare"];
        args.iter().map(String::as_str).find(|a| a.starts_with('-') && !KNOWN.contains(a))
    }
}

/// The developer harness's and the tests' own state (were `Game`
/// fields; empty without `dev-harness` and outside tests).
#[derive(Default)]
pub(crate) struct DevState {
    /// Check aid (not native, the parity flow only): the original's player
    /// place at the green; the player takes the grid slot nearest it
    /// instead of its own draw, so it settles there as the original's did
    /// (before: moved 7 m after settling on another slot, the held car kept
    /// that slot's roll, rear loads 2.382 / 2.400 against the original's
    /// symmetric 2.391).
    #[cfg(feature = "dev-harness")]
    pub(crate) parity_grid_at: Option<glam::Vec2>,
    /// G35 (was the static REPLAY_CONTROLS): the pedals and steering of
    /// the frame being replayed from a log of the original (INTRO_FRAMES
    /// lines with the analog values), instead of the keyboard's: the Wine
    /// setup's idle pedals read 1/255.
    #[cfg(feature = "dev-harness")]
    pub(crate) replay_controls: Option<crate::Controls>,
    /// The FE sound ids played (tests).
    #[cfg(test)]
    pub(crate) fe_played: Vec<u8>,
    /// `fe_played`'s length at each tick's sounds (`audio_frame`; tests:
    /// a sound played after it goes out with the next tick's).
    #[cfg(test)]
    pub(crate) fe_heard: Vec<usize>,
}

//! The developer harness's hooks in the game's paths (the facade
//! `crate::dev_hooks` calls these with the `dev-harness` feature; without
//! it its stubs return what these do with no harness run and no NFSU2_*
//! check variable set). Each does exactly what the inline code did inline
use crate::*;

/// The world clock `--shot explore_rain` sets (as NFSU2_RAIN_CLOCK=162,
/// the bench's rain scene), as f32 bits; 0 = none. Only a shot sets it.
pub(crate) static SHOT_RAIN_CLOCK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// `rain_clock`'s fallback after NFSU2_RAIN_CLOCK: the shot's clock.
pub(crate) fn rain_clock_override() -> Option<f32> {
    let bits = SHOT_RAIN_CLOCK.load(std::sync::atomic::Ordering::Relaxed);
    (bits != 0).then(|| f32::from_bits(bits))
}

/// A scripted harness run (selftest, flows, playthrough, bench, photo
/// check): an icon menu's accept acts at once instead of on FORWARD's end.
pub(crate) static INSTANT_FORWARD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether an icon menu's accept skips FORWARD's wait (`INSTANT_FORWARD`).
pub(crate) fn instant_forward() -> bool {
    INSTANT_FORWARD.load(std::sync::atomic::Ordering::Relaxed)
}

/// After each world update: the state hash line (`state_hash`,
/// NFSU2_STATE_HASH).
pub(crate) fn record_state(game: &Game, dt: f32) {
    state_hash::record(game, dt);
}

/// Before each world update: its dt is one of the tick clock's (
/// `ticks::check_dt`; panics otherwise).
pub(crate) fn check_tick_dt(game: &Game, dt: f32) {
    super::ticks::check_dt(game, dt);
}

/// Before each window step: NFSU2_PREVIEW_LOG / NFSU2_DEV_SHOP
/// (`Game::dev_teleport`).
pub(crate) fn teleport(game: &mut Game) {
    game.dev_teleport();
}

/// The driving inputs an INTRO_FRAMES replay holds
/// (`DevState::replay_controls`).
pub(crate) fn replay_controls(game: &Game) -> Option<Controls> {
    game.dev.replay_controls
}

/// The parity flow's player grid place (`Game::parity_grid_at`).
pub(crate) fn parity_grid(game: &Game) -> Option<Vec2> {
    game.dev.parity_grid_at
}

/// `NFSU2_RNG_PIN=<place>:<hex>` (the RNG comparisons against the
/// original's sequence log, docs/original/rng.md): the shared generator set to that
/// state when the game reaches `place` (`entry`: free roam's set-up, before
/// the encounter racers; `world`: free roam's world build; `race`: a race's
/// set-up, before the line-up; `race_world`: a race's world build).
pub(crate) fn rng_pin(game: &mut Game, at: &str) {
    if let Some((place, hex)) = std::env::var("NFSU2_RNG_PIN").ok().as_deref().and_then(|v| v.split_once(':'))
        && place == at
        && let Ok(v) = u32::from_str_radix(hex.trim_start_matches("0x"), 16)
    {
        game.rng.0 = v;
    }
}

/// A headless run's window size (NFSU2_SHOT_SIZE).
pub(crate) fn headless_size() -> Option<(u32, u32)> {
    env_size("NFSU2_SHOT_SIZE")
}

/// At the end of `update`: a headless audio capture's tick when no `step`
/// drives it (the shots and the old flows call `update` alone): the
/// tick's sounds as `step` posts them (`Game::audio_sounds`), then
/// [`audio_tick_end`]. Under `step` its `audio_frame` does both (
/// one sound pass per tick, the game's own).
pub(crate) fn audio_capture_frame(game: &mut Game, dt: f32) {
    if AUDIO_CAPTURE.lock().unwrap().is_none() || game.audio.as_ref().is_none_or(|a| a.stepping) {
        return;
    }
    game.audio_sounds(dt);
    audio_tick_end(game);
}

/// After a tick's sounds (`Game::audio_frame`, [`audio_capture_frame`]): a
/// headless capture renders the tick's own frames (`Game::audio_tick_frames`).
pub(crate) fn audio_tick_end(game: &mut Game) {
    if AUDIO_CAPTURE.lock().unwrap().is_none() {
        return;
    }
    game.audio_capture_tick();
}

impl Game {
    fn audio_capture_tick(&mut self) {
        let frames = self.audio_tick_frames();
        let secs = self.audio.as_ref().map_or(0.0, |a| frames as f32 / a.mixer.rate().max(1) as f32);
        // NFSU2_AUDIO_STALL=1: a device that never plays (nothing
        // rendered), as a check that nothing waits on playback.
        if std::env::var_os("NFSU2_FLOW_DEVICE").is_some() {
            // `--flow` on the device (NFSU2_FLOW_DEVICE, a private
            // sink only): the watchdog as `step` runs it, the
            // callbacks' stats every 5 s, the game paced to the wall
            // clock.
            if let Some(a) = self.audio.as_ref() {
                a.mixer.keep_alive();
                static LAST: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
                let mut last = LAST.lock().unwrap();
                if last.is_none_or(|t| t.elapsed().as_secs_f32() >= 5.0) {
                    *last = Some(std::time::Instant::now());
                    if let Some(s) = a.mixer.take_stats() {
                        eprintln!("device audio: {s}");
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_secs_f32(secs));
        } else if std::env::var_os("NFSU2_AUDIO_REALTIME").is_some() {
            // NFSU2_AUDIO_REALTIME=1: the mixer rendered by its own
            // thread at real time (`Mixer::spawn_realtime`, started
            // with the capture), the game paced to the wall clock.
            std::thread::sleep(std::time::Duration::from_secs_f32(secs));
        } else if std::env::var_os("NFSU2_AUDIO_STALL").is_some() {
            std::thread::sleep(std::time::Duration::from_secs_f32(secs));
        } else if let Some(a) = self.audio.as_ref() {
            pump_audio_capture(&a.mixer, frames, &|| self.audio_state());
        }
    }
}

/// With the harness every flag is the harness's or the game's (as before).
pub(crate) fn unknown_flag(_args: &[String]) -> Option<&str> {
    None
}

/// Tick-snapshot measuring aid: NFSU2_FRAME_TRACE=<file>: per drawn frame, the
/// wall time (ns since the game's start), the ticks the frame took, α,
/// the player's drawn position and the newest tick's (the 144 Hz stutter
/// check: second differences of the drawn positions).
pub(crate) fn frame_trace(game: &Game, poses: &crate::render::snapshot::FramePoses) {
    use std::io::Write;
    static OUT: std::sync::OnceLock<Option<std::sync::Mutex<std::fs::File>>> = std::sync::OnceLock::new();
    let out = OUT.get_or_init(|| {
        std::env::var("NFSU2_FRAME_TRACE").ok().and_then(|p| std::fs::File::create(p).ok()).map(std::sync::Mutex::new)
    });
    let (Some(out), Some(d)) = (out.as_ref(), game.sim.drive.as_ref()) else { return };
    let (p, t) = (poses.player().position, d.state.position);
    let _ = writeln!(
        out.lock().unwrap(),
        "{} {} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6}",
        game.pacing.started.elapsed().as_nanos(),
        game.pacing.frame_updates,
        poses.alpha,
        p.x,
        p.y,
        p.z,
        t.x,
        t.y,
        t.z
    );
}

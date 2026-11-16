//! The window's exit (docs/exit.md): what the game does after its
//! event loop ended (the window closed, Quit Game, a controller's Back on
//! the main menu).
//!
//! **Traced** (SPEED2.EXE): the original's quit saves nothing. WM_CLOSE
//! (Win_WndProc 0x5CCD70 → 0x5CCE6C) sets the quit flag 0x864F48 (ignored
//! while PC_Loading is up, 0x4A9330, or when already set); the main loop
//! 0x5811D0 tests it at 0x5811E4 / 0x581572 and returns; WinMain (0x580E85)
//! then calls 0x5CF970 (the audio stopped 0x481080 / 0x4839C0, the render
//! and D3D released, 0x57D960 waits for the async file queue 0x86510C) and
//! exit(0) (0x580E8C). The other setters of 0x864F48 (0x4B962D, 0x4C57AC,
//! 0x4F016E, 0x4F440B, 0x55E00D, 0x5BF1F9) are the quit question's and the
//! front end's own quits, which reach the same end. The profile and the
//! options are written when they change (the save points), never at quit;
//! the clone does the same, so nothing is to be written here.
//!
//! What the clone's exit waited for (measured, docs/exit.md): the cache
//! writes' fsyncs (`cache::flush`, 20–30 s on a busy disk; no longer
//! synced, bounded by `EXIT_FLUSH` in `startup`) and the audio device's
//! close (up to 3 s). Now: the window is hidden first, the audio is muted
//! and left to the process's end, the rest drops as before.

use crate::*;

impl Game {
    /// Tear the game down after its event loop ended (the window hidden
    /// first, so it is gone at once), logging how long each part took.
    /// The parts drop in their declaration order as before (the renderer's
    /// frames in flight before the window's surface: dropping the surface
    /// first panics in wgpu-hal with a swap-chain image still held).
    pub(crate) fn exit(mut self) {
        let t = std::time::Instant::now();
        let lap = |what: &str| log::info!("exit: {what} at {} ms", t.elapsed().as_millis());
        lap("event loop ended");
        if let Some(w) = self.platform.window.as_ref().and_then(|w| w.window.as_ref()) {
            w.set_visible(false);
        }
        lap("window hidden");
        // The log's last drive and the run's length (docs/waits-log.md).
        self.waits_end();
        // The device's close can wait seconds (a sound server draining);
        // the process's end closes it. Muted first, so nothing plays on.
        if let Some(audio) = self.audio.take() {
            audio.mixer.set_levels(nfsu2::audio::mixer::Levels { effects: 0.0, engine: 0.0, speech: 0.0, music: 0.0 });
            audio.mixer.set_stream_gain(0.0);
            std::mem::forget(audio);
        }
        lap("audio (muted, left to the process's end)");
        drop(self);
        lap("game");
    }
}

//! The window's start made side by side: the graphics device opened on a
//! thread and the sound device asked for from the moment the options are
//! in force, beside the game's data and the window's event loop; the window
//! takes them when it is made (`Game::create_window`, `resumed`). Each is
//! the object the window's own start made in turn before, so nothing drawn
//! or heard changes; only the waits overlap (docs/start-and-menus.md).
use crate::*;

/// The window's devices being made ahead.
pub(crate) struct StartAhead {
    /// The graphics device ([`nfsu2::render::Gpu::ahead`]).
    pub(crate) gpu: Option<std::thread::JoinHandle<nfsu2::render::GpuAhead>>,
    /// The sound device's stream being opened ([`Mixer::start`]).
    pub(crate) audio: Option<nfsu2::audio::mixer::MixerOpening>,
}

impl StartAhead {
    /// Started for a window (never with `NFSU2_HEADLESS_ONLY`, which opens
    /// no window and no sound device). `NFSU2_NO_START_AHEAD=1`: none (the
    /// window opens both in turn, as before).
    pub(crate) fn start() -> Option<StartAhead> {
        if debug_env().headless_only || nfsu2::env_value!("NFSU2_NO_START_AHEAD").is_some_and(|v| v != "0") {
            return None;
        }
        let gpu = std::thread::Builder::new().name("gpu ahead".into()).spawn(nfsu2::render::Gpu::ahead).ok();
        Some(StartAhead { gpu, audio: Some(Mixer::start()) })
    }
}

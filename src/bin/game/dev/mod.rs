pub(crate) mod first_drag;
pub(crate) mod first_race;
pub(crate) mod mouse_shots;
pub(crate) mod parity;
pub(crate) mod photo_check;
pub(crate) mod playthrough;
pub(crate) mod post_race;
pub(crate) mod rate;
pub(crate) mod state_hash;
// The harness on the tick cadence.
pub(crate) mod ticks;
pub(crate) mod shot;
pub(crate) mod drivers;
pub(crate) mod flow;
pub(crate) mod selftest;
pub(crate) mod util;
pub(crate) mod audio_capture;
pub(crate) mod bench;
pub(crate) mod checks;
pub(crate) mod headless;
pub(crate) mod intro_replay;
pub(crate) mod misc;
pub(crate) mod teleport;
// The hooks the game's paths call (`crate::dev_hooks`).
pub(crate) mod hooks;
// The maintainer's private set (saves), read at run time.
pub(crate) mod private;

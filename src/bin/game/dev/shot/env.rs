//! The shot's environment variables that several setups (or the setup
//! and the render) read, parsed once. Each setup keeps its own
//! default (`ms.unwrap_or(…)`), so every target reads what it read before.
use crate::*;

/// `NFSU2_SHOT_*` and the `NFSU2_FX_*` shot aids.
pub(crate) struct ShotEnv {
    /// NFSU2_SHOT_SIZE=WxH: the target size (unset: 1280x960).
    pub(crate) size: Option<(u32, u32)>,
    /// NFSU2_SHOT_MS: the shot's moment, ms (each setup has its default).
    pub(crate) ms: Option<f32>,
    /// NFSU2_SHOT_AT=x,y,z,deg: the car's place and heading.
    pub(crate) at: Option<[f32; 4]>,
    /// NFSU2_SHOT_HOLD: no gas, the car held where NFSU2_SHOT_AT put it.
    pub(crate) hold: bool,
    /// The drive shots' effect aids.
    pub(crate) fx: FxEnv,
}

/// The drive shots' `NFSU2_FX*` aids (docs/sparks-fire.md).
pub(crate) struct FxEnv {
    /// NFSU2_FX: an effects shot.
    pub(crate) on: bool,
    /// NFSU2_FX_STEER=a|d: the steering key held.
    pub(crate) steer: Option<KeyCode>,
    /// NFSU2_FX_STEER_AT: from that frame (default 0).
    pub(crate) steer_at: Option<u32>,
    /// NFSU2_FX_STEER_FOR: for that many frames.
    pub(crate) steer_for: Option<u32>,
    /// NFSU2_FX_FRAMES: the frame count (default 90).
    pub(crate) frames: Option<u32>,
    /// NFSU2_FX_SPEED=km/h[,down].
    pub(crate) speed: Vec<f32>,
    /// NFSU2_FX_POST=surface,slide,impulse[,from,to] (3 values or more).
    pub(crate) post: Option<Vec<f32>>,
    /// NFSU2_FX_GEAR=n.
    pub(crate) gear: Option<usize>,
    /// NFSU2_FX_TURBO=level.
    pub(crate) turbo: Option<u8>,
    /// NFSU2_FX_DUMP: log the effects.
    pub(crate) dump: bool,
}

fn var<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn floats(name: &str) -> Option<Vec<f32>> {
    std::env::var(name).ok().map(|s| s.split(',').filter_map(|f| f.trim().parse::<f32>().ok()).collect::<Vec<_>>())
}

impl ShotEnv {
    pub(crate) fn from_env() -> Self {
        Self {
            size: env_size("NFSU2_SHOT_SIZE"),
            ms: var("NFSU2_SHOT_MS"),
            at: floats("NFSU2_SHOT_AT").filter(|v| v.len() == 4).map(|v| [v[0], v[1], v[2], v[3]]),
            hold: std::env::var_os("NFSU2_SHOT_HOLD").is_some(),
            fx: FxEnv {
                on: std::env::var_os("NFSU2_FX").is_some(),
                steer: std::env::var("NFSU2_FX_STEER")
                    .ok()
                    .map(|k| if k == "a" { KeyCode::KeyA } else { KeyCode::KeyD }),
                steer_at: var("NFSU2_FX_STEER_AT"),
                steer_for: var("NFSU2_FX_STEER_FOR"),
                frames: var("NFSU2_FX_FRAMES"),
                speed: floats("NFSU2_FX_SPEED").unwrap_or_default(),
                post: floats("NFSU2_FX_POST").filter(|v| v.len() >= 3),
                gear: var("NFSU2_FX_GEAR"),
                turbo: var("NFSU2_FX_TURBO"),
                dump: std::env::var_os("NFSU2_FX_DUMP").is_some(),
            },
        }
    }
}

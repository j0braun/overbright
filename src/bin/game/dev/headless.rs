use crate::*;

/// A `WxH` size from the environment.
pub(crate) fn env_size(var: &str) -> Option<(u32, u32)> {
    std::env::var(var)
        .ok()
        .and_then(|s| s.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))))
        .filter(|&(w, h): &(u32, u32)| w > 0 && h > 0)
}

/// The shots' and bench's Aspect Ratio: `NFSU2_ASPECT=wide|stretched`,
/// else the options' (Wide by default).
pub(crate) fn headless_aspect(game: &mut Game) {
    match std::env::var("NFSU2_ASPECT").ok().as_deref() {
        Some("stretched") => game.settings.aspect = 1,
        Some("wide") => game.settings.aspect = 0,
        _ => {}
    }
    nfsu2::render::display::set_aspect_mode(if game.settings.aspect == 1 {
        nfsu2::render::display::AspectMode::Stretched
    } else {
        nfsu2::render::display::AspectMode::Wide
    });
}

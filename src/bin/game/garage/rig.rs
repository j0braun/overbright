use crate::*;

/// A front-end scene's light setup (render info +0x904): the FrontB
/// record of that name (0x48A2D0), else the default 0x7F3E10. Platform
/// scenes (0x4A7180) pass SHOWROOM (0x4AF464) or CRIB01 (0x4AFAD0); the
/// garage behind the other menus picks its own (`Game::frontend_scene`).
pub(crate) fn frontend_rig(caches: &Caches, root: &std::path::Path, scene: &str) -> [nfsu2::render::lighting::RigLight; 3] {
    frontend_rig_hash(caches, root, nfsu2::formats::hash::bin_hash(scene))
}

pub(crate) fn frontend_rig_hash(
    caches: &Caches,
    root: &std::path::Path,
    hash: u32,
) -> [nfsu2::render::lighting::RigLight; 3] {
    let setups = caches.rig_setups.get_or_init(|| {
        nfsu2::game_fs::read(&root, "FRONTEND/FrontB.lzc")
            .ok()
            .and_then(|raw| nfsu2::formats::jdlz::decompress(&raw).ok())
            .map(|data| nfsu2::render::lighting::parse_light_setups(&data))
            .unwrap_or_default()
    });
    setups.get(&hash).copied().unwrap_or(*nfsu2::render::lighting::DEFAULT_RIG)
}

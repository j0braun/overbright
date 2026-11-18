//! The folders Overbright keeps in its files (game-root.txt,
//! texture-pack-root.txt), in the form `platform::dirs` gives them: in a
//! portable install relative to the program folder when on the same drive
//! (`dirs::store_path`), read back from it (`dirs::load_path`). Outside
//! portable mode every function here is exactly the `setup` function it
//! wraps (docs/platform-dirs.md, "Portable mode").

use std::path::{Path, PathBuf};

use super::setup;
use crate::platform::dirs;

/// `setup::find`, with a game folder kept relative to a portable install
/// taken from the program folder rather than the current folder.
pub fn find_game() -> Option<PathBuf> {
    if std::env::var_os("NFSU2_ROOT").is_none()
        && let Some(kept) = kept_root().filter(|_| dirs::portable_base().is_some())
        && setup::looks_like_game(&kept)
    {
        return Some(kept);
    }
    setup::find()
}

/// The kept game folder (`setup::saved_root`), resolved.
pub fn kept_root() -> Option<PathBuf> {
    setup::saved_root().map(|p| dirs::load_path(&p))
}

/// Keeps `root` as the game folder (`setup::save_root`).
pub fn keep_root(root: &Path) -> std::io::Result<()> {
    setup::save_root(&dirs::store_path(root))
}

/// The kept texture-pack (HUD mod) folder (`setup::saved_texture_pack`),
/// resolved.
pub fn kept_texture_pack() -> Option<PathBuf> {
    setup::saved_texture_pack().map(|p| dirs::load_path(&p))
}

/// Keeps `dir` as the texture-pack folder, None forgets it
/// (`setup::save_texture_pack`).
pub fn keep_texture_pack(dir: Option<&Path>) -> std::io::Result<()> {
    match dir {
        Some(d) => setup::save_texture_pack(Some(&dirs::store_path(d))),
        None => setup::save_texture_pack(None),
    }
}

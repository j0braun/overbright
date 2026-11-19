//! Where the launcher finds and keeps things: the game's own folders
//! through `platform::dirs` (docs/playing/platform-dirs.md), so the launcher and
//! the game always agree, on every OS.

use std::path::PathBuf;

/// Overbright's own folder (options.txt, game-root.txt,
/// texture-pack-root.txt; `platform::dirs::config`).
pub fn data_dir() -> PathBuf {
    crate::platform::dirs::config()
}

/// options.txt (the game's settings file, `Settings::path`).
pub fn options_file() -> PathBuf {
    crate::frontend::options::Settings::path()
}

/// The log folder the game writes its log files to; the launcher's own
/// files (`launcher-*.log`) go there too.
pub fn logs() -> PathBuf {
    crate::platform::dirs::logs()
}

/// The lock file that keeps a second launcher from starting.
pub fn lock_file() -> PathBuf {
    data_dir().join("launcher.lock")
}

/// The start marker: written before the game starts, removed once the
/// start is past (or the game ended under the launcher's eyes). Left over,
/// the last start did not finish: the launcher opens on its recovery screen.
pub fn start_marker() -> PathBuf {
    data_dir().join("start-marker.txt")
}

/// Overbright's profiles folder (`<data home>/NFS Underground 2`; the
/// import of the original's saves copies into it).
pub fn profiles() -> PathBuf {
    crate::frontend::profiles::root()
}

/// The original game's save folder on this system, if it has a known one:
/// Windows `%LOCALAPPDATA%\NFS Underground 2` (0x4A9C40: SHGetFolderPathA
/// CSIDL_LOCAL_APPDATA + the folder name). Elsewhere the player picks it
/// (it lives inside a Wine prefix).
pub fn original_saves() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        crate::platform::win::local_app_data()
            .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
            .map(|d| d.join(crate::frontend::profiles::FOLDER))
    }
    #[cfg(not(windows))]
    None
}

// The home folder and the game's executable beside this program: the
// library's (`platform::dirs`), as the map editor and the support file
// need them too.
pub use crate::platform::dirs::{game_exe, home};

/// The map editor's executable beside the launcher's (`overbright-editor`,
/// `overbright-editor.exe`); `OB_EDITOR_EXE` names another (tests).
pub fn editor_exe() -> PathBuf {
    if let Some(p) = std::env::var_os("OB_EDITOR_EXE").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let name = if cfg!(windows) { "overbright-editor.exe" } else { "overbright-editor" };
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).unwrap_or_else(|| PathBuf::from(name))
}

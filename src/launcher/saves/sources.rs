//! Where the original game's saves may be on this computer: the folder
//! the original uses on Windows (`%LOCALAPPDATA%\NFS Underground 2`,
//! 0x4A9C40: SHGetFolderPathA CSIDL_LOCAL_APPDATA + the folder name), and
//! on Linux the same folder inside the Wine prefixes the usual tools make.
//! Only looked at: nothing here is written.
//!
//! Prefixes looked in: the game folder's own (walking up to `drive_c`),
//! `$WINEPREFIX`, `~/.wine`, Steam's Proton prefixes
//! (`steamapps/compatdata/*/pfx`, native and Flatpak Steam), Lutris
//! (`~/Games/*`), Bottles (native and Flatpak) and Heroic
//! (`~/Games/Heroic/Prefixes/*`). Inside each:
//! `drive_c/users/*/AppData/Local/NFS Underground 2` and, for prefixes of
//! an older Windows version, `drive_c/users/*/Local Settings/Application
//! Data/NFS Underground 2` (**inferred**: the XP layout, not seen in a
//! prefix here). The widescreen fix's own save location
//! (CustomUserFilesDirectoryInGameDir) is **open**: "Choose folder…"
//! takes it.

use std::path::{Path, PathBuf};

/// A folder of the original's saves found on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub path: PathBuf,
    /// Where it is, in words ("Wine prefix ~/.wine").
    pub label: String,
}

/// What [`discover`] looks at (the real values from [`Ctx::here`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ctx {
    pub home: Option<PathBuf>,
    pub game: Option<PathBuf>,
    pub wineprefix: Option<PathBuf>,
    pub local_app_data: Option<PathBuf>,
    /// Overbright's own profiles folder (never a source).
    pub own: PathBuf,
}

impl Ctx {
    pub fn here(game: Option<PathBuf>, own: PathBuf) -> Ctx {
        Ctx {
            home: crate::platform::dirs::home(),
            game,
            wineprefix: std::env::var_os("WINEPREFIX").filter(|v| !v.is_empty()).map(PathBuf::from),
            local_app_data: crate::launcher::dirs::original_saves().and_then(|p| p.parent().map(Path::to_path_buf)),
            own,
        }
    }
}

/// At most this many prefixes are looked in (a bounded scan).
const MAX_PREFIXES: usize = 256;

fn subdirs(d: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(d)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// "~/…" for a path in the home folder.
pub fn short(p: &Path, home: Option<&Path>) -> String {
    match home.and_then(|h| p.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => p.display().to_string(),
    }
}

/// The Wine prefixes to look in, with their words.
pub fn prefixes(c: &Ctx) -> Vec<(PathBuf, String)> {
    let mut v: Vec<(PathBuf, String)> = Vec::new();
    let home = c.home.as_deref();
    if let Some(g) = &c.game {
        let mut p = g.as_path();
        while let Some(parent) = p.parent() {
            if p.file_name().is_some_and(|n| n == "drive_c") {
                v.push((parent.to_path_buf(), format!("the game's Wine prefix {}", short(parent, home))));
                break;
            }
            p = parent;
        }
    }
    if let Some(w) = &c.wineprefix {
        v.push((w.clone(), format!("Wine prefix {}", short(w, home))));
    }
    if let Some(h) = home {
        v.push((h.join(".wine"), "Wine prefix ~/.wine".into()));
        for steam in [
            h.join(".steam/steam"),
            h.join(".local/share/Steam"),
            h.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ] {
            for d in subdirs(&steam.join("steamapps/compatdata")) {
                let id = d.file_name().map_or(String::new(), |n| n.to_string_lossy().to_string());
                v.push((d.join("pfx"), format!("Steam (Proton) prefix {id}")));
            }
        }
        for d in subdirs(&h.join("Games")) {
            if d.join("drive_c").is_dir() {
                let n = d.file_name().map_or(String::new(), |n| n.to_string_lossy().to_string());
                v.push((d.clone(), format!("Lutris: {n}")));
            }
        }
        for bottles in
            [h.join(".local/share/bottles/bottles"), h.join(".var/app/com.usebottles.bottles/data/bottles/bottles")]
        {
            for d in subdirs(&bottles) {
                let n = d.file_name().map_or(String::new(), |n| n.to_string_lossy().to_string());
                v.push((d.clone(), format!("Bottles: {n}")));
            }
        }
        let heroic = h.join("Games/Heroic/Prefixes");
        for d in subdirs(&heroic).into_iter().chain(subdirs(&heroic.join("default"))) {
            let n = d.file_name().map_or(String::new(), |n| n.to_string_lossy().to_string());
            v.push((d.clone(), format!("Heroic: {n}")));
        }
    }
    v.truncate(MAX_PREFIXES);
    v
}

/// The save folders of the original found on this computer that hold at
/// least one save (`super::save_dirs`), each once, Overbright's own left
/// out.
pub fn discover(c: &Ctx) -> Vec<Source> {
    let folder = crate::frontend::profiles::FOLDER;
    let mut cand: Vec<Source> = Vec::new();
    if let Some(l) = &c.local_app_data {
        cand.push(Source { path: l.join(folder), label: "the original game on this computer".into() });
    }
    for (pfx, label) in prefixes(c) {
        for user in subdirs(&pfx.join("drive_c/users")) {
            for rel in ["AppData/Local", "Local Settings/Application Data"] {
                cand.push(Source { path: user.join(rel).join(folder), label: label.clone() });
            }
        }
    }
    let own = std::fs::canonicalize(&c.own).ok();
    let mut seen: Vec<PathBuf> = Vec::new();
    cand.into_iter()
        .filter(|s| {
            let Ok(canon) = std::fs::canonicalize(&s.path) else { return false };
            if own.as_ref() == Some(&canon) || seen.contains(&canon) {
                return false;
            }
            seen.push(canon);
            !super::save_dirs(&s.path).is_empty()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_saves_in_the_usual_prefixes() {
        let h = std::env::temp_dir().join(format!("ob-sources-{}", crate::unique_tag()));
        let put = |rel: &str| {
            let p = h.join(rel);
            std::fs::create_dir_all(&p).unwrap();
            let n = p.file_name().unwrap().to_owned();
            std::fs::write(p.join(n), b"x").unwrap();
        };
        put(".wine/drive_c/users/kimmy/AppData/Local/NFS Underground 2/KIMMY");
        put(
            ".local/share/Steam/steamapps/compatdata/12345/pfx/drive_c/users/steamuser/AppData/Local/NFS Underground 2/RAY",
        );
        put("Games/nfsu2/drive_c/users/kimmy/Local Settings/Application Data/NFS Underground 2/OLD");
        put(".local/share/bottles/bottles/Games/drive_c/users/kimmy/AppData/Local/NFS Underground 2/BOT");
        put("Games/Heroic/Prefixes/default/NFSU2/drive_c/users/kimmy/AppData/Local/NFS Underground 2/HER");
        // An empty save folder and Overbright's own are not offered.
        std::fs::create_dir_all(h.join("other/drive_c/users/x/AppData/Local/NFS Underground 2")).unwrap();
        put("data/NFS Underground 2/MINE");
        let c = Ctx {
            home: Some(h.clone()),
            game: Some(h.join(".wine/drive_c/Games/NFSU2")),
            wineprefix: Some(h.join("other")),
            local_app_data: None,
            own: h.join("data/NFS Underground 2"),
        };
        let found = discover(&c);
        let labels: Vec<&str> = found.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "the game's Wine prefix ~/.wine",
                "Steam (Proton) prefix 12345",
                "Lutris: nfsu2",
                "Bottles: Games",
                "Heroic: NFSU2"
            ],
            "{found:?}"
        );
        let mut own = c.clone();
        own.local_app_data = Some(h.join("data"));
        assert_eq!(discover(&own).len(), 5, "Overbright's own folder is never a source");
        let _ = std::fs::remove_dir_all(&h);
    }
}

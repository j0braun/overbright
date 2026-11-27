//! The save sidecar (docs/modding/asset-format/layering.md §8): which mod packs a profile was last saved with. A small
//! text file of Overbright's own, `<data home>/NFS Underground 2 Mods/<NAME>.toml`, beside the profiles' folder and
//! never inside it, so the native save file and the profiles' folder stay exactly what the original game reads.
//!
//! The game writes it with each profile save, only while a pack is on (a save made with none removes the file), and
//! reads it when a profile is loaded: a pack that affects play ("affects play" tag) and was on at the last save but
//! is off now, or has changed since, is reported ([`Notice`]), never silently. Nothing is deleted or changed in the
//! profile by a notice.

use std::path::{Path, PathBuf};

use ob_asset_format::modpack;

use crate::tools::map::format::MapMod;

/// The folder name beside the profiles' folder.
pub const FOLDER: &str = "NFS Underground 2 Mods";

/// One pack that was on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pack {
    pub id: String,
    pub version: String,
    /// It changes something a save depends on (races, placements, collision, or what its manifest says).
    pub affects_play: bool,
    /// The digest of a pack file (None for a folder pack).
    pub digest: Option<String>,
}

/// What a load found different from the save's last write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// An affects-play pack the save was made with is off or not installed.
    Missing { id: String, version: String },
    /// It is on, but another version or other content.
    Changed { id: String, was: String, now: String },
}

impl Notice {
    /// The line for a log or a page.
    pub fn line(&self) -> String {
        match self {
            Notice::Missing { id, version } => format!("this save was made with the mod pack {id} {version}, which is off now (nothing was changed in the save)"),
            Notice::Changed { id, was, now } => format!("this save was made with the mod pack {id} {was}; {now} is on now (nothing was changed in the save)"),
        }
    }
}

/// Whether a pack affects play: what its manifest says, or (the stricter value, layering.md §8) when its map data holds
/// races, placements or collision edits.
pub fn affects_play(manifest: &ob_asset_format::manifest::Manifest, map: Option<&MapMod>) -> bool {
    manifest.affects_play.unwrap_or(false)
        || map.is_some_and(|m| !m.props.is_empty() || !m.races.is_empty() || !m.collision.is_empty())
}

/// The digest of a pack file.
pub fn digest_of(p: &modpack::Installed) -> Option<String> {
    use ob_asset_format::bundle::{self, Bundle};
    match &p.source {
        modpack::Source::File(f) => Bundle::open(f, bundle::Kind::PackFile).ok().map(|b| b.digest().to_string()),
        modpack::Source::Folder(_) => None,
    }
}

/// The sidecar's text.
pub fn to_text(packs: &[Pack]) -> String {
    let mut t = String::from("# Overbright: the mod packs this profile was last saved with (the profile file itself is the original's format).\nformat = 1\n");
    for p in packs {
        t += &format!("\n[[pack]]\nid = {}\nversion = {}\naffects_play = {}\n", q(&p.id), q(&p.version), p.affects_play);
        if let Some(d) = &p.digest {
            t += &format!("digest = {}\n", q(d));
        }
    }
    t
}

fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// The packs a sidecar text names (an unreadable text names none).
pub fn parse(text: &str) -> Vec<Pack> {
    let Ok(t) = text.parse::<toml::Table>() else { return Vec::new() };
    t.get("pack")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let v = v.as_table()?;
            let s = |k: &str| v.get(k).and_then(toml::Value::as_str).map(String::from);
            Some(Pack {
                id: s("id")?,
                version: s("version").unwrap_or_default(),
                affects_play: v.get("affects_play").and_then(toml::Value::as_bool).unwrap_or(false),
                digest: s("digest"),
            })
        })
        .collect()
}

/// What differs between the packs the save was made with and the packs that are on.
pub fn compare(saved: &[Pack], now: &[Pack]) -> Vec<Notice> {
    let mut out = Vec::new();
    for s in saved.iter().filter(|s| s.affects_play) {
        match now.iter().find(|n| n.id == s.id) {
            None => out.push(Notice::Missing { id: s.id.clone(), version: s.version.clone() }),
            Some(n) if n.version != s.version || (n.digest.is_some() && s.digest.is_some() && n.digest != s.digest) => {
                out.push(Notice::Changed { id: s.id.clone(), was: s.version.clone(), now: n.version.clone() })
            }
            Some(_) => {}
        }
    }
    out
}

/// `<root>/<NAME>.toml`; the name is made safe as a file name.
pub fn path_in(root: &Path, name: &str) -> PathBuf {
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' }).collect();
    root.join(format!("{}.toml", safe.to_lowercase()))
}

/// The folder in use: beside the profiles' folder (a trial run's copy has its own).
pub fn root() -> PathBuf {
    crate::platform::dirs::data_home().join(FOLDER)
}

/// Records `packs` for the profile `name` in `root`: a file when any pack is on, none when none is (and a file left
/// from an earlier save is removed).
pub fn write_in(root: &Path, name: &str, packs: &[Pack]) -> std::io::Result<()> {
    let path = path_in(root, name);
    if packs.is_empty() {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    std::fs::create_dir_all(root)?;
    crate::qol::saves::write_atomic(&path, to_text(packs).as_bytes(), false)
}

/// The packs recorded for `name` in `root` (none when there is no sidecar).
pub fn read_in(root: &Path, name: &str) -> Vec<Pack> {
    std::fs::read_to_string(path_in(root, name)).map(|t| parse(&t)).unwrap_or_default()
}

/// The notices for loading the profile `name` against the packs on now.
pub fn check_in(root: &Path, name: &str, now: &[Pack]) -> Vec<Notice> {
    compare(&read_in(root, name), now)
}

/// After a profile save: records the packs that are on for `name`.
pub fn record(name: &str) {
    if let Err(e) = write_in(&root(), name, &super::active()) {
        log::warn!("mod pack record for {name}: {e}");
    }
}

/// At a profile load: the notices, logged as warnings.
pub fn check(name: &str) -> Vec<Notice> {
    let n = check_in(&root(), name, &super::active());
    for x in &n {
        log::warn!("profile {name}: {}", x.line());
    }
    n
}

//! Sharing a map mod (docs/modding/studio.md, "The Mod page"; docs/modding/map-editor.md, "Import and export"):
//! what a shared copy holds, the export as one `.zip` (or a folder) that imports on another computer, and the
//! import that brings it back. Studio, the map editor and the launcher all export and import through here, so a
//! mod's painted textures travel with its map whichever program shares it.
//!
//! - **What is shared** ([`files`]): the mod's folder without its work files ([`SKIPPED`]: `work/`, the editor's
//!   `.editor/` and every other name starting with a dot). The map (`map.obmap`) is the map-mod format; the
//!   texture list and the pack data (`looks.toml`, `textures/…`) come along unchanged.
//! - **The `.zip`** holds `<folder>/<file>` for each; [`import`] takes the map and the other shared files back.
//! - **Provenance** ([`PROVENANCE`]): every export writes `provenance.toml` beside the map: the build that wrote
//!   it and the textures whose pixels started from the original game's ([`derived`]). Exporting a mod with such
//!   textures needs the author's yes ([`Refused::Derived`]); the shared copy says so.

use std::fmt;
use std::path::{Path, PathBuf};

use super::format::{MAX_FILE_BYTES, MapMod};
use super::store;

/// The provenance file of a shared copy.
pub const PROVENANCE: &str = "provenance.toml";
/// The provenance file's format version.
pub const FORMAT: u32 = 1;
/// The texture list of a mod (Studio Looks' `looks.toml`).
pub const LOOKS: &str = "looks.toml";
/// The top-level folders of a mod that are never shared (besides every name starting with a dot).
pub const SKIPPED: &[&str] = &["work"];
/// A shared copy holds at most this many files…
pub const MAX_FILES: usize = 20_000;
/// …and this many bytes.
pub const MAX_BYTES: u64 = 2 << 30;

/// One file of the shared copy: its path in the mod's folder (`/`-separated) and its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub path: String,
    pub size: u64,
}

/// Whether `rel` (a `/`-separated path in the mod's folder) is shared (`provenance.toml`, written new at every
/// export, is not in [`files`]).
pub fn shared(rel: &str) -> bool {
    let first = rel.split('/').next().unwrap_or_default();
    !SKIPPED.contains(&first) && !rel.split('/').any(|p| p.starts_with('.'))
}

/// The files of the mod in `dir` a shared copy holds, sorted (links are not followed; `provenance.toml` is written
/// new at every export).
pub fn files(dir: &Path) -> std::io::Result<Vec<File>> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<File>) -> std::io::Result<()> {
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            let Some(name) = e.file_name().to_str().map(str::to_string) else { continue };
            let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            if !shared(&r) || r == PROVENANCE {
                continue;
            }
            let t = e.file_type()?;
            if t.is_dir() {
                walk(&e.path(), &r, out)?;
            } else if t.is_file() {
                out.push(File { path: r, size: e.metadata()?.len() });
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, "", &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// The textures the mod in `dir` changes: its pack files (`textures/world/<HASH>.png`, `textures/car/<HASH>.png`)
/// as texture ids (`world:0x1A2B3C4D`), sorted.
pub fn textures(dir: &Path) -> Vec<String> {
    let mut v = Vec::new();
    for d in ["world", "car"] {
        let Ok(rd) = std::fs::read_dir(dir.join(store::TEXTURES).join(d)) else { continue };
        for e in rd.flatten() {
            let n = e.file_name();
            let Some(stem) = n.to_str().and_then(|n| n.strip_suffix(".png").or_else(|| n.strip_suffix(".PNG"))) else {
                continue;
            };
            if e.file_type().is_ok_and(|t| t.is_file())
                && let Ok(h) = u32::from_str_radix(stem, 16)
                && stem.len() <= 8
            {
                v.push(format!("{d}:0x{h:08X}"));
            }
        }
    }
    v.sort();
    v.dedup();
    v
}

/// A texture of the mod whose pixels started from the original game's (or whose start is not known).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Derived {
    /// `world:0x1A2B3C4D`.
    pub id: String,
    pub name: String,
}

/// The mod's textures made from the original game's pixels: those its `looks.toml` lists with `origin = "game"`,
/// and every pack file it does not list as `blank` (no record of where its pixels started: warned about too).
pub fn derived(dir: &Path) -> Vec<Derived> {
    let mut listed: Vec<(String, String, bool)> = Vec::new();
    if let Ok(text) = std::fs::read_to_string(dir.join(LOOKS))
        && let Ok(v) = text.parse::<toml::Table>()
        && let Some(list) = v.get("texture").and_then(toml::Value::as_array)
    {
        for t in list {
            let s = |k: &str| t.get(k).and_then(toml::Value::as_str).unwrap_or_default().to_string();
            listed.push((normal_id(&s("id")), s("name"), s("origin") == "blank"));
        }
    }
    let mut out: Vec<Derived> = listed
        .iter()
        .filter(|(_, _, blank)| !blank)
        .map(|(id, name, _)| Derived { id: id.clone(), name: name.clone() })
        .collect();
    for id in textures(dir) {
        if !listed.iter().any(|(l, ..)| *l == id) {
            out.push(Derived { id, name: String::new() });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out.dedup_by(|a, b| a.id == b.id);
    out
}

/// `world:0x1a2b3c4d`, `world:1A2B3C4D` → `world:0x1A2B3C4D` (other text as it is).
fn normal_id(id: &str) -> String {
    let Some((d, h)) = id.split_once(':') else { return id.to_string() };
    let h = h.strip_prefix("0x").or_else(|| h.strip_prefix("0X")).unwrap_or(h);
    match u32::from_str_radix(h, 16) {
        Ok(n) if !h.is_empty() && h.len() <= 8 => format!("{d}:0x{n:08X}"),
        _ => id.to_string(),
    }
}

/// The words of the warning before a mod with derived textures is shared.
pub fn derived_warning(n: usize) -> String {
    let what = if n == 1 { "1 texture is".to_string() } else { format!("{n} textures are") };
    format!(
        "{what} made from the original game's pixels. That is fine in your game, but sharing the mod shares the \
         original game's art. You are responsible for what you share."
    )
}

/// The text of `provenance.toml` (`build`: the program that writes it, `Overbright 0.5.0 (build …)`).
pub fn provenance(build: &str, derived: &[Derived]) -> String {
    let mut s =
        format!("# Written by Overbright when the mod was exported.\nformat = {FORMAT}\nbuild = {}\n", q(build));
    for d in derived {
        s += &format!("\n[[texture]]\nid = {}\nname = {}\norigin = \"game\"\n", q(&d.id), q(&d.name));
    }
    s
}

/// A TOML basic string (a JSON string is one).
fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// Why an export did not run.
#[derive(Debug)]
pub enum Refused {
    /// Textures from the original game's pixels, and the author did not say yes.
    Derived(Vec<Derived>),
    Failed(String),
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refused::Derived(d) => write!(f, "{}", derived_warning(d.len())),
            Refused::Failed(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Refused {}

fn failed(e: impl fmt::Display) -> Refused {
    Refused::Failed(e.to_string())
}

/// What an export wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exported {
    pub to: PathBuf,
    /// The files shared (`provenance.toml` included).
    pub files: usize,
    pub bytes: u64,
    pub derived: usize,
    /// The textures the mod changes ([`textures`]).
    pub textures: usize,
}

/// The `.zip` an export to `exports` writes for `folder`: `<exports>/<folder>.zip`.
pub fn default_zip(exports: &Path, folder: &str) -> PathBuf {
    exports.join(format!("{folder}.zip"))
}

/// Export the mod in `<maps>/<folder>` to `to`: a `.zip` (`<folder>/<file>` entries; an existing file is
/// replaced) or, for any other name, a new or empty folder. `derived_ok`: the author said yes to sharing textures
/// made from the original game's pixels. `build` goes into the provenance.
pub fn export(maps: &Path, folder: &str, to: &Path, derived_ok: bool, build: &str) -> Result<Exported, Refused> {
    export_dir(&maps.join(folder), folder, to, derived_ok, build)
}

/// [`export`] of the mod in `dir`, its entries in the `.zip` under `folder/`.
pub fn export_dir(dir: &Path, folder: &str, to: &Path, derived_ok: bool, build: &str) -> Result<Exported, Refused> {
    let dir = dir.to_path_buf();
    store::read_file(&dir.join(store::FILE)).map_err(|e| failed(format!("{e:#}")))?;
    let derived = derived(&dir);
    if !derived.is_empty() && !derived_ok {
        return Err(Refused::Derived(derived));
    }
    let list = files(&dir).map_err(|e| failed(format!("{}: {e}", dir.display())))?;
    let bytes: u64 = list.iter().map(|f| f.size).sum();
    if list.len() >= MAX_FILES || bytes > MAX_BYTES {
        return Err(failed(format!("{folder} is too large to share ({} files, {bytes} bytes)", list.len())));
    }
    let prov = provenance(build, &derived);
    let mut data: Vec<(String, Vec<u8>)> = Vec::with_capacity(list.len() + 1);
    for f in &list {
        let b = std::fs::read(dir.join(&f.path)).map_err(|e| failed(format!("{}: {e}", f.path)))?;
        data.push((f.path.clone(), b));
    }
    data.push((PROVENANCE.to_string(), prov.into_bytes()));
    let total = data.iter().map(|(_, b)| b.len() as u64).sum();
    if to.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        let named: Vec<(String, &[u8], u16)> = data
            .iter()
            .map(|(p, b)| {
                // PNG is compressed already.
                let method = if p.ends_with(".png") { 0 } else { 8 };
                (format!("{folder}/{p}"), b.as_slice(), method)
            })
            .collect();
        let entries: Vec<(&str, &[u8], u16)> = named.iter().map(|(n, b, m)| (n.as_str(), *b, *m)).collect();
        let zip = crate::formats::zip::build(&entries);
        if let Some(p) = to.parent() {
            std::fs::create_dir_all(p).map_err(|e| failed(format!("{}: {e}", p.display())))?;
        }
        crate::platform::fs::write_atomic(to, &zip, true).map_err(|e| failed(format!("{}: {e}", to.display())))?;
    } else {
        if to.exists() && std::fs::read_dir(to).map_or(true, |mut d| d.next().is_some()) {
            return Err(failed(format!("{} is not empty: pick a new folder", to.display())));
        }
        for (p, b) in &data {
            let out = to.join(p);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|e| failed(format!("{}: {e}", parent.display())))?;
            }
            std::fs::write(&out, b).map_err(|e| failed(format!("{}: {e}", out.display())))?;
        }
    }
    Ok(Exported {
        to: to.to_path_buf(),
        files: data.len(),
        bytes: total,
        derived: derived.len(),
        textures: textures(&dir).len(),
    })
}

/// A path from a shared copy that is safe to write under the new mod's folder.
pub fn safe(rel: &str) -> bool {
    !rel.is_empty()
        && !rel.starts_with('/')
        && !rel.contains('\\')
        && !rel.contains(':')
        && rel.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}

/// Import a shared copy (a `.zip` [`export`] wrote, an older map-only `.zip`, an exported folder, or a bare
/// `map.obmap`) into a new folder under `maps` (never over an existing one): the map is checked first; the other
/// shared files follow. Returns the new folder's name.
pub fn import(maps: &Path, from: &Path) -> Result<String, String> {
    let at = |e: &dyn fmt::Display| format!("{}: {e}", from.display());
    let mut other: Vec<(String, Vec<u8>)> = Vec::new();
    let map_bytes: Vec<u8>;
    let stem;
    if from.is_dir() {
        map_bytes = std::fs::read(from.join(store::FILE)).map_err(|e| at(&e))?;
        for f in files(from).map_err(|e| at(&e))? {
            if f.path != store::FILE {
                other.push((f.path.clone(), std::fs::read(from.join(&f.path)).map_err(|e| at(&e))?));
            }
        }
        stem = from.file_name().and_then(|s| s.to_str()).unwrap_or("map").to_string();
    } else if from.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        let len = std::fs::metadata(from).map_err(|e| at(&e))?.len();
        if len > MAX_BYTES {
            return Err(at(&"too large for a mod"));
        }
        let zip = crate::formats::zip::Zip::open(from).map_err(|e| at(&format!("{e:#}")))?;
        let entry = zip
            .entries
            .iter()
            .filter(|e| {
                e.name == store::FILE
                    || e.name.strip_suffix(store::FILE).is_some_and(|d| d.ends_with('/') && d.matches('/').count() == 1)
            })
            .min_by_key(|e| e.name.len())
            .ok_or_else(|| at(&format!("no {} in the archive", store::FILE)))?;
        if zip.entries.len() > MAX_FILES {
            return Err(at(&"too many files for a mod"));
        }
        if entry.size as u64 > MAX_FILE_BYTES {
            return Err(at(&format!("{} too large", store::FILE)));
        }
        let prefix = entry.name.strip_suffix(store::FILE).unwrap_or_default().to_string();
        map_bytes = zip.read(entry).map_err(|e| at(&format!("{e:#}")))?;
        for e in &zip.entries {
            let Some(rel) = e.name.strip_prefix(&prefix) else { continue };
            if rel == store::FILE || rel.ends_with('/') {
                continue;
            }
            if !safe(rel) {
                return Err(at(&format!("{} is not a file name a mod may have", e.name)));
            }
            if shared(rel) {
                other.push((rel.to_string(), zip.read(e).map_err(|e| at(&format!("{e:#}")))?));
            }
        }
        stem = from.file_stem().and_then(|s| s.to_str()).unwrap_or("map").to_string();
    } else if from.file_name().is_some_and(|n| n == store::FILE) {
        map_bytes = std::fs::read(from).map_err(|e| at(&e))?;
        stem = from.parent().and_then(Path::file_name).and_then(|s| s.to_str()).unwrap_or("map").to_string();
    } else {
        return Err(at(&format!("not a map mod folder, {} or .zip", store::FILE)));
    }
    if map_bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(at(&format!("{} too large", store::FILE)));
    }
    let text = std::str::from_utf8(&map_bytes).map_err(|_| at(&format!("{} is not text", store::FILE)))?;
    let m = MapMod::parse(text).map_err(|e| at(&format!("{}: {e}", store::FILE)))?;
    let name = if m.name.trim().is_empty() { stem } else { m.name.clone() };
    let folder = store::free_folder(maps, &name);
    let dir = maps.join(&folder);
    let r = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(&dir)?;
        for (rel, b) in &other {
            let out = dir.join(rel);
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::write(&out, b)?;
        }
        // The map last: a folder with `map.obmap` is a mod.
        crate::platform::fs::write_atomic(&dir.join(store::FILE), &map_bytes, true)
    })();
    if let Err(e) = r {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(format!("{}: {e}", dir.display()));
    }
    Ok(folder)
}

#[cfg(test)]
mod tests;

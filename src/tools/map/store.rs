//! Where map mods live: `<Overbright data>/maps/<folder>/map.obmap`, the
//! ones in use listed in `<Overbright data>/maps/enabled.txt` (one folder
//! name a line, applied in that order). Import and export as a folder or a
//! `.zip` holding `map.obmap`. Nothing here reads or writes the game's own
//! files.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use super::format::{MAX_FILE_BYTES, MapMod, valid_id};

/// The mod file's name inside its folder.
pub const FILE: &str = "map.obmap";
/// The list of mods in use.
pub const ENABLED: &str = "enabled.txt";

/// The map mods' folder in Overbright's data folder (portable-aware).
pub fn maps_dir() -> PathBuf {
    crate::platform::dirs::app().join("maps")
}

/// A map mod read from `path` (a `map.obmap`), refused cleanly when it is
/// too large, not text, or not a valid map mod.
pub fn read_file(path: &Path) -> Result<MapMod> {
    let len = std::fs::metadata(path).with_context(|| format!("reading {}", path.display()))?.len();
    ensure!(len <= MAX_FILE_BYTES, "{}: {} bytes, more than a map mod may have", path.display(), len);
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    parse_bytes(&bytes).with_context(|| path.display().to_string())
}

fn parse_bytes(bytes: &[u8]) -> Result<MapMod> {
    let text = std::str::from_utf8(bytes).context("not UTF-8 text")?;
    Ok(MapMod::parse(text)?)
}

/// Write `m` as `<dir>/<folder>/map.obmap` (the folder made first; the
/// file replaced through a temporary file, so a failed write leaves the
/// old one).
pub fn save(dir: &Path, folder: &str, m: &MapMod) -> Result<PathBuf> {
    ensure!(valid_id(folder), "`{folder}` is not a map mod folder name (a-z, 0-9, - and _)");
    let d = dir.join(folder);
    std::fs::create_dir_all(&d).with_context(|| format!("making {}", d.display()))?;
    let path = d.join(FILE);
    let tmp = d.join(format!("{FILE}.tmp"));
    std::fs::write(&tmp, m.write()).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, &path).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// The folder names listed in `<dir>/enabled.txt` (none when it is
/// missing); blank lines and `#` comments skipped, invalid names dropped.
pub fn enabled(dir: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(dir.join(ENABLED)) else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        if valid_id(l) && !out.iter().any(|o| o == l) {
            out.push(l.to_string());
        } else if !valid_id(l) {
            log::warn!("map mods: `{l}` in {ENABLED} is not a folder name, skipped");
        }
    }
    out
}

/// Turn `folder` on or off in `<dir>/enabled.txt`.
pub fn set_enabled(dir: &Path, folder: &str, on: bool) -> Result<()> {
    ensure!(valid_id(folder), "`{folder}` is not a map mod folder name");
    let mut list = enabled(dir);
    list.retain(|f| f != folder);
    if on {
        list.push(folder.to_string());
    }
    std::fs::create_dir_all(dir)?;
    let mut text = String::from("# Map mods in use, applied in this order (one folder name a line).\n");
    for f in list {
        text.push_str(&f);
        text.push('\n');
    }
    std::fs::write(dir.join(ENABLED), text)?;
    Ok(())
}

/// The mod folders under `dir` (those with a `map.obmap`), sorted.
pub fn list(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().join(FILE).is_file())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| valid_id(n))
        .collect();
    v.sort();
    v
}

/// The enabled mods read in order; a mod that cannot be read is skipped
/// with its reason logged (the others still apply).
pub fn load_enabled(dir: &Path) -> Vec<(String, MapMod)> {
    enabled(dir)
        .into_iter()
        .filter_map(|f| match read_file(&dir.join(&f).join(FILE)) {
            Ok(m) => Some((f, m)),
            Err(e) => {
                log::warn!("map mod {f} not loaded: {e:#}");
                None
            }
        })
        .collect()
}

/// A folder name for an imported mod: `name` cleaned, else `map`, with a
/// number added while the folder exists.
fn free_folder(dir: &Path, name: &str) -> String {
    let mut base: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(32)
        .collect();
    if base.is_empty() {
        base = "map".into();
    }
    let mut folder = base.clone();
    let mut n = 2;
    while dir.join(&folder).exists() {
        folder = format!("{base}-{n}");
        n += 1;
    }
    folder
}

/// Import a mod from a folder holding `map.obmap`, a `map.obmap` file, or
/// a `.zip` with `map.obmap` at its top or in one folder: validated first,
/// then written to a new folder under `dir` (never over an existing one).
/// Returns the new folder's name.
pub fn import(dir: &Path, from: &Path) -> Result<String> {
    let (m, stem) = if from.is_dir() {
        (read_file(&from.join(FILE))?, from.file_name())
    } else if from.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        let len = std::fs::metadata(from)?.len();
        ensure!(len <= 4 * MAX_FILE_BYTES, "{}: too large for a map mod archive", from.display());
        let zip = crate::formats::zip::Zip::open(from)?;
        let entry = zip
            .entries
            .iter()
            .filter(|e| {
                e.name == FILE
                    || e.name.strip_suffix(FILE).is_some_and(|d| d.ends_with('/') && d.matches('/').count() == 1)
            })
            .min_by_key(|e| e.name.len())
            .with_context(|| format!("{}: no {FILE} in the archive", from.display()))?;
        ensure!(entry.size as u64 <= MAX_FILE_BYTES, "{}: {FILE} too large", from.display());
        let bytes = zip.read(entry)?;
        (parse_bytes(&bytes).with_context(|| format!("{}: {FILE}", from.display()))?, from.file_stem())
    } else if from.file_name().is_some_and(|n| n == FILE) {
        (read_file(from)?, from.parent().and_then(Path::file_name))
    } else {
        bail!("{}: not a map mod folder, {FILE} or .zip", from.display());
    };
    let name =
        if m.name.is_empty() { stem.and_then(|s| s.to_str()).unwrap_or("map").to_string() } else { m.name.clone() };
    let folder = free_folder(dir, &name);
    save(dir, &folder, &m)?;
    Ok(folder)
}

/// Export the mod in `<dir>/<folder>` as a folder (`to` without a `.zip`
/// extension: `to/map.obmap`) or a `.zip` holding `<folder>/map.obmap`.
pub fn export(dir: &Path, folder: &str, to: &Path) -> Result<()> {
    let m = read_file(&dir.join(folder).join(FILE))?;
    let text = m.write();
    if to.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        let zip = zip_one(&format!("{folder}/{FILE}"), text.as_bytes());
        std::fs::write(to, zip).with_context(|| format!("writing {}", to.display()))?;
    } else {
        std::fs::create_dir_all(to)?;
        std::fs::write(to.join(FILE), text).with_context(|| format!("writing {}", to.display()))?;
    }
    Ok(())
}

/// A ZIP archive of one deflated file.
fn zip_one(name: &str, data: &[u8]) -> Vec<u8> {
    let body = miniz_oxide::deflate::compress_to_vec(data, 6);
    let crc = crate::formats::tpf::zip_crc(data);
    let fields = |v: &mut Vec<u8>| {
        v.extend_from_slice(&8u16.to_le_bytes()); // deflate
        v.extend_from_slice(&[0; 4]); // time, date
        v.extend_from_slice(&crc.to_le_bytes());
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(&(name.len() as u16).to_le_bytes());
        v.extend_from_slice(&[0, 0]); // extra length
    };
    let mut zip = b"PK\x03\x04\x14\x00\x00\x00".to_vec();
    fields(&mut zip);
    zip.extend_from_slice(name.as_bytes());
    zip.extend_from_slice(&body);
    let cd_at = zip.len() as u32;
    let mut cd = b"PK\x01\x02\x14\x00\x14\x00\x00\x00".to_vec();
    fields(&mut cd);
    cd.extend_from_slice(&[0; 10]); // comment, disk, attributes
    cd.extend_from_slice(&0u32.to_le_bytes()); // local header at 0
    cd.extend_from_slice(name.as_bytes());
    let cd_len = cd.len() as u32;
    zip.extend_from_slice(&cd);
    zip.extend_from_slice(b"PK\x05\x06\x00\x00\x00\x00\x01\x00\x01\x00");
    zip.extend_from_slice(&cd_len.to_le_bytes());
    zip.extend_from_slice(&cd_at.to_le_bytes());
    zip.extend_from_slice(&[0, 0]);
    zip
}

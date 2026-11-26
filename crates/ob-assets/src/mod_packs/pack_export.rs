//! A Studio map mod as a mod pack file (`<id>-<version>.obpk`, layout.md §4): `pack.toml` first (manifest.md), then
//! the mod's content as kind files (placements, races, texture images: [`super::content`]) and the provenance the
//! `.zip` export writes too.

use std::path::{Path, PathBuf};

use ob_asset_format::manifest::{Container, Manifest};
use ob_asset_format::modpack::{self, BaseId};

use super::content;
use crate::tools::map::{share, store};

/// What the author chose for an export.
pub struct ExportOpts<'a> {
    pub version: semver::Version,
    /// An SPDX expression; without one the pack is `local-only` (a public pack needs a licence, `MAN.4`).
    pub licence: Option<&'a str>,
    /// The author said yes to textures made from the original game's pixels (the pack is then local-only).
    pub derived_ok: bool,
    /// The build line for the provenance.
    pub build: String,
}

/// What an export wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exported {
    pub to: PathBuf,
    pub id: String,
    pub files: usize,
    pub bytes: u64,
}

pub fn q(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// What a pack file is made of (the parts the two writers, the export and the conversion, differ in).
pub struct Build<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub authors: &'a str,
    pub description: &'a str,
    pub origin: &'a str,
    pub share: &'a str,
    pub contents: Vec<&'static str>,
    pub affects_play: bool,
    /// The files other than `pack.toml` (logical path, bytes).
    pub files: Vec<(String, Vec<u8>)>,
    pub provenance: String,
}

/// Writes `b` to `to` (a `.obpk` path, or a folder in which `<id>-<version>.obpk` is made; an existing file is replaced).
pub fn write_pack(b: Build, to: &Path, base: Option<&BaseId>, o: &ExportOpts) -> Result<Exported, String> {
    let mut t = format!(
        "format = 2\nkind = \"mod\"\nid = {}\nname = {}\nversion = {}\n",
        q(b.id),
        q(b.name),
        q(&o.version.to_string())
    );
    if !b.authors.trim().is_empty() {
        t += &format!("authors = [{}]\n", q(b.authors.trim()));
    }
    if !b.description.trim().is_empty() {
        t += &format!("description = {}\n", q(b.description.trim()));
    }
    if let Some(l) = o.licence {
        t += &format!("licence = {}\n", q(l));
    }
    let env = crate::mod_packs::engine();
    t += &format!(
        "overbright = {}\norigin = \"{}\"\nshare = \"{}\"\n",
        q(&format!(">={}.{}", env.major, env.minor)),
        b.origin,
        b.share
    );
    t += &format!("contents = [{}]\n", b.contents.iter().map(|c| q(c)).collect::<Vec<_>>().join(", "));
    t += &format!("affects_play = {}\n", b.affects_play);
    if let Some(x) = base {
        t += &format!("\n[game]\nbases = [{}]\nfingerprint = [{}]\n", q(&x.id), q(&x.identity));
    }
    Manifest::parse(t.as_bytes(), Container::Unknown).map_err(|e| format!("pack.toml: {e}"))?;
    let mut files: Vec<(String, Vec<u8>)> = vec![("pack.toml".into(), t.into_bytes())];
    files.extend(b.files);
    files.push((share::PROVENANCE.into(), b.provenance.into_bytes()));
    let bytes = modpack::pack_file_bytes(&files).map_err(|e| e.to_string())?;
    let out = if to.extension().is_some_and(|e| e.eq_ignore_ascii_case(modpack::EXTENSION)) {
        to.to_path_buf()
    } else {
        to.join(modpack::file_name(b.id, &o.version))
    };
    if let Some(p) = out.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    crate::platform::fs::write_atomic(&out, &bytes, true).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(Exported { to: out, id: b.id.to_string(), files: files.len(), bytes: bytes.len() as u64 })
}

/// `provenance.toml`: the export's, and for a converted legacy mod where it came from.
pub fn provenance_text(
    build: &str,
    derived: &[share::Derived],
    converted_from: Option<&str>,
    credits: &[super::credits::Credit],
) -> String {
    let mut t = share::provenance(build, derived);
    t.push_str(&super::credits::toml(credits));
    match converted_from {
        Some(from) => {
            t.replacen("\nformat = ", &format!("\nconverted = true\nconverted_from = {}\nformat = ", q(from)), 1)
        }
        None => t,
    }
}

/// The pack's texture images of a map mod's folder: `textures/<world|car>/<hash>.png` as image-only entries.
pub fn texture_files(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut out = Vec::new();
    for d in ["world", "car"] {
        let Ok(rd) = std::fs::read_dir(dir.join(store::TEXTURES).join(d)) else { continue };
        let mut names: Vec<_> = rd.flatten().collect();
        names.sort_by_key(|e| e.file_name());
        for e in names {
            let n = e.file_name().to_string_lossy().to_string();
            let Some(stem) = n.strip_suffix(".png").or_else(|| n.strip_suffix(".PNG")) else { continue };
            if let Some(path) = content::texture_path(d, stem)
                && e.file_type().is_ok_and(|t| t.is_file())
            {
                out.push((path, std::fs::read(e.path()).map_err(|x| format!("{n}: {x}"))?));
            }
        }
    }
    Ok(out)
}

/// Exports the map mod `<maps>/<folder>` to `to`: a `.obpk` path, or a folder in which `<id>-<version>.obpk` is
/// written (an existing file is replaced). `base`: the base set the pack is made for (its id and the copy of the game
/// it came from go into `[game]`), None for a pack that runs on any.
pub fn export_map_mod(
    maps: &Path,
    folder: &str,
    to: &Path,
    base: Option<&BaseId>,
    o: &ExportOpts,
) -> Result<Exported, String> {
    export_dir(&maps.join(folder), folder, to, base, o, None)
}

/// [`export_map_mod`] for the mod in `dir` (its pack id is `folder`).
pub fn export_mod_dir(
    dir: &Path,
    folder: &str,
    to: &Path,
    base: Option<&BaseId>,
    o: &ExportOpts,
) -> Result<Exported, String> {
    export_dir(dir, folder, to, base, o, None)
}

/// [`export_mod_dir`] with `converted_from`: where a legacy mod came from (the
/// provenance says so).
pub fn export_dir(
    dir: &Path,
    folder: &str,
    to: &Path,
    base: Option<&BaseId>,
    o: &ExportOpts,
    converted_from: Option<&str>,
) -> Result<Exported, String> {
    if !ob_asset_format::names::is_pack_id(folder) {
        return Err(format!("{folder} is not a pack id (a-z, 0-9, - and _)"));
    }
    let map = store::read_file(&dir.join(store::FILE)).map_err(|e| format!("{e:#}"))?;
    let derived = share::derived(dir);
    if !derived.is_empty() && !o.derived_ok {
        return Err(share::derived_warning(derived.len()));
    }
    let list = share::files(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if list.len() >= share::MAX_FILES || list.iter().map(|f| f.size).sum::<u64>() > share::MAX_BYTES {
        return Err(format!("{folder} is too large to share"));
    }
    let name = if map.name.trim().is_empty() { folder } else { map.name.trim() };
    let mut files = content::write(&map);
    files.extend(texture_files(dir)?);
    let textures = files.iter().filter(|f| f.0.contains("/textures/images/")).count();
    let mut contents = Vec::new();
    if !map.props.is_empty() || !map.collision.is_empty() {
        contents.push("world");
    }
    if !map.races.is_empty() {
        contents.push("races");
    }
    if textures > 0 {
        contents.push("textures");
    }
    let (origin, share_as) = if !derived.is_empty() {
        ("from-game-files", "local-only")
    } else if o.licence.is_some() {
        ("own-work", "public")
    } else {
        ("own-work", "local-only")
    };
    let provenance = provenance_text(&o.build, &derived, converted_from, &super::credits::of_dir(dir));
    write_pack(
        Build {
            id: folder,
            name,
            authors: &map.author,
            description: &map.description,
            origin,
            share: share_as,
            contents,
            affects_play: !map.props.is_empty() || !map.races.is_empty() || !map.collision.is_empty(),
            files,
            provenance,
        },
        to,
        base,
        o,
    )
}

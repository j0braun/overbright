//! A Studio map mod as a mod pack file (`<id>-<version>.obpk`, layout.md §4): `pack.toml` first (manifest.md), then the
//! mod's shared files (`map.obmap`, the texture list and `textures/…`) and the provenance, exactly as the `.zip` export
//! shares them.

use std::path::{Path, PathBuf};

use ob_asset_format::manifest::{Container, Manifest};
use ob_asset_format::modpack::{self, BaseId};

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

fn q(s: &str) -> String {
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
    if !ob_asset_format::names::is_pack_id(folder) {
        return Err(format!("{folder} is not a pack id (a-z, 0-9, - and _)"));
    }
    let dir = maps.join(folder);
    let map = store::read_file(&dir.join(store::FILE)).map_err(|e| format!("{e:#}"))?;
    let derived = share::derived(&dir);
    if !derived.is_empty() && !o.derived_ok {
        return Err(share::derived_warning(derived.len()));
    }
    let list = share::files(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if list.len() >= share::MAX_FILES || list.iter().map(|f| f.size).sum::<u64>() > share::MAX_BYTES {
        return Err(format!("{folder} is too large to share"));
    }
    let name = if map.name.trim().is_empty() { folder } else { map.name.trim() };
    let textures = share::textures(&dir).len();
    let mut contents = Vec::new();
    if !map.props.is_empty() {
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
    let mut t = format!(
        "format = 2\nkind = \"mod\"\nid = {}\nname = {}\nversion = {}\n",
        q(folder),
        q(name),
        q(&o.version.to_string())
    );
    if let Some(l) = o.licence {
        t += &format!("licence = {}\n", q(l));
    }
    let env = crate::mod_packs::engine();
    t += &format!(
        "overbright = {}\norigin = \"{origin}\"\nshare = \"{share_as}\"\n",
        q(&format!(">={}.{}", env.major, env.minor))
    );
    t += &format!("contents = [{}]\n", contents.iter().map(|c| q(c)).collect::<Vec<_>>().join(", "));
    t += &format!("affects_play = {}\n", !map.props.is_empty() || !map.races.is_empty());
    if let Some(b) = base {
        t += &format!("\n[game]\nbases = [{}]\nfingerprint = [{}]\n", q(&b.id), q(&b.identity));
    }
    Manifest::parse(t.as_bytes(), Container::Unknown).map_err(|e| format!("pack.toml: {e}"))?;
    let mut files: Vec<(String, Vec<u8>)> = vec![("pack.toml".into(), t.into_bytes())];
    for f in &list {
        let b = std::fs::read(dir.join(&f.path)).map_err(|e| format!("{}: {e}", f.path))?;
        files.push((f.path.clone(), b));
    }
    files.push((share::PROVENANCE.into(), share::provenance(&o.build, &derived).into_bytes()));
    let bytes = modpack::pack_file_bytes(&files).map_err(|e| e.to_string())?;
    let out = if to.extension().is_some_and(|e| e.eq_ignore_ascii_case(modpack::EXTENSION)) {
        to.to_path_buf()
    } else {
        to.join(modpack::file_name(folder, &o.version))
    };
    if let Some(p) = out.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    crate::platform::fs::write_atomic(&out, &bytes, true).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(Exported { to: out, id: folder.to_string(), files: files.len(), bytes: bytes.len() as u64 })
}

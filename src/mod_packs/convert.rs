//! A legacy mod as a mod pack (layering.md §7): a plain map mod (a folder or `.zip` holding `map.obmap`, as the map
//! editor and the launcher read it) or an Overbright texture pack of format 1 (`world/`, `car/` and `any/` image
//! folders, the TexMod and TexWizard imports among them) is written as a `.obpk`. The provenance says it was
//! converted and from what; what cannot be mapped is refused with its name, never dropped silently.

use std::path::{Path, PathBuf};

use ob_asset_format::modpack::BaseId;

use super::content::GLOBAL_IMAGES;
use super::credits;
use super::pack_export::{self, Build, ExportOpts, Exported, q};
use crate::tools::map::{share, store};

/// What a conversion wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Converted {
    pub exported: Exported,
    /// `map mod` or `texture pack`.
    pub kind: &'static str,
    /// Files left out because the pack format has no place for them (each named).
    pub skipped: Vec<String>,
}

fn scratch() -> Result<PathBuf, String> {
    let d = std::env::temp_dir().join(format!("overbright-convert-{}", ob_core::unique_tag()));
    std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    Ok(d)
}

/// The pack id a folder name makes: lower case, other characters `-`.
fn id_of(name: &str) -> String {
    let s: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' { c } else { '-' })
        .collect();
    s.trim_matches('-').to_string()
}

/// Converts `from` (a folder or `.zip`) to a pack file at `to` (a `.obpk` path or a folder). `id`: the pack id (default:
/// made from the folder name, or the map mod's name). `base`: as [`pack_export::export_map_mod`].
pub fn convert_legacy(
    from: &Path,
    to: &Path,
    id: Option<&str>,
    base: Option<&BaseId>,
    o: &ExportOpts,
) -> Result<Converted, String> {
    let at = |e: &dyn std::fmt::Display| format!("{}: {e}", from.display());
    let tmp = scratch()?;
    let r = (|| {
        let is_zip = from.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"));
        if !from.is_dir() && !is_zip && !from.file_name().is_some_and(|n| n == store::FILE) {
            return Err(at(&"not a folder or .zip of a map mod or a texture pack"));
        }
        let is_map = if from.is_dir() {
            from.join(store::FILE).is_file()
        } else {
            from.file_name().is_some_and(|n| n == store::FILE) || zip_has(from, store::FILE)
        };
        if is_map {
            let folder = share::import(&tmp, from)?;
            let dir = tmp.join(&folder);
            let pid = match id {
                Some(i) => i.to_string(),
                None => {
                    // The source's own name; the map's name (what the import names the folder by) when that makes no id.
                    let stem = if from.is_dir() { from.file_name() } else { from.file_stem() };
                    let own = stem.and_then(|n| n.to_str()).map(id_of).unwrap_or_default();
                    if ob_asset_format::names::is_pack_id(&own) { own } else { id_of(&folder) }
                }
            };
            let src = from.file_name().and_then(|n| n.to_str()).unwrap_or("a map mod");
            let e = pack_export::export_dir(&dir, &pid, to, base, o, Some(&format!("map mod {src}")))?;
            let mut skipped: Vec<String> = share::files(&dir)
                .map_err(|e| at(&e))?
                .into_iter()
                .map(|f| f.path)
                .filter(|p| p != store::FILE && !p.starts_with("textures/") && p != share::LOOKS && p != credits::FILE)
                .collect();
            skipped.sort();
            Ok(Converted { exported: e, kind: "map mod", skipped })
        } else {
            let root = if from.is_dir() { from.to_path_buf() } else { unzip(from, &tmp)? };
            texture_pack(&root, from, to, id, base, o)
        }
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    r
}

fn zip_has(zip: &Path, name: &str) -> bool {
    crate::formats::zip::Zip::open(zip).is_ok_and(|z| {
        z.entries.iter().any(|e| e.name == name || e.name.strip_suffix(name).is_some_and(|d| d.matches('/').count() == 1 && d.ends_with('/')))
    })
}

/// Unpacks `zip` into `into` (checked names, bounded sizes) and returns the folder that holds the pack: the archive's
/// root, or its one top folder.
fn unzip(zip: &Path, into: &Path) -> Result<PathBuf, String> {
    let at = |e: &dyn std::fmt::Display| format!("{}: {e}", zip.display());
    if std::fs::metadata(zip).map_err(|e| at(&e))?.len() > share::MAX_BYTES {
        return Err(at(&"too large for a mod"));
    }
    let z = crate::formats::zip::Zip::open(zip).map_err(|e| at(&format!("{e:#}")))?;
    if z.entries.len() > share::MAX_FILES {
        return Err(at(&"too many files"));
    }
    let mut total = 0u64;
    for e in z.entries.iter().filter(|e| !e.name.ends_with('/')) {
        if !share::safe(&e.name) {
            return Err(at(&format!("{} is not a file name a pack may have", e.name)));
        }
        total += e.size as u64;
        if total > share::MAX_BYTES {
            return Err(at(&"too large for a mod"));
        }
        let out = into.join(&e.name);
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).map_err(|x| at(&x))?;
        }
        std::fs::write(&out, z.read(e).map_err(|x| at(&format!("{x:#}")))?).map_err(|x| at(&x))?;
    }
    let tops: Vec<PathBuf> = std::fs::read_dir(into).map_err(|e| at(&e))?.flatten().map(|e| e.path()).collect();
    let is_pack = |d: &Path| ["world", "car", "any"].iter().any(|s| d.join(s).is_dir());
    Ok(match tops.as_slice() {
        [one] if one.is_dir() && !is_pack(into) => one.clone(),
        _ => into.to_path_buf(),
    })
}

struct Image {
    folder: &'static str,
    rel: String,
    key: String,
    hash: u32,
    path: PathBuf,
}

fn collect(root: &Path, folder: &'static str, out: &mut Vec<Image>, skipped: &mut Vec<String>) -> Result<(), String> {
    fn walk(
        dir: &Path,
        rel: &str,
        depth: usize,
        folder: &'static str,
        out: &mut Vec<Image>,
        skipped: &mut Vec<String>,
    ) -> Result<(), String> {
        let mut es: Vec<_> = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten().collect();
        es.sort_by_key(|e| e.file_name());
        for e in es {
            let name = e.file_name().to_string_lossy().to_string();
            let r = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            let t = e.file_type().map_err(|x| format!("{r}: {x}"))?;
            if name.starts_with('.') {
                continue;
            }
            if t.is_dir() {
                if depth >= crate::obpack::index::MAX_DEPTH {
                    return Err(format!("{folder}/{r}: folders go deeper than {} levels", crate::obpack::index::MAX_DEPTH));
                }
                walk(&e.path(), &r, depth + 1, folder, out, skipped)?;
            } else if t.is_file() {
                let (stem, ext) = match name.rsplit_once('.') {
                    Some((s, x)) => (s.to_string(), x.to_ascii_lowercase()),
                    None => (name.clone(), String::new()),
                };
                if ext != "png" && ext != "dds" {
                    skipped.push(format!("{folder}/{r}"));
                    continue;
                }
                let Some((hash, _)) = crate::obpack::index::key_of_stem(&stem) else {
                    return Err(format!("{folder}/{r}: not a texture name"));
                };
                out.push(Image { folder, rel: r, key: stem, hash, path: e.path() });
            } else {
                skipped.push(format!("{folder}/{r}"));
            }
        }
        Ok(())
    }
    let d = root.join(folder);
    if d.is_dir() {
        walk(&d, "", 1, folder, out, skipped)?;
    }
    Ok(())
}

fn texture_pack(
    root: &Path,
    from: &Path,
    to: &Path,
    id: Option<&str>,
    base: Option<&BaseId>,
    o: &ExportOpts,
) -> Result<Converted, String> {
    let at = |e: &dyn std::fmt::Display| format!("{}: {e}", from.display());
    let mut images = Vec::new();
    let mut skipped = Vec::new();
    for f in ["world", "car", "any"] {
        collect(root, f, &mut images, &mut skipped)?;
    }
    if images.is_empty() {
        return Err(at(&"neither a map mod (no map.obmap) nor a texture pack (no images in world/, car/ or any/)"));
    }
    if images.len() > crate::obpack::index::MAX_FILES {
        return Err(at(&"too many images for a pack"));
    }
    // Two images for one key in one folder are refused (TEX.14): the pack would not say which one wins.
    let mut seen = std::collections::HashSet::new();
    for i in &images {
        if !seen.insert((i.folder, i.hash)) {
            return Err(at(&format!("{}/{}: another image in {}/ has the same texture", i.folder, i.rel, i.folder)));
        }
    }
    let manifest = match std::fs::read_to_string(root.join(crate::obpack::manifest::FILE)) {
        Ok(t) => crate::obpack::manifest::Manifest::parse(&t).map_err(|e| at(&format!("{e:#}")))?,
        Err(_) => crate::obpack::manifest::Manifest::bare(
            &root.file_name().and_then(|n| n.to_str()).map(str::to_string).unwrap_or_else(|| "textures".into()),
        ),
    };
    let src_name = root.file_name().and_then(|n| n.to_str()).unwrap_or("textures");
    let pid = match id {
        Some(i) => i.to_string(),
        None => id_of(src_name),
    };
    if !ob_asset_format::names::is_pack_id(&pid) {
        return Err(at(&format!("{pid} is not a pack id (a-z, 0-9, - and _); give one with --id")));
    }
    let has = |f: &str, h: u32| images.iter().any(|i| i.folder == f && i.hash == h);
    let mut text = String::from("schema = \"ob.texture-overrides/1.0\"\n");
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for i in &images {
        // `any/` feeds the domains that have no image of their own for the key (src/obpack/index.rs).
        let mut draw: Vec<&str> = match i.folder {
            "world" => vec!["world"],
            "car" => vec!["car"],
            _ => ["world", "car"].into_iter().filter(|d| !has(d, i.hash)).collect(),
        };
        let png = i.rel.to_ascii_lowercase().ends_with(".png");
        if draw.is_empty() {
            skipped.push(format!("{}/{} (hidden by a world/ and a car/ image of the same texture)", i.folder, i.rel));
            continue;
        }
        let key = if i.key.starts_with("0x") || i.key.starts_with("0X") || (i.key.len() == 8 && u32::from_str_radix(&i.key, 16).is_ok()) {
            format!("hash = {}", q(&format!("0x{:08X}", i.hash)))
        } else {
            format!("name = {}", q(&i.key))
        };
        text += &format!(
            "\n[[texture]]\n{key}\nimage = {}\ndraw = [{}]\n",
            q(&format!("images/{}/{}", i.folder, i.rel)),
            draw.drain(..).map(q).collect::<Vec<_>>().join(", ")
        );
        if png && manifest.compress == crate::obpack::manifest::Compress::None {
            text += "compress = \"none\"\n";
        }
        files.push((
            format!("{GLOBAL_IMAGES}/{}/{}", i.folder, i.rel),
            std::fs::read(&i.path).map_err(|e| format!("{}: {e}", i.rel))?,
        ));
    }
    files.push(("global/textures/overrides.toml".into(), text.into_bytes()));
    let name = if manifest.name.trim().is_empty() { pid.clone() } else { manifest.name.clone() };
    // The credits the pack's folder lists, and its own author as the credit for the images (added by itself: the pack
    // names who made the art whatever the converting player is).
    let mut credits = credits::of_dir(root);
    if !manifest.author.trim().is_empty() {
        let own = credits::Credit {
            name: name.clone(),
            by: manifest.author.trim().to_string(),
            licence: Some(manifest.licence.trim().to_string()).filter(|l| !l.is_empty()),
            source: Some(manifest.source.trim().to_string()).filter(|l| !l.is_empty()),
        };
        if !credits.iter().any(|c| c.by == own.by) {
            credits.insert(0, own);
        }
    }
    let provenance = pack_export::provenance_text(
        &o.build,
        &[],
        Some(&format!("texture pack {}", from.file_name().and_then(|n| n.to_str()).unwrap_or("a texture pack"))),
        &credits,
    );
    let e = pack_export::write_pack(
        Build {
            id: &pid,
            name: &name,
            authors: &manifest.author,
            description: &manifest.description,
            // Where the pixels come from is not known: the pack stays on this computer (MAN.4).
            origin: "from-game-files",
            share: "local-only",
            contents: vec!["textures"],
            affects_play: false,
            files,
            provenance,
        },
        to,
        base,
        o,
    )?;
    Ok(Converted { exported: e, kind: "texture pack", skipped })
}

//! A pack's textures: every `.png` / `.dds` under `world/`, `car/` and `any/`
//! (subfolders searched), keyed by the game's texture hash.

use super::Domain;
use anyhow::{Result, bail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The most image files a pack may hold.
pub const MAX_FILES: usize = 65_536;
/// How deep subfolders are searched below a domain folder.
pub const MAX_DEPTH: usize = 8;

/// The domain folders, and the domains each feeds.
pub const FOLDERS: [(&str, &[Domain]); 3] =
    [("world", &[Domain::World]), ("car", &[Domain::Car]), ("any", &[Domain::World, Domain::Car])];

/// The texture a file stem stands for: `0x` and 1–8 hex digits, or exactly
/// 8 hex digits, is the hash itself; anything else is a texture name, hashed
/// as the game hashes names (`bin_hash`, exactly as written: the game's names
/// are usually upper case). None for an empty stem or one with a path in it.
pub fn key_of_stem(stem: &str) -> Option<(u32, Option<String>)> {
    if stem.is_empty() || stem.contains(['/', '\\']) || stem.chars().any(char::is_control) {
        return None;
    }
    let hex = stem.strip_prefix("0x").or_else(|| stem.strip_prefix("0X"));
    if let Some(h) = hex
        && !h.is_empty()
        && h.len() <= 8
        && h.bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Some((u32::from_str_radix(h, 16).ok()?, None));
    }
    if stem.len() == 8 && stem.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Some((u32::from_str_radix(stem, 16).ok()?, None));
    }
    Some((crate::formats::hash::bin_hash(stem), Some(stem.to_string())))
}

/// An image's kind, by its extension (case ignored).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Png,
    Dds,
}

/// One texture of a pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub hash: u32,
    /// The name the file is called by (None: named by its hash).
    pub name: Option<String>,
    /// The file, relative to the pack's folder.
    pub file: PathBuf,
    pub kind: Kind,
}

/// The pack's textures per domain (a key in `world/` or `car/` wins over the
/// same key in `any/`), and the files that are not images it reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Index {
    pub world: BTreeMap<u32, Entry>,
    pub car: BTreeMap<u32, Entry>,
    pub ignored: Vec<PathBuf>,
}

impl Index {
    pub fn domain(&self, d: Domain) -> &BTreeMap<u32, Entry> {
        match d {
            Domain::World => &self.world,
            Domain::Car => &self.car,
        }
    }

    /// Distinct image files.
    pub fn files(&self) -> usize {
        let mut f: Vec<&PathBuf> = self.world.values().chain(self.car.values()).map(|e| &e.file).collect();
        f.sort();
        f.dedup();
        f.len()
    }
}

fn walk(dir: &Path, rel: &Path, depth: usize, out: &mut Vec<PathBuf>) -> Result<()> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Ok(()) };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let ft = e.file_type()?;
        let r = rel.join(&name);
        if ft.is_dir() {
            if depth >= MAX_DEPTH {
                bail!("{}: folders nested deeper than {MAX_DEPTH}", r.display());
            }
            walk(&e.path(), &r, depth + 1, out)?;
        } else if ft.is_file() {
            out.push(r);
            if out.len() > MAX_FILES {
                bail!("more than {MAX_FILES} files");
            }
        }
    }
    Ok(())
}

/// The pack in `dir` read. Refused: two images for one texture in one
/// domain folder (`ROAD.png` and `ROAD.dds`, or a name and its hash), too
/// many files or folders too deep.
pub fn scan(dir: &Path) -> Result<Index> {
    let mut idx = Index::default();
    for (folder, domains) in FOLDERS {
        let mut files = Vec::new();
        walk(&dir.join(folder), Path::new(folder), 0, &mut files)?;
        let mut own: BTreeMap<u32, Entry> = BTreeMap::new();
        for f in files {
            let ext = f.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
            let kind = match ext.as_deref() {
                Some("png") => Kind::Png,
                Some("dds") => Kind::Dds,
                _ => {
                    idx.ignored.push(f);
                    continue;
                }
            };
            let Some((hash, name)) = f.file_stem().and_then(|s| s.to_str()).and_then(key_of_stem) else {
                idx.ignored.push(f);
                continue;
            };
            if let Some(prev) = own.get(&hash) {
                bail!("{} and {} replace the same texture ({hash:08X}): keep one", prev.file.display(), f.display());
            }
            own.insert(hash, Entry { hash, name, file: f, kind });
        }
        for d in domains {
            let map = match d {
                Domain::World => &mut idx.world,
                Domain::Car => &mut idx.car,
            };
            for (k, e) in &own {
                // The domain's own folder comes first (FOLDERS' order), `any/` fills in.
                map.entry(*k).or_insert_with(|| e.clone());
            }
        }
    }
    Ok(idx)
}

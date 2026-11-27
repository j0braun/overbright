//! Mounting a finished base set as the game's asset layer (import.md §13 "Reading through the asset layer";
//! layout.md §5 "Verification on mount").
//!
//! [`BaseSet::open`] reads `import.toml`, refuses a set made by a newer importer (`VER.4`) or one whose checks did
//! not pass, maps every listed bundle (missing or of another size: `LAY.3`), verifies each bundle's digest against
//! the record unless its size, modification time and inode match the stamp of an earlier verification (kept in the
//! cache folder when one is given), and indexes every stage-0 kept file (a stored, aligned entry of the recorded
//! size). The set then answers [`ob_core::game_fs::Layer::get`] with slices of the maps: the original's bytes, so the
//! game's parsers run on them unchanged.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::bundle::{self, Bundle};
use crate::digest::Digest;
use crate::import::{self, kept_path, unit_of};
use crate::{Refusal, Rule};

/// A mounted base set: every kept file by its upper-case game path.
pub struct BaseSet {
    id: String,
    files: HashMap<String, ob_core::game_fs::Mapped>,
    /// The recorded SHA-256 of every kept file (the asset cache's source keys).
    digests: HashMap<String, [u8; 32]>,
    /// What the import's scan skipped inside the content folders (upper case): not known absent.
    skipped: Vec<String>,
    /// The folders' entries (upper-case folder key, "" the root, to its (name, is a folder) pairs), made on the first listing.
    dirs: std::sync::OnceLock<HashMap<String, Vec<(String, bool)>>>,
    /// Bytes hashed at this mount (0 when every bundle's stamp matched).
    pub hashed: u64,
}

/// The stamp file of a set: one `<bundle> <size> <mtime ns> <inode> <digest>` line per verified bundle.
fn stamp_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("base-verified-{id}.txt"))
}

/// Forgets the verification stamps of set `id` in `stamps` (the cache folder): the next mount hashes every bundle
/// again. For a full check that found a part damaged although its size, time and inode are unchanged (bit rot, a copy
/// that kept the times): its stamp would still be trusted and the damaged set mounted.
pub fn forget_verified(stamps: &Path, id: &str) {
    let _ = std::fs::remove_file(stamp_path(stamps, id));
}

fn file_stamp(m: &std::fs::Metadata) -> (u64, u64, u64) {
    let mtime =
        m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as u64);
    #[cfg(unix)]
    let ino = std::os::unix::fs::MetadataExt::ino(m);
    #[cfg(not(unix))]
    let ino = 0;
    (m.len(), mtime, ino)
}

impl BaseSet {
    /// Opens and verifies the finished base set in `set` (`<base>/<id>/`). `stamps`: the folder for the
    /// verification stamps (the cache folder); None verifies every bundle's digest at every mount.
    pub fn open(set: &Path, stamps: Option<&Path>) -> Result<BaseSet, Refusal> {
        let rec = import::read_record(set)?;
        let p = set.join("import.toml");
        let doc: toml::Table = std::fs::read_to_string(&p)
            .ok()
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| Refusal::new(Rule::Lay3, format!("{}: unreadable", p.display())))?;
        let epoch = doc.get("importer").and_then(|t| t.get("epoch")).and_then(|v| v.as_integer()).unwrap_or(-1);
        if epoch < 0 {
            return Err(Refusal::new(Rule::Lay3, format!("{}: no importer.epoch", p.display())).in_pack(&rec.id));
        }
        if epoch as u64 > import::EPOCH as u64 {
            return Err(Refusal::new(
                Rule::Ver4,
                format!("import epoch {epoch} is newer than this Overbright's ({})", import::EPOCH),
            )
            .in_pack(&rec.id));
        }
        let checks = doc.get("checks").and_then(|v| v.as_table());
        for c in ["read_back", "rebuild", "profile"] {
            if checks.and_then(|t| t.get(c)).and_then(|v| v.as_str()) != Some("pass") {
                return Err(
                    Refusal::new(Rule::Lay3, format!("{}: checks.{c} did not pass", p.display())).in_pack(&rec.id)
                );
            }
        }
        let kept: HashMap<String, (u64, u64, u64, String)> = stamps
            .and_then(|d| std::fs::read_to_string(stamp_path(d, &rec.id)).ok())
            .map(|t| {
                t.lines()
                    .filter_map(|l| {
                        let f: Vec<&str> = l.split(' ').collect();
                        let [n, s, m, i, d] = f[..] else { return None };
                        Some((n.to_string(), (s.parse().ok()?, m.parse().ok()?, i.parse().ok()?, d.to_string())))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Open every bundle; hash those whose stamp does not match, in parallel.
        let names: Vec<(&String, &(u64, Digest))> = rec.bundles.iter().collect();
        type Opened = Result<(Bundle, (u64, u64, u64), bool), Refusal>;
        let check = |name: &str, size: u64, digest: &Digest| -> Opened {
            let lay3 = |m: String| Refusal::new(Rule::Lay3, m).in_pack(&rec.id);
            let path = set.join("bundles").join(format!("{name}.obz"));
            let meta = std::fs::metadata(&path).map_err(|e| lay3(format!("{}: {e}", path.display())))?;
            let stamp = file_stamp(&meta);
            if stamp.0 != size {
                return Err(lay3(format!("{} is {} bytes; the record says {size}", path.display(), stamp.0)));
            }
            let b = Bundle::open_shared(&path, bundle::Kind::Bundle).map_err(|r| r.in_pack(&rec.id))?;
            let trusted = kept.get(name).is_some_and(|k| (k.0, k.1, k.2) == stamp && k.3 == digest.to_string());
            if !trusted {
                let got = b.digest();
                if got != *digest {
                    return Err(lay3(format!("{} has digest {got}; the record says {digest}", path.display())));
                }
            }
            Ok((b, stamp, !trusted))
        };
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8);
        let mut opened: Vec<Option<Opened>> = (0..names.len()).map(|_| None).collect();
        std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    let (names, check) = (&names, &check);
                    s.spawn(move || {
                        (t..names.len())
                            .step_by(threads)
                            .map(|i| (i, check(names[i].0, names[i].1.0, &names[i].1.1)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            for h in handles {
                for (i, r) in h.join().expect("a bundle check panicked") {
                    opened[i] = Some(r);
                }
            }
        });
        let mut bundles: HashMap<String, Bundle> = HashMap::new();
        let mut stamp_lines = String::new();
        let mut hashed = 0;
        for (r, (name, (_, digest))) in opened.into_iter().zip(names.iter().copied()) {
            let (b, st, was_hashed) = r.expect("every bundle checked")?;
            if was_hashed {
                hashed += st.0;
            }
            stamp_lines += &format!("{name} {} {} {} {digest}\n", st.0, st.1, st.2);
            bundles.insert(name.clone(), b);
        }
        // Index every kept file.
        let mut files = HashMap::new();
        let mut digests = HashMap::new();
        for f in &rec.scan.files {
            let unit = unit_of(&f.path);
            let lay3 = |m: String| Refusal::new(Rule::Lay3, m).in_pack(&rec.id);
            let b = bundles.get(&unit).ok_or_else(|| lay3(format!("no bundle {unit}.obz for {}", f.path)))?;
            let e =
                b.get(&kept_path(&f.path)).ok_or_else(|| lay3(format!("{unit}.obz has no kept copy of {}", f.path)))?;
            if !e.mapped() || e.size != f.size {
                return Err(lay3(format!(
                    "{unit}.obz: the kept copy of {} is not a stored aligned copy of {} bytes",
                    f.path, f.size
                )));
            }
            let map = b.shared_map().expect("opened shared");
            let start = e.data_offset as usize;
            let m = ob_core::game_fs::Mapped::new(map, start..start + e.size as usize)
                .ok_or_else(|| lay3(format!("{unit}.obz: {} lies outside the bundle", f.path)))?;
            let key = f.path.replace('\\', "/").to_ascii_uppercase();
            if let Some(d) = f.sha256 {
                digests.insert(key.clone(), d);
            }
            files.insert(key, m);
        }
        if let Some(d) = stamps
            && hashed > 0
        {
            // A failed write only means the next mount hashes again.
            let _ = std::fs::create_dir_all(d);
            let tmp = d.join(format!(".base-verified-{}.{}", rec.id, ob_core::unique_tag()));
            if std::fs::write(&tmp, &stamp_lines).is_ok() && std::fs::rename(&tmp, stamp_path(d, &rec.id)).is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
        }
        let skipped = rec.scan.skipped.iter().map(|(p, _)| p.replace('\\', "/").to_ascii_uppercase()).collect();
        Ok(BaseSet { id: rec.id, files, digests, skipped, dirs: std::sync::OnceLock::new(), hashed })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// The set's files whose copy in the game folder `root` now has another size (replaced after the import, as a
    /// mod that swaps a file does), sorted. The set keeps serving its own bytes; the caller says so. Same-size
    /// changes are not seen (only sizes are compared: one `stat` per file).
    pub fn changed_in(&self, root: &Path) -> Vec<String> {
        let mut v: Vec<String> = self
            .files
            .iter()
            .filter(|(key, m)| {
                ob_core::game_fs::find(root, key)
                    .and_then(|p| std::fs::metadata(p).ok())
                    .is_some_and(|md| md.is_file() && md.len() != m.len() as u64)
            })
            .map(|(key, _)| key.clone())
            .collect();
        v.sort();
        v
    }

    /// How many game files the set holds.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

impl ob_core::game_fs::Layer for BaseSet {
    fn get(&self, key: &str) -> Option<ob_core::game_fs::Mapped> {
        self.files.get(key).cloned()
    }
    fn digest(&self, key: &str) -> Option<[u8; 32]> {
        self.digests.get(key).copied()
    }
    fn name(&self) -> &str {
        &self.id
    }
    /// The entries of a folder the set holds files in, unless the import skipped something inside it.
    fn list(&self, dir: &str) -> Option<Vec<(String, bool)>> {
        let dirs = self.dirs.get_or_init(|| {
            let mut sets: HashMap<String, std::collections::BTreeMap<String, bool>> = HashMap::new();
            for key in self.files.keys() {
                let parts: Vec<&str> = key.split('/').collect();
                for i in 0..parts.len() {
                    let folder = parts[..i].join("/");
                    sets.entry(folder).or_default().insert(parts[i].to_string(), i + 1 < parts.len());
                }
            }
            sets.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect()
        });
        if dir.is_empty()
            || self.skipped.iter().any(|s| s == dir || s.strip_prefix(dir).is_some_and(|r| r.starts_with('/')))
        {
            return None;
        }
        dirs.get(dir).cloned()
    }
    /// A name in one of the content folders the import scanned whole (import.md §2), that the scan did not
    /// skip, and that the set does not hold: the install had no such file (the original asks for some, such as
    /// `NIS/SCENE_ENDINGNIS04_EVTTRK.TXT`, and goes on without them).
    fn absent(&self, key: &str) -> bool {
        let top = key.split('/').next().unwrap_or_default();
        key.contains('/')
            && import::CONTENT_FOLDERS.iter().any(|c| c.eq_ignore_ascii_case(top))
            && !self.files.contains_key(key)
            && !self.files.keys().any(|f| f.strip_prefix(key).is_some_and(|r| r.starts_with('/')))
            && !self
                .skipped
                .iter()
                .any(|s| key == s || key.strip_prefix(s.as_str()).is_some_and(|r| r.starts_with('/')))
    }
}

/// The base set a flag value names: a folder holding `import.toml`; `active` (the set `<base>/active.txt` names,
/// else the only set in `<base>`); or a set id under `<base>`.
pub fn resolve(spec: &str, base: &Path) -> Result<PathBuf, Refusal> {
    let p = Path::new(spec);
    if p.join("import.toml").is_file() {
        return Ok(p.to_path_buf());
    }
    let id = if spec == "active" {
        match std::fs::read(base.join("active.txt")) {
            Ok(b) => crate::order::parse_active(&b)?.ok_or_else(|| {
                Refusal::new(Rule::Lay5, format!("{} names no base set", base.join("active.txt").display()))
            })?,
            Err(_) => {
                let sets: Vec<String> = std::fs::read_dir(base)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| e.path().join("import.toml").is_file())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect();
                match &sets[..] {
                    [one] => one.clone(),
                    [] => return Err(Refusal::new(Rule::Lay5, format!("no base set in {}", base.display()))),
                    _ => {
                        return Err(Refusal::new(
                            Rule::Lay5,
                            format!("several base sets in {} and no active.txt", base.display()),
                        ));
                    }
                }
            }
        }
    } else {
        spec.to_string()
    };
    if !crate::names::is_pack_id(&id) {
        return Err(Refusal::new(Rule::Lay5, format!("{spec:?} is neither a base set folder nor a set id")));
    }
    let set = base.join(&id);
    if !set.join("import.toml").is_file() {
        return Err(Refusal::new(Rule::Lay5, format!("no finished base set {}", set.display())));
    }
    Ok(set)
}

#[cfg(test)]
mod tests;

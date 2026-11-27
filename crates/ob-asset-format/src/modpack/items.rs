//! Per-item fingerprints (manifest.md §4.2, format 3): `items.toml` lists every file of a mod pack other than
//! `pack.toml` and itself with its `xxh3-128` digest, so a changed, missing or added item is named exactly.
//! Packs without it (every older pack) are read as before and report no fingerprints.

use std::collections::BTreeMap;

use super::{Installed, Source};
use crate::bundle::{self, Bundle};
use crate::digest::{Alg, Digest};
use crate::text::toml_string;
use crate::{Refusal, Rule};

/// The usual name of the items file (the `items` field of `pack.toml` names the real one).
pub const ITEMS_FILE: &str = "items.toml";
/// The schema line of the items file.
pub const SCHEMA: &str = "ob.items/1.0";
/// The items file's size limit.
pub const MAX_BYTES: usize = 4 << 20;

/// Whether a logical path takes part in the fingerprints (everything but the manifest, the items file itself and
/// Studio's working data).
fn counts(path: &str, items_file: &str) -> bool {
    path != crate::manifest::FILE && path != items_file && !path.starts_with(".studio/")
}

/// The items file for `files` (logical path, bytes): paths sorted, one digest each. Deterministic.
pub fn items_text(files: &[(String, Vec<u8>)]) -> String {
    let mut rows: Vec<(&str, Digest)> = files
        .iter()
        .filter(|(p, _)| counts(p, ITEMS_FILE))
        .map(|(p, b)| (p.as_str(), Alg::Xxh3_128.of(b)))
        .collect();
    rows.sort_by(|a, b| a.0.cmp(b.0));
    let mut t = format!("schema = \"{SCHEMA}\"\n\n[items]\n");
    for (p, d) in rows {
        t += &format!("{} = {}\n", toml_string(p), toml_string(&d.to_string()));
    }
    t
}

/// Reads an items file: path -> digest.
pub fn parse_items(bytes: &[u8]) -> Result<BTreeMap<String, Digest>, Refusal> {
    let bad = |m: String| Refusal::new(Rule::Man10, m).at_path(ITEMS_FILE);
    let text = crate::text::decode(bytes, MAX_BYTES, Rule::Man10)?;
    let doc = crate::text::parse_toml(text)?;
    match doc.get("schema").and_then(|s| s.as_str()) {
        Some(s) if s.starts_with("ob.items/1.") => {}
        other => return Err(bad(format!("schema {other:?} is not {SCHEMA}"))),
    }
    let table = doc.get("items").and_then(|i| i.as_table()).ok_or_else(|| bad("no [items] table".into()))?;
    let mut out = BTreeMap::new();
    for (k, v) in table {
        crate::names::logical_path(k, false).map_err(|r| bad(format!("{k:?}: {r}")))?;
        let d = v.as_str().ok_or_else(|| bad(format!("{k:?}: not a digest string")))?;
        out.insert(k.clone(), Digest::parse(d).map_err(|r| bad(r.to_string()))?);
    }
    Ok(out)
}

/// What a comparison of a pack's files against its fingerprints found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemReport {
    /// How many items the pack lists.
    pub listed: usize,
    /// Items whose bytes differ from the listed digest.
    pub changed: Vec<String>,
    /// Listed items the pack no longer holds.
    pub missing: Vec<String>,
    /// Files the pack holds that are not listed.
    pub added: Vec<String>,
}

impl ItemReport {
    pub fn is_clean(&self) -> bool {
        self.changed.is_empty() && self.missing.is_empty() && self.added.is_empty()
    }

    /// One line per finding, for the look-inside page.
    pub fn lines(&self) -> Vec<String> {
        let mut v = Vec::new();
        for (what, l) in [("Changed", &self.changed), ("Missing", &self.missing), ("Not in its fingerprints", &self.added)] {
            v.extend(l.iter().map(|p| format!("{what}: {p}")));
        }
        v
    }
}

/// Compares the listed digests with the digests of the files the pack holds.
pub fn compare(listed: &BTreeMap<String, Digest>, actual: &BTreeMap<String, Digest>) -> ItemReport {
    let mut r = ItemReport { listed: listed.len(), ..Default::default() };
    for (p, d) in listed {
        match actual.get(p) {
            None => r.missing.push(p.clone()),
            Some(a) if a != d => r.changed.push(p.clone()),
            Some(_) => {}
        }
    }
    r.added = actual.keys().filter(|p| !listed.contains_key(*p)).cloned().collect();
    r
}

fn walk(root: &std::path::Path, rel: &str, out: &mut Vec<(String, std::path::PathBuf)>) -> std::io::Result<()> {
    for e in std::fs::read_dir(root.join(rel))? {
        let e = e?;
        let name = e.file_name().to_string_lossy().to_string();
        let path = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        let t = e.file_type()?;
        if t.is_dir() {
            walk(root, &path, out)?;
        } else if t.is_file() {
            out.push((path, e.path()));
        }
    }
    Ok(())
}

/// Checks the installed or opened pack `p` against its fingerprints. `Ok(None)`: the pack has none (an older pack;
/// nothing is checked and nothing is wrong). Read only.
pub fn verify(p: &Installed) -> Result<Option<ItemReport>, String> {
    let Some(file) = p.manifest.items.as_deref() else { return Ok(None) };
    let mut actual = BTreeMap::new();
    let listed = match &p.source {
        Source::Folder(d) => {
            let bytes = std::fs::read(d.join(file)).map_err(|e| format!("{file}: {e}"))?;
            let listed = parse_items(&bytes).map_err(|e| e.to_string())?;
            let mut files = Vec::new();
            walk(d, "", &mut files).map_err(|e| format!("{}: {e}", d.display()))?;
            for (path, abs) in files.into_iter().filter(|(p, _)| counts(p, file)) {
                let b = std::fs::read(&abs).map_err(|e| format!("{path}: {e}"))?;
                actual.insert(path, Alg::Xxh3_128.of(&b));
            }
            listed
        }
        Source::File(f) => {
            let b = Bundle::open(f, bundle::Kind::PackFile).map_err(|e| e.to_string())?;
            let e = b.get(file).ok_or_else(|| format!("{file}: listed in pack.toml but not in the pack"))?;
            let listed = parse_items(&b.read(e).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            for e in b.entries().iter().filter(|e| counts(&e.path, file)) {
                actual.insert(e.path.clone(), Alg::Xxh3_128.of(&b.read(e).map_err(|x| x.to_string())?));
            }
            listed
        }
    };
    Ok(Some(compare(&listed, &actual)))
}

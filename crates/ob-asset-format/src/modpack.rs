//! Mod packs on the player's machine (layering.md, layout.md §1): the packs in `<app>/mods` (a folder with a
//! format-2 `pack.toml`, or `<id>-<version>.obpk`), the player's order and switches in `order.txt`, the stack the
//! game mounts from them, the checks that refuse a pack (made for another Overbright, another base set, with
//! residual data), and the import of a pack file.
//!
//! Nothing here reads the game's files or the content of a pack beyond its manifest and the names of its files;
//! what a pack holds is read by the kinds that use it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::bundle::{self, Bundle};
use crate::manifest::{Container, Manifest, PackKind};
use crate::order::{Line, Order};
use crate::{Refusal, Rule};

/// The mods folder's name inside Overbright's folder.
pub const MODS_DIR: &str = "mods";
/// The order file inside it.
pub const ORDER_FILE: &str = "order.txt";
/// A pack file's extension.
pub const EXTENSION: &str = "obpk";

/// Where a pack is stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Folder(PathBuf),
    File(PathBuf),
}

impl Source {
    pub fn path(&self) -> &Path {
        match self {
            Source::Folder(p) | Source::File(p) => p,
        }
    }
}

/// An installed mod pack (its manifest was read).
#[derive(Clone, Debug)]
pub struct Installed {
    pub id: String,
    pub manifest: Manifest,
    pub source: Source,
    pub warnings: Vec<String>,
}

impl Installed {
    /// Whether the pack holds a file or folder at the logical path `rel` (`residual`, `import.toml`).
    fn has(&self, rel: &str) -> bool {
        match &self.source {
            Source::Folder(d) => d.join(rel).exists(),
            Source::File(f) => Bundle::open(f, bundle::Kind::PackFile).is_ok_and(|b| {
                b.get(rel).is_some()
                    || b.entries().iter().any(|e| e.path.strip_prefix(rel).is_some_and(|r| r.starts_with('/')))
            }),
        }
    }
}

/// What was found in the mods folder.
#[derive(Debug, Default)]
pub struct Scan {
    pub packs: Vec<Installed>,
    /// Entries that look like packs but cannot be read: (name, why).
    pub problems: Vec<(String, String)>,
}

/// The mods folder `app/mods`.
pub fn mods_dir(app: &Path) -> PathBuf {
    app.join(MODS_DIR)
}

/// Reads the packs in `dir`, sorted by id. A folder without `pack.toml`, a legacy (format 1) manifest and files that are
/// not `.obpk` are not mod packs of this kind and are skipped silently; a pack that is broken is reported.
pub fn scan(dir: &Path) -> Scan {
    let mut out = Scan::default();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let Some(name) = e.file_name().to_str().map(str::to_string) else { continue };
        if name.starts_with('.') {
            continue;
        }
        let path = e.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        let read = if meta.is_dir() {
            std::fs::read(path.join(crate::manifest::FILE))
                .ok()
                .map(|b| Manifest::parse(&b, Container::Folder(&name)).map(|r| (r, Source::Folder(path.clone()))))
        } else if meta.is_file() && name.ends_with(&format!(".{EXTENSION}")) {
            Some(read_pack_file(&path, &name).map(|(m, w)| ((m, w), Source::File(path.clone()))))
        } else {
            None
        };
        match read {
            None => {}
            Some(Ok(((m, warnings), source))) => {
                if m.format < 2 {
                    continue;
                }
                if m.kind != PackKind::Mod {
                    out.problems.push((name, "a base set, not a mod pack".into()));
                } else if let Some(have) = out.packs.iter().find(|p| p.id == m.id) {
                    out.problems
                        .push((name, format!("{} is already installed ({})", m.id, have.source.path().display())));
                } else {
                    out.packs.push(Installed { id: m.id.clone(), manifest: m, source, warnings });
                }
            }
            Some(Err(r)) => out.problems.push((name, r.to_string())),
        }
    }
    out.packs.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn read_pack_file(path: &Path, name: &str) -> Result<(Manifest, Vec<String>), Refusal> {
    let b = Bundle::open(path, bundle::Kind::PackFile)?;
    let e = b
        .get(crate::manifest::FILE)
        .ok_or_else(|| Refusal::new(Rule::Lay1, "a pack file without pack.toml").at_path(crate::manifest::FILE))?;
    let bytes = b.read(e)?;
    Manifest::parse(&bytes, Container::PackFile(name))
}

// ---- base set -----------------------------------------------------------------------------------------------------

/// The base set in use: its id and the identity of the copy of the game it was made from (`sha256:<hex>`, the
/// import record's `source.identity`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseId {
    pub id: String,
    pub identity: String,
}

fn glob(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((head, tail)) => {
            let Some(rest) = text.strip_prefix(head) else { return false };
            if !tail.contains('*') {
                return rest.ends_with(tail) && rest.len() >= tail.len();
            }
            (0..=rest.len()).filter(|&i| rest.is_char_boundary(i)).any(|i| glob(tail, &rest[i..]))
        }
    }
}

/// Whether a pack made for `m.game_bases` / `m.game_fingerprint` may run on `base` (None: no imported copy is in
/// use, the game reads its folder). Every pack without a `[game]` table runs anywhere. The message names what the
/// pack was made for and what is in use.
pub fn check_base(m: &Manifest, base: Option<&BaseId>) -> Result<(), String> {
    let bases: Vec<&str> = m.game_bases.iter().flatten().map(String::as_str).filter(|b| *b != "*").collect();
    let prints: Vec<&str> = m.game_fingerprint.iter().flatten().map(String::as_str).collect();
    if bases.is_empty() && prints.is_empty() {
        return Ok(());
    }
    let made_for = if bases.is_empty() { "another copy of the game".to_string() } else { bases.join(", ") };
    let Some(base) = base else {
        return Err(format!(
            "made for the base set {made_for}, but no imported copy of the game is in use (import your game first)"
        ));
    };
    if !bases.is_empty() && !bases.iter().any(|p| glob(p, &base.id)) {
        return Err(format!("made for the base set {made_for}; the one in use is {}", base.id));
    }
    if !prints.is_empty() && !prints.iter().any(|p| *p == base.identity) {
        return Err(format!(
            "made for another copy of the game than the base set in use ({}): its fingerprint does not match",
            base.id
        ));
    }
    Ok(())
}

// ---- the plan -----------------------------------------------------------------------------------------------------

/// What the game mounts.
#[derive(Debug, Default)]
pub struct Plan {
    /// The packs to mount, weakest first (the last is the strongest: `order.txt`'s first line).
    pub layers: Vec<Installed>,
    /// Packs that are on but refused, with the reason in words.
    pub refused: Vec<(String, String)>,
    /// What else the reader says: unreadable packs, `order.txt` lines skipped.
    pub notices: Vec<String>,
}

fn read_order(dir: &Path) -> Result<(Order, Vec<String>), String> {
    match std::fs::read(dir.join(ORDER_FILE)) {
        Err(_) => Ok((Order::default(), Vec::new())),
        Ok(b) => Order::parse(&b)
            .map(|(o, n)| (o, n.iter().map(|n| format!("{ORDER_FILE}: {n:?}")).collect()))
            .map_err(|r| r.to_string()),
    }
}

/// The stack of the packs that are on in `dir`'s `order.txt`, with the engine range, the base set, residual data and
/// the dependency and conflict rules checked. A refused pack is not mounted at all.
pub fn plan(dir: &Path, base: Option<&BaseId>, engine: &semver::Version) -> Plan {
    let scan = scan(dir);
    let mut plan = Plan::default();
    for (n, why) in &scan.problems {
        plan.notices.push(format!("{n}: {why}"));
    }
    let (order, notes) = match read_order(dir) {
        Ok(v) => v,
        Err(why) => {
            plan.notices.push(why);
            return plan;
        }
    };
    plan.notices.extend(notes);
    let installed: BTreeMap<&str, &Installed> = scan.packs.iter().map(|p| (p.id.as_str(), p)).collect();
    let (on, missing) = order.enabled(|id| installed.contains_key(id));
    for m in missing {
        plan.notices.push(format!("{ORDER_FILE}: {m:?}, skipped"));
    }
    let mut ok: Vec<String> = Vec::new();
    for id in &on {
        let p = installed[id.as_str()];
        let why = p
            .manifest
            .check_engine(engine)
            .map_err(|r| r.message)
            .and_then(|()| p.manifest.check_extensions(&[]).map_err(|r| r.message))
            .and_then(|()| check_base(&p.manifest, base))
            .and_then(|()| {
                if p.has("residual") || p.has("import.toml") {
                    Err("holds residual data or an import record, which only a base set may (MRG.6)".to_string())
                } else {
                    Ok(())
                }
            });
        match why {
            Ok(()) => ok.push(id.clone()),
            Err(w) => plan.refused.push((id.clone(), w)),
        }
    }
    let manifests: BTreeMap<String, Manifest> =
        ok.iter().map(|id| (id.clone(), installed[id.as_str()].manifest.clone())).collect();
    let stack = crate::layering::build_stack(base.map_or("base", |b| b.id.as_str()), &ok, &manifests);
    for (id, r) in &stack.refused {
        plan.refused.push((id.clone(), r.message.clone()));
    }
    for id in stack.layers.iter().skip(1) {
        plan.layers.push((*installed[id.as_str()]).clone());
    }
    plan
}

/// One pack as the launcher lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub name: String,
    pub version: String,
    pub on: bool,
    /// Present in the mods folder but not in `order.txt`: off, shown as new.
    pub new: bool,
    /// Why it cannot be used though it is on (or could not be read).
    pub refused: Option<String>,
}

/// The packs in the order the launcher shows them: those `order.txt` lists (strongest first), then the unlisted
/// ones (off, new) by id; packs that cannot be read come last.
pub fn rows(dir: &Path, base: Option<&BaseId>, engine: &semver::Version) -> Vec<Row> {
    let scan = scan(dir);
    let plan = plan(dir, base, engine);
    let order = read_order(dir).map(|(o, _)| o).unwrap_or_default();
    let refused = |id: &str, m: &Manifest| {
        plan.refused
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, w)| w.clone())
            // A pack that is off is checked too, so the list says so before the player turns it on.
            .or_else(|| check_base(m, base).err())
    };
    let mut rows: Vec<Row> = Vec::new();
    for (id, on) in order.packs() {
        if let Some(p) = scan.packs.iter().find(|p| p.id == id) {
            rows.push(Row {
                id: id.to_string(),
                name: p.manifest.name.clone(),
                version: p.manifest.version.to_string(),
                on,
                new: false,
                refused: refused(id, &p.manifest),
            });
        }
    }
    for p in &scan.packs {
        if !rows.iter().any(|r| r.id == p.id) {
            rows.push(Row {
                id: p.id.clone(),
                name: p.manifest.name.clone(),
                version: p.manifest.version.to_string(),
                on: false,
                new: true,
                refused: refused(&p.id, &p.manifest),
            });
        }
    }
    for (n, why) in scan.problems {
        rows.push(Row { id: n.clone(), name: n, version: String::new(), on: false, new: false, refused: Some(why) });
    }
    rows
}

// ---- order.txt writes ---------------------------------------------------------------------------------------------

fn write_order(dir: &Path, o: &Order) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let p = dir.join(ORDER_FILE);
    ob_core::platform::fs::write_atomic(&p, o.render().as_bytes(), false).map_err(|e| format!("{}: {e}", p.display()))
}

/// `order.txt` with every installed pack listed (unlisted ones at the weak end, off), so a position the launcher shows
/// is a position in the file.
fn listed(dir: &Path) -> Result<Order, String> {
    let (mut order, _) = read_order(dir)?;
    for p in scan(dir).packs {
        if !order.packs().any(|(id, _)| id == p.id) {
            order.set_enabled(&p.id, false);
        }
    }
    Ok(order)
}

/// Switches a pack on or off in `order.txt`. Turning a pack on also turns on its `[dependencies]` and puts each
/// below it; returns the packs turned on besides it, so the launcher can say so (layering.md §2).
pub fn set_enabled(dir: &Path, id: &str, on: bool) -> Result<Vec<String>, String> {
    let scan = scan(dir);
    if !scan.packs.iter().any(|p| p.id == id) {
        return Err(format!("{id} is not installed"));
    }
    let mut order = read_order(dir)?.0;
    order.set_enabled(id, on);
    let mut also = Vec::new();
    if on {
        // Dependencies, depth first, each below what needs it.
        let mut stack = vec![id.to_string()];
        while let Some(top) = stack.pop() {
            let Some(p) = scan.packs.iter().find(|p| p.id == top) else { continue };
            for dep in p.manifest.dependencies.keys() {
                if !scan.packs.iter().any(|q| &q.id == dep) {
                    continue;
                }
                if !order.packs().any(|(i, o)| i == dep && o) {
                    order.set_enabled(dep, true);
                    also.push(dep.clone());
                }
                // A dependency sits below what needs it (later in the file).
                let pos = |o: &Order, x: &str| o.packs().position(|(i, _)| i == x);
                while let (Some(a), Some(b)) = (pos(&order, &top), pos(&order, dep)) {
                    if b > a || !order.move_pack(dep, 1) {
                        break;
                    }
                }
                stack.push(dep.clone());
            }
        }
    }
    write_order(dir, &order)?;
    Ok(also)
}

/// Moves a pack `by` places in `order.txt` (negative: stronger).
pub fn move_pack(dir: &Path, id: &str, by: isize) -> Result<(), String> {
    let mut order = listed(dir)?;
    if !order.move_pack(id, by) {
        return Err(format!("{id} cannot move further"));
    }
    write_order(dir, &order)
}

/// Whether `order.txt` lists the id (tests).
pub fn is_listed(dir: &Path, id: &str) -> bool {
    read_order(dir).is_ok_and(|(o, _)| o.lines.iter().any(|l| matches!(l, Line::Pack { id: i, .. } if i == id)))
}

// ---- writing and importing pack files -----------------------------------------------------------------------------

/// The file name of a pack file: `<id>-<version>.obpk`.
pub fn file_name(id: &str, version: &semver::Version) -> String {
    format!("{id}-{version}.{EXTENSION}")
}

/// The bytes of a pack file holding `files` (logical path, bytes), `pack.toml` first; text entries deflated, the
/// rest stored. The same files always give the same bytes.
pub fn pack_file_bytes(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, Refusal> {
    let mut w = bundle::Writer::new(bundle::Kind::PackFile);
    let mut sorted: Vec<&(String, Vec<u8>)> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (path, data) in sorted {
        let stored = path.ends_with(".png") || path.ends_with(".dds");
        let method = if stored { bundle::Method::Stored } else { bundle::Method::Deflated };
        w.add(path, data.as_slice(), method)?;
    }
    w.finish()
}

/// Adds the pack file `from` to the mods folder `dir` as `<id>-<version>.obpk`: every entry's CRC checked, the
/// manifest read, a base set, residual data and a version that needs another Overbright refused. A pack with the
/// same id is never replaced. Returns the id. `order.txt` is not touched: the pack is new, so it is off.
pub fn import(dir: &Path, from: &Path, engine: &semver::Version) -> Result<String, String> {
    let at = |e: &dyn std::fmt::Display| format!("{}: {e}", from.display());
    let name = from.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if !name.to_ascii_lowercase().ends_with(&format!(".{EXTENSION}")) {
        return Err(at(&format!("not a pack file (.{EXTENSION})")));
    }
    let b = Bundle::open(from, bundle::Kind::PackFile).map_err(|r| at(&r))?;
    b.verify().map_err(|r| at(&r))?;
    let e = b.get(crate::manifest::FILE).ok_or_else(|| at(&"no pack.toml in the file"))?;
    let bytes = b.read(e).map_err(|r| at(&r))?;
    // The file's own name is not trusted: the id and version inside name the copy.
    let (m, _) = Manifest::parse(&bytes, Container::Unknown).map_err(|r| at(&r))?;
    if m.format < 2 {
        return Err(at(&"an old texture pack, not a format-2 mod pack"));
    }
    if m.kind != PackKind::Mod {
        return Err(at(&"a base set, not a mod pack"));
    }
    m.check_engine(engine).map_err(|r| at(&r))?;
    if b.entries().iter().any(|e| e.path == "import.toml" || e.path.starts_with("residual/")) {
        return Err(at(&"holds residual data or an import record, which only a base set may (MRG.6)"));
    }
    if let Some(have) = scan(dir).packs.iter().find(|p| p.id == m.id) {
        return Err(at(&format!(
            "{} {} is already installed ({}); remove it first to replace it",
            m.id,
            have.manifest.version,
            have.source.path().display()
        )));
    }
    let to = dir.join(file_name(&m.id, &m.version));
    std::fs::create_dir_all(dir).map_err(|e| at(&e))?;
    ob_core::platform::fs::write_atomic(&to, b.bytes(), false).map_err(|e| at(&e))?;
    Ok(m.id)
}

/// Removes an installed pack (its file or folder) and its line in `order.txt`.
pub fn remove(dir: &Path, id: &str) -> Result<(), String> {
    let scan = scan(dir);
    let p = scan.packs.iter().find(|p| p.id == id).ok_or_else(|| format!("{id} is not installed"))?;
    match &p.source {
        Source::Folder(d) => std::fs::remove_dir_all(d),
        Source::File(f) => std::fs::remove_file(f),
    }
    .map_err(|e| format!("{id}: {e}"))
}

#[cfg(test)]
mod tests;

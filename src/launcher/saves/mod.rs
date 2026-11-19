//! The Saves page's files (docs/launcher.md, "Saves"): Overbright's
//! profiles read for the list ([`summary`]), backed up, restored, renamed,
//! copied and deleted ([`store`]), the original game's saves found on this
//! computer ([`sources`]) and imported from a folder or a `.zip`, and a
//! profile exported as a `.zip` ([`export`]).
//!
//! A save is a folder holding a file of its own name (any case), as the
//! original keeps them (`<folder>\<NAME>\<NAME>`, 0x4D9AB0): profiles
//! `NAME/NAME` and saved covers `NAME DVD n/…`, `NAME Magazine n/…`.
//!
//! An import never overwrites and never touches its source (only read).
//! Per profile it is one of: new (copied as it is), already here (the same
//! bytes under that name: left alone), a copy (Overbright has another
//! profile of that name in any case: imported under a free name,
//! `store::copy_name`, its folder, file, payload name and covers renamed,
//! so the game shows it under that name), or skipped (the game can't load
//! it). The source's file times are kept.

pub mod jobs;
pub mod sources;
pub mod store;
pub mod summary;

use std::path::{Path, PathBuf};

use crate::formats::native_save::{self as ns, NativeSave};
use store::{FileCopy, Files};

pub fn kb(b: u64) -> String {
    format!("{} KB", b.div_ceil(1024))
}

/// The save folders in `dir`: subfolders holding a file of their own name
/// (any case). `dir` itself when it is such a folder (the player picked
/// one profile's folder).
pub fn save_dirs(dir: &Path) -> Vec<(PathBuf, String)> {
    let own = |d: &Path| -> bool {
        let Some(name) = d.file_name().and_then(|n| n.to_str()) else { return false };
        std::fs::read_dir(d).is_ok_and(|rd| {
            rd.filter_map(|e| e.ok())
                .any(|e| e.path().is_file() && e.file_name().to_str().is_some_and(|f| f.eq_ignore_ascii_case(name)))
        })
    };
    if own(dir) {
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        return vec![(dir.to_path_buf(), name)];
    }
    let mut v: Vec<(PathBuf, String)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter_map(|e| Some((e.path(), e.file_name().into_string().ok()?)))
                .filter(|(p, n)| !n.ends_with(".part") && own(p))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|(_, n)| n.to_ascii_uppercase());
    v
}

/// What an import does with one profile of the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    New,
    /// The same bytes are here under that name already.
    Same,
    /// Imported under this name (the name is taken).
    Copy(String),
    /// The game can't load it.
    Bad(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanItem {
    pub files: Files,
    pub act: Act,
}

impl PlanItem {
    pub fn name(&self) -> &str {
        &self.files.profile.name
    }

    /// The plan's line for it.
    pub fn line(&self) -> String {
        let n = self.name();
        let covers = match self.files.covers.len() {
            0 => String::new(),
            1 => " and its saved cover".into(),
            k => format!(" and its {k} saved covers"),
        };
        match &self.act {
            Act::New => format!("{n}: new{covers}"),
            Act::Same => format!("{n}: already here"),
            Act::Copy(to) => format!("{n}: a profile of this name is already here: imported as {to}{covers}"),
            Act::Bad(_) => format!("{n}: can't be loaded by the game: skipped"),
        }
    }
}

/// An import's plan: shown first, done by [`import`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub from: PathBuf,
    pub zip: bool,
    pub items: Vec<PlanItem>,
    /// Saved covers without their profile in the source (imported when
    /// their name is free).
    pub loose: Vec<FileCopy>,
    /// What a zip held that is not a save (left out), in words.
    pub skipped: Vec<String>,
}

impl Plan {
    /// Profiles that will be written.
    pub fn to_copy(&self) -> usize {
        self.items.iter().filter(|i| matches!(i.act, Act::New | Act::Copy(_))).count()
    }

    pub fn here(&self) -> usize {
        self.items.iter().filter(|i| i.act == Act::Same).count()
    }

    /// "2 new · 1 already here".
    pub fn counts(&self) -> String {
        let mut v = vec![format!("{} new", self.to_copy())];
        if self.here() > 0 {
            v.push(format!("{} already here", self.here()));
        }
        let bad = self.items.iter().filter(|i| matches!(i.act, Act::Bad(_))).count();
        if bad > 0 {
            v.push(format!("{bad} can't be loaded"));
        }
        v.join(" · ")
    }
}

/// The plan for `files` (profiles with their covers, loose covers) going
/// into the profiles folder `to`.
fn plan_for(from: &Path, zip: bool, sets: Vec<Files>, loose: Vec<FileCopy>, skipped: Vec<String>, to: &Path) -> Plan {
    let mut taken = summary::dirs_in(to);
    let mut items = Vec::new();
    for f in sets {
        let act = match NativeSave::parse(&f.profile.bytes) {
            Err(e) => Act::Bad(e.to_string()),
            Ok(_) => {
                let here = crate::frontend::profiles::find_in(to, &f.profile.name);
                if !here.is_file() && !taken.iter().any(|t| t.eq_ignore_ascii_case(&f.profile.name)) {
                    Act::New
                } else if already_here(to, &f) {
                    Act::Same
                } else {
                    let covers: Vec<String> = f.covers.iter().map(|c| c.name.clone()).collect();
                    Act::Copy(store::copy_name(&f.profile.name, &taken, &covers))
                }
            }
        };
        let mut f = f;
        if act == Act::New {
            f.covers.retain(|c| !taken.iter().any(|t| t.eq_ignore_ascii_case(&c.name)));
        }
        if let Act::Copy(n) = &act {
            taken.push(n.clone());
            taken.extend(f.covers.iter().map(|c| format!("{n}{}", &c.name[f.profile.name.len()..])));
        } else if act == Act::New {
            taken.extend(f.names().map(String::from));
        }
        items.push(PlanItem { files: f, act });
    }
    let loose = loose.into_iter().filter(|c| !taken.iter().any(|t| t.eq_ignore_ascii_case(&c.name))).collect();
    Plan { from: from.to_path_buf(), zip, items, loose, skipped }
}

/// The profile is here already: the same bytes under its own name, or
/// an earlier import's copy of it (the same bytes but the name).
fn already_here(to: &Path, f: &Files) -> bool {
    crate::frontend::profiles::list_in(to).iter().any(|d| {
        let Ok(here) = std::fs::read(crate::frontend::profiles::find_in(to, d)) else { return false };
        if d.eq_ignore_ascii_case(&f.profile.name) {
            return here == f.profile.bytes;
        }
        NativeSave::parse(&here).is_ok_and(|s| s.name() == *d) && f.renamed(d).is_ok_and(|r| r.profile.bytes == here)
    })
}

/// Groups save folders (name, bytes, time) into profiles with their covers
/// and loose covers.
fn group(all: Vec<FileCopy>) -> (Vec<Files>, Vec<FileCopy>) {
    let (covers, profiles): (Vec<FileCopy>, Vec<FileCopy>) =
        all.into_iter().partition(|f| summary::cover_parts(&f.name).is_some());
    let mut used = vec![false; covers.len()];
    let mut sets: Vec<Files> = profiles
        .into_iter()
        .map(|p| {
            let mine: Vec<FileCopy> = covers
                .iter()
                .enumerate()
                .filter(|(_, c)| summary::cover_parts(&c.name).is_some_and(|(n, _, _)| n.eq_ignore_ascii_case(&p.name)))
                .map(|(i, c)| {
                    used[i] = true;
                    c.clone()
                })
                .collect();
            Files { profile: p, covers: mine }
        })
        .collect();
    sets.sort_by_key(|f| f.profile.name.to_ascii_uppercase());
    let loose = covers.into_iter().zip(used).filter(|(_, u)| !u).map(|(c, _)| c).collect();
    (sets, loose)
}

/// The plan for importing the saves in folder `from` into `to`. Reads
/// only.
pub fn plan_folder(from: &Path, to: &Path) -> Result<Plan, String> {
    if let (Ok(f), Ok(t)) = (std::fs::canonicalize(from), std::fs::canonicalize(to))
        && (t.starts_with(&f) || f.starts_with(&t))
    {
        return Err("that is Overbright's own save folder".into());
    }
    let mut all = Vec::new();
    for (dir, name) in save_dirs(from) {
        let file = crate::frontend::profiles::find_in(dir.parent().unwrap_or(&dir), &name);
        let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        all.push(FileCopy { name, bytes, mtime: store::mtime(&file) });
    }
    let (sets, loose) = group(all);
    Ok(plan_for(from, false, sets, loose, Vec::new(), to))
}

/// A zip's limits: at most this many entries; a profile is the game's
/// size exactly, a cover too.
pub const ZIP_MAX_ENTRIES: usize = 64;

/// The plan for importing a `.zip` of saves (as [`export::export`] writes
/// them, or by hand): entries `[any folders/]X/X`, each a profile
/// (`native_save::FILE_LEN` bytes) or a saved cover (`PICTURE_FILE_LEN`).
/// Anything else, a path with `..`, an absolute or drive path, or a second
/// entry of the same save is listed and left out.
pub fn plan_zip(from: &Path, to: &Path) -> Result<Plan, String> {
    let z = crate::formats::zip::Zip::open(from).map_err(|e| format!("not a zip file the launcher can read: {e}"))?;
    if z.entries.len() > ZIP_MAX_ENTRIES {
        return Err(format!("it has {} files; a zip of saves has at most {ZIP_MAX_ENTRIES}", z.entries.len()));
    }
    let mut all: Vec<FileCopy> = Vec::new();
    let mut skipped = Vec::new();
    for e in &z.entries {
        let parts: Vec<&str> = e.name.split('/').collect();
        let bad_path = e.name.starts_with('/')
            || parts.iter().any(|p| *p == ".." || p.contains(':') || p.contains('\\'))
            || parts.len() < 2;
        let (dir, file) = if parts.len() >= 2 { (parts[parts.len() - 2], parts[parts.len() - 1]) } else { ("", "") };
        let cover = summary::cover_parts(dir).is_some();
        let size = if cover { ns::PICTURE_FILE_LEN } else { ns::FILE_LEN };
        if bad_path || !dir.eq_ignore_ascii_case(file) || dir.is_empty() {
            skipped.push(format!("{}: not a save", e.name));
            continue;
        }
        if e.size != size {
            skipped.push(format!("{}: {} bytes, a {} has {size}", e.name, e.size, if cover { "saved cover" } else { "profile" }));
            continue;
        }
        if all.iter().any(|f| f.name.eq_ignore_ascii_case(dir)) {
            skipped.push(format!("{}: {dir} is in the zip twice", e.name));
            continue;
        }
        let bytes = z.read(e).map_err(|err| format!("{}: {err}", e.name))?;
        all.push(FileCopy { name: dir.to_string(), bytes, mtime: None });
    }
    let (sets, loose) = group(all);
    Ok(plan_for(from, true, sets, loose, skipped, to))
}

/// The plan for a folder or a `.zip`.
pub fn plan(from: &Path, to: &Path) -> Result<Plan, String> {
    if from.is_file() { plan_zip(from, to) } else { plan_folder(from, to) }
}

/// Does `plan` into the profiles folder `to`; returns the names imported
/// (a copy's new name). Nothing is overwritten: a name that turned up
/// since the plan stops that profile alone.
pub fn import(plan: &Plan, to: &Path) -> (Vec<String>, Vec<String>) {
    let (mut done, mut errors) = (Vec::new(), Vec::new());
    for i in &plan.items {
        let files = match &i.act {
            Act::New => Ok(i.files.clone()),
            Act::Copy(n) => i.files.renamed(n),
            _ => continue,
        };
        match files.and_then(|f| f.write_all(to).map(|()| f.profile.name)) {
            Ok(n) => done.push(n),
            Err(e) => errors.push(format!("{}: not imported: {e}", i.name())),
        }
    }
    for c in &plan.loose {
        let p = to.join(&c.name).join(&c.name);
        if store::write_new(&p, &c.bytes, c.mtime).is_ok() {
            done.push(c.name.clone());
        }
    }
    (done, errors)
}

pub mod export {
    //! A profile as a `.zip`: stored entries `NFS Underground 2/<NAME>/<NAME>`
    //! and its covers beside it, the layout of the original's folder, so it
    //! imports on any computer and unpacks by hand into the original's.

    use std::path::{Path, PathBuf};

    /// Writes `<to>/<NAME>.zip` (or `<NAME> (2).zip`, … when that is taken;
    /// nothing is overwritten). Returns the file.
    pub fn export(profiles: &Path, name: &str, to: &Path) -> Result<PathBuf, String> {
        let files = super::store::Files::read(profiles, name, true)?;
        let folder = crate::frontend::profiles::FOLDER;
        let named: Vec<(String, &[u8])> = std::iter::once(&files.profile)
            .chain(&files.covers)
            .map(|f| (format!("{folder}/{0}/{0}", f.name), f.bytes.as_slice()))
            .collect();
        let entries: Vec<(&str, &[u8], u16)> = named.iter().map(|(n, b)| (n.as_str(), *b, 0u16)).collect();
        let zip = crate::formats::zip::build(&entries);
        let base = &files.profile.name;
        for k in 1u32..1000 {
            let p = if k == 1 { to.join(format!("{base}.zip")) } else { to.join(format!("{base} ({k}).zip")) };
            match super::store::write_new(&p, &zip, None) {
                Ok(()) => return Ok(p),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("{}: {e}", p.display())),
            }
        }
        Err("no free file name".into())
    }
}

#[cfg(test)]
mod tests {
    use super::store::tests::{profile, tree};
    use super::*;

    fn put(root: &Path, name: &str, b: &[u8]) {
        std::fs::create_dir_all(root.join(name)).unwrap();
        std::fs::write(root.join(name).join(name), b).unwrap();
    }

    /// New, already here, a name clash imported as a copy, a broken file
    /// skipped; the source never changes (bytes and times), also when it
    /// can't be written to; the copies byte for byte; a second import
    /// finds everything here.
    #[test]
    fn import_plan_and_copy() {
        let base = std::env::temp_dir().join(format!("ob-import-{}", crate::unique_tag()));
        let (from, to) = (base.join("orig/NFS Underground 2"), base.join("ob/NFS Underground 2"));
        let night = profile("NIGHTOWL", 3000);
        put(&from, "NIGHTOWL", &night);
        put(&from, "NIGHTOWL DVD 1", &ns::picture(&[1, 1, 1, 255], 1, 1));
        put(&from, "tuner", &profile("tuner", 10));
        put(&from, "SAME", &profile("SAME", 5));
        put(&from, "BROKEN", b"not a save");
        put(&from, "LONE Magazine 2", &ns::picture(&[2, 2, 2, 255], 1, 1));
        std::fs::create_dir_all(from.join("junk")).unwrap();
        std::fs::write(from.join("junk/other.txt"), b"x").unwrap();
        put(&to, "TUNER", &profile("TUNER", 99));
        put(&to, "SAME", &profile("SAME", 5));
        let t0 = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
        std::fs::File::options().write(true).open(from.join("NIGHTOWL/NIGHTOWL")).unwrap().set_modified(t0).unwrap();
        let before = tree(&from);
        #[cfg(unix)]
        let ro = |p: &Path, mode: u32| {
            use std::os::unix::fs::PermissionsExt;
            for d in [p.to_path_buf()].into_iter().chain(save_dirs(p).into_iter().map(|x| x.0)) {
                std::fs::set_permissions(&d, std::fs::Permissions::from_mode(mode)).unwrap();
            }
        };
        #[cfg(unix)]
        ro(&from, 0o555);
        let plan = plan_folder(&from, &to).unwrap();
        let lines: Vec<String> = plan.items.iter().map(PlanItem::line).collect();
        assert_eq!(
            lines,
            [
                "BROKEN: can't be loaded by the game: skipped",
                "NIGHTOWL: new and its saved cover",
                "SAME: already here",
                "tuner: a profile of this name is already here: imported as tuner2",
            ]
        );
        assert_eq!(plan.counts(), "2 new · 1 already here · 1 can't be loaded");
        let (done, errors) = import(&plan, &to);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(done, ["NIGHTOWL", "tuner2", "LONE Magazine 2"]);
        #[cfg(unix)]
        ro(&from, 0o755);
        assert_eq!(tree(&from), before, "the source unchanged, times too");
        assert_eq!(std::fs::read(to.join("NIGHTOWL/NIGHTOWL")).unwrap(), night);
        assert_eq!(store::mtime(&to.join("NIGHTOWL/NIGHTOWL")), Some(t0), "the time kept");
        assert!(to.join("NIGHTOWL DVD 1/NIGHTOWL DVD 1").is_file());
        assert_eq!(NativeSave::parse(&std::fs::read(to.join("tuner2/tuner2")).unwrap()).unwrap().name(), "tuner2");
        assert_eq!(NativeSave::parse(&std::fs::read(to.join("TUNER/TUNER")).unwrap()).unwrap().u32(ns::CASH), 99, "kept");
        // Again: nothing new.
        let again = plan_folder(&from, &to).unwrap();
        assert_eq!(again.to_copy(), 0, "{:?}", again.items);
        assert_eq!(again.items.iter().filter(|i| i.act == Act::Same).count(), 3, "tuner2 is tuner's earlier copy");
        // One profile's folder picked: that profile.
        put(&base.join("one"), "KIMMY", &profile("KIMMY", 1));
        let one = plan_folder(&base.join("one/KIMMY"), &to).unwrap();
        assert_eq!(import(&one, &to).0, ["KIMMY"]);
        // Overbright's own folder: refused.
        assert!(plan_folder(&to, &to).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Export, then import the zip elsewhere: the same bytes.
    #[test]
    fn export_round_trip_and_bad_zips() {
        let base = std::env::temp_dir().join(format!("ob-zip-{}", crate::unique_tag()));
        let (a, b, out) = (base.join("a"), base.join("b"), base.join("out"));
        let kimmy = profile("KIMMY", 77);
        let cover = ns::picture(&[3, 3, 3, 255], 1, 1);
        put(&a, "KIMMY", &kimmy);
        put(&a, "KIMMY DVD 1", &cover);
        std::fs::create_dir_all(&out).unwrap();
        let z = export::export(&a, "KIMMY", &out).unwrap();
        assert_eq!(z, out.join("KIMMY.zip"));
        assert_eq!(export::export(&a, "KIMMY", &out).unwrap(), out.join("KIMMY (2).zip"), "never over a file");
        let plan = plan_zip(&z, &b).unwrap();
        assert!(plan.skipped.is_empty());
        assert_eq!(import(&plan, &b).0, ["KIMMY"]);
        assert_eq!(std::fs::read(b.join("KIMMY/KIMMY")).unwrap(), kimmy);
        assert_eq!(std::fs::read(b.join("KIMMY DVD 1/KIMMY DVD 1")).unwrap(), cover);
        // Bad zips: a path out of the archive, a wrong size, a second copy,
        // a stray file; too many entries.
        let bad = crate::formats::zip::build(&[
            ("../KIMMY/KIMMY", &kimmy, 0),
            ("x/RAY/RAY", &kimmy[..100], 0),
            ("KIMMY/KIMMY", &kimmy, 0),
            ("y/kimmy/KIMMY", &kimmy, 0),
            ("readme.txt", b"hi", 0),
        ]);
        std::fs::write(base.join("bad.zip"), bad).unwrap();
        let p = plan_zip(&base.join("bad.zip"), &base.join("c")).unwrap();
        // The path out of the archive is dropped by the zip reader itself.
        assert_eq!(p.items.len(), 1);
        assert_eq!(p.items[0].name(), "KIMMY");
        assert_eq!(p.skipped.len(), 3, "{:?}", p.skipped);
        let many: Vec<(String, Vec<u8>)> = (0..65).map(|i| (format!("N{i}/N{i}"), vec![0u8; 4])).collect();
        let entries: Vec<(&str, &[u8], u16)> = many.iter().map(|(n, b)| (n.as_str(), b.as_slice(), 0)).collect();
        std::fs::write(base.join("many.zip"), crate::formats::zip::build(&entries)).unwrap();
        assert!(plan_zip(&base.join("many.zip"), &base.join("c")).is_err());
        std::fs::write(base.join("not.zip"), b"hello").unwrap();
        assert!(plan_zip(&base.join("not.zip"), &base.join("c")).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}

//! "Import saves" (docs/launcher.md): the original game's profiles copied
//! into Overbright's own profiles folder. A copy, never a move: the
//! original's folder is only read. Its layout is the one Overbright keeps
//! (`<folder>\<NAME>\<NAME>`, 0x4D9AB0), so each save folder is copied as
//! it is: a profile (`NAME/NAME`) and its saved magazine covers and DVD
//! pictures (`NAME DVD 1/NAME DVD 1`, …). A save whose folder name is
//! already in Overbright's folder (in any case, as the original's file
//! names compare) is left alone and listed as kept.

use std::path::{Path, PathBuf};

/// One save folder of the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The folder's name (the profile's or the picture's).
    pub name: String,
    /// A profile (else a saved cover picture).
    pub profile: bool,
    /// The files in the folder, by name.
    pub files: Vec<String>,
    pub bytes: u64,
    /// Overbright already has a save of this name: not copied.
    pub exists: bool,
    /// The source folder.
    pub dir: PathBuf,
}

/// What a source folder holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    pub from: PathBuf,
    pub items: Vec<Item>,
}

impl Scan {
    pub fn to_copy(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|i| !i.exists)
    }

    /// The scan's lines for the screen: one per profile; the pictures
    /// counted in one line.
    pub fn lines(&self) -> Vec<(bool, String)> {
        let mut v = Vec::new();
        for i in self.items.iter().filter(|i| i.profile) {
            if i.exists {
                v.push((false, format!("{}: Overbright already has a profile of this name; kept as it is", i.name)));
            } else {
                v.push((true, format!("{}: will be copied ({})", i.name, kb(i.bytes))));
            }
        }
        let pics = self.items.iter().filter(|i| !i.profile && !i.exists).count();
        if pics > 0 {
            v.push((true, format!("{pics} saved magazine cover / DVD {}: will be copied", plural(pics, "picture"))));
        }
        v
    }
}

fn plural(n: usize, w: &str) -> String {
    if n == 1 { w.to_string() } else { format!("{w}s") }
}

pub fn kb(b: u64) -> String {
    format!("{} KB", b.div_ceil(1024))
}

/// The folder of a save in `dir`: a subfolder holding a file of its own
/// name (any case). `dir` itself when it is such a folder (the player
/// picked one profile's folder).
fn save_dirs(dir: &Path) -> Vec<(PathBuf, String)> {
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
                .filter(|(p, _)| own(p))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|(_, n)| n.to_ascii_uppercase());
    v
}

/// A saved cover's folder name (`NAME DVD 3`, `NAME Magazine 12`,
/// `profiles::cover_name`).
fn is_cover(name: &str) -> bool {
    let mut parts = name.rsplitn(3, ' ');
    let (n, kind) = (parts.next(), parts.next());
    parts.next().is_some()
        && matches!(kind, Some("DVD" | "Magazine"))
        && n.is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// What `from` holds and what of it `to` (Overbright's profiles folder)
/// has already. Reads only.
pub fn scan(from: &Path, to: &Path) -> Scan {
    let have: Vec<String> = std::fs::read_dir(to)
        .map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok()).collect())
        .unwrap_or_default();
    let items = save_dirs(from)
        .into_iter()
        .map(|(dir, name)| {
            let mut files: Vec<(String, u64)> = std::fs::read_dir(&dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .filter_map(|e| {
                            let m = e.metadata().ok().filter(|m| m.is_file())?;
                            Some((e.file_name().into_string().ok()?, m.len()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            files.sort();
            Item {
                profile: !is_cover(&name),
                exists: have.iter().any(|h| h.eq_ignore_ascii_case(&name)),
                bytes: files.iter().map(|f| f.1).sum(),
                files: files.into_iter().map(|f| f.0).collect(),
                name,
                dir,
            }
        })
        .collect();
    Scan { from: from.to_path_buf(), items }
}

/// Copies the saves of `s` that Overbright does not have into `to`; each
/// file is written beside its place and renamed into it. Returns the
/// files copied (`NAME/NAME`, …). The source is only read.
pub fn import(s: &Scan, to: &Path) -> Result<Vec<String>, String> {
    let (from_c, to_c) = (std::fs::canonicalize(&s.from).ok(), std::fs::canonicalize(to).ok());
    if let (Some(f), Some(t)) = (&from_c, &to_c)
        && (t.starts_with(f) || f.starts_with(t))
    {
        return Err("that is Overbright's own save folder".into());
    }
    let mut done = Vec::new();
    for item in s.to_copy() {
        let src = &item.dir;
        let dst = to.join(&item.name);
        if dst.exists() {
            continue;
        }
        std::fs::create_dir_all(&dst).map_err(|e| format!("{}: {e}", dst.display()))?;
        for f in &item.files {
            let part = dst.join(format!("{f}.part"));
            std::fs::copy(src.join(f), &part).map_err(|e| format!("{}: {e}", src.join(f).display()))?;
            std::fs::rename(&part, dst.join(f)).map_err(|e| format!("{}: {e}", dst.join(f).display()))?;
            done.push(format!("{}/{f}", item.name));
        }
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(p: &Path, bytes: usize) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, vec![7u8; bytes]).unwrap();
    }

    /// Two profiles and a cover copied; one already there kept; the
    /// source unchanged; a folder that is not a save left out.
    #[test]
    fn copies_new_saves_and_never_touches_the_source() {
        let base = std::env::temp_dir().join(format!("ob-import-{}", crate::unique_tag()));
        let _ = std::fs::remove_dir_all(&base);
        let (from, to) = (base.join("orig/NFS Underground 2"), base.join("ob/NFS Underground 2"));
        put(&from.join("NIGHTOWL/NIGHTOWL"), 3000);
        put(&from.join("tuner/TUNER"), 10);
        put(&from.join("NIGHTOWL DVD 1/NIGHTOWL DVD 1"), 50);
        put(&from.join("junk/other.txt"), 5);
        put(&to.join("TUNER/TUNER"), 99);
        let before: Vec<_> = walk(&from);
        let s = scan(&from, &to);
        let names: Vec<_> = s.items.iter().map(|i| (i.name.as_str(), i.profile, i.exists)).collect();
        assert_eq!(names, [("NIGHTOWL", true, false), ("NIGHTOWL DVD 1", false, false), ("tuner", true, true)]);
        let lines = s.lines();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].1.contains("NIGHTOWL: will be copied (3 KB)") && lines[1].1.contains("already has"));
        let done = import(&s, &to).unwrap();
        assert_eq!(done, ["NIGHTOWL/NIGHTOWL", "NIGHTOWL DVD 1/NIGHTOWL DVD 1"]);
        assert_eq!(std::fs::read(to.join("NIGHTOWL/NIGHTOWL")).unwrap().len(), 3000);
        assert_eq!(std::fs::read(to.join("TUNER/TUNER")).unwrap().len(), 99, "kept");
        assert_eq!(walk(&from), before, "the source unchanged");
        // Again: nothing left to copy.
        assert!(import(&scan(&from, &to), &to).unwrap().is_empty());
        // One profile's folder picked: that profile.
        put(&base.join("one/KIM/KIM"), 20);
        let s1 = scan(&base.join("one/KIM"), &to);
        assert_eq!(import(&s1, &to).unwrap(), ["KIM/KIM"]);
        // Overbright's own folder as the source: refused.
        assert!(import(&scan(&to, &to), &to).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    fn walk(d: &Path) -> Vec<(PathBuf, u64)> {
        let mut v = Vec::new();
        for e in std::fs::read_dir(d).unwrap().flatten() {
            if e.path().is_dir() {
                v.extend(walk(&e.path()));
            } else {
                v.push((e.path(), e.metadata().unwrap().len()));
            }
        }
        v.sort();
        v
    }

    #[test]
    fn cover_names() {
        assert!(is_cover("RACER9 DVD 1") && is_cover("RACER9 Magazine 12") && is_cover("A B DVD 3"));
        assert!(!is_cover("RACER9") && !is_cover("DVD 1") && !is_cover("RACER9 DVD x"));
    }
}

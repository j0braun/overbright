//! The run files of a mod: `<mod>/.editor/runs/<name>.obrun` (the editor's own folder, so a run is never part of
//! what the mod shares). A simulation that ends is kept here; a run file put in the folder is listed too. The
//! list reads each file's first line (the header) and its last (the end), not the run; rename and delete are
//! plain file work with the names checked. Plain data and files, so it is tested without a window.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::format::{Line, SimMode};

/// A run file's extension.
pub const EXT: &str = "obrun";
/// The longest name (characters).
pub const MAX_NAME: usize = 64;

/// The folder of the runs of the mod `folder` under `maps`.
pub fn dir(maps: &Path, folder: &str) -> PathBuf {
    ob_map::tools::map::store::editor_dir(maps, folder).join("runs")
}

/// One run file in the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Its name: the file's name without `.obrun`.
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
    /// A race's id, or "world" (free roam); `?` when the header does not read.
    pub what: String,
    pub seed: Option<u64>,
    /// How the run ended (`finish`, `limit`, …); none while it is still being written or when it was cut off.
    pub end: Option<String>,
}

/// The runs in `dir`, the newest first (then by name). No folder: none.
pub fn list(dir: &Path) -> Vec<Entry> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<Entry> = rd
        .filter_map(|e| {
            let e = e.ok()?;
            let path = e.path();
            if path.extension()?.to_str()? != EXT || !e.file_type().ok()?.is_file() {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            let meta = e.metadata().ok()?;
            let (head, tail) = ends(&path, meta.len());
            let (what, seed) = match head.as_deref().map(Line::parse) {
                Some(Ok(Some(Line::Header(h)))) => (
                    match h.mode {
                        SimMode::World => "world".to_string(),
                        SimMode::Race => h.race,
                    },
                    Some(h.seed),
                ),
                _ => ("?".to_string(), None),
            };
            let end = tail.and_then(|t| match Line::parse(&t) {
                Ok(Some(Line::End(why))) => Some(why),
                _ => None,
            });
            Some(Entry { name, path, bytes: meta.len(), modified: meta.modified().ok(), what, seed, end })
        })
        .collect();
    v.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    v
}

/// The first and the last line of the file (the last read from its final kilobyte).
fn ends(path: &Path, len: u64) -> (Option<String>, Option<String>) {
    let Ok(mut f) = std::fs::File::open(path) else { return (None, None) };
    let mut buf = vec![0u8; 4096.min(len as usize)];
    let head = f.read(&mut buf).ok().map(|n| {
        let t = String::from_utf8_lossy(&buf[..n]).into_owned();
        t.lines().next().unwrap_or_default().to_string()
    });
    let back = 1024.min(len);
    let mut tail = vec![0u8; back as usize];
    let tail = f
        .seek(SeekFrom::Start(len - back))
        .and_then(|_| f.read_exact(&mut tail))
        .ok()
        .and_then(|_| String::from_utf8_lossy(&tail).lines().rev().find(|l| !l.is_empty()).map(str::to_string));
    (head, tail)
}

/// Whether `name` may name a run: 1 to [`MAX_NAME`] letters, digits, `-`, `_` and `.` inside, not starting with
/// a dot. Err: why not, in words.
pub fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("A run needs a name".into());
    }
    if name.chars().count() > MAX_NAME {
        return Err(format!("A run's name is at most {MAX_NAME} characters"));
    }
    if name.starts_with('.') || !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) {
        return Err("A run's name has letters, digits, - _ and . (not first)".into());
    }
    Ok(())
}

fn file(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.{EXT}"))
}

/// Rename run `from` to `to`; it never replaces another run.
pub fn rename(dir: &Path, from: &str, to: &str) -> Result<(), String> {
    check_name(to)?;
    if from == to {
        return Ok(());
    }
    let (a, b) = (file(dir, from), file(dir, to));
    if !a.is_file() {
        return Err(format!("{from} is not in the list any more"));
    }
    if b.exists() {
        return Err(format!("There is a run called {to} already"));
    }
    std::fs::rename(&a, &b).map_err(|e| format!("{to}: {e}"))
}

/// Delete run `name`.
pub fn delete(dir: &Path, name: &str) -> Result<(), String> {
    check_name(name)?;
    std::fs::remove_file(file(dir, name)).map_err(|e| format!("{name}: {e}"))
}

/// Write `text` as a new run named from `base` (`-2`, `-3` … when the name is taken); the name it got.
pub fn save(dir: &Path, base: &str, text: &str) -> Result<String, String> {
    let base: String =
        base.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '-' }).collect();
    let base: String = base.chars().take(MAX_NAME - 4).collect();
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut name = base.clone();
    let mut n = 1;
    while file(dir, &name).exists() {
        n += 1;
        name = format!("{base}-{n}");
    }
    std::fs::write(file(dir, &name), text).map_err(|e| format!("{name}: {e}"))?;
    Ok(name)
}

/// The path of run `name`.
pub fn path(dir: &Path, name: &str) -> PathBuf {
    file(dir, name)
}

#[cfg(test)]
mod tests {
    use super::super::summary::tests::lines;
    use super::*;

    fn text(seed: u64) -> String {
        lines(seed, true, false).iter().map(|l| l.write() + "\n").collect()
    }

    #[test]
    fn saved_runs_are_listed_renamed_and_deleted() {
        let d = crate::testkit::Scratch::new("runs-library");
        let dir = dir(d.path(), "docks");
        assert!(list(&dir).is_empty(), "no folder: no runs");
        assert_eq!(save(&dir, "sim-r-seed1", &text(1)).unwrap(), "sim-r-seed1");
        assert_eq!(save(&dir, "sim-r-seed1", &text(1)).unwrap(), "sim-r-seed1-2", "a taken name is not replaced");
        assert_eq!(save(&dir, "a b/c", &text(2)).unwrap(), "a-b-c");
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let l = list(&dir);
        assert_eq!(l.len(), 3, "only run files");
        let e = l.iter().find(|e| e.name == "a-b-c").unwrap();
        assert_eq!((e.what.as_str(), e.seed, e.end.as_deref()), ("r", Some(2), Some("limit")));
        assert_eq!(e.bytes, text(2).len() as u64);
        // A run being written has no end yet; one that does not read says so.
        let cut: String = text(3).lines().take(40).map(|l| format!("{l}\n")).collect();
        std::fs::write(path(&dir, "live"), cut).unwrap();
        std::fs::write(path(&dir, "junk"), "hello\n").unwrap();
        let l = list(&dir);
        let live = l.iter().find(|e| e.name == "live").unwrap();
        assert_eq!((live.seed, live.end.clone()), (Some(3), None));
        assert_eq!(l.iter().find(|e| e.name == "junk").unwrap().what, "?");

        rename(&dir, "a-b-c", "mine").unwrap();
        assert!(path(&dir, "mine").is_file() && !path(&dir, "a-b-c").exists());
        assert!(rename(&dir, "mine", "live").unwrap_err().contains("already"));
        assert!(rename(&dir, "mine", "../x").is_err());
        assert!(rename(&dir, "gone", "x").unwrap_err().contains("not in the list"));
        delete(&dir, "mine").unwrap();
        assert!(delete(&dir, "mine").is_err());
        assert!(delete(&dir, "../notes").is_err());
        assert_eq!(list(&dir).len(), 4, "the two saved, live and junk");
    }

    #[test]
    fn names_are_checked() {
        for ok in ["a", "sim-r-seed1", "Run_2.final"] {
            assert!(check_name(ok).is_ok(), "{ok}");
        }
        for bad in ["", ".hidden", "a/b", "a b", "ä", &"x".repeat(65)] {
            assert!(check_name(bad).is_err(), "{bad}");
        }
    }

    /// Runs are the editor's, not the mod's: sharing the mod leaves them out.
    #[test]
    fn a_mod_shared_has_no_runs() {
        let d = crate::testkit::Scratch::new("runs-not-shared");
        let mdir = d.path().join("docks");
        std::fs::create_dir_all(&mdir).unwrap();
        let rdir = dir(d.path(), "docks");
        save(&rdir, "x", &text(1)).unwrap();
        let files = ob_map::tools::map::share::files(&mdir).unwrap();
        assert!(
            files.iter().all(|f| !f.path.contains("runs")),
            "{:?}",
            files.iter().map(|f| &f.path).collect::<Vec<_>>()
        );
    }
}

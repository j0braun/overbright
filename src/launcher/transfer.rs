//! Copying Overbright's files from one place to another (Settings ›
//! Advanced › Storage: portable mode on or off, a folder put elsewhere;
//! docs/playing/platform-dirs.md, "Copying between places").
//!
//! The rules:
//! * a file already in the target always wins: nothing there is
//!   overwritten unless the player explicitly chooses to replace, and
//!   then the file replaced is first kept as `<name>.before-copy`;
//! * an incoming file that differs from the one already there is kept
//!   beside it as `<name>.from-<source>` (`game-root.txt.from-portable`);
//!   the same bytes are not copied again;
//! * settings files (options.txt, game-root.txt, launcher.txt,
//!   locations.txt, …: [`is_settings`]) are never copied over an existing
//!   one, replace or not;
//! * the files that keep a folder path (game-root.txt,
//!   texture-pack-root.txt, locations.txt) are rewritten for the target:
//!   absolute outside a portable install, relative to the program folder
//!   inside one when on the same drive (`platform::dirs::rebase_kept_text`);
//! * the launcher's lock and start marker, temporary files and the caches
//!   are never copied.
//!
//! [`plan`] looks first (nothing is written) so the question can list what
//! will be copied, what is already there and what differs; [`apply`] then
//! copies, checks every file arrived whole and deletes nothing.

use std::path::{Path, PathBuf};

/// One folder to copy into another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    pub from: PathBuf,
    pub to: PathBuf,
    /// The folder's name as the question shows its files ("" for
    /// Overbright's own folder, `NFS Underground 2` for the saves).
    pub label: String,
    /// Top-level entries of `from` left out (the saves copied by their own
    /// job, the caches).
    pub skip: Vec<String>,
}

/// How the kept paths are read and written ([`crate::platform::dirs::rebase_path`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Paths {
    /// The program folder when the source is a portable install's.
    pub from: Option<PathBuf>,
    /// The program folder when the target is a portable install's.
    pub to: Option<PathBuf>,
}

/// A file of the plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub from: PathBuf,
    pub to: PathBuf,
    /// As the question shows it (`options.txt`, `NFS Underground 2/A/A`).
    pub shown: String,
    pub len: u64,
    /// A settings file ([`is_settings`]): never replaced.
    pub settings: bool,
}

/// What a copy would do, looked at before anything is written.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// New in the target: copied.
    pub copy: Vec<Item>,
    /// Already in the target with the same bytes: left alone.
    pub same: Vec<Item>,
    /// Already in the target with other bytes: the target's kept, the
    /// incoming one kept beside it ([`kept_name`]), or with
    /// [`Choice::Replace`] (not settings files) the target's backed up and
    /// replaced.
    pub differ: Vec<Item>,
    /// The suffix's source word (`portable`, `home`, `previous-folder`).
    pub source: String,
    pub paths: Paths,
}

/// What the player chose for the files that differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// The default: the target's files stay, incoming ones go beside them.
    Keep,
    /// The target's file kept as `<name>.before-copy`, then replaced
    /// (settings files still kept as with [`Choice::Keep`]).
    Replace,
}

/// What [`apply`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Done {
    /// Files written into place (new ones, and replaced ones).
    pub copied: usize,
    pub bytes: u64,
    /// Incoming files kept beside a different one (`.from-…`).
    pub beside: Vec<PathBuf>,
    /// Files backed up before a replace (`.before-copy`).
    pub backups: Vec<PathBuf>,
}

/// Overbright's settings files: never copied over an existing one.
pub const SETTINGS_FILES: [&str; 11] = [
    "options.txt",
    "game-root.txt",
    "texture-pack-root.txt",
    "launcher.txt",
    "locations.txt",
    "editor-recent.txt",
    "texture-mods.txt",
    "mods-paused.txt",
    "order.txt",
    "enabled.txt",
    "editor.toml",
];

/// Never copied: the launcher's lock and start marker (they belong to the
/// running launcher), the portable marker.
const NEVER: [&str; 3] = ["launcher.lock", "start-marker.txt", "portable.txt"];

/// Whether the file at `shown` (as the plan shows it) is a settings file:
/// one of [`SETTINGS_FILES`] anywhere, or any `.txt` / `.toml` file at the
/// top of Overbright's folder.
pub fn is_settings(shown: &str) -> bool {
    let name = shown.rsplit('/').next().unwrap_or(shown);
    SETTINGS_FILES.contains(&name) || (!shown.contains('/') && (name.ends_with(".txt") || name.ends_with(".toml")))
}

/// The name an incoming file is kept under beside a different one:
/// `<name>.from-<source>`, then `-2`, `-3`, … when that is taken by other
/// bytes; `true` when the name holds these bytes already.
fn kept_name(dst: &Path, source: &str, bytes: &[u8]) -> (PathBuf, bool) {
    let name = dst.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    for k in 1.. {
        let n = if k == 1 { format!("{name}.from-{source}") } else { format!("{name}.from-{source}-{k}") };
        let p = dst.with_file_name(n);
        match std::fs::read(&p) {
            Ok(b) if b == bytes => return (p, true),
            Ok(_) => continue,
            Err(_) if p.exists() => continue,
            Err(_) => return (p, false),
        }
    }
    unreachable!()
}

/// The backup name of a file about to be replaced: `<name>.before-copy`,
/// then `-2`, … (never over an earlier backup).
fn backup_name(dst: &Path) -> PathBuf {
    let name = dst.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    (1..)
        .map(|k| {
            dst.with_file_name(if k == 1 { format!("{name}.before-copy") } else { format!("{name}.before-copy-{k}") })
        })
        .find(|p| !p.exists())
        .unwrap_or_else(|| dst.with_file_name(format!("{name}.before-copy")))
}

/// The bytes `src` arrives with: a file that keeps paths rewritten for
/// the target ([`Paths`]), any other as it is.
fn incoming(src: &Path, shown: &str, paths: &Paths) -> std::io::Result<Vec<u8>> {
    let bytes = std::fs::read(src)?;
    if !keeps_paths(shown) {
        return Ok(bytes);
    }
    let Ok(text) = std::str::from_utf8(&bytes) else { return Ok(bytes) };
    Ok(crate::platform::dirs::rebase_kept_text(shown, text, paths.from.as_deref(), paths.to.as_deref())
        .map_or(bytes, String::into_bytes))
}

/// Whether the file shown as `shown` keeps folder paths: Overbright's own
/// top-level game-root.txt, texture-pack-root.txt and locations.txt.
fn keeps_paths(shown: &str) -> bool {
    use crate::platform::dirs::{LOCATIONS_FILE, PATH_FILES};
    !shown.contains('/') && (PATH_FILES.contains(&shown) || shown == LOCATIONS_FILE)
}

/// Whether the file at `dst` holds exactly `bytes` (sizes first).
fn holds(dst: &Path, len: u64, bytes: impl FnOnce() -> std::io::Result<Vec<u8>>) -> std::io::Result<bool> {
    let have = std::fs::metadata(dst)?.len();
    if have != len {
        return Ok(false);
    }
    Ok(std::fs::read(dst)? == bytes()?)
}

/// What copying `jobs` would do; nothing is written. `source` names the
/// files kept beside different ones (`.from-<source>`).
pub fn plan(jobs: &[Job], source: &str, paths: Paths) -> Plan {
    fn walk(from: &Path, rel: &str, job: &Job, out: &mut Vec<(PathBuf, PathBuf, String, u64)>) {
        let Ok(rd) = std::fs::read_dir(from) else { return };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let sub = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if rel.is_empty() && job.skip.iter().any(|s| *s == name) {
                continue;
            }
            if NEVER.contains(&name.as_str()) || name.ends_with(".tmp") {
                continue;
            }
            let Ok(t) = e.file_type() else { continue };
            let src = e.path();
            if t.is_dir() {
                // Never into itself (a new place inside the old one).
                if job.to.starts_with(&src) {
                    continue;
                }
                walk(&src, &sub, job, out);
            } else if t.is_file() {
                let dst = job.to.join(&sub);
                let len = e.metadata().map(|m| m.len()).unwrap_or(0);
                out.push((src, dst, sub, len));
            }
        }
    }
    let mut p = Plan { source: source.to_string(), paths, ..Plan::default() };
    for job in jobs {
        if !job.from.is_dir() || job.from == job.to {
            continue;
        }
        let mut files = Vec::new();
        walk(&job.from, "", job, &mut files);
        for (src, dst, sub, len) in files {
            let shown = if job.label.is_empty() { sub.clone() } else { format!("{}/{sub}", job.label) };
            // Paths rewritten: the length the target would get.
            let rewritten = if keeps_paths(&shown) { incoming(&src, &shown, &p.paths).ok() } else { None };
            let len = rewritten.as_ref().map_or(len, |b| b.len() as u64);
            let item = Item {
                from: src.clone(),
                to: dst.clone(),
                settings: job.label.is_empty() && is_settings(&sub),
                shown,
                len,
            };
            if !dst.exists() {
                p.copy.push(item);
                continue;
            }
            let same = holds(&dst, len, || match &rewritten {
                Some(b) => Ok(b.clone()),
                None => std::fs::read(&src),
            })
            .unwrap_or(false);
            if same { p.same.push(item) } else { p.differ.push(item) }
        }
    }
    p
}

/// The files written as `bytes` at `dst` (through a temporary file, then
/// renamed) and checked.
fn write_whole(dst: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(d) = dst.parent() {
        std::fs::create_dir_all(d)?;
    }
    let name = dst.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dst.with_file_name(format!("{name}.copy.tmp"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, dst)?;
    match std::fs::metadata(dst) {
        Ok(m) if m.len() == bytes.len() as u64 => Ok(()),
        _ => Err(std::io::Error::other(format!("{} did not arrive whole", dst.display()))),
    }
}

/// Copies what `plan` lists. Every file is looked at again first: one
/// that appeared in the target since is treated as differing (it wins).
/// Nothing is deleted; nothing in the target is overwritten unless
/// `choice` is [`Choice::Replace`], and then not settings files, and only
/// after a backup.
pub fn apply(plan: &Plan, choice: Choice) -> Result<Done, String> {
    let mut done = Done::default();
    let err = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    for it in plan.copy.iter().chain(&plan.differ) {
        let bytes = incoming(&it.from, &it.shown, &plan.paths).map_err(|e| err(&it.from, e))?;
        if !it.to.exists() {
            write_whole(&it.to, &bytes).map_err(|e| err(&it.to, e))?;
            done.copied += 1;
            done.bytes += bytes.len() as u64;
            continue;
        }
        if std::fs::read(&it.to).is_ok_and(|b| b == bytes) {
            continue;
        }
        if choice == Choice::Replace && !it.settings {
            let backup = backup_name(&it.to);
            std::fs::copy(&it.to, &backup).map_err(|e| err(&backup, e))?;
            write_whole(&it.to, &bytes).map_err(|e| err(&it.to, e))?;
            done.backups.push(backup);
            done.copied += 1;
            done.bytes += bytes.len() as u64;
        } else {
            let (beside, there) = kept_name(&it.to, &plan.source, &bytes);
            if !there {
                write_whole(&beside, &bytes).map_err(|e| err(&beside, e))?;
            }
            done.beside.push(beside);
        }
    }
    Ok(done)
}

/// The copy of portable mode turned on (`on`: Overbright's folder and the
/// saves into the program folder's `data`) or off (back: `data` into
/// Overbright's folder, its saves into the data home). `saves`: the
/// profiles' folder in use now. The caches are not copied (they rebuild).
/// None when the program folder is not known.
pub fn portable_plan(p: &super::model::PortableInfo, saves: &Path, on: bool) -> Option<Plan> {
    let (data, program) = (p.data.clone()?, p.program.clone()?);
    let folder = crate::frontend::profiles::FOLDER;
    let backups = format!("{folder} Backups");
    let skip: Vec<String> = [folder, backups.as_str(), "cache", "cache-dev"].map(str::to_string).to_vec();
    let job = |from: PathBuf, to: PathBuf, label: &str, skip: Vec<String>| Job { from, to, label: label.into(), skip };
    Some(if on {
        let saves_home = saves.parent().map_or_else(|| p.home_saves.clone(), Path::to_path_buf);
        plan(
            &[
                job(p.home.clone(), data.clone(), "", skip),
                job(saves.to_path_buf(), data.join(folder), folder, Vec::new()),
                job(saves_home.join(&backups), data.join(&backups), &backups, Vec::new()),
            ],
            "home",
            Paths { from: None, to: Some(program) },
        )
    } else {
        plan(
            &[
                job(data.clone(), p.home.clone(), "", skip),
                job(data.join(folder), p.home_saves.join(folder), folder, Vec::new()),
                job(data.join(&backups), p.home_saves.join(&backups), &backups, Vec::new()),
            ],
            "portable",
            Paths { from: Some(program), to: None },
        )
    })
}

/// The copy of the folder `loc` put elsewhere (Storage, Change…): `from`
/// into `to`, kept paths read and written for `portable` (the program
/// folder of a portable install in force, else None). The saves' folder
/// is the profiles' parent (a shared data folder): only the profiles and
/// their backups are copied from it.
pub fn location_plan(loc: crate::platform::dirs::Location, from: &Path, to: &Path, portable: Option<&Path>) -> Plan {
    let paths = Paths { from: portable.map(Path::to_path_buf), to: portable.map(Path::to_path_buf) };
    let jobs: Vec<Job> = if loc == crate::platform::dirs::Location::Saves {
        let folder = crate::frontend::profiles::FOLDER;
        [folder.to_string(), format!("{folder} Backups")]
            .into_iter()
            .map(|n| Job { from: from.join(&n), to: to.join(&n), label: n, skip: Vec::new() })
            .collect()
    } else {
        vec![Job { from: from.into(), to: to.into(), label: String::new(), skip: Vec::new() }]
    };
    plan(&jobs, "old-folder", paths)
}

impl Plan {
    /// Bytes of the files to copy.
    pub fn copy_bytes(&self) -> u64 {
        self.copy.iter().map(|i| i.len).sum()
    }

    /// Whether a replace would change anything a keep does not (a
    /// differing file that is not a settings file).
    pub fn replaceable(&self) -> bool {
        self.differ.iter().any(|i| !i.settings)
    }

    /// The files of `items` as a short list: top-level files by name,
    /// folders with their count (`maps (3 files)`), at most `max` entries
    /// then "and N more".
    pub fn short_list(items: &[Item], max: usize) -> String {
        let mut groups: Vec<(String, usize)> = Vec::new();
        for i in items {
            let (head, deeper) = match i.shown.split_once('/') {
                Some((h, _)) => (h.to_string(), true),
                None => (i.shown.clone(), false),
            };
            match groups.iter_mut().find(|(g, _)| *g == head) {
                Some(g) if deeper => g.1 += 1,
                _ => groups.push((head, if deeper { 1 } else { 0 })),
            }
        }
        let mut parts: Vec<String> = groups
            .iter()
            .take(max)
            .map(|(g, n)| match n {
                0 => g.clone(),
                1 => format!("{g} (1 file)"),
                n => format!("{g} ({n} files)"),
            })
            .collect();
        if groups.len() > max {
            parts.push(format!("and {} more", groups.len() - max));
        }
        parts.join(", ")
    }

    /// The question's lines: what is copied, what is already there, what
    /// differs and what happens to it.
    pub fn words(&self) -> String {
        let mut s = String::new();
        if self.copy.is_empty() {
            s += "Nothing new to copy.";
        } else {
            s += &format!(
                "To copy: {} ({}): {}.",
                super::model::count(self.copy.len(), "file", "files"),
                mb(self.copy_bytes()),
                Plan::short_list(&self.copy, 6)
            );
        }
        if !self.same.is_empty() {
            s += &format!("\nAlready there and the same: {}.", super::model::count(self.same.len(), "file", "files"));
        }
        if !self.differ.is_empty() {
            s += &format!(
                "\nAlready there and different, kept as they are: {}. The incoming copies are saved beside them ({}).",
                Plan::short_list(&self.differ, 6),
                format_args!("name.from-{}", self.source)
            );
        }
        s
    }

    /// The whole plan for the log, one line per file.
    pub fn log_lines(&self) -> Vec<String> {
        let mut v = Vec::new();
        for (what, items) in [("copy", &self.copy), ("same", &self.same), ("differ", &self.differ)] {
            for i in items {
                v.push(format!("{what} {} -> {}", i.from.display(), i.to.display()));
            }
        }
        v
    }
}

fn mb(bytes: u64) -> String {
    if bytes < 1024 * 1024 {
        format!("{} KB", bytes.div_ceil(1024))
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ob-transfer-{tag}-{}", crate::unique_tag()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(p: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    fn job(from: &Path, to: &Path) -> Job {
        Job { from: from.into(), to: to.into(), label: String::new(), skip: Vec::new() }
    }

    /// Existing files win: a different file in the target is never
    /// overwritten (the incoming one goes beside it), the same bytes are
    /// left alone, new files arrive whole; the lock and marker stay behind.
    #[test]
    fn existing_files_win_and_differing_ones_go_beside() {
        let d = tmp("keep");
        let (from, to) = (d.join("from"), d.join("to"));
        put(&from.join("options.txt"), b"incoming options, longer");
        put(&from.join("save.bin"), b"xyz");
        put(&from.join("same.bin"), b"same");
        put(&from.join("maps/a/map.obmap"), b"map");
        put(&from.join("launcher.lock"), b"");
        put(&from.join("start-marker.txt"), b"1");
        put(&to.join("options.txt"), b"mine");
        put(&to.join("save.bin"), b"abc");
        put(&to.join("same.bin"), b"same");
        let p = plan(&[job(&from, &to)], "portable", Paths::default());
        let shown = |v: &[Item]| v.iter().map(|i| i.shown.clone()).collect::<Vec<_>>();
        assert_eq!(shown(&p.copy), ["maps/a/map.obmap"]);
        assert_eq!(shown(&p.same), ["same.bin"]);
        assert_eq!(shown(&p.differ), ["options.txt", "save.bin"]);
        assert!(p.words().contains("kept as they are: options.txt, save.bin"), "{}", p.words());
        let done = apply(&p, Choice::Keep).unwrap();
        assert_eq!(done.copied, 1);
        assert_eq!(std::fs::read(to.join("options.txt")).unwrap(), b"mine");
        assert_eq!(std::fs::read(to.join("save.bin")).unwrap(), b"abc");
        assert_eq!(std::fs::read(to.join("options.txt.from-portable")).unwrap(), b"incoming options, longer");
        assert_eq!(std::fs::read(to.join("save.bin.from-portable")).unwrap(), b"xyz");
        assert_eq!(std::fs::read(to.join("maps/a/map.obmap")).unwrap(), b"map");
        assert!(!to.join("launcher.lock").exists() && !to.join("start-marker.txt").exists());
        // Again: nothing new, the kept copies not doubled.
        let again = apply(&plan(&[job(&from, &to)], "portable", Paths::default()), Choice::Keep).unwrap();
        assert_eq!(again.copied, 0);
        assert!(!to.join("save.bin.from-portable-2").exists());
        // Replace: the target's file backed up first; settings still kept.
        let done = apply(&plan(&[job(&from, &to)], "portable", Paths::default()), Choice::Replace).unwrap();
        assert_eq!(std::fs::read(to.join("save.bin")).unwrap(), b"xyz");
        assert_eq!(std::fs::read(to.join("save.bin.before-copy")).unwrap(), b"abc");
        assert_eq!(done.backups, [to.join("save.bin.before-copy")]);
        assert_eq!(std::fs::read(to.join("options.txt")).unwrap(), b"mine", "settings never replaced");
        assert!(from.join("save.bin").is_file(), "nothing deleted");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A file that appears in the target after the plan was made wins too.
    #[test]
    fn a_file_that_appeared_since_the_plan_wins() {
        let d = tmp("late");
        let (from, to) = (d.join("from"), d.join("to"));
        put(&from.join("a.bin"), b"incoming");
        let p = plan(&[job(&from, &to)], "home", Paths::default());
        assert_eq!(p.copy.len(), 1);
        put(&to.join("a.bin"), b"late");
        let done = apply(&p, Choice::Replace).unwrap();
        assert_eq!(std::fs::read(to.join("a.bin")).unwrap(), b"incoming");
        assert_eq!(std::fs::read(to.join("a.bin.before-copy")).unwrap(), b"late", "backed up, never lost");
        assert_eq!(done.copied, 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Kept paths: relative to a portable program folder become absolute
    /// in an ordinary folder, and absolute ones become relative going into
    /// a portable install on the same drive; the compared bytes are the
    /// rewritten ones.
    #[cfg(unix)]
    #[test]
    fn kept_paths_rewritten_both_ways() {
        let d = tmp("paths");
        let prog = d.join("Games/overbright");
        let game = d.join("Games/Need for Speed Underground 2");
        let hud = d.join("Mods/hud");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::create_dir_all(&hud).unwrap();
        let data = prog.join("data");
        let home = d.join("home/.local/share/overbright");
        put(&data.join("game-root.txt"), b"../Need for Speed Underground 2\n");
        put(&data.join("texture-pack-root.txt"), b"../../Mods/hud\n");
        put(&data.join("locations.txt"), b"# x\nscreenshots ../../Pictures\n");
        let off = Paths { from: Some(prog.clone()), to: None };
        let p = plan(&[job(&data, &home)], "portable", off);
        apply(&p, Choice::Keep).unwrap();
        assert_eq!(std::fs::read_to_string(home.join("game-root.txt")).unwrap(), format!("{}\n", game.display()));
        assert_eq!(
            std::fs::read_to_string(home.join("texture-pack-root.txt")).unwrap(),
            format!("{}\n", hud.display())
        );
        assert_eq!(
            std::fs::read_to_string(home.join("locations.txt")).unwrap(),
            format!("# x\nscreenshots {}\n", d.join("Pictures").display())
        );
        // Back into a fresh portable folder: relative again.
        let data2 = d.join("Games/ob2/data");
        let on = Paths { from: None, to: Some(d.join("Games/ob2")) };
        apply(&plan(&[job(&home, &data2)], "home", on.clone()), Choice::Keep).unwrap();
        assert_eq!(std::fs::read_to_string(data2.join("game-root.txt")).unwrap(), "../Need for Speed Underground 2\n");
        assert_eq!(std::fs::read_to_string(data2.join("texture-pack-root.txt")).unwrap(), "../../Mods/hud\n");
        // The same once rewritten: not a difference.
        let again = plan(&[job(&home, &data2)], "home", on);
        assert!(again.differ.is_empty() && again.copy.is_empty(), "{again:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn settings_files_and_short_lists() {
        for s in ["options.txt", "game-root.txt", "launcher.txt", "locations.txt", "foo.txt", "maps/enabled.txt"] {
            assert!(is_settings(s), "{s}");
        }
        for s in ["NFS Underground 2/A/A", "logs/game-1.log", "maps/a/map.obmap", "logs/x.txt"] {
            assert!(!is_settings(s), "{s}");
        }
        let it = |s: &str| Item { from: s.into(), to: s.into(), shown: s.into(), len: 1, settings: false };
        let items: Vec<Item> = ["a.txt", "maps/x/1", "maps/y/2", "logs/l"].into_iter().map(it).collect();
        assert_eq!(Plan::short_list(&items, 6), "a.txt, maps (2 files), logs (1 file)");
        assert_eq!(Plan::short_list(&items, 1), "a.txt, and 2 more");
    }
}

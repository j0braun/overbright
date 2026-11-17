use std::path::{Path, PathBuf};

/// The files the clone cannot start without, with the sizes of the
/// European 1.2 release (SPEED2.EXE without copy protection, 4,788,224
/// bytes) and, for the
/// smaller ones, their XXH3-64. A different size or hash is reported as
/// "different version or modded", not an error; a missing file is.
/// SPEED2.EXE (L1, docs/launcher.md): the clone never runs it, but an
/// install without it is not a whole game folder (and the fingerprint and
/// the launcher's version line read it); its name is matched without
/// regard to case ([`key_file_path`]).
pub const KEY_FILES: [(&str, u64, Option<u64>); 10] = [
    ("SPEED2.EXE", 4_788_224, Some(0x153f_cccc_6ea8_2f90)),
    ("GLOBAL/GLOBALB.BUN", 8_008_064, Some(0x18a8_155e_0fa1_6b74)),
    ("GLOBAL/GlobalB.lzc", 5_145_778, Some(0x8759_a0f3_dfa2_069c)),
    ("GLOBAL/InGameCommon.bun", 1_595_640, Some(0xa513_2edd_efdc_655a)),
    ("FRONTEND/FrontB.lzc", 1_750_771, Some(0x3453_cbd2_852d_b4db)),
    ("LANGUAGES/English.bin", 270_456, Some(0x9435_6359_3fcc_01c4)),
    ("CARS/TEXTURES.BIN", 1_895_360, Some(0xae6f_eccb_df68_894f)),
    ("CARS/WHEELS/TEXTURES.BIN", 3_791_808, Some(0x2a51_52ce_de88_0e87)),
    ("TRACKS/L4RA.BUN", 422_528, Some(0x3be3_701a_0350_0ff9)),
    ("TRACKS/STREAML4RA.BUN", 118_489_088, None),
];

/// What [`check`] found.
#[derive(Debug, Default)]
pub struct Check {
    pub missing: Vec<String>,
    /// Files whose size or hash differs from the known release.
    pub different: Vec<String>,
}

impl Check {
    pub fn ok(&self) -> bool {
        self.missing.is_empty()
    }
}

/// Check `root` for [`KEY_FILES`] (only reads): each file there and
/// of the known size, and with `hash` of the known XXH3 (about 22 MB
/// read).
pub fn check(root: &Path, hash: bool) -> Check {
    let mut c = Check::default();
    for (file, size, known) in KEY_FILES {
        let path = key_file_path(root, file);
        match std::fs::metadata(&path) {
            Ok(m) if m.is_file() => {
                let same_hash = !hash
                    || known.is_none_or(|h| std::fs::read(&path).is_ok_and(|b| xxhash_rust::xxh3::xxh3_64(&b) == h));
                if m.len() != size || !same_hash {
                    c.different.push(file.to_string());
                }
            }
            _ => c.missing.push(file.to_string()),
        }
    }
    c
}

/// Where a [`KEY_FILES`] entry is under `root`: every game file is found
/// without regard to case ([`crate::game_fs`]), SPEED2.EXE included.
pub fn key_file_path(root: &Path, file: &str) -> PathBuf {
    crate::game_fs::path(root, file)
}

/// Whether `root` looks like the game folder (its main data files are
/// there).
pub fn looks_like_game(root: &Path) -> bool {
    crate::game_fs::path(root, "GLOBAL/GLOBALB.BUN").is_file() && crate::game_fs::path(root, "FRONTEND/FrontB.lzc").is_file()
}

/// The file a folder chosen by `--set-root` or the prompt is kept in
/// (`platform::dirs::config()/game-root.txt`; not options.txt).
pub fn saved_root_file() -> PathBuf {
    crate::platform::dirs::config().join("game-root.txt")
}

pub fn saved_root() -> Option<PathBuf> {
    let text = std::fs::read_to_string(saved_root_file()).ok()?;
    let p = PathBuf::from(text.trim());
    (!text.trim().is_empty()).then_some(p)
}

pub fn save_root(root: &Path) -> std::io::Result<()> {
    super::saves::write_atomic(&saved_root_file(), format!("{}\n", root.display()).as_bytes(), false)
}

/// L1 (docs/launcher.md): the file the texture-pack folder chosen in the
/// launcher is kept in (`texture-pack-root.txt` beside game-root.txt).
/// Read by the HD UI pack's lookup after `NFSU2_TEXTURE_PACK`
/// (`render::enh::assets::hd_ui`).
pub fn saved_texture_pack_file() -> PathBuf {
    saved_root_file().with_file_name("texture-pack-root.txt")
}

/// The kept texture-pack folder, if one was chosen.
pub fn saved_texture_pack() -> Option<PathBuf> {
    let text = std::fs::read_to_string(saved_texture_pack_file()).ok()?;
    let p = PathBuf::from(text.trim());
    (!text.trim().is_empty()).then_some(p)
}

/// Keeps `dir` as the texture-pack folder (the same atomic write as
/// [`save_root`]); `None` forgets it (the file removed).
pub fn save_texture_pack(dir: Option<&Path>) -> std::io::Result<()> {
    let file = saved_texture_pack_file();
    match dir {
        Some(d) => super::saves::write_atomic(&file, format!("{}\n", d.display()).as_bytes(), false),
        None => match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    }
}

/// The language files of `root`'s `LANGUAGES` folder, by name (`English`,
/// `French`, …; any case, sorted), without the shared tables that are not
/// a language (`Labels`, `Largest`, `LanguageTextures`). The clone loads
/// `English.bin` only for now (`frontend::assets`).
pub fn installed_languages(root: &Path) -> Vec<String> {
    let Some(dir) = crate::formats::viv::find_ci(root, "LANGUAGES") else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let (stem, ext) = name.rsplit_once('.')?;
            (ext.eq_ignore_ascii_case("bin")
                && !["labels", "largest", "languagetextures"].contains(&stem.to_ascii_lowercase().as_str()))
            .then(|| language_name(stem))
        })
        .collect();
    v.sort_by_key(|s| s.to_ascii_lowercase());
    v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    v
}

/// A language file's name as players read it, whatever the folder's case
/// (`gERMAN`, `GERMAN` → `German`).
fn language_name(stem: &str) -> String {
    let lower = stem.to_lowercase();
    let mut c = lower.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// How a line of [`check_lines`] reads: fine, a warning (the game may
/// still run), a note, or an error (the folder cannot be used).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Ok,
    Warn,
    Info,
    Error,
}

/// L1 (docs/launcher.md): the game-folder check in plain words, one line
/// per finding, shared by `game --check-game` and the launcher's check
/// screen so both say the same. `fp` (the SHA-256 fingerprint) adds the
/// version line and the mod files; without it the version is judged by
/// [`KEY_FILES`]' sizes and hashes alone.
pub fn check_lines(c: &Check, fp: Option<&crate::fingerprint::Fingerprint>) -> Vec<(Mark, String)> {
    use crate::fingerprint::{State, Verdict};
    let mut v = Vec::new();
    // SPEED2.EXE judged by its tables (the note or the error below): not
    // also listed as "differs" with "may still run".
    let mut exe_judged = false;
    match fp.map(|f| (f, f.verdict())) {
        Some((_, Verdict::Known(e))) => v.push((Mark::Ok, format!("Version: {e} — recognised"))),
        Some((f, Verdict::Unknown(files))) => {
            let exe = f.files.iter().find(|p| p.file == "SPEED2.EXE");
            let line = match exe.map(|p| (&p.state, p.edition)) {
                Some((State::Read { size, sha256 }, None)) => match crate::fingerprint::other_exe("SPEED2.EXE", *size, sha256) {
                    Some(n) => format!("Version: {n} — not the version Overbright was made with"),
                    None => format!("Version: SPEED2.EXE ({size} bytes) is not a version Overbright knows"),
                },
                // SPEED2.EXE missing or unreadable: the missing lines say it.
                _ if !c.missing.is_empty() => String::new(),
                _ => format!("Version not recognised ({} {} differ from the known release)", files.len(), if files.len() == 1 { "file" } else { "files" }),
            };
            // Overbright reads its tables from SPEED2.EXE ([`crate::exe_tables`]):
            // another executable is fine when every table is the supported
            // build's (a patched copy), else the game does not start.
            let exe_unknown = matches!(exe.map(|p| (&p.state, p.edition)), Some((State::Read { .. }, None)));
            if exe_unknown {
                exe_judged = true;
                match crate::exe_tables::check_folder(&f.root) {
                    Ok(_) => v.push((Mark::Info, exe_modified_line().to_string())),
                    Err(crate::exe_tables::Error::TablesDiffer { version, .. }) => {
                        // A line of its own: capitalised ("This SPEED2.EXE is …").
                        let line = exe_differs_line(version).replacen("this ", "This ", 1);
                        v.push((Mark::Error, line + "; Overbright does not start with it"))
                    }
                    Err(crate::exe_tables::Error::Protected { .. }) => v.push((Mark::Error, exe_protected_line())),
                    Err(e) => v.push((Mark::Error, e.to_string().lines().next().unwrap_or_default().to_string())),
                }
            } else if !line.is_empty() {
                v.push((Mark::Warn, line + "; the game may look or play differently"));
            }
        }
        None if c.ok() && c.different.is_empty() => v.push((Mark::Ok, "Version: the known release (sizes and checksums match)".into())),
        None => {}
    }
    let n = KEY_FILES.len();
    if c.missing.is_empty() {
        v.push((Mark::Ok, format!("All {n} files Overbright needs are present")));
    }
    let different: Vec<&String> = c.different.iter().filter(|f| !(exe_judged && f.as_str() == KEY_FILES[0].0)).collect();
    if !different.is_empty() {
        let k = different.len();
        v.push((
            Mark::Warn,
            format!(
                "{k} {} from the known release (modded or another edition): {} — the game may still run",
                if k == 1 { "file differs" } else { "files differ" },
                different.iter().map(|f| f.as_str()).collect::<Vec<_>>().join(", ")
            ),
        ));
    }
    if let Some(f) = fp
        && !f.mods.is_empty()
    {
        v.push((Mark::Info, format!("Mod files found next to the game: {} (Overbright ignores them)", f.mods.join(", "))));
    }
    if c.missing.len() > 3 {
        // Most of the game is not there: one line, not a list.
        v.push((
            Mark::Error,
            format!(
                "{} of the {n} files Overbright needs are missing ({}, …): this is not a complete game folder",
                c.missing.len(),
                c.missing[..2].join(", ")
            ),
        ));
    } else {
        for m in &c.missing {
            v.push((Mark::Error, format!("Missing: {m} — reinstall or copy the full game folder")));
        }
    }
    v
}

/// What is wrong with the game folder's SPEED2.EXE ([`exe_help`]).
pub enum ExeProblem<'a> {
    /// Not there or not readable.
    Missing,
    /// Not the supported build and its data tables differ from it:
    /// its size, the version if known, the tables that differ.
    Differs { size: u64, version: Option<&'a str>, tables: &'a [String] },
    /// The copy-protected retail executable (SafeDisc): its size.
    Protected { size: u64 },
}

/// Where the work on reading the copy-protected retail executable is
/// followed.
pub const PROTECTED_EXE_ISSUE: &str = "https://github.com/j0braun/overbright/issues/16";

/// The check screens' line for the copy-protected retail SPEED2.EXE.
pub fn exe_protected_line() -> String {
    format!(
        "This SPEED2.EXE is the copy-protected retail executable (SafeDisc): Overbright cannot read its game data yet. \
         Support for it is being worked on: {PROTECTED_EXE_ISSUE}"
    )
}

/// The check screens' and the log's note for a SPEED2.EXE that is not the
/// supported build byte for byte but whose every data table is
/// ([`crate::exe_tables::Edition::Modified`]: a widescreen patch, for one).
pub fn exe_modified_line() -> &'static str {
    "SPEED2.EXE is a modified copy of the supported version; its game data matches"
}

/// The check screens' line for a SPEED2.EXE whose data tables differ from
/// the supported build's: its version when the fingerprint knows it.
pub fn exe_differs_line(version: Option<&str>) -> String {
    format!(
        "this SPEED2.EXE is {}: its data tables differ from the supported 1.2 release",
        version.unwrap_or("an unknown version")
    )
}

/// The message when the game folder's SPEED2.EXE cannot be used
/// ([`crate::exe_tables`]): missing, or its data tables differ from the
/// supported build's, or the copy-protected retail executable. Overbright
/// reads numbers and names from SPEED2.EXE at start, so it starts only when
/// they are the supported build's (a patched copy with the same data is
/// fine); the same voice and folder advice as [`help`]. The protected
/// executable's message says what it is and where the work on it is
/// followed, without folder advice.
pub fn exe_help(exe: &Path, problem: &ExeProblem) -> String {
    let mut s = match problem {
        ExeProblem::Differs { size, version, tables } => {
            let list = if tables.len() > 4 {
                format!("{}, … ({} in all)", tables[..4].join(", "), tables.len())
            } else {
                tables.join(", ")
            };
            format!(
                "{} ({size} bytes): {}.\nDiffering: {list}.\nOverbright reads its tables from the European 1.2 release's SPEED2.EXE without copy protection ({} bytes; a copy of it whose tables are unchanged also works) and does not start with other data: it would drive and sound wrong.\n",
                exe.display(),
                exe_differs_line(*version),
                KEY_FILES[0].1
            )
        }
        ExeProblem::Missing => format!("{} is missing.\n", exe.display()),
        ExeProblem::Protected { size } => {
            return format!(
                "{} ({size} bytes) is the copy-protected retail SPEED2.EXE (SafeDisc). Its code and game data are encrypted, \
                 and Overbright cannot read them yet: support for this executable is being worked on ({PROTECTED_EXE_ISSUE}).\n\
                 Overbright does not remove copy protection and does not change the game's files.\n",
                exe.display()
            );
        }
    };
    s += "Choose an install of Need for Speed Underground 2 (PC), version 1.2.\n";
    let root = exe.parent();
    let tail = help(root, None);
    // The folder advice of `help` without its first line.
    s += tail.split_once('\n').map_or("", |(_, rest)| rest);
    s
}

/// The usual places of an install. Windows: [`windows_candidates`] with
/// the registry's `Install Dir` and the exe's folder. Elsewhere: Steam's
/// and the common Wine, Lutris, Bottles, Heroic and CrossOver prefixes'
/// `drive_c/Program Files*/EA GAMES/Need for Speed Underground 2`, in
/// that order.
pub fn candidates() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let exe = std::env::current_exe().ok();
        windows_candidates(
            &|name| std::env::var_os(name),
            crate::platform::win::registry_install_dirs(),
            exe.as_deref().and_then(Path::parent),
        )
    }
    #[cfg(not(windows))]
    unix_candidates()
}

/// The Windows places, in this order (docs/platform-dirs.md): the
/// original's registry `Install Dir` values as read (`registry`; the hive
/// and view are unverified, so each is only a candidate), the installer's
/// default folder `EA GAMES\Need for Speed Underground 2` under
/// `%ProgramFiles(x86)%` and `%ProgramFiles%`, then Overbright's own
/// folder (`exe_dir`: unpacked into the install) and its parent (unpacked
/// next to it). Duplicates are left out. Pure (tested on any OS).
pub fn windows_candidates(
    var: &dyn Fn(&str) -> Option<std::ffi::OsString>,
    registry: Vec<PathBuf>,
    exe_dir: Option<&Path>,
) -> Vec<PathBuf> {
    let mut v = registry;
    for pf in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(d) = var(pf).filter(|d| !d.is_empty()) {
            v.push(PathBuf::from(d).join("EA GAMES").join("Need for Speed Underground 2"));
        }
    }
    if let Some(exe) = exe_dir {
        v.push(exe.to_path_buf());
        v.extend(exe.parent().map(Path::to_path_buf));
    }
    let mut seen = Vec::new();
    v.retain(|p| {
        let new = !seen.contains(p);
        seen.push(p.clone());
        new
    });
    v
}

#[cfg_attr(windows, allow(dead_code))]
fn unix_candidates() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return Vec::new() };
    let game = "EA GAMES/Need for Speed Underground 2";
    let mut prefixes: Vec<PathBuf> = vec![home.join(".wine")];
    let mut v: Vec<PathBuf> = Vec::new();
    for steam in [".steam/steam", ".local/share/Steam", ".var/app/com.valvesoftware.Steam/.local/share/Steam"] {
        let lib = home.join(steam).join("steamapps");
        v.push(lib.join("common/Need for Speed Underground 2"));
        // Proton prefixes of non-Steam installs.
        if let Ok(d) = std::fs::read_dir(lib.join("compatdata")) {
            prefixes.extend(d.filter_map(|e| e.ok()).map(|e| e.path().join("pfx")));
        }
    }
    for parent in [
        "Games",
        ".local/share/lutris/prefixes",
        ".var/app/com.usebottles.bottles/data/bottles/bottles",
        ".local/share/bottles/bottles",
        "Games/Heroic/Prefixes",
        ".cxoffice",
    ] {
        if let Ok(d) = std::fs::read_dir(home.join(parent)) {
            prefixes.extend(d.filter_map(|e| e.ok()).map(|e| e.path()));
        }
    }
    for p in prefixes {
        for pf in ["Program Files (x86)", "Program Files"] {
            v.push(p.join("drive_c").join(pf).join(game));
        }
    }
    // A folder named like the game right in a Games folder.
    v.push(home.join("Games/Need for Speed Underground 2"));
    v
}

/// Where the game is, without asking: `$NFSU2_ROOT` when set (as
/// before, unchecked), then the folder saved by `--set-root`, the
/// current folder, and [`candidates`] — the first that
/// [`looks_like_game`].
pub fn find() -> Option<PathBuf> {
    if let Some(r) = std::env::var_os("NFSU2_ROOT") {
        return Some(PathBuf::from(r));
    }
    let mut list = Vec::new();
    list.extend(saved_root());
    list.push(PathBuf::from("."));
    list.extend(candidates());
    list.into_iter().find(|p| looks_like_game(p))
}

/// The message shown when the game folder is not found or misses files,
/// in this system's words ([`help_for`]).
pub fn help(root: Option<&Path>, check: Option<&Check>) -> String {
    help_for(root, check, cfg!(windows))
}

/// [`help`] for Windows (`windows`) or Linux and other systems: plain
/// words for players, the program names as shipped (`overbright` the
/// launcher, `game` the game; `.exe` on Windows) and an example path of
/// that system. Shares its voice with [`check_lines`].
pub fn help_for(root: Option<&Path>, check: Option<&Check>, windows: bool) -> String {
    let mut s = String::new();
    match (root, check) {
        (Some(r), Some(c)) if !c.ok() => {
            s += &format!("The game folder {} is missing files Overbright needs:\n", r.display());
            for m in &c.missing {
                s += &format!("  {m}\n");
            }
            s += "Choose a complete install of Need for Speed Underground 2 (PC).\n";
        }
        _ => s += "Overbright could not find your copy of Need for Speed Underground 2 (PC).\n",
    }
    s += "The game folder is the one that holds SPEED2.EXE and the GLOBAL, FRONTEND and TRACKS folders.\n";
    let (launcher, game, example) = if windows {
        ("overbright.exe", "game.exe", r#""C:\Program Files (x86)\EA GAMES\Need for Speed Underground 2""#)
    } else {
        ("overbright", "./game", r#""$HOME/Games/Need for Speed Underground 2""#)
    };
    s += &format!("Start {launcher} (the launcher) and choose the folder there, or give it to the game once:\n");
    s += &format!("  {game} --set-root {example}\n");
    s += "Overbright keeps the folder for later starts. It only reads your game files and never changes them.\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_candidates_in_order_without_duplicates() {
        let var = |name: &str| match name {
            "ProgramFiles(x86)" => Some(std::ffi::OsString::from("C:/Program Files (x86)")),
            "ProgramFiles" => Some(std::ffi::OsString::from("C:/Program Files")),
            _ => None,
        };
        let game = Path::new("C:/Program Files (x86)").join("EA GAMES").join("Need for Speed Underground 2");
        let reg = vec![PathBuf::from("D:/NFSU2"), game.clone()];
        let v = windows_candidates(&var, reg, Some(Path::new("D:/NFSU2/Overbright")));
        assert_eq!(
            v,
            vec![
                PathBuf::from("D:/NFSU2"),
                game,
                Path::new("C:/Program Files").join("EA GAMES").join("Need for Speed Underground 2"),
                PathBuf::from("D:/NFSU2/Overbright"),
            ]
        );
        assert!(windows_candidates(&|_| None, Vec::new(), None).is_empty());
    }

    /// The setup help in each system's words: no developer wording, the
    /// shipped program names.
    #[test]
    fn help_is_worded_for_each_system() {
        let c = Check { missing: vec!["TRACKS/L4RA.BUN".into()], different: Vec::new() };
        for windows in [false, true] {
            for h in [help_for(None, None, windows), help_for(Some(Path::new("/g")), Some(&c), windows)] {
                assert!(h.contains("Overbright") && h.contains("--set-root"), "{h}");
                assert!(!h.contains("clone") && !h.contains("NFSU2_ROOT") && !h.contains("/path/to"), "{h}");
                assert_eq!(h.contains("game.exe") && h.contains("C:\\"), windows, "{h}");
                assert_eq!(h.contains("./game") && h.contains("$HOME"), !windows, "{h}");
            }
        }
        assert!(help_for(Some(Path::new("/g")), Some(&c), false).contains("  TRACKS/L4RA.BUN\n"));
    }
}

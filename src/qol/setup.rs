use std::path::{Path, PathBuf};

/// The files the clone cannot start without, with the sizes of the
/// European 1.2 release and, for the smaller ones, their XXH3-64. A
/// different size or hash is reported as "different version or modded",
/// not an error; a missing file is. SPEED2.EXE is not among them:
/// Overbright never runs or reads it (its tables are in the code,
/// [`crate::exe_tables`]), so any retail copy works, copy-protected or not.
pub const KEY_FILES: [(&str, u64, Option<u64>); 9] = [
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
    /// The text language the check was made for (the game's file for it is
    /// required in place of `English.bin`).
    pub language: String,
    /// Required files that are there but have no known size or checksum
    /// (a language other than English): "not verified", not an error.
    pub unverified: Vec<String>,
    /// The full check's asset report: the edition, the movies and menu
    /// files the game will look for ([`crate::game_assets::scan`]).
    pub assets: Option<crate::game_assets::Report>,
}

impl Check {
    pub fn ok(&self) -> bool {
        self.missing.is_empty()
    }
}

/// Check `root` for [`KEY_FILES`] (only reads): each file there and
/// of the known size, and with `hash` of the known XXH3 (about 22 MB
/// read) and the asset report ([`crate::game_assets::scan`]). The text
/// file required is the chosen language's ([`language_in_use`]).
pub fn check(root: &Path, hash: bool) -> Check {
    check_for(root, hash, &language_in_use(root))
}

/// [`check`] for the text language `language`. Its file takes the place
/// of `LANGUAGES/English.bin` in [`KEY_FILES`]: it must be there, and a
/// language other than English has no known size or checksum, so it is
/// listed in [`Check::unverified`] instead of being compared.
pub fn check_for(root: &Path, hash: bool, language: &str) -> Check {
    let mut c = Check { language: language.to_string(), ..Default::default() };
    let english = crate::game_assets::text_file_of(crate::game_assets::DEFAULT_LANGUAGE);
    for (file, size, known) in KEY_FILES {
        if file == english && !language.eq_ignore_ascii_case(crate::game_assets::DEFAULT_LANGUAGE) {
            let own = crate::game_assets::text_file_of(language);
            if key_file_path(root, &own).is_file() { c.unverified.push(own) } else { c.missing.push(own) }
            continue;
        }
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
    if hash {
        c.assets = Some(crate::game_assets::scan_for(root, language));
    }
    c
}

impl Check {
    /// The text file this check required (English's when it was not made
    /// for a language).
    pub fn text_file(&self) -> String {
        let l = if self.language.is_empty() { crate::game_assets::DEFAULT_LANGUAGE } else { &self.language };
        crate::game_assets::text_file_of(l)
    }

    /// The files this check required: [`KEY_FILES`]' names, with the
    /// chosen language's text file in place of English's.
    pub fn key_files(&self) -> Vec<String> {
        let english = crate::game_assets::text_file_of(crate::game_assets::DEFAULT_LANGUAGE);
        KEY_FILES.iter().map(|k| if k.0 == english { self.text_file() } else { k.0.to_string() }).collect()
    }

    /// The file the fingerprint's verdict may leave out when it is missing
    /// ([`crate::fingerprint::Fingerprint::verdict_skipping`]): the English
    /// text file, when the check was made for another language.
    pub fn verdict_skip(&self) -> Option<&'static str> {
        (!self.language.is_empty() && !self.language.eq_ignore_ascii_case(crate::game_assets::DEFAULT_LANGUAGE))
            .then_some("LANGUAGES/English.bin")
    }
}

/// The language this process plays or checks with: the one the game was
/// started with ([`crate::game_assets::set_text_language`]), else the one
/// [`choose_language`] gives for `root` from the kept choice.
pub fn language_in_use(root: &Path) -> String {
    crate::game_assets::active_language().map_or_else(|| choose_language(root, None), str::to_string)
}

/// The text language to use in `root`, by name as [`installed_languages`]
/// gives it: `requested` (the command line's `--language`), else the kept
/// choice ([`saved_language`]), when that language's file is in `root` and
/// [`crate::game_assets::language_support`] says the engine can show it.
/// Otherwise English; and a copy without `English.bin` falls back to its
/// first supported language (a single-language edition) rather than to a
/// file that is not there. A choice that cannot be used is logged.
pub fn choose_language(root: &Path, requested: Option<&str>) -> String {
    use crate::game_assets::{DEFAULT_LANGUAGE, Support, language_support, valid_language_name};
    let installed = installed_languages(root);
    let wanted = requested.map(str::to_string).or_else(saved_language);
    if let Some(w) = wanted.filter(|w| !w.eq_ignore_ascii_case(DEFAULT_LANGUAGE)) {
        match installed.iter().find(|i| i.eq_ignore_ascii_case(&w)) {
            Some(n) if valid_language_name(n) && language_support(n) == Support::Yes => return n.clone(),
            Some(n) => log::warn!("language {n} is not supported by this build; using {DEFAULT_LANGUAGE}"),
            None => log::warn!("language {w} is not in the game folder's LANGUAGES; using {DEFAULT_LANGUAGE}"),
        }
    }
    let has_default = installed.iter().any(|i| i.eq_ignore_ascii_case(DEFAULT_LANGUAGE));
    if has_default || installed.is_empty() {
        return DEFAULT_LANGUAGE.to_string();
    }
    installed
        .iter()
        .find(|i| valid_language_name(i) && language_support(i) == Support::Yes)
        .cloned()
        .unwrap_or_else(|| DEFAULT_LANGUAGE.to_string())
}

/// Where a [`KEY_FILES`] entry is under `root`: every game file is found
/// without regard to case ([`crate::game_fs`]).
pub fn key_file_path(root: &Path, file: &str) -> PathBuf {
    crate::game_fs::path(root, file)
}

/// Whether `root` looks like the game folder (its main data files are
/// there).
pub fn looks_like_game(root: &Path) -> bool {
    crate::game_fs::path(root, "GLOBAL/GlobalB.lzc").is_file()
        && crate::game_fs::path(root, "FRONTEND/FrontB.lzc").is_file()
        && crate::game_fs::path(root, "TRACKS").is_dir()
}

/// The file a folder chosen by `--set-root` or the prompt is kept in
/// (`platform::dirs::config()/game-root.txt`; not options.txt).
pub fn saved_root_file() -> PathBuf {
    crate::platform::dirs::config().join("game-root.txt")
}

/// The kept game folder, resolved: a path kept relative to a portable
/// install is taken from its program folder (`platform::dirs::load_path`).
pub fn saved_root() -> Option<PathBuf> {
    let text = std::fs::read_to_string(saved_root_file()).ok()?;
    let p = PathBuf::from(text.trim());
    (!text.trim().is_empty()).then(|| crate::platform::dirs::load_path(&p))
}

/// The game folder Studio's programs (editor, window, command line) use when
/// no `--game` is given: `NFSU2_ROOT` when set and not empty, as the game
/// reads it, else the one the launcher kept ([`saved_root`]).
pub fn game_root() -> Option<PathBuf> {
    game_root_from(std::env::var_os("NFSU2_ROOT"), saved_root)
}

/// [`game_root`] from the variable's value and the kept folder.
pub fn game_root_from(var: Option<std::ffi::OsString>, saved: impl FnOnce() -> Option<PathBuf>) -> Option<PathBuf> {
    match var.filter(|v| !v.is_empty()) {
        Some(v) => Some(PathBuf::from(v)),
        None => saved(),
    }
}

/// Keeps `root` as the game folder, in a portable install relative to the
/// program folder when on the same drive (`platform::dirs::store_path`;
/// docs/playing/platform-dirs.md, "Portable mode").
pub fn save_root(root: &Path) -> std::io::Result<()> {
    let root = crate::platform::dirs::store_path(root);
    super::saves::write_atomic(&saved_root_file(), format!("{}\n", root.display()).as_bytes(), false)
}

/// L1 (docs/launcher.md): the file the texture-pack folder chosen in the
/// launcher is kept in (`texture-pack-root.txt` beside game-root.txt).
/// Read by the HD UI pack's lookup after `NFSU2_TEXTURE_PACK`
/// (`render::enh::assets::hd_ui`).
pub fn saved_texture_pack_file() -> PathBuf {
    saved_root_file().with_file_name("texture-pack-root.txt")
}

/// The kept texture-pack folder, if one was chosen, resolved as
/// [`saved_root`] resolves.
pub fn saved_texture_pack() -> Option<PathBuf> {
    let text = std::fs::read_to_string(saved_texture_pack_file()).ok()?;
    let p = PathBuf::from(text.trim());
    (!text.trim().is_empty()).then(|| crate::platform::dirs::load_path(&p))
}

/// Keeps `dir` as the texture-pack folder (the same atomic write and
/// portable form as [`save_root`]); `None` forgets it (the file removed).
pub fn save_texture_pack(dir: Option<&Path>) -> std::io::Result<()> {
    let file = saved_texture_pack_file();
    match dir {
        Some(d) => {
            let d = crate::platform::dirs::store_path(d);
            super::saves::write_atomic(&file, format!("{}\n", d.display()).as_bytes(), false)
        }
        None => match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    }
}

/// The file the text language chosen in the launcher is kept in
/// (`language.txt` beside game-root.txt). The game reads it when it is
/// started without `--language`, so it also starts in that language
/// without the launcher.
pub fn saved_language_file() -> PathBuf {
    saved_root_file().with_file_name("language.txt")
}

/// The kept language name, if one was chosen and is a name that can be
/// part of a path ([`crate::game_assets::valid_language_name`]).
pub fn saved_language() -> Option<String> {
    let text = std::fs::read_to_string(saved_language_file()).ok()?;
    let t = text.trim();
    crate::game_assets::valid_language_name(t).then(|| t.to_string())
}

/// Keeps `name` as the text language (the same atomic write as
/// [`save_root`]); `None` forgets it (the file removed: English).
pub fn save_language(name: Option<&str>) -> std::io::Result<()> {
    let file = saved_language_file();
    match name {
        Some(n) => super::saves::write_atomic(&file, format!("{n}\n").as_bytes(), false),
        None => match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    }
}

/// The language files of `root`'s `LANGUAGES` folder, by name (`English`,
/// `French`, …; any case, sorted), without the shared tables that are not
/// a language (`Labels`, `Largest`, `LanguageTextures`). Which of them the
/// engine can show is [`crate::game_assets::language_support`].
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
    use crate::fingerprint::Verdict;
    let mut v = Vec::new();
    match fp.map(|f| (f, f.verdict_skipping(c.verdict_skip()))) {
        Some((_, Verdict::Known(e))) => v.push((Mark::Ok, format!("Version: {e} — recognised"))),
        // Missing files: the missing lines say it.
        Some((_, Verdict::Unknown(_))) if !c.missing.is_empty() => {}
        Some((_, Verdict::Unknown(files))) => v.push((
            Mark::Warn,
            format!(
                "Version not recognised ({} {} differ from the known release); the game may look or play differently",
                files.len(),
                if files.len() == 1 { "file" } else { "files" }
            ),
        )),
        None if c.ok() && c.different.is_empty() => v.push((Mark::Ok, "Version: the known release (sizes and checksums match)".into())),
        None => {}
    }
    let n = KEY_FILES.len();
    if c.missing.is_empty() {
        v.push((Mark::Ok, format!("All {n} files Overbright needs are present")));
    }
    if !c.unverified.is_empty() {
        v.push((
            Mark::Info,
            format!(
                "{}: not verified (Overbright knows no size or checksum for it); it is used as it is",
                c.unverified.join(", ")
            ),
        ));
    }
    if !c.different.is_empty() {
        let k = c.different.len();
        v.push((
            Mark::Warn,
            format!(
                "{k} {} from the known release (modded or another edition): {} — the game may still run",
                if k == 1 { "file differs" } else { "files differ" },
                c.different.join(", ")
            ),
        ));
    }
    if let Some(note) = fp.and_then(|f| f.exe_note()) {
        v.push((Mark::Info, note));
    }
    if let Some(f) = fp
        && !f.mods.is_empty()
    {
        v.push((Mark::Info, format!("Mod files found next to the game: {} (Overbright ignores them)", f.mods.join(", "))));
    }
    // The edition and the movies (not for a folder that is mostly not a
    // game: its error line says it).
    if let Some(a) = c.assets.as_ref().filter(|_| c.missing.len() <= 3) {
        v.extend(a.summary());
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

/// The asset report's full list ([`crate::game_assets::Report::details`]),
/// empty without one: every movie and menu file the game will look for,
/// the file it opens or the names tried (`--check-game`, the support
/// file).
pub fn check_details(c: &Check) -> Vec<String> {
    c.assets.as_ref().map(|a| a.details()).unwrap_or_default()
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

/// The Windows places, in this order (docs/playing/platform-dirs.md): the
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
    // An empty NFSU2_ROOT counts as unset (a start script that clears it).
    if let Some(r) = std::env::var_os("NFSU2_ROOT").filter(|r| !r.is_empty()) {
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
    s += "The game folder is the one that holds the GLOBAL, FRONTEND and TRACKS folders.\n";
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

    /// Studio's programs look for the game as the game does: `NFSU2_ROOT`
    /// first (an empty one is not set), then the launcher's folder.
    #[test]
    fn studio_game_root_reads_the_variable_first() {
        let kept = || Some(PathBuf::from("/kept"));
        assert_eq!(game_root_from(Some("/env".into()), kept), Some(PathBuf::from("/env")));
        assert_eq!(game_root_from(Some("".into()), kept), Some(PathBuf::from("/kept")));
        assert_eq!(game_root_from(None, kept), Some(PathBuf::from("/kept")));
        assert_eq!(game_root_from(None, || None), None);
    }

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
        let c = Check { missing: vec!["TRACKS/L4RA.BUN".into()], ..Default::default() };
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

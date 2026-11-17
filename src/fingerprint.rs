//! Game-version fingerprint (clone tooling; no native counterpart).
//!
//! SHA-256 of SPEED2.EXE, GLOBAL/GlobalB.lzc and the other key files,
//! compared with the editions the clone knows, so `--check-game`, the log
//! and crash reports say clearly which game build the player runs: the
//! clone was traced on one SPEED2.EXE (addresses in docs/formats.md are
//! that build's), and a modified GlobalB.lzc changes cars, upgrades and
//! career rows. An unknown or modified data file is a warning only: the
//! game keeps running; an unknown SPEED2.EXE stops the start, since the
//! clone reads its tables from it ([`crate::exe_tables`]). Reads only;
//! docs/game-fingerprint.md.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The files fingerprinted, in report order: the executable first (its
/// build decides whether the traced addresses apply), then the data the
/// clone reads at start.
pub const FILES: [&str; 9] = [
    "SPEED2.EXE",
    "GLOBAL/GlobalB.lzc",
    "GLOBAL/GLOBALB.BUN",
    "GLOBAL/InGameCommon.bun",
    "FRONTEND/FrontB.lzc",
    "LANGUAGES/English.bin",
    "CARS/TEXTURES.BIN",
    "CARS/WHEELS/TEXTURES.BIN",
    "TRACKS/L4RA.BUN",
];

/// An edition the clone knows: its name and each file's size and SHA-256.
pub struct Edition {
    pub name: &'static str,
    pub files: &'static [(&'static str, u64, &'static str)],
}

/// The editions with every hash known. The first is the build the clone
/// was traced and checked on (the player's install, 2026-11-05).
pub const EDITIONS: &[Edition] = &[Edition {
    name: "Need for Speed Underground 2 for PC, version 1.2",
    files: &[
        ("SPEED2.EXE", 4_788_224, "8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297"),
        ("GLOBAL/GlobalB.lzc", 5_145_778, "ad015fdb24e3bfce3aff8bcdfbbef3cb981b524115884a8eed4bf3ce4a2d6e90"),
        ("GLOBAL/GLOBALB.BUN", 8_008_064, "07e1c37a2e5ba4b3e6a17b257e653409c31d5f0b3f9df3464aec36adbbced9d0"),
        ("GLOBAL/InGameCommon.bun", 1_595_640, "6b83fb688c56c2a7f1a5224b52b39e0db76cd064e4b54a6462b9adc881b7551d"),
        ("FRONTEND/FrontB.lzc", 1_750_771, "813e8f966ca9af9a5374454199b3a1e57460ff9374a8c3cc7822bfbcd017418e"),
        ("LANGUAGES/English.bin", 270_456, "86cc5bdb5ca051b2fcb01e953fb6216520df5978c521fbeeec0ba3158e9a4448"),
        ("CARS/TEXTURES.BIN", 1_895_360, "8f23df9a4c096862e39dbc67b37481535643e5942144d79eed437f8d9eec2a54"),
        ("CARS/WHEELS/TEXTURES.BIN", 3_791_808, "54360b64e137621e3da27837e6e38e875af1b2fee0cf9a8d671d33d23c383f63"),
        ("TRACKS/L4RA.BUN", 422_528, "92962314e0c0e59a5335e04d51caf4e820d6381bb19ec1169b9df13d3bc89257"),
    ],
}];

/// Other SPEED2.EXE builds known only by size and part of their hash
/// (prefix, suffix): another build, whose addresses differ from the
/// traced one. The 4,800,512-byte build is the one the OpenUG2 project
/// documents ($OB_PRIVATE/research/openug2-study-2026-11-05.md; only
// prov: inferred fingerprint.OTHER_EXE_BUILDS
/// the hash's ends were recorded, so this match is **inferred**).
/// The 5,987,978-byte build is the European 1.2 release's copy-protected
/// retail executable (SafeDisc; recognised by its sections whatever its
/// hash, [`crate::exe_tables::Exe::protected`]).
pub const OTHER_EXE_BUILDS: &[(u64, &str, &str, &str)] = &[
    (4_800_512, "f9dd86c0", "24168a7", "another SPEED2.EXE build (4,800,512 bytes)"),
    (5_987_978, "4d51643c", "818f", "the European 1.2 release's copy-protected SPEED2.EXE (5,987,978 bytes)"),
];

/// Files of loaders and mods next to the game (they do not change what the
/// clone reads, but tell a report reader the install is modded).
pub const MOD_MARKERS: [&str; 4] = ["dinput8.dll", "d3d9.dll", "SCRIPTS", "scripts"];

/// One file's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Missing,
    /// Not readable (permissions, a folder in its place).
    Unreadable(String),
    /// Read: size and SHA-256 (lowercase hex).
    Read { size: u64, sha256: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePrint {
    pub file: &'static str,
    pub state: State,
    /// The known edition this file matches, if any.
    pub edition: Option<&'static str>,
}

/// What [`compute`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    pub root: PathBuf,
    pub files: Vec<FilePrint>,
    /// The loader / mod files present ([`MOD_MARKERS`]).
    pub mods: Vec<String>,
}

/// The overall verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Every file is one known edition's.
    Known(&'static str),
    /// Some files are missing, unreadable or unlike every known edition
    /// (the names), or the files mix editions.
    Unknown(Vec<String>),
}

/// SHA-256 of a byte slice, lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in d {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// SHA-256 of a file, streamed (lowercase hex) and its size.
pub fn sha256_file(path: &Path) -> std::io::Result<(u64, String)> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut size = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        h.update(&buf[..n]);
    }
    let mut s = String::with_capacity(64);
    for b in h.finalize() {
        s.push_str(&format!("{b:02x}"));
    }
    Ok((size, s))
}

/// A file under the game folder, the name matched case-insensitively
/// (installs copied from Windows differ in case).
fn locate(root: &Path, rel: &str) -> Option<PathBuf> {
    crate::game_fs::find(root, rel)
}

/// Fingerprint the game folder `root` (reads about 27 MB).
pub fn compute(root: &Path) -> Fingerprint {
    let files = FILES
        .iter()
        .map(|&file| {
            let state = match locate(root, file) {
                None => State::Missing,
                Some(p) if !p.is_file() => State::Unreadable("not a file".into()),
                Some(p) => match sha256_file(&p) {
                    Ok((size, sha256)) => State::Read { size, sha256 },
                    Err(e) => State::Unreadable(e.to_string()),
                },
            };
            let edition = match &state {
                State::Read { size, sha256 } => EDITIONS.iter().find(|e| e.files.iter().any(|(f, s, h)| *f == file && s == size && h == sha256)).map(|e| e.name),
                _ => None,
            };
            FilePrint { file, state, edition }
        })
        .collect();
    // Found in any case (a copied or Linux-unpacked install), each once.
    let mut mods: Vec<String> = Vec::new();
    for m in MOD_MARKERS {
        let p = crate::game_fs::path(root, m);
        let name = p.file_name().map_or_else(|| m.to_string(), |n| n.to_string_lossy().into_owned());
        if p.exists() && !mods.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
            mods.push(name);
        }
    }
    Fingerprint { root: root.to_path_buf(), files, mods }
}

impl Fingerprint {
    pub fn verdict(&self) -> Verdict {
        let first = self.files.first().and_then(|f| f.edition);
        if let Some(e) = first
            && self.files.iter().all(|f| f.edition == Some(e))
        {
            return Verdict::Known(e);
        }
        Verdict::Unknown(self.files.iter().filter(|f| f.edition.is_none() || f.edition != first).map(|f| f.file.to_string()).collect())
    }

    /// A short line for logs and crash reports.
    pub fn summary(&self) -> String {
        let exe = self.files.first().map_or(String::new(), |f| match &f.state {
            State::Read { size, sha256 } => format!("SPEED2.EXE {size} B sha256 {sha256}"),
            State::Missing => "SPEED2.EXE missing".into(),
            State::Unreadable(e) => format!("SPEED2.EXE unreadable ({e})"),
        });
        let lzc = self.files.iter().find(|f| f.file == "GLOBAL/GlobalB.lzc").map_or(String::new(), |f| match &f.state {
            State::Read { size, sha256 } => format!("GlobalB.lzc {size} B sha256 {sha256}"),
            State::Missing => "GlobalB.lzc missing".into(),
            State::Unreadable(e) => format!("GlobalB.lzc unreadable ({e})"),
        });
        let verdict = match self.verdict() {
            Verdict::Known(e) => format!("known edition: {e}"),
            Verdict::Unknown(v) => format!("UNKNOWN OR MODIFIED: {}", v.join(", ")),
        };
        let mods = if self.mods.is_empty() { String::new() } else { format!("; mod files present: {}", self.mods.join(", ")) };
        format!("{verdict}; {exe}; {lzc}{mods}")
    }

    /// The `--check-game` block: one line per file, then the verdict.
    pub fn report(&self) -> String {
        let mut s = String::from("fingerprint (SHA-256):\n");
        for f in &self.files {
            let state = match &f.state {
                State::Missing => "MISSING".to_string(),
                State::Unreadable(e) => format!("UNREADABLE ({e})"),
                State::Read { size, sha256 } => {
                    let note = match f.edition {
                        Some(_) => "known".to_string(),
                        None => other_exe(f.file, *size, sha256).map_or_else(|| "MODIFIED or unknown version".to_string(), |n| format!("{n} (inferred)")),
                    };
                    format!("{size} bytes {sha256} {note}")
                }
            };
            s += &format!("  {}: {state}\n", f.file);
        }
        match self.verdict() {
            Verdict::Known(e) => s += &format!("edition: {e}\n"),
            Verdict::Unknown(v) => {
                s += &format!("edition: UNKNOWN OR MODIFIED ({}).\n", v.join(", "));
                if self.files.first().is_some_and(|f| f.edition.is_none() && matches!(f.state, State::Read { .. })) {
                    // Overbright reads its tables from SPEED2.EXE: a patched
                    // copy with the supported data runs, other data does not.
                    match crate::exe_tables::check_folder(&self.root) {
                        Ok(_) => s += &format!("note: {}.\n", crate::qol::setup::exe_modified_line()),
                        Err(crate::exe_tables::Error::TablesDiffer { version, .. }) => {
                            s += &format!("ERROR: {}; Overbright does not start with it.\n", crate::qol::setup::exe_differs_line(version))
                        }
                        Err(crate::exe_tables::Error::Protected { .. }) => {
                            s += &format!("ERROR: {}\n", crate::qol::setup::exe_protected_line())
                        }
                        Err(e) => s += &format!("ERROR: {}\n", e.to_string().lines().next().unwrap_or_default()),
                    }
                } else {
                    s += "WARNING: Overbright is made for one build of SPEED2.EXE and its data; another version or a modded install may look or play differently. Overbright still runs.\n";
                }
            }
        }
        if !self.mods.is_empty() {
            s += &format!("mod files present (ignored by Overbright): {}\n", self.mods.join(", "));
        }
        crate::qol::crash::scrub(&s)
    }
}

/// A known other SPEED2.EXE build's description for this size and hash.
pub fn other_exe(file: &str, size: u64, sha256: &str) -> Option<&'static str> {
    (file == "SPEED2.EXE").then(|| OTHER_EXE_BUILDS.iter().find(|(s, pre, suf, _)| *s == size && sha256.starts_with(pre) && sha256.ends_with(suf)).map(|b| b.3)).flatten()
}

static CURRENT: OnceLock<Fingerprint> = OnceLock::new();

/// The running game folder's fingerprint, once [`start`]'s thread has it.
pub fn current() -> Option<&'static Fingerprint> {
    CURRENT.get()
}

/// Fingerprint `root` on a background thread (the window start; headless
/// check modes skip it): the summary is logged (a warning when unknown or
/// modified) and kept for crash reports.
pub fn start(root: PathBuf) {
    let spawned = std::thread::Builder::new().name("fingerprint".into()).spawn(move || {
        let f = compute(&root);
        match f.verdict() {
            Verdict::Known(_) => log::info!("game files: {}", f.summary()),
            Verdict::Unknown(_) => log::warn!("game files: {} (Overbright keeps running; see --check-game)", f.summary()),
        }
        let _ = CURRENT.set(f);
    });
    if let Err(e) = spawned {
        log::warn!("game fingerprint skipped (no thread: {e})");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn editions_list_every_file_once_with_a_full_hash() {
        for e in EDITIONS {
            assert_eq!(e.files.len(), FILES.len());
            for (i, (f, _, h)) in e.files.iter().enumerate() {
                assert_eq!(*f, FILES[i]);
                assert_eq!(h.len(), 64);
                assert!(h.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
            }
        }
    }

    fn fake_root(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nfsu2-fp-{tag}-{}", crate::unique_tag()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn an_empty_folder_is_unknown_with_every_file_missing() {
        let d = fake_root("empty");
        let f = compute(&d);
        assert!(f.files.iter().all(|f| f.state == State::Missing));
        assert_eq!(f.verdict(), Verdict::Unknown(FILES.iter().map(|s| s.to_string()).collect()));
        assert!(f.report().contains("UNKNOWN OR MODIFIED"));
        assert!(f.summary().starts_with("UNKNOWN OR MODIFIED"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_modified_file_is_flagged_and_mod_files_listed() {
        let d = fake_root("mod");
        std::fs::create_dir_all(d.join("global")).unwrap();
        std::fs::write(d.join("SPEED2.EXE"), b"not the game").unwrap();
        // Case-insensitive lookup, as for the game's own files.
        std::fs::write(d.join("global/globalb.lzc"), b"patched").unwrap();
        std::fs::write(d.join("dinput8.dll"), b"x").unwrap();
        let f = compute(&d);
        assert!(matches!(&f.files[1].state, State::Read { size: 7, .. }), "{:?}", f.files[1]);
        let r = f.report();
        assert!(r.contains("SPEED2.EXE: 12 bytes"), "{r}");
        assert!(r.contains("MODIFIED or unknown version"), "{r}");
        assert!(r.contains("mod files present (ignored by Overbright): dinput8.dll"), "{r}");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_other_exe_build_is_named() {
        assert!(other_exe("SPEED2.EXE", 4_800_512, "f9dd86c0aaaa24168a7").is_some());
        assert!(other_exe("SPEED2.EXE", 4_800_512, "00dd86c0aaaa24168a7").is_none());
        assert!(other_exe("GLOBAL/GlobalB.lzc", 4_800_512, "f9dd86c0aaaa24168a7").is_none());
        assert!(other_exe("SPEED2.EXE", 5_987_978, "4d51643caaaa818f").is_some_and(|n| n.contains("copy-protected")));
    }

    /// The player's install (NFSU2_ROOT) is the known edition.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_reference_install_is_the_known_edition() {
        let root = crate::testkit::game_root();
        let f = compute(&root);
        assert_eq!(f.verdict(), Verdict::Known(EDITIONS[0].name), "{}", f.report());
    }

    /// A patched copy of GlobalB.lzc (one byte changed) is flagged, the
    /// rest still known (the OpenUG2 study's case). Unix only: the other
    /// files are symlinked (Windows symlinks need a privilege).
    #[cfg(unix)]
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_patched_global_lzc_is_flagged() {
        let root = crate::testkit::game_root();
        let mut lzc = std::fs::read(root.join("GLOBAL/GlobalB.lzc")).unwrap();
        let d = fake_root("patched");
        for f in FILES {
            let to = d.join(f);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            if f == "GLOBAL/GlobalB.lzc" {
                lzc[0x3E0] ^= 1;
                std::fs::write(&to, &lzc).unwrap();
            } else {
                std::os::unix::fs::symlink(root.join(f), &to).unwrap();
            }
        }
        let f = compute(&d);
        assert_eq!(f.verdict(), Verdict::Unknown(vec!["GLOBAL/GlobalB.lzc".into()]));
        assert!(f.report().contains("GLOBAL/GlobalB.lzc: 5145778 bytes"));
        std::fs::remove_dir_all(&d).unwrap();
    }
}

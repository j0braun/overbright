//! The test kit (docs/testing.md): one way for every test to reach the
//! player's game files, fixtures regenerated from them, and private
//! captures, so a test that cannot run says so (IGNORED, with the reason)
//! instead of passing without checking anything.
//!
//! One source file, compiled into each place that needs it:
//! * integration tests (`tests/*.rs`): `mod common;`
//! * the library's unit tests: `crate::testkit` (src/lib.rs, `cfg(test)`)
//! * the game binary's test modules: `#[path = "../../../tests/common/mod.rs"] mod testkit;`
//!   inside the module (src/bin/game/*.rs).
//!
//! It is never part of a shipped binary. It needs no dependency beyond the
//! game crate itself (`nfsu2::fingerprint` for the game's fingerprint and
//! SHA-256).
//!
//! # Data tests
//!
//! build.rs sets `cfg(ob_game)` when `NFSU2_ROOT` is set at build time and
//! `cfg(ob_private)` when the private set ([`private_root`]) exists. A test
//! that needs the game carries
//!
//! ```ignore
//! #[test]
//! #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
//! fn bayview_loads() {
//!     let root = testkit::game_root();
//!     ...
//! }
//! ```
//!
//! so without the game `cargo test` lists it as `ignored, needs NFSU2_ROOT
//! ...`, and with the game it runs, and every load error FAILS it (no
//! `.ok()?`, no early `return`). [`game`] panics when it is reached without
//! `cfg(ob_game)`: a data test that forgot the attribute fails instead of
//! passing. scripts/checks/checks.sh counts the ignores by their reason.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The ignore reasons (the `ignore = "..."` literals must repeat them
/// word for word: checks.sh counts them).
pub const NEEDS_GAME: &str = "needs NFSU2_ROOT (the player's own game files)";
pub const NEEDS_PRIVATE: &str = "needs OB_PRIVATE (private captures and saves)";

/// The player's game folder, as the tests see it.
pub struct Game {
    pub root: PathBuf,
    fingerprint: OnceLock<String>,
}

impl Game {
    /// A short key for the installed game's files: SHA-256 over every
    /// fingerprinted file's name, size and hash (src/fingerprint.rs,
    /// `FILES`), first 16 hex digits. Computed once per test process
    /// (reads about 27 MB).
    pub fn fingerprint(&self) -> &str {
        self.fingerprint.get_or_init(|| {
            let print = nfsu2::fingerprint::compute(&self.root);
            let mut lines = String::new();
            for f in &print.files {
                lines += &format!("{}:{:?}\n", f.file, f.state);
            }
            nfsu2::fingerprint::sha256_hex(lines.as_bytes())[..16].to_string()
        })
    }

    /// The fingerprint's one-line verdict (known edition or not), for
    /// failure messages.
    pub fn describe(&self) -> String {
        nfsu2::fingerprint::compute(&self.root).summary()
    }
}

/// The game, from `NFSU2_ROOT`: the one place tests read it. Panics (the
/// test FAILS) when the variable is unset or not a folder; data tests are
/// ignored before they get here when it was unset at build time.
pub fn game() -> &'static Game {
    static GAME: OnceLock<Game> = OnceLock::new();
    GAME.get_or_init(|| {
        // What the game's files are parsed into is cached in the tests' own
        // folder under the target folder, never in the player's or the
        // developer's cache (`NFSU2_CACHE_DIR` still comes first).
        nfsu2::platform::dirs::use_test_cache(nfsu2::platform::dirs::test_cache_folder());
        assert!(
            cfg!(ob_game),
            "this test reads the game but is not marked `#[cfg_attr(not(ob_game), ignore = \"{NEEDS_GAME}\")]` (docs/testing.md), \
             or NFSU2_ROOT was unset when the tests were built"
        );
        let root = std::env::var_os("NFSU2_ROOT").filter(|r| !r.is_empty()).map(PathBuf::from);
        let root = root.expect("NFSU2_ROOT was set when the tests were built but is not set now");
        assert!(root.is_dir(), "NFSU2_ROOT={} is not a folder", root.display());
        Game { root, fingerprint: OnceLock::new() }
    })
}

/// The asset cache folder for a game process a test starts: the caller's
/// `NFSU2_CACHE_DIR`, else the tests' own folder (a child process is not a
/// test binary, so it would default to the developer cache).
pub fn child_cache_dir() -> PathBuf {
    std::env::var_os("NFSU2_CACHE_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(nfsu2::platform::dirs::test_cache_folder)
}

/// Shorthand for `game().root`.
pub fn game_root() -> PathBuf {
    game().root.clone()
}

// ---- fixtures regenerated from the game ------------------------------------

/// The pinned fixtures: `name@version sha256 size`, one a line
/// (tests/fixtures.lock).
const LOCK: &str = include_str!("../fixtures.lock");

/// A fixture's pinned SHA-256 and size from tests/fixtures.lock.
pub fn pinned(key: &str) -> Option<(&'static str, usize)> {
    LOCK.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()).find_map(|l| {
        let mut w = l.split_whitespace();
        (w.next()? == key).then_some(())?;
        let sha = w.next()?;
        let size = w.next()?.parse().ok()?;
        Some((sha, size))
    })
}

/// The fixture cache folder: `$XDG_CACHE_HOME/overbright-test-fixtures`
/// (else `~/.cache/...`), outside the repository; one sub-folder per game
/// fingerprint. Not inside the game's own cache folder (`overbright`): a
/// test run never writes into the player's cache.
pub fn fixture_cache() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME").filter(|d| !d.is_empty()).map(PathBuf::from);
    let base = base.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")));
    base.unwrap_or_else(std::env::temp_dir).join("overbright-test-fixtures")
}

/// A fixture made from the player's game files: `generate` runs once per
/// game (the result is cached under [`fixture_cache`]); the bytes must
/// hash to the SHA-256 pinned in tests/fixtures.lock under `name@version`
/// (taken from the file the repository used to carry), so the test checks
/// exactly what it checked before. A mismatch FAILS the test, naming the
/// game build. Bump `version` (and the lock line) when the generator's
/// output is meant to change.
pub fn fixture(name: &str, version: u32, generate: impl FnOnce(&Game) -> Vec<u8>) -> Vec<u8> {
    let key = format!("{name}@{version}");
    let (sha, size) = pinned(&key).unwrap_or_else(|| panic!("fixture {key} has no line in tests/fixtures.lock"));
    let g = game();
    let path = fixture_cache().join(g.fingerprint()).join(&key);
    if let Ok(bytes) = std::fs::read(&path)
        && bytes.len() == size
        && nfsu2::fingerprint::sha256_hex(&bytes) == sha
    {
        return bytes;
    }
    let bytes = generate(g);
    let got = nfsu2::fingerprint::sha256_hex(&bytes);
    assert!(
        bytes.len() == size && got == sha,
        "fixture {key}: generated {} bytes sha256 {got}, tests/fixtures.lock pins {size} bytes sha256 {sha}.\n\
         The game ({}) is not the build the fixture was pinned on, or the generator changed.",
        bytes.len(),
        g.describe()
    );
    // Written beside and renamed into place: parallel tests never read half a file.
    if let Some(dir) = path.parent()
        && std::fs::create_dir_all(dir).is_ok()
    {
        let tmp = dir.join(format!(".{key}.{}.{:?}", std::process::id(), std::thread::current().id()));
        if std::fs::write(&tmp, &bytes).is_ok() && std::fs::rename(&tmp, &path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
    bytes
}

/// Bytes `len` at virtual address `va` of SPEED2.EXE (its sections sit at
/// their virtual offsets in the file: file offset = VA − 0x400000).
pub fn exe_bytes(g: &Game, va: usize, len: usize) -> Vec<u8> {
    let exe = std::fs::read(g.root.join("SPEED2.EXE")).expect("reading SPEED2.EXE");
    let at = va.checked_sub(0x40_0000).expect("a VA above the image base");
    exe.get(at..at + len).unwrap_or_else(|| panic!("VA {va:#x}+{len:#x} outside SPEED2.EXE")).to_vec()
}

/// The stock PEUGOT car type record as the boundary-replay and
/// cached-wheel-count captures took it: 0x900 bytes from the start of its
/// 0x890-byte record in GLOBALB.BUN's car type table (chunk 0x34600), so
/// the first 0x70 bytes of the next record come along.
pub fn handling_peugot() -> Vec<u8> {
    fixture("handling-peugot", 1, |g| {
        const RECORD: usize = 0x890;
        let globalb = std::fs::read(g.root.join("GLOBAL/GLOBALB.BUN")).expect("reading GLOBAL/GLOBALB.BUN");
        let table = nfsu2::formats::bundle::chunks(&globalb, 0).find(|c| c.id == 0x0003_4600).expect("the car type table (0x34600)").payload();
        let at = table.chunks_exact(RECORD).position(|r| r.starts_with(b"PEUGOT\0")).expect("the PEUGOT car type") * RECORD;
        table.get(at..at + 0x900).expect("0x900 bytes from the PEUGOT record").to_vec()
    })
}

// ---- private captures --------------------------------------------------------

/// Where the private set lives: `$OB_PRIVATE`, else (for now) the
/// repository itself, where the captures still are.
pub fn private_root() -> PathBuf {
    std::env::var_os("OB_PRIVATE").filter(|d| !d.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

/// A file of the private set (recorded traces of the original, the
/// maintainer's saves: data that cannot be regenerated from the game), by its
/// path under [`private_root`]. Tests that call it carry
/// `#[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]`;
/// with the set present a missing file FAILS the test.
pub fn private(rel: &str) -> Vec<u8> {
    assert!(cfg!(ob_private), "this test reads a private file but is not marked `#[cfg_attr(not(ob_private), ignore = \"{NEEDS_PRIVATE}\")]`");
    let path = private_root().join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("private file {}: {e}", path.display()))
}

/// [`private`] as text.
pub fn private_str(rel: &str) -> String {
    String::from_utf8(private(rel)).unwrap_or_else(|e| panic!("private file {rel}: not UTF-8: {e}"))
}

/// A scratch folder for one test, removed when dropped (no shared paths
/// between tests, no `set_var`).
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("nfsu2-test-{tag}-{}", nfsu2::unique_tag()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("scratch folder {}: {e}", dir.display()));
        Scratch(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The text of every `.rs` file under `paths` (each a file or a folder,
/// relative to `root`, the crate root, which the caller passes as the
/// manifest folder from `env!`;
/// folders walked recursively, sorted by path),
/// joined in the order given. Tests that look at source text read a tree
/// with this, not one file, so they keep passing while code moves between
/// files (the maintainer's private notes).
pub fn source_tree(root: &str, paths: &[&str]) -> String {
    fn walk(p: &Path, out: &mut Vec<PathBuf>) {
        if p.is_dir() {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())).map(|e| e.unwrap().path()).collect();
            entries.sort();
            for e in entries {
                walk(&e, out);
            }
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p.to_path_buf());
        }
    }
    let root = Path::new(root);
    let mut files = Vec::new();
    for p in paths {
        let p = root.join(p);
        assert!(p.exists(), "{} is missing", p.display());
        walk(&p, &mut files);
    }
    files.iter().map(|f| std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()))).collect::<Vec<_>>().join("\n")
}

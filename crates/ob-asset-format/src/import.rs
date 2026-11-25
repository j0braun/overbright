//! The importer at stage 0 (docs/modding/asset-format/import.md): turns the player's installed copy of the game into
//! a base set once, keeping every content file byte for byte (`kept`, §4.1) in the bundle of the load unit that reads
//! it (layout.md §5).
//!
//! - [`scan`] lists the content folders (§2), found without regard to case; everything else is skipped.
//! - [`fingerprint`] hashes every content file (SHA-256, in parallel) and gives the order-independent identity (§3);
//!   [`Edition::lookup`] names the edition (`known`) or says which files differ (`custom`).
//! - [`import`] runs the steps of §5: free-space check (§10), staging folder with a journal (a stopped import
//!   resumes with the units not yet done), one bundle per unit, read-back of every kept file against its SHA-256,
//!   `pack.toml` and `import.toml` (§6) written last, the staging folder renamed into place.
//! - [`verify`] re-checks a finished base set; [`repair`] rebuilds its damaged bundles from the game folder (§8).
//!
//! **Read only.** The game folder is only ever opened for reading: no file there is written, renamed, locked or
//! created. `SPEED2.EXE` is hashed for the record and never read for content (it may be copy-protected; nothing here
//! decodes, decrypts or patches it).

use crate::bundle::{self, Bundle, Method, Writer};
use crate::digest::{Alg, Digest};
use crate::{Refusal, Rule};
use sha2::Digest as _;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// The content folders (§2), in the original's spelling.
pub const CONTENT_FOLDERS: &[&str] = &[
    "CARS",
    "CREDITS",
    "FRONTEND",
    "GLOBAL",
    "LANGUAGES",
    "MOVIES",
    "NIS",
    "SDATA",
    "SOUND",
    "SUBTITLES",
    "TRACKS",
    "memcard",
];

/// The executable: size and SHA-256 for the record only.
pub const EXE: &str = "SPEED2.EXE";

/// The import epoch (versioning.md §1).
pub const EPOCH: u32 = 1;

/// The version of every stage-0 import unit: a whole-file copy.
pub const UNIT_VERSION: u32 = 1;

/// The journal's first line.
pub const JOURNAL_HEADER: &str = "# Overbright import journal v1";

/// Bytes added per file to the space estimate: local and central headers, the alignment padding, the file map.
const PER_FILE_OVERHEAD: u64 = 4096;

/// The editions the importer knows (§3). The manifests hold names, sizes and hashes only, no game data.
pub fn editions() -> Vec<Edition> {
    vec![
        Edition::parse(
            "pc12-eu",
            "Need for Speed Underground 2 (PC 1.2, European), imported",
            "eu",
            include_str!("../editions/pc12-eu.tsv"),
        )
        .expect("the built-in edition manifest parses"),
    ]
}

/// One content file of the game folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// The path below the game folder, original spelling, `/`-separated.
    pub path: String,
    pub size: u64,
    /// SHA-256, once [`fingerprint`] has run.
    pub sha256: Option<[u8; 32]>,
}

/// What [`scan`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scan {
    pub root: PathBuf,
    /// Content files sorted by upper-cased path.
    pub files: Vec<SourceFile>,
    /// Entries not read, with the reason (top-level names, and files whose names the format cannot hold).
    pub skipped: Vec<(String, String)>,
    /// `SPEED2.EXE`'s size and SHA-256, when present and readable.
    pub exe: Option<(u64, [u8; 32])>,
}

impl Scan {
    /// The bytes of all content files.
    pub fn total(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// The order-independent identity (§3): SHA-256 of the sorted `<PATH>\t<size>\t<sha256>` lines, paths
    /// upper-cased, each line ended by `\n`. None before [`fingerprint`].
    pub fn identity(&self) -> Option<[u8; 32]> {
        let mut lines = Vec::with_capacity(self.files.len());
        for f in &self.files {
            lines.push((f.path.to_ascii_uppercase(), f.size, f.sha256?));
        }
        Some(identity_of(lines))
    }
}

fn identity_of(mut lines: Vec<(String, u64, [u8; 32])>) -> [u8; 32] {
    lines.sort();
    let mut h = sha2::Sha256::new();
    for (p, s, d) in &lines {
        h.update(format!("{p}\t{s}\t{}\n", hex(d)).as_bytes());
    }
    h.finalize().into()
}

/// Lowercase hex.
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

fn imp1(msg: impl Into<String>) -> Refusal {
    Refusal::new(Rule::Imp1, msg)
}

fn io_err(what: &str, p: &Path, e: std::io::Error) -> Refusal {
    // Disk errors have no rule of their own; IMP.1 is the importer's "cannot read or write what it needs".
    imp1(format!("{what} {}: {e}", p.display()))
}

/// Lists the content folders of `root` (§2). Reads only.
pub fn scan(root: &Path) -> Result<Scan, Refusal> {
    let top = std::fs::read_dir(root).map_err(|e| io_err("cannot read the game folder", root, e))?;
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    let mut exe = None;
    let mut found = 0;
    let mut names: Vec<(String, PathBuf)> = Vec::new();
    for e in top {
        let e = e.map_err(|e| io_err("cannot read the game folder", root, e))?;
        names.push((e.file_name().to_string_lossy().into_owned(), e.path()));
    }
    names.sort();
    for (name, path) in names {
        let is_dir = std::fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false);
        if is_dir && CONTENT_FOLDERS.iter().any(|c| c.eq_ignore_ascii_case(&name)) {
            found += 1;
            walk(&path, &name, &mut files, &mut skipped)?;
        } else if !is_dir && name.eq_ignore_ascii_case(EXE) {
            exe = hash_file(&path).ok();
        } else {
            skipped.push((name, "not game content".to_string()));
        }
    }
    if found == 0 {
        return Err(imp1(format!(
            "{} has none of the game's content folders ({}): not a game folder",
            root.display(),
            CONTENT_FOLDERS.join(", ")
        )));
    }
    files.sort_by_key(|f: &SourceFile| f.path.to_ascii_uppercase());
    if let Some(w) = files.windows(2).find(|w| w[0].path.eq_ignore_ascii_case(&w[1].path)) {
        return Err(imp1(format!("{} and {} differ only in case", w[0].path, w[1].path)));
    }
    Ok(Scan { root: root.to_path_buf(), files, skipped, exe })
}

fn walk(
    dir: &Path,
    rel: &str,
    files: &mut Vec<SourceFile>,
    skipped: &mut Vec<(String, String)>,
) -> Result<(), Refusal> {
    let rd = std::fs::read_dir(dir).map_err(|e| io_err("cannot read", dir, e))?;
    for e in rd {
        let e = e.map_err(|e| io_err("cannot read", dir, e))?;
        let name = e.file_name().to_string_lossy().into_owned();
        let path = e.path();
        let rel = format!("{rel}/{name}");
        let link = std::fs::symlink_metadata(&path).map(|m| m.file_type().is_symlink()).unwrap_or(false);
        let meta = std::fs::metadata(&path).map_err(|e| io_err("cannot read", &path, e))?;
        if meta.is_dir() {
            if link {
                skipped.push((rel, "a linked folder".to_string()));
            } else {
                walk(&path, &rel, files, skipped)?;
            }
        } else if crate::names::logical_path(&kept_path(&rel), false).is_err() {
            skipped.push((rel, "a name the format cannot hold".to_string()));
        } else {
            files.push(SourceFile { path: rel, size: meta.len(), sha256: None });
        }
    }
    Ok(())
}

/// Size and SHA-256 of a file, read in 1 MiB pieces.
fn hash_file(p: &Path) -> std::io::Result<(u64, [u8; 32])> {
    let mut f = std::fs::File::open(p)?;
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut n = 0u64;
    loop {
        let k = f.read(&mut buf)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        n += k as u64;
    }
    Ok((n, h.finalize().into()))
}

type HashResult = Result<(usize, [u8; 32]), Refusal>;
type UnitJob<'a, T> = dyn Fn(&str, &[usize]) -> Result<T, Refusal> + Sync + 'a;

/// What the importer reports while it runs: `key=value` lines (§9).
pub type Progress<'a> = &'a (dyn Fn(&str) + Sync);

/// SHA-256 of every content file, on `threads` threads. A file whose size changed since the scan is refused.
pub fn fingerprint(scan: &mut Scan, threads: usize, progress: Progress) -> Result<(), Refusal> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let n = scan.files.len();
    let root = scan.root.clone();
    let results: Mutex<Vec<HashResult>> = Mutex::new(Vec::with_capacity(n));
    let files = &scan.files;
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    let f = &files[i];
                    let p = root.join(&f.path);
                    let r = match hash_file(&p) {
                        Ok((size, d)) if size == f.size => Ok((i, d)),
                        Ok((size, _)) => {
                            Err(imp1(format!("{} changed while it was read ({} bytes, then {size})", f.path, f.size)))
                        }
                        Err(e) => Err(io_err("cannot read", &p, e)),
                    };
                    results.lock().unwrap().push(r);
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if d.is_multiple_of(50) || d == n {
                        progress(&format!("progress={d}/{n} files"));
                    }
                }
            });
        }
    });
    for r in results.into_inner().unwrap() {
        let (i, d) = r?;
        scan.files[i].sha256 = Some(d);
    }
    Ok(())
}

/// An edition the importer knows (§3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edition {
    pub key: String,
    /// The base set's display name.
    pub name: String,
    /// `eu` or `na` (manifest.md `[base] region`).
    pub region: String,
    /// Upper-cased path → (size, SHA-256).
    pub files: BTreeMap<String, (u64, [u8; 32])>,
}

/// How a scanned install compares with the known editions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Exactly this edition's content files.
    Known(Edition),
    /// Anything else; the files that differ from the closest edition (`MODIFIED`, `unknown` or `missing`), if any.
    Custom { differing: Vec<(String, &'static str)> },
}

impl Edition {
    /// Reads a manifest: one `<path>\t<size>\t<sha256>` line per content file (`#` lines are comments).
    pub fn parse(key: &str, name: &str, region: &str, text: &str) -> Result<Edition, String> {
        let mut files = BTreeMap::new();
        for (i, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let w: Vec<&str> = line.split('\t').collect();
            let bad = || format!("{key} manifest line {}: {line:?}", i + 1);
            if w.len() != 3 {
                return Err(bad());
            }
            let size = w[1].parse().map_err(|_| bad())?;
            let d = unhex32(w[2]).ok_or_else(bad)?;
            files.insert(w[0].to_ascii_uppercase(), (size, d));
        }
        Ok(Edition { key: key.to_string(), name: name.to_string(), region: region.to_string(), files })
    }

    /// The edition's identity (§3).
    pub fn identity(&self) -> [u8; 32] {
        identity_of(self.files.iter().map(|(p, (s, d))| (p.clone(), *s, *d)).collect())
    }

    /// The manifest of a scanned install, in [`Edition::parse`]'s form (original spelling).
    pub fn manifest_of(scan: &Scan) -> String {
        let mut out = String::from("# path\tsize\tsha256 (names, sizes and hashes only)\n");
        for f in &scan.files {
            out += &format!("{}\t{}\t{}\n", f.path, f.size, f.sha256.map(|d| hex(&d)).unwrap_or_default());
        }
        out
    }

    /// The verdict for a fingerprinted scan.
    pub fn lookup(scan: &Scan, editions: &[Edition]) -> Verdict {
        let id = scan.identity();
        if let Some(e) = editions.iter().find(|e| Some(e.identity()) == id) {
            return Verdict::Known(e.clone());
        }
        // The closest edition: the most files equal.
        let have: BTreeMap<String, (u64, Option<[u8; 32]>)> =
            scan.files.iter().map(|f| (f.path.to_ascii_uppercase(), (f.size, f.sha256))).collect();
        let score = |e: &Edition| e.files.iter().filter(|(p, (s, d))| have.get(*p) == Some(&(*s, Some(*d)))).count();
        let Some(best) = editions.iter().max_by_key(|e| score(e)) else {
            return Verdict::Custom { differing: Vec::new() };
        };
        let mut differing = Vec::new();
        for f in &scan.files {
            match best.files.get(&f.path.to_ascii_uppercase()) {
                Some((s, d)) if *s == f.size && Some(*d) == f.sha256 => {}
                Some(_) => differing.push((f.path.clone(), "MODIFIED")),
                None => differing.push((f.path.clone(), "unknown")),
            }
        }
        for p in best.files.keys() {
            if !have.contains_key(p) {
                differing.push((p.clone(), "missing"));
            }
        }
        Verdict::Custom { differing }
    }
}

impl Verdict {
    /// The edition key: the known edition's, or `custom`.
    pub fn key(&self) -> &str {
        match self {
            Verdict::Known(e) => &e.key,
            Verdict::Custom { .. } => "custom",
        }
    }
}

/// The base set's id (§3): `nfsu2-<key>`, or `nfsu2-custom-<8 hex digits of the identity>`.
pub fn base_id(verdict: &Verdict, identity: &[u8; 32]) -> String {
    match verdict {
        Verdict::Known(e) => format!("nfsu2-{}", e.key),
        Verdict::Custom { .. } => format!("nfsu2-custom-{}", &hex(identity)[..8]),
    }
}

/// The logical path of a stage-0 kept file in its bundle.
pub fn kept_path(original: &str) -> String {
    format!("residual/{original}.kept.bin")
}

/// The logical path of a file map.
pub fn map_path(original: &str) -> String {
    format!("residual/{original}.map.toml")
}

/// The import unit (= bundle name without `.obz`) of a content file (layout.md §2, §5; audio.md §2.1; text.md §2).
/// Originals several units read (`SDATA/sdat.viv`: speech and music) go in `global`, as layout.md §5 says.
pub fn unit_of(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let top = parts[0].to_ascii_uppercase();
    let name = *parts.last().unwrap();
    let upper = name.to_ascii_uppercase();
    let stem = |n: &str| n.split('.').next().unwrap_or(n).to_ascii_lowercase();
    match top.as_str() {
        "GLOBAL" | "SDATA" => "global".into(),
        "FRONTEND" => "frontend".into(),
        "NIS" => "scenes".into(),
        "MOVIES" | "SUBTITLES" => "movies".into(),
        "CARS" if parts.len() > 2 => format!("cars-{}", parts[1]),
        "CARS" => "cars-common".into(),
        "TRACKS" if parts.len() > 2 => match parts[1].to_ascii_uppercase().strip_prefix("ROUTES") {
            Some(r) if !r.is_empty() => format!("world-{}", &parts[1][6..]),
            _ => "global".into(),
        },
        "TRACKS" => {
            if upper.ends_with(".BUN") {
                let s = &name[..name.len() - 4];
                if upper.starts_with("STREAM") && s.len() > 6 {
                    format!("world-{}", &s[6..])
                } else {
                    format!("world-{s}")
                }
            } else {
                "global".into()
            }
        }
        "LANGUAGES" => {
            if upper == "LANGUAGETEXTURES.BIN" {
                "frontend".into()
            } else if let Some(ext) = upper.strip_prefix("AGREE").and_then(|r| r.rsplit('.').next()) {
                let lang = match ext {
                    "CHI" => "chinese",
                    "DUT" => "dutch",
                    "ENG" | "USA" => "english",
                    "FRE" => "french",
                    "GER" => "german",
                    "ITA" => "italian",
                    "KOR" => "korean",
                    "SPA" => "spanish",
                    "SWE" => "swedish",
                    _ => return "global".into(),
                };
                format!("text-{lang}")
            } else if upper.ends_with(".BIN") {
                format!("text-{}", stem(name))
            } else {
                "global".into()
            }
        }
        "CREDITS" => match upper.as_str() {
            "UK_ENGLISH.TXT" | "NA_ENGLISH.TXT" => "text-english".into(),
            _ => format!("text-{}", stem(name)),
        },
        "MEMCARD" => match upper.strip_prefix("LOCALE_") {
            Some(_) => format!("text-{}", stem(&name[7..])),
            None => "global".into(),
        },
        "SOUND" if parts.len() > 2 => match parts[1].to_ascii_uppercase().as_str() {
            "FE" | "GLOBAL" => "audio-frontend".into(),
            "IG_GLOBAL" | "SKIDS" | "NOS" | "TURBO" | "SHIFTING" => "audio-ingame".into(),
            "ENGINE" => "audio-engine".into(),
            _ => "audio-common".into(),
        },
        _ => "audio-common".into(),
    }
}

/// The units of a scan: unit name → indices of its files (in scan order, which is the bundle's data order).
pub fn units(scan: &Scan) -> BTreeMap<String, Vec<usize>> {
    let mut u: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, f) in scan.files.iter().enumerate() {
        u.entry(unit_of(&f.path)).or_default().push(i);
    }
    u
}

/// A stage-0 file map (residual.md §3): `container = "plain"`, one `kept` node over the whole file.
pub fn stage0_map(f: &SourceFile, content_digest: &Digest) -> String {
    let name = f.path.rsplit('/').next().unwrap_or(&f.path);
    format!(
        "schema = \"ob.residual-map/1.0\"\noriginal = {:?}\noriginal_size = {}\noriginal_digest = \"sha256:{}\"\n\
         container = \"plain\"\ncontent_size = {}\ncontent_digest = \"{content_digest}\"\n\n[[node]]\npath = \"file\"\n\
         offset = 0\nsize = {}\nfate = \"kept\"\nkept = {{ file = {:?}, offset = 0, count = {}, layout = \"raw_u8\" }}\n\
         reason = \"stage 0: kept whole until its kind graduates\"\n",
        f.path,
        f.size,
        hex(&f.sha256.unwrap_or_default()),
        f.size,
        f.size,
        format!("{name}.kept.bin"),
        f.size
    )
}

/// Options of one run.
pub struct Options<'a> {
    /// Re-import even when a complete base set of this id exists.
    pub force: bool,
    /// Threads for hashing and writing.
    pub threads: usize,
    /// The editions to recognise ([`editions`]).
    pub editions: Vec<Edition>,
    /// Free bytes on the disk holding a folder ([`ob_core::platform::fs::free_space`]; tests plant a value).
    pub free_space: fn(&Path) -> Option<u64>,
    /// Set from another thread to stop between units; the staging folder and its journal stay for a resume.
    pub stop: Option<&'a AtomicBool>,
    /// The `overbright` version written into `import.toml`.
    pub version: String,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Options {
            force: false,
            threads: std::thread::available_parallelism().map_or(4, |n| n.get()).min(8),
            editions: editions(),
            free_space: ob_core::platform::fs::free_space,
            stop: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A base set was written (or already complete): its id and folder.
    Done { id: String, dir: PathBuf, fresh: bool },
    /// Stopped on request; the staging folder holds the units done so far.
    Stopped { id: String, units_done: usize, units: usize },
}

struct Journal {
    done: BTreeMap<String, (u64, Digest)>,
}

fn journal_identity_line(identity: &[u8; 32]) -> String {
    format!("identity={} epoch={EPOCH} unit_version={UNIT_VERSION}", hex(identity))
}

fn read_journal(path: &Path, identity: &[u8; 32]) -> Option<Journal> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    if lines.next()? != JOURNAL_HEADER || lines.next()? != journal_identity_line(identity) {
        return None;
    }
    let mut done = BTreeMap::new();
    for l in lines {
        let w: Vec<&str> = l.split(' ').collect();
        // A torn last line (a stop mid-write) is ignored; its unit is redone.
        if w.len() == 4
            && w[0] == "unit"
            && let (Ok(size), Ok(d)) = (w[2].parse::<u64>(), Digest::parse(w[3]))
        {
            done.insert(w[1].to_string(), (size, d));
        }
    }
    Some(Journal { done })
}

/// A writer that hashes what passes through it (the bundle's `xxh3-128` digest).
struct Hashing<W: Write> {
    inner: W,
    h: xxhash_rust::xxh3::Xxh3,
    n: u64,
}

impl<W: Write> Write for Hashing<W> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let k = self.inner.write(b)?;
        self.h.update(&b[..k]);
        self.n += k as u64;
        Ok(k)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Writes one unit's bundle to `dir/<unit>.obz` through a temporary name; returns its size and digest.
fn write_unit(scan: &Scan, unit: &str, idx: &[usize], dir: &Path) -> Result<(u64, Digest), Refusal> {
    let mut maps = Vec::with_capacity(idx.len());
    let mut mapped = Vec::with_capacity(idx.len());
    for &i in idx {
        let f = &scan.files[i];
        let p = scan.root.join(&f.path);
        let file = std::fs::File::open(&p).map_err(|e| io_err("cannot read", &p, e))?;
        let len = file.metadata().map_err(|e| io_err("cannot read", &p, e))?.len();
        if len != f.size {
            return Err(imp1(format!("{} changed since it was read ({} bytes, now {len})", f.path, f.size)));
        }
        // An empty file cannot be mapped.
        let data: Box<dyn std::ops::Deref<Target = [u8]>> = if len == 0 {
            Box::new(Vec::new())
        } else {
            // SAFETY: a read-only map of a file the importer only reads; the read-back step re-hashes every byte
            // written, so a file changed underneath is caught there (IMP.1).
            Box::new(unsafe { memmap2::Mmap::map(&file) }.map_err(|e| io_err("cannot read", &p, e))?)
        };
        maps.push(stage0_map(f, &Alg::Xxh3_128.of(&data)));
        mapped.push(data);
    }
    let mut w = Writer::new(bundle::Kind::Bundle);
    for (k, &i) in idx.iter().enumerate() {
        let f = &scan.files[i];
        w.add(&kept_path(&f.path), &mapped[k][..], Method::Stored).map_err(|r| r.in_pack(unit))?;
        w.add(&map_path(&f.path), maps[k].as_bytes(), Method::Deflated).map_err(|r| r.in_pack(unit))?;
    }
    let target = dir.join(format!("{unit}.obz"));
    let tmp = dir.join(format!("{unit}.obz.tmp"));
    let out = std::fs::File::create(&tmp).map_err(|e| io_err("cannot write", &tmp, e))?;
    let mut hw =
        Hashing { inner: std::io::BufWriter::with_capacity(1 << 20, out), h: xxhash_rust::xxh3::Xxh3::new(), n: 0 };
    w.write_to(&mut hw).map_err(|r| r.in_pack(unit))?;
    let size = hw.n;
    let digest = Digest::Xxh3_128(hw.h.digest128().to_be_bytes());
    let out = hw.inner.into_inner().map_err(|e| io_err("cannot write", &tmp, e.into_error()))?;
    out.sync_all().map_err(|e| io_err("cannot write", &tmp, e))?;
    drop(out);
    ob_core::platform::fs::rename_replace(&tmp, &target).map_err(|e| io_err("cannot write", &target, e))?;
    Ok((size, digest))
}

/// Reads a unit's bundle back (§5 step 5): its size and digest as recorded, every CRC, and every kept file equal to
/// the original's SHA-256, its file map naming the same digest.
fn check_unit(
    scan: &Scan,
    unit: &str,
    idx: &[usize],
    path: &Path,
    want: Option<(u64, Digest)>,
) -> Result<(u64, Digest), Refusal> {
    let lay3 = |m: String| Refusal::new(Rule::Lay3, m).in_pack(unit);
    let b = Bundle::open(path, bundle::Kind::Bundle).map_err(|r| r.in_pack(unit))?;
    let got = (b.bytes().len() as u64, b.digest());
    if let Some(w) = want
        && w != got
    {
        return Err(lay3(format!(
            "{} is {} bytes, {}; the record says {} bytes, {}",
            path.display(),
            got.0,
            got.1,
            w.0,
            w.1
        )));
    }
    b.verify().map_err(|r| r.in_pack(unit))?;
    if b.entries().len() != idx.len() * 2 {
        return Err(lay3(format!("{} holds {} entries, not {}", path.display(), b.entries().len(), idx.len() * 2)));
    }
    for &i in idx {
        let f = &scan.files[i];
        let e = b
            .get(&kept_path(&f.path))
            .ok_or_else(|| lay3(format!("{} has no kept copy of {}", path.display(), f.path)))?;
        if !e.mapped() || e.size != f.size {
            return Err(lay3(format!(
                "{}: the kept copy of {} is not a stored aligned copy of {} bytes",
                path.display(),
                f.path,
                f.size
            )));
        }
        let data = b.read(e)?;
        let d: [u8; 32] = sha2::Sha256::digest(&data).into();
        if Some(d) != f.sha256 {
            return Err(Refusal::new(
                Rule::Imp4,
                format!(
                    "the kept copy of {} differs from the original (sha256 {}, expected {})",
                    f.path,
                    hex(&d),
                    hex(&f.sha256.unwrap_or_default())
                ),
            )
            .in_pack(unit));
        }
        let m = b
            .get(&map_path(&f.path))
            .ok_or_else(|| lay3(format!("{} has no file map of {}", path.display(), f.path)))?;
        let map = b.read(m)?;
        let want = format!("original_digest = \"sha256:{}\"", hex(&d));
        if !std::str::from_utf8(&map).is_ok_and(|t| t.contains(&want)) {
            return Err(
                Refusal::new(Rule::Imp4, format!("the file map of {} names another digest", f.path)).in_pack(unit)
            );
        }
    }
    Ok(got)
}

/// Runs `job` for every unit on `threads` threads; the first refusal wins. `stop` ends the run between units.
fn for_units<T: Send>(
    units: &[(&String, &Vec<usize>)],
    threads: usize,
    stop: Option<&AtomicBool>,
    job: &UnitJob<'_, T>,
) -> Result<Vec<(String, T)>, Refusal> {
    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let out: Mutex<Vec<(String, T)>> = Mutex::new(Vec::new());
    let err: Mutex<Option<Refusal>> = Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    if failed.load(Ordering::Relaxed) || stop.is_some_and(|f| f.load(Ordering::Relaxed)) {
                        break;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((name, idx)) = units.get(i) else { break };
                    match job(name, idx) {
                        Ok(t) => out.lock().unwrap().push(((*name).clone(), t)),
                        Err(r) => {
                            failed.store(true, Ordering::Relaxed);
                            err.lock().unwrap().get_or_insert(r);
                        }
                    }
                }
            });
        }
    });
    match err.into_inner().unwrap() {
        Some(r) => Err(r),
        None => Ok(out.into_inner().unwrap()),
    }
}

fn mib(b: u64) -> String {
    if b >= 1 << 30 { format!("{:.1} GB", b as f64 / 1e9) } else { format!("{:.0} MB", (b as f64 / 1e6).ceil()) }
}

/// The whole import (§5) of the game folder `game` into the base-set folder `base` (`<app>/base`).
pub fn import(game: &Path, base: &Path, opt: &Options, progress: Progress) -> Result<Outcome, Refusal> {
    progress("import=1");
    progress("step=1/6 scan");
    let mut scan = scan(game)?;
    progress(&format!("files={} bytes={}", scan.files.len(), scan.total()));
    progress("step=2/6 fingerprint");
    fingerprint(&mut scan, opt.threads, progress)?;
    let identity = scan.identity().expect("fingerprinted");
    let verdict = Edition::lookup(&scan, &opt.editions);
    let id = base_id(&verdict, &identity);
    progress(&format!(
        "edition={} {}",
        verdict.key(),
        if matches!(verdict, Verdict::Known(_)) { "known" } else { "custom" }
    ));
    if let Verdict::Custom { differing } = &verdict {
        for (p, why) in differing.iter().take(50) {
            progress(&format!("differs={p} {why}"));
        }
    }
    let final_dir = base.join(&id);
    if final_dir.join("import.toml").is_file() && !opt.force {
        progress(&format!("note=base set {id} is already complete (--force imports again)"));
        return Ok(Outcome::Done { id, dir: final_dir, fresh: false });
    }

    progress("step=3/6 plan");
    let units = units(&scan);
    let staging = base.join(format!(".staging-{id}"));
    let bundles = staging.join("bundles");
    let jpath = staging.join("journal.txt");
    let mut journal = match read_journal(&jpath, &identity) {
        Some(j) => j,
        None => {
            if staging.exists() {
                let r =
                    Refusal::new(Rule::Imp6, format!("{} is from another import; starting over", staging.display()));
                progress(&format!("note={r}"));
                std::fs::remove_dir_all(&staging).map_err(|e| io_err("cannot clear", &staging, e))?;
            }
            Journal { done: BTreeMap::new() }
        }
    };
    // A journalled unit counts as done only if its bundle is still there and whole.
    journal.done.retain(|u, (size, _)| {
        units.contains_key(u) && std::fs::metadata(bundles.join(format!("{u}.obz"))).is_ok_and(|m| m.len() == *size)
    });
    let todo: Vec<(&String, &Vec<usize>)> = units.iter().filter(|(u, _)| !journal.done.contains_key(*u)).collect();
    if !journal.done.is_empty() {
        progress(&format!("resume={} of {} units already done", journal.done.len(), units.len()));
    }

    // §10: the space check comes before anything is written.
    let need: u64 =
        todo.iter().flat_map(|(_, idx)| idx.iter()).map(|&i| scan.files[i].size + PER_FILE_OVERHEAD).sum::<u64>()
            + (1 << 20);
    progress(&format!("space_needed={need}"));
    let mut probe = base.to_path_buf();
    while !probe.exists() && probe.pop() {}
    if let Some(free) = (opt.free_space)(base)
        && free < need
    {
        return Err(Refusal::new(
            Rule::Imp2,
            format!("the import needs {}, {} free on {}", mib(need), mib(free), probe.display()),
        ));
    }

    std::fs::create_dir_all(&bundles).map_err(|e| io_err("cannot write", &bundles, e))?;
    if journal.done.is_empty() {
        let head = format!("{JOURNAL_HEADER}\n{}\n", journal_identity_line(&identity));
        ob_core::platform::fs::write_atomic(&jpath, head.as_bytes(), true)
            .map_err(|e| io_err("cannot write", &jpath, e))?;
    }

    progress("step=4/6 convert");
    let jfile = Mutex::new(
        std::fs::OpenOptions::new().append(true).open(&jpath).map_err(|e| io_err("cannot write", &jpath, e))?,
    );
    let written = AtomicU64::new(
        units
            .iter()
            .filter(|(u, _)| journal.done.contains_key(*u))
            .flat_map(|(_, i)| i.iter())
            .map(|&i| scan.files[i].size)
            .sum(),
    );
    let count = AtomicUsize::new(journal.done.len());
    let total = scan.total();
    let nunits = units.len();
    let results = for_units(&todo, opt.threads.min(4), opt.stop, &|unit, idx| {
        let (size, d) = write_unit(&scan, unit, idx, &bundles)?;
        {
            let mut j = jfile.lock().unwrap();
            j.write_all(format!("unit {unit} {size} {d}\n").as_bytes())
                .and_then(|_| j.sync_data())
                .map_err(|e| io_err("cannot write", &jpath, e))?;
        }
        written.fetch_add(idx.iter().map(|&i| scan.files[i].size).sum(), Ordering::Relaxed);
        let c = count.fetch_add(1, Ordering::Relaxed) + 1;
        progress(&format!("unit={unit} done"));
        progress(&format!("progress={c}/{nunits} units"));
        progress(&format!("bytes={}/{total}", written.load(Ordering::Relaxed)));
        Ok((size, d))
    })?;
    for (u, r) in results {
        journal.done.insert(u, r);
    }
    if opt.stop.is_some_and(|f| f.load(Ordering::Relaxed)) && journal.done.len() < nunits {
        progress("stopped=1");
        return Ok(Outcome::Stopped { id, units_done: journal.done.len(), units: nunits });
    }

    progress("step=5/6 verify");
    let all: Vec<(&String, &Vec<usize>)> = units.iter().collect();
    let checked = AtomicUsize::new(0);
    for_units(&all, opt.threads.min(4), None, &|unit, idx| {
        let want = journal.done.get(unit).copied();
        let r = check_unit(&scan, unit, idx, &bundles.join(format!("{unit}.obz")), want);
        let c = checked.fetch_add(1, Ordering::Relaxed) + 1;
        progress(&format!("progress={c}/{nunits} checked"));
        r
    })
    .inspect_err(|_| {
        // A bundle that fails its read-back is not trusted on the next run either.
        let _ = std::fs::remove_file(&jpath);
    })?;

    progress("step=6/6 finish");
    let langs = languages(&units, &scan);
    if langs.is_empty() {
        return Err(Refusal::new(
            Rule::Imp5,
            "the install has no string table (LANGUAGES/<Lang>.bin): no language to play in",
        ));
    }
    let pack = pack_toml(&id, &verdict, &langs);
    crate::manifest::Manifest::parse(pack.as_bytes(), crate::manifest::Container::Folder(&id))?;
    let record = import_toml(&scan, &verdict, &identity, &units, &journal.done, &opt.version);
    write(&staging.join("pack.toml"), pack.as_bytes())?;
    std::fs::remove_file(&jpath).map_err(|e| io_err("cannot write", &jpath, e))?;
    write(&staging.join("import.toml"), record.as_bytes())?;
    if final_dir.exists() {
        // `--force`: the old set moves aside and goes only once the new one is in place.
        let aside = base.join(format!(".replaced-{id}"));
        let _ = std::fs::remove_dir_all(&aside);
        std::fs::rename(&final_dir, &aside).map_err(|e| io_err("cannot replace", &final_dir, e))?;
        std::fs::rename(&staging, &final_dir).map_err(|e| io_err("cannot write", &final_dir, e))?;
        std::fs::remove_dir_all(&aside).map_err(|e| io_err("cannot remove", &aside, e))?;
    } else {
        std::fs::rename(&staging, &final_dir).map_err(|e| io_err("cannot write", &final_dir, e))?;
    }
    let active = base.join("active.txt");
    if !active.exists() {
        write(&active, crate::order::render_active(&id).as_bytes())?;
    }
    progress(&format!("base={id}"));
    Ok(Outcome::Done { id, dir: final_dir, fresh: true })
}

fn write(p: &Path, b: &[u8]) -> Result<(), Refusal> {
    ob_core::platform::fs::write_atomic(p, b, true).map_err(|e| io_err("cannot write", p, e))
}

fn languages(units: &BTreeMap<String, Vec<usize>>, scan: &Scan) -> Vec<String> {
    // Player languages: text units that hold a string table (`LANGUAGES/<Lang>.bin`), not the labels or layout test.
    let mut out: BTreeSet<String> = BTreeSet::new();
    for (u, idx) in units {
        if let Some(l) = u.strip_prefix("text-")
            && l != "labels"
            && l != "largest"
            && idx.iter().any(|&i| {
                let p = scan.files[i].path.to_ascii_uppercase();
                p.starts_with("LANGUAGES/") && p.ends_with(".BIN")
            })
        {
            out.insert(l.to_string());
        }
    }
    out.into_iter().collect()
}

/// `pack.toml` of an imported base set (manifest.md §2).
pub fn pack_toml(id: &str, verdict: &Verdict, languages: &[String]) -> String {
    let (edition, name, region) = match verdict {
        Verdict::Known(e) => (e.key.as_str(), e.name.clone(), e.region.as_str()),
        Verdict::Custom { .. } => ("custom", "Need for Speed Underground 2 (own install), imported".to_string(), "eu"),
    };
    format!(
        "format = 2\nkind = \"base\"\nid = \"{id}\"\nname = {name:?}\nversion = \"1.0.0\"\nlicence = \"LicenseRef-player-copy\"\n\
         overbright = \">=0.5\"\norigin = \"imported\"\nshare = \"local-only\"\n\n[base]\nedition = \"{edition}\"\nregion = \"{region}\"\n\
         languages = [{}]\nprofile = \"nfsu2/1\"\nimport = \"import.toml\"\nstorage = \"owned\"\n",
        languages.iter().map(|l| format!("{l:?}")).collect::<Vec<_>>().join(", ")
    )
}

fn now_utc() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = ((s / 86_400) as i64, s % 86_400);
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem / 60 % 60, rem % 60)
}

/// `import.toml` (§6). Names no folder of the player's machine.
fn import_toml(
    scan: &Scan,
    verdict: &Verdict,
    identity: &[u8; 32],
    units: &BTreeMap<String, Vec<usize>>,
    bundles: &BTreeMap<String, (u64, Digest)>,
    version: &str,
) -> String {
    let mut t = format!(
        "schema = \"ob.import/1.0\"\n\n[importer]\noverbright = \"{version}\"\nepoch = {EPOCH}\nfinished = \"{}\"\n\n[source]\n\
         edition = \"{}\"\nverdict = \"{}\"\nidentity = \"sha256:{}\"\nfile_count = {}\n",
        now_utc(),
        verdict.key(),
        if matches!(verdict, Verdict::Known(_)) { "known" } else { "custom" },
        hex(identity),
        scan.files.len()
    );
    if let Some((size, d)) = scan.exe {
        t += &format!("exe = {{ size = {size}, sha256 = \"sha256:{}\" }}\n", hex(&d));
    }
    for f in &scan.files {
        t += &format!(
            "\n[[source.file]]\npath = {:?}\nsize = {}\nsha256 = \"sha256:{}\"\nunit = \"{}\"\n",
            f.path,
            f.size,
            hex(&f.sha256.unwrap_or_default()),
            unit_of(&f.path)
        );
    }
    for (p, why) in &scan.skipped {
        t += &format!("\n[[source.skipped]]\npath = {p:?}\nreason = {why:?}\n");
    }
    for (u, (size, d)) in bundles {
        t += &format!("\n[[bundle]]\nname = \"{u}.obz\"\nsize = {size}\ndigest = \"{d}\"\n");
    }
    let mut kept_bytes = 0;
    for (u, idx) in units {
        let files: Vec<String> = idx.iter().map(|&i| format!("{:?}", scan.files[i].path)).collect();
        let entries: Vec<(&str, Digest)> = idx
            .iter()
            .map(|&i| (scan.files[i].path.as_str(), Digest::Sha256(scan.files[i].sha256.unwrap_or_default())))
            .collect();
        let model = crate::digest::list(Alg::Sha256, b"ob-import-kept/1\0", &entries);
        kept_bytes += idx.iter().map(|&i| scan.files[i].size).sum::<u64>();
        t += &format!(
            "\n[[unit]]\nname = \"{u}\"\nversion = {UNIT_VERSION}\nstate = \"kept\"\nfiles = [{}]\nitems = 0\nmodel = \"{model}\"\n\
             proof = {{ bytes = 0, patched = 0, values = 0, kept = {}, copy = 0, fill = 0, external = 0 }}\n",
            files.join(", "),
            idx.len()
        );
    }
    t += &format!(
        "\n[checks]\nread_back = \"pass\"\nrebuild = \"pass\"\nprofile = \"pass\"\n\n[residual]\nkept_bytes = {kept_bytes}\nshadow_bytes = 0\n"
    );
    t
}

/// A finished base set's record, as far as [`verify`] and [`repair`] need it.
pub struct Record {
    pub id: String,
    pub identity: [u8; 32],
    pub scan: Scan,
    pub bundles: BTreeMap<String, (u64, Digest)>,
}

/// Reads `<set>/import.toml`.
pub fn read_record(set: &Path) -> Result<Record, Refusal> {
    let p = set.join("import.toml");
    let text = std::fs::read_to_string(&p).map_err(|e| Refusal::new(Rule::Lay5, format!("{}: {e}", p.display())))?;
    let doc: toml::Table = text.parse().map_err(|e| Refusal::new(Rule::Lay3, format!("{}: {e}", p.display())))?;
    let bad = |what: &str| Refusal::new(Rule::Lay3, format!("{}: {what}", p.display()));
    let src = doc.get("source").and_then(|v| v.as_table()).ok_or_else(|| bad("no [source]"))?;
    let identity = src
        .get("identity")
        .and_then(|v| v.as_str())
        .and_then(|s| s.strip_prefix("sha256:"))
        .and_then(unhex32)
        .ok_or_else(|| bad("no source.identity"))?;
    let mut files = Vec::new();
    for f in src.get("file").and_then(|v| v.as_array()).ok_or_else(|| bad("no source.file"))? {
        let path = f.get("path").and_then(|v| v.as_str()).ok_or_else(|| bad("a file without path"))?.to_string();
        let size = f.get("size").and_then(|v| v.as_integer()).ok_or_else(|| bad("a file without size"))? as u64;
        let d = f.get("sha256").and_then(|v| v.as_str()).and_then(|s| s.strip_prefix("sha256:")).and_then(unhex32);
        files.push(SourceFile { path, size, sha256: Some(d.ok_or_else(|| bad("a file without sha256"))?) });
    }
    // What the scan left out (a linked folder, a name the format cannot hold): such names are not known absent.
    let skipped = src
        .get("skipped")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let path = s.get("path")?.as_str()?.to_string();
            Some((path, s.get("reason").and_then(|v| v.as_str()).unwrap_or_default().to_string()))
        })
        .collect();
    let mut bundles = BTreeMap::new();
    for b in doc.get("bundle").and_then(|v| v.as_array()).ok_or_else(|| bad("no bundle"))? {
        let name = b
            .get("name")
            .and_then(|v| v.as_str())
            .and_then(|s| s.strip_suffix(".obz"))
            .ok_or_else(|| bad("a bundle without name"))?;
        let size = b.get("size").and_then(|v| v.as_integer()).ok_or_else(|| bad("a bundle without size"))? as u64;
        let d = Digest::parse(b.get("digest").and_then(|v| v.as_str()).ok_or_else(|| bad("a bundle without digest"))?)?;
        bundles.insert(name.to_string(), (size, d));
    }
    let id = set.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(Record { id, identity, scan: Scan { root: PathBuf::new(), files, skipped, exe: None }, bundles })
}

/// Re-checks every bundle of a finished base set (`base verify`): size, digest, CRCs and every kept file's SHA-256.
/// Returns the damaged units with the reason; empty when the set is whole.
pub fn verify(set: &Path, threads: usize, progress: Progress) -> Result<Vec<(String, Refusal)>, Refusal> {
    let rec = read_record(set)?;
    let units = units(&rec.scan);
    let all: Vec<(&String, &Vec<usize>)> = units.iter().collect();
    let damaged = Mutex::new(Vec::new());
    for_units(&all, threads, None, &|unit, idx| {
        let r = check_unit(
            &rec.scan,
            unit,
            idx,
            &set.join("bundles").join(format!("{unit}.obz")),
            rec.bundles.get(unit).copied().or(Some((0, Alg::Xxh3_128.of(b"")))),
        );
        match r {
            Ok(_) => progress(&format!("unit={unit} ok")),
            Err(e) => {
                progress(&format!("unit={unit} damaged"));
                damaged.lock().unwrap().push((unit.to_string(), e));
            }
        }
        Ok(())
    })?;
    let mut d = damaged.into_inner().unwrap();
    d.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(d)
}

/// Rebuilds the damaged bundles of a base set from the game folder (§8 step 1), when the folder still holds the
/// same files (its identity matches the record). The rebuilt bundles are identical to the originals (the writer
/// is deterministic), so `import.toml` stays as it is.
pub fn repair(set: &Path, game: &Path, opt: &Options, progress: Progress) -> Result<Vec<String>, Refusal> {
    let damaged = verify(set, opt.threads, progress)?;
    if damaged.is_empty() {
        return Ok(Vec::new());
    }
    let rec = read_record(set)?;
    let mut scan = scan(game)?;
    fingerprint(&mut scan, opt.threads, progress)?;
    if scan.identity() != Some(rec.identity) {
        return Err(Refusal::new(
            Rule::Lay3,
            format!("{} no longer holds the files this base set was imported from; import again", game.display()),
        )
        .in_pack(&rec.id));
    }
    let units = units(&scan);
    let dir = set.join("bundles");
    let mut fixed = Vec::new();
    for (unit, _) in damaged {
        let idx = units.get(&unit).ok_or_else(|| Refusal::new(Rule::Lay3, format!("unknown unit {unit}")))?;
        write_unit(&scan, &unit, idx, &dir)?;
        check_unit(&scan, &unit, idx, &dir.join(format!("{unit}.obz")), rec.bundles.get(&unit).copied())?;
        progress(&format!("unit={unit} repaired"));
        fixed.push(unit);
    }
    Ok(fixed)
}

#[cfg(test)]
pub(crate) mod tests;

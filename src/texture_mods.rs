//! Texture mods set up in the launcher (docs/texture-packs.md, "TexMod
//! packages"): TexMod packages (`.tpf`, or a folder of loose TexMod files)
//! and TexWizard packs (a folder with `meta.json` and `textures.bin`, or a
//! ZIP of one) the player has, converted into Overbright's data folder, one
//! folder per package, and the list of them the game uses, in order (first
//! wins).
//! Overbright ships no texture art: with no list, or nothing on in it, the
//! game shows the original textures (the loader's path is unchanged).
//!
//! * A package lives in `<packs>/imported/<id>/` (`platform::dirs::packs`):
//!   `world/` and `car/` (the pack folders, docs/texture-packs.md),
//!   `import-tpf.tsv` or `import-texwizard.tsv` (the importer's report) and
//!   `package.txt` (what the launcher shows). The single `imported/{world,car}` an earlier import
//!   tool run wrote stays the World Texture Pack row's (not in the list).
//! * The list is `texture-mods.txt` beside game-root.txt: a header line,
//!   then `on <id>` / `off <id>` per package, first wins. Written
//!   atomically, as game-root.txt.
//! * An import writes into `imported/.import-<id>` and renames it into
//!   place only when it finished: a failed or interrupted import leaves no
//!   package (the next import or listing removes the leftover).
//! * The game reads the list when a world or a car loads
//!   ([`enabled_roots`]; at most every half second): a change takes effect
//!   at the next load (the next start, a new race, a garage visit), live
//!   with Reload Packs When Changed. `NFSU2_WORLD_TEXTURE_PACK` /
//!   `NFSU2_CAR_TEXTURE_PACK` still come first.
//!
//! The package and the game folder are only read. Nothing is downloaded.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};

/// The list file's first line.
pub const HEADER: &str = "# Overbright texture mods v1";

/// `texture-mods.txt` beside game-root.txt.
pub fn list_file() -> PathBuf {
    crate::qol::setup::saved_root_file().with_file_name("texture-mods.txt")
}

/// Where packages are kept: `<packs>/imported`.
pub fn packages_dir() -> PathBuf {
    crate::platform::dirs::packs().join("imported")
}

/// One line of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub on: bool,
}

/// A package id: lower-case letters, digits and `-`, 1..=64 long, not
/// `world` / `car` (the earlier single import's folders).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !matches!(id, "world" | "car")
}

/// The list's text read (unknown and invalid lines left out, each id once).
pub fn parse(text: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let Some((state, id)) = l.split_once(char::is_whitespace) else { continue };
        let id = id.trim();
        let on = match state {
            "on" => true,
            "off" => false,
            _ => continue,
        };
        if valid_id(id) && !out.iter().any(|e| e.id == id) {
            out.push(Entry { id: id.to_string(), on });
        }
    }
    out
}

pub fn format(list: &[Entry]) -> String {
    let mut s = format!("{HEADER}\n# first wins; written by the launcher\n");
    for e in list {
        s += &format!("{} {}\n", if e.on { "on" } else { "off" }, e.id);
    }
    s
}

/// The list as kept (empty when there is none).
pub fn load() -> Vec<Entry> {
    std::fs::read_to_string(list_file()).map(|t| parse(&t)).unwrap_or_default()
}

/// Keeps the list (atomic write; an empty list removes the file).
pub fn save(list: &[Entry]) -> std::io::Result<()> {
    let f = list_file();
    if list.is_empty() {
        return match std::fs::remove_file(&f) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    crate::qol::saves::write_atomic(&f, format(list).as_bytes(), false)
}

/// The folders of the packages that are on, in order, for the car or the
/// world renderer (only those that exist).
pub fn roots_in(list: &[Entry], packages: &Path, car: bool) -> Vec<PathBuf> {
    list.iter()
        .filter(|e| e.on)
        .map(|e| packages.join(&e.id).join(if car { "car" } else { "world" }))
        .filter(|d| d.is_dir())
        .collect()
}

/// [`roots_in`] for the kept list, read again at most every half second
/// (the loader asks for every texture).
pub fn enabled_roots(car: bool) -> Vec<PathBuf> {
    struct Seen {
        at: Instant,
        file: PathBuf,
        world: Vec<PathBuf>,
        car: Vec<PathBuf>,
    }
    static CACHE: Mutex<Option<Seen>> = Mutex::new(None);
    let file = list_file();
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let fresh = c.as_ref().is_some_and(|s| s.file == file && s.at.elapsed() < Duration::from_millis(500));
    if !fresh {
        let list = load();
        let dir = packages_dir();
        *c = Some(Seen {
            at: Instant::now(),
            file,
            world: roots_in(&list, &dir, false),
            car: roots_in(&list, &dir, true),
        });
    }
    let s = c.as_ref().expect("filled above");
    if car { s.car.clone() } else { s.world.clone() }
}

/// What the launcher shows about an installed package (`package.txt`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub id: String,
    /// The package's own comment (first line), else its file name.
    pub name: String,
    /// The file or folder it was imported from (its name only).
    pub source: String,
    /// What it was: `TexMod` or `TexWizard` (empty: an older import, TexMod).
    pub kind: String,
    pub images: usize,
    pub matched: usize,
    pub unmatched: usize,
    pub unsupported: usize,
    pub files: usize,
    /// Bytes on disk.
    pub bytes: u64,
}

impl Info {
    fn text(&self) -> String {
        format!(
            "name={}\nsource={}\nkind={}\nimages={}\nmatched={}\nunmatched={}\nunsupported={}\nfiles={}\nbytes={}\n",
            one_line(&self.name),
            one_line(&self.source),
            one_line(&self.kind),
            self.images,
            self.matched,
            self.unmatched,
            self.unsupported,
            self.files,
            self.bytes
        )
    }

    fn read(id: &str, text: &str) -> Info {
        let mut i = Info { id: id.to_string(), ..Default::default() };
        for l in text.lines() {
            let Some((k, v)) = l.split_once('=') else { continue };
            let n = || v.trim().parse().unwrap_or(0);
            match k.trim() {
                "name" => i.name = v.trim().to_string(),
                "source" => i.source = v.trim().to_string(),
                "kind" => i.kind = v.trim().to_string(),
                "images" => i.images = n(),
                "matched" => i.matched = n(),
                "unmatched" => i.unmatched = n(),
                "unsupported" => i.unsupported = n(),
                "files" => i.files = n(),
                "bytes" => i.bytes = v.trim().parse().unwrap_or(0),
                _ => {}
            }
        }
        if i.name.is_empty() {
            i.name = id.to_string();
        }
        i
    }
}

fn one_line(s: &str) -> String {
    s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(120).collect()
}

/// An installed package's description (None: no such package folder).
pub fn info(packages: &Path, id: &str) -> Option<Info> {
    let d = packages.join(id);
    d.is_dir().then(|| Info::read(id, &std::fs::read_to_string(d.join("package.txt")).unwrap_or_default()))
}

/// The list with each package's description, in order; packages listed
/// but gone are left out, package folders not listed are added at the
/// end (off). Leftovers of an unfinished import are removed.
pub fn installed(packages: &Path, list: &[Entry]) -> Vec<(Entry, Info)> {
    let mut out: Vec<(Entry, Info)> = list.iter().filter_map(|e| Some((e.clone(), info(packages, &e.id)?))).collect();
    if let Ok(rd) = std::fs::read_dir(packages) {
        let mut extra: Vec<String> = Vec::new();
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(".import-") {
                let _ = std::fs::remove_dir_all(e.path());
            } else if valid_id(&name) && e.path().join("package.txt").is_file() && !list.iter().any(|l| l.id == name) {
                extra.push(name);
            }
        }
        extra.sort();
        for id in extra {
            if let Some(i) = info(packages, &id) {
                out.push((Entry { id, on: false }, i));
            }
        }
    }
    out
}

/// A free id for a package named `name` (its file stem, lower case,
/// other characters as `-`).
pub fn new_id(packages: &Path, name: &str) -> String {
    let mut base: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    base.truncate(48);
    let base = base.trim_end_matches('-').to_string();
    let base = if valid_id(&base) { base } else { format!("package-{base}").trim_end_matches('-').to_string() };
    let mut id = base.clone();
    let mut n = 2;
    while packages.join(&id).exists() || packages.join(format!(".import-{id}")).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Free bytes on the disk holding `dir` (its nearest existing parent);
/// None when the system does not say.
pub fn free_space(dir: &Path) -> Option<u64> {
    let mut d = dir;
    while !d.exists() {
        d = d.parent()?;
    }
    free_space_of(d)
}

#[cfg(unix)]
fn free_space_of(d: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(d.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a NUL-terminated path, `s` a writable statvfs.
    (unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0).then(|| s.f_bavail as u64 * s.f_frsize as u64)
}

#[cfg(windows)]
fn free_space_of(d: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetDiskFreeSpaceExW(dir: *const u16, avail: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    }
    let w: Vec<u16> = d.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut avail = 0u64;
    // SAFETY: `w` is NUL-terminated; the out pointers may be null except `avail`.
    (unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, std::ptr::null_mut(), std::ptr::null_mut()) } != 0)
        .then_some(avail)
}

#[cfg(not(any(unix, windows)))]
fn free_space_of(_d: &Path) -> Option<u64> {
    None
}

/// A package read from the player's files.
pub enum Package {
    /// A TexMod package (`.tpf`, or a folder of loose TexMod files).
    TexMod(crate::formats::tpf::Package),
    /// A TexWizard pack (`formats::texwizard`).
    TexWizard(crate::formats::texwizard::Pack),
}

impl From<crate::formats::tpf::Package> for Package {
    fn from(p: crate::formats::tpf::Package) -> Package {
        Package::TexMod(p)
    }
}

impl Package {
    /// `TexMod` or `TexWizard`.
    pub fn kind(&self) -> &'static str {
        match self {
            Package::TexMod(_) => "TexMod",
            Package::TexWizard(_) => "TexWizard",
        }
    }

    /// The name the package gives itself (a TexMod comment's first line, a
    /// TexWizard pack's folder name; may be empty).
    pub fn own_name(&self) -> String {
        match self {
            Package::TexMod(p) => one_line(&p.comment),
            Package::TexWizard(p) => one_line(&p.name),
        }
    }

    fn import(
        &self,
        sources: &[crate::tpf_import::Source],
        out: &Path,
        opts: &crate::tpf_import::ImportOptions,
        progress: &(dyn Fn(usize, usize) + Sync),
    ) -> Result<crate::tpf_import::ImportSummary> {
        match self {
            Package::TexMod(p) => crate::tpf_import::import_package(p, sources, out, opts, progress),
            Package::TexWizard(p) => crate::texwizard_import::import_pack(p, sources, out, opts, progress),
        }
    }
}

/// The space an import of `package` may take: its images' bytes, twice
/// (completed mip chains, PNGs of uncompressed art), plus 16 MB.
pub fn space_needed(package: &Package) -> u64 {
    let images = match package {
        Package::TexMod(p) => p.entries.iter().map(|e| e.data.len() as u64).sum::<u64>(),
        Package::TexWizard(p) => p.bin_bytes,
    };
    images * 2 + (16 << 20)
}

/// What one import did, in plain words for the launcher.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// The package's id when it was installed.
    pub id: Option<String>,
    pub name: String,
    /// The result line (what matched, what not, why nothing).
    pub line: String,
    pub ok: bool,
}

/// Reads a package from a `.tpf` file, a folder of loose TexMod files or a
/// TexWizard pack folder (a ZIP of TexWizard packs: [`read_sources`]).
pub fn read_source(path: &Path) -> Result<Package> {
    if path.is_dir() {
        if crate::formats::texwizard::is_pack_dir(path) {
            return Ok(Package::TexWizard(crate::formats::texwizard::read_dir(path)?));
        }
        Ok(Package::TexMod(crate::tpf_import::read_folder(path).map_err(|e| {
            e.context("not a TexMod folder (texmod.def) nor a TexWizard pack (meta.json and textures.bin)")
        })?))
    } else if is_zip_name(path) {
        let mut v = read_sources(path)?;
        ensure!(v.len() == 1, "{}: {} TexWizard packs in one archive", path.display(), v.len());
        Ok(v.remove(0).1)
    } else {
        let raw = std::fs::read(path).with_context(|| path.display().to_string())?;
        Ok(Package::TexMod(crate::formats::tpf::read(&raw)?))
    }
}

fn is_zip_name(path: &Path) -> bool {
    path.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip"))
}

/// The packages in `path`, each with the name it is reported under: a ZIP
/// gives each TexWizard pack in it (`archive.zip: Pack`; the archive's
/// name alone when it holds one), anything else what [`read_source`] reads.
pub fn read_sources(path: &Path) -> Result<Vec<(String, Package)>> {
    let file = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    if !(path.is_file() && is_zip_name(path)) {
        return Ok(vec![(file, read_source(path)?)]);
    }
    let zip = crate::formats::zip::Zip::open(path)?;
    let packs = crate::formats::texwizard::zip_packs(&zip);
    ensure!(!packs.is_empty(), "{file}: no TexWizard pack in this archive (no meta.json with a textures.bin)");
    let stem = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let n = packs.len();
    packs
        .into_iter()
        .map(|(dir, name)| {
            let name = if name.is_empty() { stem.clone() } else { name };
            let pack = crate::formats::texwizard::read_zip_pack(&zip, &dir, &name)
                .with_context(|| format!("{file}: {}", if dir.is_empty() { "the pack" } else { &dir }))?;
            Ok((if n == 1 { file.clone() } else { format!("{file}: {name}") }, Package::TexWizard(pack)))
        })
        .collect()
}

/// A ZIP archive that holds a TexWizard pack (its directory only is read).
pub fn zip_has_texwizard_pack(path: &Path) -> bool {
    crate::formats::zip::Zip::open(path).is_ok_and(|z| !crate::formats::texwizard::zip_packs(&z).is_empty())
}

/// Imports `package` (read from `source`) for the game textures `sources`
/// into `packages/<new id>` (a temporary folder renamed into place when it
/// finished). Returns the installed package's description and the
/// summary. Nothing is installed when no image matched.
pub fn install(
    package: &Package,
    source: &Path,
    sources: &[crate::tpf_import::Source],
    packages: &Path,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<(Option<Info>, crate::tpf_import::ImportSummary)> {
    let file = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let own = package.own_name();
    let stem = match package {
        // A TexWizard pack is named by its folder (in a ZIP: its folder there).
        Package::TexWizard(_) if !own.is_empty() => own.clone(),
        _ if source.is_dir() => file.clone(),
        _ => source.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| file.clone()),
    };
    std::fs::create_dir_all(packages).with_context(|| packages.display().to_string())?;
    let need = space_needed(package);
    if let Some(free) = free_space(packages)
        && free < need
    {
        bail!(
            "not enough free disk space: the import needs about {} MB, {} MB are free in {}",
            need >> 20,
            free >> 20,
            packages.display()
        );
    }
    let id = new_id(packages, &stem);
    let tmp = packages.join(format!(".import-{id}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let opts = crate::tpf_import::ImportOptions::default();
    let r = package.import(sources, &tmp, &opts, progress);
    let sum = match r {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    if sum.files_written() == 0 {
        let _ = std::fs::remove_dir_all(&tmp);
        return Ok((None, sum));
    }
    let info = Info {
        id: id.clone(),
        name: if own.is_empty() { stem } else { own },
        source: file,
        kind: package.kind().into(),
        images: sum.images,
        matched: sum.matched,
        unmatched: sum.unmatched,
        unsupported: sum.unsupported,
        files: sum.files_written(),
        bytes: sum.bytes,
    };
    let done = (|| -> Result<()> {
        std::fs::write(tmp.join("package.txt"), info.text())?;
        std::fs::rename(&tmp, packages.join(&id))?;
        Ok(())
    })();
    if let Err(e) = done {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    Ok((Some(info), sum))
}

/// The result of an import in plain words.
pub fn outcome_line(name: &str, r: &Result<(Option<Info>, crate::tpf_import::ImportSummary)>) -> Outcome {
    match r {
        Err(e) => Outcome { id: None, name: name.into(), line: format!("{name}: not imported: {e:#}"), ok: false },
        Ok((info, s)) => {
            let ok = info.is_some();
            let line = if s.images == 0 {
                format!("{name}: not imported: it holds no texture images")
            } else if let Some(p) = s.other_game() {
                format!("{name}: not imported: it was made for another game ({p}), not Need for Speed Underground 2")
            } else if s.matched == 0 {
                format!(
                    "{name}: not imported: none of its {} textures is in your game's files (made for another game or another version?)",
                    s.images
                )
            } else if !ok {
                format!(
                    "{name}: nothing to install: its {} matching textures are the same as the game's own or could not be read",
                    s.matched
                )
            } else {
                let mut l = format!("{name}: {} of {} textures replace the game's", s.matched, s.images);
                if s.unmatched > 0 {
                    l += &format!("; {} not found in your game's files (left out)", s.unmatched);
                }
                if s.unsupported > 0 {
                    l += &format!("; {} in a format Overbright cannot read (left out)", s.unsupported);
                }
                if s.missing > 0 {
                    l += &format!("; {} name a texture the pack does not hold (left out)", s.missing);
                }
                l
            };
            Outcome { id: info.as_ref().map(|i| i.id.clone()), name: name.into(), line, ok }
        }
    }
}

/// Removes an installed package's folder (only Overbright's converted
/// copy; the player's own package file is never touched) and its line.
pub fn remove(packages: &Path, list: &mut Vec<Entry>, id: &str) -> Result<()> {
    ensure!(valid_id(id), "not a package id: {id}");
    let d = packages.join(id);
    if d.exists() {
        // Only a folder this module made: it holds package.txt.
        ensure!(d.join("package.txt").is_file(), "{} is not an imported package", d.display());
        std::fs::remove_dir_all(&d).with_context(|| d.display().to_string())?;
    }
    list.retain(|e| e.id != id);
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;

//! The HUD mod set up from what a player downloads (docs/modding/texture-packs.md,
//! "The HUD mod"): the game's HD UI + Maps pack (`hd_ui_pack`) is two
//! third-party mods made for the original's "Binary" mod tool, converted
//! into one folder of PNGs keyed by the texture's bin hash:
//!
//! * the **HUD part** (a 7-Zip archive; this module reads its extracted
//!   folder, or the same files in a ZIP): `assets/userstart.end`, whose
//!   `bind_textures synchronize <file> TPKBlocks <collection> "<folder>"`
//!   lines name the folders of DDS images that replace the game's
//!   textures (the optional `UnlimiterTextures` are for another mod and
//!   left out);
//! * the **map part** (a ZIP, LZMA-compressed, or its extracted
//!   folder): `Binary/script.end`, whose `replace_texture` lines name one
//!   DDS per track map (the "Default" choice; "Beta" is left out) and whose
//!   `update_collection GLOBAL\GLOBALB.LZC Tracks <n> <field> <value>`
//!   lines become the map calibration (`map-overrides.tsv`).
//!
//! The scripts are parsed as data and never run; nothing is written into
//! the game folder or beside the downloads. The pack is written into
//! `<data>/overbright/texture-pack/.hud-mod-import` and renamed to
//! `hud-mod` when it is finished (an older one replaced only then); a
//! failed or interrupted conversion leaves the older pack as it was.
//! Pixels are kept exactly (the DDS's top level decoded to RGBA, saved as
//! PNG, read back and compared). The output matches the maintainers'
//! Pillow-based converters (`scripts/import_hud_pack.py`,
//! `scripts/import_map_pack.py`, kept outside the release as an
//! independent cross-check): the same file names, the same decoded pixels,
//! the same calibration rows.
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, anyhow, bail, ensure};
use rayon::prelude::*;

use crate::formats::hash::bin_hash;

/// The converted pack's folder name under `<data>/overbright/texture-pack`.
pub const PACK_DIR: &str = "hud-mod";
/// The file that marks a folder this module made (removed only then).
pub const MARKER: &str = "hud-mod.txt";
/// The images of the version of the two mods Overbright knows (the game's
/// own completeness check, `hd_ui_pack`, wants exactly these counts).
pub const KNOWN_HUD_IMAGES: usize = 595;
pub const KNOWN_MAP_IMAGES: usize = 115;

/// Where the launcher puts the converted pack.
pub fn pack_dir() -> PathBuf {
    crate::platform::dirs::packs().join(PACK_DIR)
}

// ------------------------------------------------------------- sources

/// Files of a download: an extracted folder or a ZIP archive (with an
/// optional folder prefix inside it). Paths are `/`-separated, relative to
/// the mod's root, looked up without case.
#[derive(Clone)]
pub enum Source {
    Dir(PathBuf),
    Zip { path: PathBuf, zip: Arc<crate::formats::zip::Zip>, prefix: String },
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path().display())
    }
}

impl PartialEq for Source {
    fn eq(&self, o: &Source) -> bool {
        self.path() == o.path()
    }
}

/// `dir/rel` with each part matched without case (an extracted archive
/// keeps the author's case; Windows ignores it, Linux does not).
fn resolve_ci(dir: &Path, rel: &str) -> Option<PathBuf> {
    let mut p = dir.to_path_buf();
    for part in rel.split(['/', '\\']).filter(|s| !s.is_empty()) {
        let exact = p.join(part);
        if exact.exists() {
            p = exact;
            continue;
        }
        let found = std::fs::read_dir(&p)
            .ok()?
            .flatten()
            .find(|e| e.file_name().to_str().is_some_and(|n| n.eq_ignore_ascii_case(part)))?;
        p = found.path();
    }
    Some(p)
}

impl Source {
    /// The folder or the archive the player chose.
    pub fn path(&self) -> &Path {
        match self {
            Source::Dir(p) | Source::Zip { path: p, .. } => p,
        }
    }

    pub fn is_file(&self, rel: &str) -> bool {
        match self {
            Source::Dir(d) => resolve_ci(d, rel).is_some_and(|p| p.is_file()),
            Source::Zip { zip, prefix, .. } => zip.find(&format!("{prefix}{rel}")).is_some(),
        }
    }

    pub fn read(&self, rel: &str) -> Result<Vec<u8>> {
        match self {
            Source::Dir(d) => {
                let p = resolve_ci(d, rel).filter(|p| p.is_file()).with_context(|| format!("{rel} is missing"))?;
                std::fs::read(&p).with_context(|| p.display().to_string())
            }
            Source::Zip { zip, prefix, .. } => {
                let e = zip.find(&format!("{prefix}{rel}")).with_context(|| format!("{rel} is missing"))?;
                zip.read(e)
            }
        }
    }

    /// A file's size in bytes (0 when it is not there).
    pub fn size(&self, rel: &str) -> u64 {
        match self {
            Source::Dir(d) => resolve_ci(d, rel).and_then(|p| p.metadata().ok()).map_or(0, |m| m.len()),
            Source::Zip { zip, prefix, .. } => zip.find(&format!("{prefix}{rel}")).map_or(0, |e| e.size as u64),
        }
    }

    /// The files directly in `rel_dir` (their names as stored), sorted;
    /// None when the folder is not there.
    pub fn files_in(&self, rel_dir: &str) -> Option<Vec<String>> {
        let mut v = match self {
            Source::Dir(d) => {
                let p = resolve_ci(d, rel_dir).filter(|p| p.is_dir())?;
                std::fs::read_dir(p)
                    .ok()?
                    .flatten()
                    .filter(|e| e.path().is_file())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect::<Vec<_>>()
            }
            Source::Zip { zip, prefix, .. } => {
                let want = format!("{prefix}{}/", rel_dir.trim_end_matches('/')).to_ascii_lowercase();
                let v: Vec<String> = zip
                    .entries
                    .iter()
                    .filter_map(|e| {
                        let low = e.name.to_ascii_lowercase();
                        let rest = low.strip_prefix(&want)?;
                        (!rest.contains('/')).then(|| e.name[want.len()..].to_string())
                    })
                    .collect();
                let dir_known = zip.entries.iter().any(|e| e.name.to_ascii_lowercase().starts_with(&want));
                if !dir_known {
                    return None;
                }
                v
            }
        };
        v.sort();
        Some(v)
    }

    /// Every file under `rel_dir`, as paths relative to the mod's root.
    fn files_under(&self, rel_dir: &str) -> Vec<String> {
        let mut out = Vec::new();
        match self {
            Source::Dir(d) => {
                fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
                    let Ok(rd) = std::fs::read_dir(dir) else { return };
                    for e in rd.flatten() {
                        let Ok(name) = e.file_name().into_string() else { continue };
                        let r = format!("{rel}/{name}");
                        if e.path().is_dir() {
                            walk(&e.path(), &r, out);
                        } else {
                            out.push(r);
                        }
                    }
                }
                if let Some(p) = resolve_ci(d, rel_dir) {
                    walk(&p, rel_dir, &mut out);
                }
            }
            Source::Zip { zip, prefix, .. } => {
                let want = format!("{prefix}{rel_dir}/").to_ascii_lowercase();
                out.extend(
                    zip.entries
                        .iter()
                        .filter(|e| e.name.to_ascii_lowercase().starts_with(&want))
                        .map(|e| e.name[prefix.len()..].to_string()),
                );
            }
        }
        out.sort();
        out
    }
}

/// The installer script that marks each mod.
const HUD_SCRIPT: &str = "assets/userstart.end";
const MAP_SCRIPT: &str = "Binary/script.end";

/// What a chosen file or folder is.
#[derive(Clone, Debug, PartialEq)]
pub enum Found {
    /// The HUD part (its root: the folder that holds `assets`).
    Hud(Source),
    /// The map part (its root: the folder that holds `Binary`).
    Map(Source),
    /// An archive Overbright does not read (7-Zip, RAR): the words for the
    /// player.
    Unreadable(String),
    /// Something else.
    Other,
}

fn ext_of(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase()
}

/// What `path` is (reads only a folder's names or an archive's directory).
pub fn identify(path: &Path) -> Found {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if path.is_dir() {
        let dir = Source::Dir(path.to_path_buf());
        if dir.is_file(HUD_SCRIPT) {
            return Found::Hud(dir);
        }
        if dir.is_file(MAP_SCRIPT) {
            return Found::Map(dir);
        }
        // The `assets` or `Binary` folder itself: its parent is the mod.
        if let Some(parent) = path.parent() {
            if name.eq_ignore_ascii_case("assets") && Source::Dir(path.to_path_buf()).is_file("userstart.end") {
                return Found::Hud(Source::Dir(parent.to_path_buf()));
            }
            if name.eq_ignore_ascii_case("binary") && Source::Dir(path.to_path_buf()).is_file("script.end") {
                return Found::Map(Source::Dir(parent.to_path_buf()));
            }
        }
        return Found::Other;
    }
    match ext_of(path).as_str() {
        "zip" => match crate::formats::zip::Zip::open(path) {
            Ok(zip) => {
                let zip = Arc::new(zip);
                // The script at the root, or under one folder.
                let root_of = |script: &str| -> Option<String> {
                    let low = script.to_ascii_lowercase();
                    zip.entries.iter().find_map(|e| {
                        let n = e.name.to_ascii_lowercase();
                        let prefix = n.strip_suffix(&low)?;
                        (prefix.is_empty() || (prefix.ends_with('/') && prefix.matches('/').count() == 1))
                            .then(|| e.name[..prefix.len()].to_string())
                    })
                };
                let src = |prefix| Source::Zip { path: path.to_path_buf(), zip: zip.clone(), prefix };
                if let Some(p) = root_of(HUD_SCRIPT) {
                    Found::Hud(src(p))
                } else if let Some(p) = root_of(MAP_SCRIPT) {
                    Found::Map(src(p))
                } else {
                    Found::Other
                }
            }
            Err(_) => Found::Other,
        },
        "7z" => Found::Unreadable(format!(
            "“{name}” is a 7-Zip archive, which Overbright does not open: extract it first (with 7-Zip, or your \
             file manager), then choose the extracted folder."
        )),
        "rar" => Found::Unreadable(format!(
            "“{name}” is a RAR archive, which Overbright does not open: extract it first, then choose the \
             extracted folder."
        )),
        _ => Found::Other,
    }
}

/// The two parts found so far, and what was chosen that is neither.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parts {
    pub hud: Option<Source>,
    pub map: Option<Source>,
    /// Lines for things chosen that cannot be used.
    pub notes: Vec<String>,
}

impl Parts {
    pub fn complete(&self) -> bool {
        self.hud.is_some() && self.map.is_some()
    }

    fn take(&mut self, f: Found) -> bool {
        match f {
            Found::Hud(s) if self.hud.is_none() => self.hud = Some(s),
            Found::Map(s) if self.map.is_none() => self.map = Some(s),
            _ => return false,
        }
        true
    }
}

/// The folders and archives in `dir` the HUD mod may be in.
fn candidates(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() || matches!(ext_of(p).as_str(), "zip" | "7z" | "rar"))
        .take(1000)
        .collect();
    v.sort();
    v
}

/// What the player chose (`path`) added to `parts`: the file or folder
/// itself, else what is directly in it (a folder that holds both
/// downloads); once a part is known, the other one is also looked for
/// beside it (both downloads in one folder, an archive beside its
/// extracted folder).
pub fn add(parts: &mut Parts, path: &Path) {
    parts.notes.clear();
    let mut unreadable = None;
    let mut took = false;
    match identify(path) {
        Found::Other if path.is_dir() => {
            for c in candidates(path) {
                match identify(&c) {
                    Found::Unreadable(t) => unreadable = unreadable.or(Some(t)),
                    f => took |= parts.take(f),
                }
            }
        }
        Found::Unreadable(t) => unreadable = Some(t),
        f => took |= parts.take(f),
    }
    // The other part beside what was found (or beside the 7-Zip file: its
    // extracted folder).
    if !parts.complete()
        && (took || unreadable.is_some())
        && let Some(parent) = path.parent()
    {
        for c in candidates(parent) {
            if c != path && !matches!(identify(&c), Found::Unreadable(_)) {
                let f = identify(&c);
                took |= parts.take(f);
            }
        }
        if took && unreadable.is_some() && parts.hud.is_some() {
            unreadable = None;
        }
    }
    if let Some(t) = unreadable {
        parts.notes.push(t);
    } else if !took && parts.hud.is_none() && parts.map.is_none() {
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        parts.notes.push(format!(
            "“{name}” is not the HUD mod: it holds neither its HUD part (assets/userstart.end) nor its \
             map part (Binary/script.end)."
        ));
    }
}

// ------------------------------------------------------------- scripts

/// One `bind_textures` folder of the HUD mod.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    /// Under `assets/`, `/`-separated.
    pub folder: String,
    pub game_file: String,
    pub collection: String,
}

/// A relative script path made safe: no root, no drive, no `..`.
fn safe_rel(path: &str) -> Option<Vec<&str>> {
    let parts: Vec<&str> = path.split(['\\', '/']).filter(|s| !s.is_empty()).collect();
    let bad = path.starts_with(['\\', '/']) || path.contains(':') || parts.is_empty() || parts.contains(&"..");
    (!bad).then_some(parts)
}

/// The HUD mod's `bind_textures` folders (UnlimiterTextures left out).
pub fn parse_hud_script(text: &str) -> Result<Vec<Category>> {
    let mut out: Vec<Category> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        let Some(rest) = line.strip_prefix("bind_textures") else { continue };
        let Some(q) = rest.find('"') else { continue };
        let words: Vec<&str> = rest[..q].split_whitespace().collect();
        let quoted = &rest[q + 1..];
        let Some(end) = quoted.find('"') else { continue };
        if words.len() != 4
            || words[0] != "synchronize"
            || words[2] != "TPKBlocks"
            || !quoted[end + 1..].trim().is_empty()
        {
            continue;
        }
        let folder = &quoted[..end];
        let parts = safe_rel(folder).with_context(|| format!("line {}: an unsafe folder {folder:?}", n + 1))?;
        if parts[0].eq_ignore_ascii_case("UnlimiterTextures") {
            continue;
        }
        let c = Category { folder: parts.join("/"), game_file: words[1].into(), collection: words[3].into() };
        match out.iter().find(|o| o.folder.eq_ignore_ascii_case(&c.folder)) {
            Some(o) if o != &c => bail!("line {}: the folder {} is bound twice differently", n + 1, c.folder),
            Some(_) => {}
            None => out.push(c),
        }
    }
    ensure!(!out.is_empty(), "its installer binds no texture folders");
    Ok(out)
}

/// The map calibration fields a script may set (`frontend::map_pack`).
const MAP_FIELDS: [&str; 6] = [
    "MenuMapZoomOffsetX",
    "MenuMapZoomOffsetY",
    "MenuMapZoomWidth",
    "TrackMapCalibrationOffsetX",
    "TrackMapCalibrationOffsetY",
    "TrackMapCalibrationZoomIn",
];

/// The map part's script read: the track maps (texture name → path
/// under `Binary/`) and the calibration ((track, field) → value as
/// written). Anything the script does that is not known is refused.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapScript {
    pub textures: BTreeMap<String, String>,
    pub rows: BTreeMap<(u32, String), String>,
}

/// `TRACKMAP` + digits.
fn trackmap(s: &str) -> bool {
    s.strip_prefix("TRACKMAP").is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
}

/// `TRACKS\TRACKMAPnnnn.BIN` → `TRACKMAPnnnn`.
fn trackmap_file(s: &str) -> Option<&str> {
    let name = s.strip_prefix("TRACKS\\")?.strip_suffix(".BIN")?;
    trackmap(name).then_some(name)
}

pub fn parse_map_script(text: &str) -> Result<MapScript> {
    let mut m = MapScript::default();
    let mut branch = "common";
    let (mut saw_default, mut saw_beta) = (false, false);
    let mut guard: Option<String> = None;
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with("//") || line == "[VERSN2]" {
            continue;
        }
        if line.starts_with("combobox ") {
            ensure!(line == "combobox \"Default\" \"Beta\" \"Select map version\"", "line {n}: an unknown choice");
            continue;
        }
        if line == "\"Default\"" || line == "\"Beta\"" {
            branch = if line == "\"Default\"" { "Default" } else { "Beta" };
            saw_default |= branch == "Default";
            saw_beta |= branch == "Beta";
            continue;
        }
        if branch == "Beta" {
            continue;
        }
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.first().copied() {
            Some("replace_texture") => {
                // replace_texture TRACKS\TRACKMAPn.BIN TPKBlocks <TRACKMAPn|DEFAULT> TRACKMAPn "path"
                let q = line.find('"').with_context(|| format!("line {n}: no path"))?;
                let head: Vec<&str> = line[..q].split_whitespace().collect();
                let path = line[q..].trim().strip_prefix('"').and_then(|s| s.strip_suffix('"'));
                let ok = head.len() == 5
                    && head[2] == "TPKBlocks"
                    && trackmap_file(head[1]).is_some_and(|f| f == head[4])
                    && (head[3] == head[4] || head[3] == "DEFAULT");
                let (Some(path), true) = (path, ok) else { bail!("line {n}: an unknown replace_texture") };
                let name = head[4];
                if let Some(g) = &guard {
                    ensure!(g == name, "line {n}: {name} under the condition for {g}");
                }
                let parts = safe_rel(path).with_context(|| format!("line {n}: an unsafe path"))?;
                let allowed: &[&str] = if branch == "common" { &["Map"] } else { &["Map", "Default"] };
                let (file, dirs) = parts.split_last().expect("not empty");
                let stem = file.rsplit_once('.').filter(|(_, e)| e.eq_ignore_ascii_case("dds")).map(|(s, _)| s);
                ensure!(dirs == allowed && stem == Some(name), "line {n}: an unexpected map path {path}");
                let rel = parts.join("/");
                if let Some(old) = m.textures.insert(name.to_string(), rel.clone()) {
                    ensure!(old == rel, "line {n}: {name} named twice with different files");
                }
            }
            Some("update_collection") => {
                ensure!(
                    w.len() == 6 && w[1] == "GLOBAL\\GLOBALB.LZC" && w[2] == "Tracks" && branch == "common",
                    "line {n}: an unknown update_collection"
                );
                let track: u32 = w[3].parse().with_context(|| format!("line {n}: track"))?;
                let (field, value) = (w[4], w[5]);
                ensure!(
                    MAP_FIELDS.contains(&field) && value.parse::<f64>().is_ok_and(f64::is_finite),
                    "line {n}: an unknown field or value"
                );
                if let Some(old) = m.rows.insert((track, field.to_string()), value.to_string()) {
                    ensure!(old == value, "line {n}: {track} {field} set twice differently");
                }
            }
            Some("if") => {
                let ok = w.len() == 5
                    && w[1] == "collection_exists"
                    && w[3] == "TPKBlocks"
                    && trackmap_file(w[2]).is_some_and(|f| f == w[4]);
                ensure!(ok, "line {n}: an unknown condition");
                guard = Some(w[4].to_string());
            }
            Some("do" | "else") if w.len() == 1 => {}
            Some("end") if w.len() == 1 => guard = None,
            _ => bail!("line {n}: an unknown instruction"),
        }
    }
    ensure!(saw_default && saw_beta && !m.textures.is_empty(), "no Default / Beta map choice and maps");
    Ok(m)
}

// ---------------------------------------------------------- conversion

/// Why a conversion did not set the mod up.
#[derive(Debug)]
pub enum Problem {
    /// What was chosen is not this mod (the words for the player).
    NotTheMod(String),
    /// Files the scripts name are missing or unreadable (relative paths).
    Missing(Vec<String>),
    Other(anyhow::Error),
}

impl From<anyhow::Error> for Problem {
    fn from(e: anyhow::Error) -> Problem {
        Problem::Other(e)
    }
}

/// What a finished conversion holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub dir: PathBuf,
    /// DDS files of the HUD and the PNGs they became (identical images
    /// sharing a name are one PNG).
    pub hud_sources: usize,
    pub hud_images: usize,
    pub map_images: usize,
    pub rows: usize,
    pub tracks: usize,
    /// The game's own check of the result.
    pub check: crate::hd_ui_pack::Inspect,
}

/// One PNG to write: its hash, the sources (relative paths) that must
/// all decode to the same pixels.
struct Job {
    hash: u32,
    name: String,
    part: usize,
    sources: Vec<String>,
}

/// The space a conversion may take: half the DDS bytes (the PNGs are
/// smaller than that) plus 64 MB.
pub fn space_needed(dds_bytes: u64) -> u64 {
    dds_bytes / 2 + (64 << 20)
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o += "\\\"",
            '\\' => o += "\\\\",
            c if (c as u32) < 0x20 => o += &format!("\\u{:04x}", c as u32),
            c => o.push(c),
        }
    }
    o + "\""
}

fn sha256_hex(b: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn decode(src: &Source, rel: &str) -> Result<(u32, u32, Vec<u8>)> {
    let bytes = src.read(rel)?;
    let img = crate::tpf_import::read_image(rel, &bytes).with_context(|| format!("{rel}: not a readable DDS"))?;
    let (w, h) = img.size();
    ensure!(w.max(h) <= 4096, "{rel}: larger than 4096 pixels");
    let rgba = match &img {
        crate::tpf_import::Image::Bc { format, data, .. } => decode_bc(*format, w, h, data),
        crate::tpf_import::Image::Rgba { .. } => img.top_rgba(),
    };
    Ok((w, h, rgba))
}

/// RGB565 widened by repeating the top bits (the D3D and common reference
/// rounding; the game's own decoder truncates, which differs by 1 on some
/// values: the pack keeps the reference pixels the mod's authors saw).
fn rgb565_wide(c: u16) -> [u32; 3] {
    let (r, g, b) = (u32::from(c >> 11) & 31, u32::from(c >> 5) & 63, u32::from(c) & 31);
    [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)]
}

/// The top level of a DXT1/3/5 image as RGBA8: the palette in integer
/// steps (2a+b)/3, (a+b)/2; DXT1's c0 <= c1 has transparent black; the
/// DXT5 alpha ramp (6a+b)/7 or (4a+b)/5 with 0 and 255.
pub fn decode_bc(format: crate::formats::texture::PixelFormat, w: u32, h: u32, data: &[u8]) -> Vec<u8> {
    use crate::formats::texture::PixelFormat;
    let (w, h) = (w as usize, h as usize);
    let mut out = vec![0u8; w * h * 4];
    let size = if format == PixelFormat::Dxt1 { 8 } else { 16 };
    let bx = w.div_ceil(4);
    for (n, block) in data.chunks_exact(size).take(bx * h.div_ceil(4)).enumerate() {
        let (alpha, color) = if size == 8 { (None, block) } else { (Some(&block[..8]), &block[8..]) };
        let c0 = u16::from_le_bytes([color[0], color[1]]);
        let c1 = u16::from_le_bytes([color[2], color[3]]);
        let (a, b) = (rgb565_wide(c0), rgb565_wide(c1));
        let mix = |f: &dyn Fn(u32, u32) -> u32| [f(a[0], b[0]) as u8, f(a[1], b[1]) as u8, f(a[2], b[2]) as u8, 255];
        let four = c0 > c1 || format != PixelFormat::Dxt1;
        let pal: [[u8; 4]; 4] = if four {
            [mix(&|x, _| x), mix(&|_, y| y), mix(&|x, y| (2 * x + y) / 3), mix(&|x, y| (x + 2 * y) / 3)]
        } else {
            [mix(&|x, _| x), mix(&|_, y| y), mix(&|x, y| (x + y) / 2), [0, 0, 0, 0]]
        };
        let bits = u32::from_le_bytes([color[4], color[5], color[6], color[7]]);
        let mut texels: [[u8; 4]; 16] = std::array::from_fn(|i| pal[((bits >> (2 * i)) & 3) as usize]);
        match (format, alpha) {
            (PixelFormat::Dxt3, Some(al)) => {
                for (i, t) in texels.iter_mut().enumerate() {
                    t[3] = ((al[i / 2] >> ((i % 2) * 4)) & 15) * 17;
                }
            }
            (PixelFormat::Dxt5, Some(al)) => {
                let (a0, a1) = (u32::from(al[0]), u32::from(al[1]));
                let ramp: [u8; 8] = std::array::from_fn(|i| {
                    let i = i as u32;
                    (match i {
                        0 => a0,
                        1 => a1,
                        _ if a0 > a1 => ((8 - i) * a0 + (i - 1) * a1) / 7,
                        6 => 0,
                        7 => 255,
                        _ => ((6 - i) * a0 + (i - 1) * a1) / 5,
                    }) as u8
                });
                let bits = al[2..8].iter().rev().fold(0u64, |acc, &x| (acc << 8) | u64::from(x));
                for (i, t) in texels.iter_mut().enumerate() {
                    t[3] = ramp[((bits >> (3 * i)) & 7) as usize];
                }
            }
            _ => {}
        }
        let (x0, y0) = ((n % bx) * 4, (n / bx) * 4);
        for (i, t) in texels.iter().enumerate() {
            let (x, y) = (x0 + i % 4, y0 + i / 4);
            if x < w && y < h {
                out[(y * w + x) * 4..][..4].copy_from_slice(t);
            }
        }
    }
    out
}

/// Converts the two downloads into `packs/hud-mod`. `progress(done,
/// total)` counts the images.
pub fn convert(
    hud: &Source,
    map: &Source,
    packs: &Path,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> std::result::Result<Summary, Problem> {
    // The scripts first: what to convert.
    let hud_text = hud.read(HUD_SCRIPT).map_err(|_| Problem::NotTheMod("it has no assets/userstart.end".into()))?;
    let cats = parse_hud_script(&String::from_utf8_lossy(&hud_text))
        .map_err(|e| Problem::NotTheMod(format!("its assets/userstart.end is not the HUD part's ({e:#})")))?;
    let map_text = map.read(MAP_SCRIPT).map_err(|_| Problem::NotTheMod("it has no Binary/script.end".into()))?;
    let ms = parse_map_script(&String::from_utf8_lossy(&map_text))
        .map_err(|e| Problem::NotTheMod(format!("its Binary/script.end is not the map part's ({e:#})")))?;

    // The HUD's images by hash (the full name's hash: truncated names in
    // the game's files still match), the maps after them.
    let mut missing = Vec::new();
    let mut hud_jobs: BTreeMap<u32, Job> = BTreeMap::new();
    let mut origin: HashMap<String, &Category> = HashMap::new();
    let mut hud_sources = 0;
    for c in &cats {
        let dir = format!("assets/{}", c.folder);
        let Some(files) = hud.files_in(&dir) else {
            missing.push(format!("{dir}/"));
            continue;
        };
        for f in files.iter().filter(|f| f.to_ascii_lowercase().ends_with(".dds")) {
            let name = &f[..f.len() - 4];
            let rel = format!("{dir}/{f}");
            let hash = bin_hash(name);
            let job = hud_jobs.entry(hash).or_insert_with(|| Job {
                hash,
                name: name.to_string(),
                part: 0,
                sources: Vec::new(),
            });
            job.sources.push(rel.clone());
            origin.insert(rel, c);
            hud_sources += 1;
        }
    }
    let mut map_jobs = Vec::new();
    for (name, rel) in &ms.textures {
        let rel = format!("Binary/{rel}");
        if !map.is_file(&rel) {
            missing.push(rel);
            continue;
        }
        let hash = bin_hash(name);
        if hud_jobs.contains_key(&hash) {
            return Err(anyhow!("the map {name} has the same hash as a HUD image").into());
        }
        map_jobs.push(Job { hash, name: name.clone(), part: 1, sources: vec![rel] });
    }
    if !missing.is_empty() {
        return Err(Problem::Missing(missing));
    }
    if hud_jobs.is_empty() {
        return Err(Problem::NotTheMod("its texture folders hold no DDS images".into()));
    }
    let excluded =
        hud.files_under("assets").iter().filter(|p| p.to_ascii_lowercase().ends_with(".dds")).count() - hud_sources;

    // Disk space, from the DDS files' sizes.
    std::fs::create_dir_all(packs).with_context(|| packs.display().to_string())?;
    let jobs: Vec<Job> = hud_jobs.into_values().chain(map_jobs).collect();
    let dds: u64 = jobs.iter().map(|j| (if j.part == 0 { hud } else { map }).size(&j.sources[0])).sum();
    let need = space_needed(dds);
    if let Some(free) = crate::texture_mods::free_space(packs)
        && free < need
    {
        return Err(anyhow!(
            "not enough free disk space: it needs about {} MB, {} MB are free in {}",
            need >> 20,
            free >> 20,
            packs.display()
        )
        .into());
    }

    let stage = packs.join(".hud-mod-import");
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).with_context(|| stage.display().to_string())?;
    let result = (|| -> std::result::Result<Summary, Problem> {
        let total = jobs.len();
        let done = AtomicUsize::new(0);
        progress(0, total);
        // Each PNG: every source decoded and compared, the first saved,
        // the file read back and compared again.
        let records: Vec<std::result::Result<String, (String, String)>> = jobs
            .par_iter()
            .map(|job| {
                let src = if job.part == 0 { hud } else { map };
                let mut first: Option<(u32, u32, Vec<u8>)> = None;
                let mut srcs = Vec::new();
                for rel in &job.sources {
                    let d = decode(src, rel).map_err(|e| (rel.clone(), format!("{e:#}")))?;
                    match &first {
                        Some(f) if (f.0, f.1) != (d.0, d.1) || f.2 != d.2 => {
                            return Err((rel.clone(), format!("a different image with the hash {:08X}", job.hash)));
                        }
                        Some(_) => {}
                        None => first = Some(d),
                    }
                    let c = origin.get(rel);
                    srcs.push(format!(
                        "{{\"path\": {}, \"game_file\": {}, \"collection\": {}}}",
                        json_str(rel),
                        json_str(c.map_or("", |c| c.game_file.as_str())),
                        json_str(c.map_or("", |c| c.collection.as_str()))
                    ));
                }
                let (w, h, rgba) = first.expect("one source at least");
                let png = format!("{:08X}.png", job.hash);
                let out = stage.join(&png);
                let fail = |e: String| (png.clone(), e);
                image::save_buffer_with_format(&out, &rgba, w, h, image::ExtendedColorType::Rgba8, image::ImageFormat::Png)
                    .map_err(|e| fail(e.to_string()))?;
                let back = image::open(&out).map_err(|e| fail(e.to_string()))?.into_rgba8();
                if back.dimensions() != (w, h) || back.as_raw() != &rgba {
                    return Err(fail("the saved image differs from the source".into()));
                }
                progress(done.fetch_add(1, Ordering::Relaxed) + 1, total);
                Ok(format!(
                    "    {{\"texture_name\": {}, \"texture_hash\": \"{:08X}\", \"png\": \"{png}\", \"width\": {w}, \"height\": {h}, \"rgba_sha256\": \"{}\", \"sources\": [{}]}}",
                    json_str(&job.name),
                    job.hash,
                    sha256_hex(&rgba),
                    srcs.join(", ")
                ))
            })
            .collect();
        let mut bad = Vec::new();
        let mut hud_rec = Vec::new();
        let mut map_rec = Vec::new();
        for (job, r) in jobs.iter().zip(records) {
            match r {
                Ok(t) if job.part == 0 => hud_rec.push(t),
                Ok(t) => map_rec.push(t),
                Err((rel, e)) => bad.push(format!("{rel} ({e})")),
            }
        }
        if !bad.is_empty() {
            return Err(Problem::Missing(bad));
        }
        // The calibration, the index, the manifests, the authors' notes.
        let mut tsv = String::from("# track\tfield\tvalue\n");
        let mut rows_json = Vec::new();
        for ((track, field), value) in &ms.rows {
            tsv += &format!("{track}\t{field}\t{value}\n");
            rows_json.push(format!(
                "    {{\"track\": {track}, \"field\": {}, \"value\": {}}}",
                json_str(field),
                json_str(value)
            ));
        }
        let tracks = ms.rows.keys().map(|(t, _)| *t).collect::<std::collections::BTreeSet<_>>().len();
        let write = |name: &str, data: &[u8]| -> Result<()> {
            std::fs::write(stage.join(name), data).with_context(|| format!("{name} could not be written"))
        };
        write("map-overrides.tsv", tsv.as_bytes())?;
        let mut names: Vec<String> = jobs.iter().map(|j| format!("{:08X}.png", j.hash)).collect();
        names.sort();
        write(
            "hd-ui-pack.list",
            format!("NFSU2 HD UI pack v1\n{}", names.iter().map(|n| format!("{n}\n")).collect::<String>()).as_bytes(),
        )?;
        write(
            "manifest.json",
            format!(
                "{{\n  \"schema\": 2,\n  \"made_by\": \"Overbright's launcher (hud_mod)\",\n  \"source\": {},\n  \"policy\": \"bind_textures folders only; UnlimiterTextures left out; the top level's RGBA kept exactly\",\n  \"installer_sha256\": \"{}\",\n  \"counts\": {{\"source_dds\": {hud_sources}, \"excluded_dds\": {excluded}, \"png_files\": {}}},\n  \"textures\": [\n{}\n  ]\n}}\n",
                json_str(&hud.path().display().to_string()),
                sha256_hex(&hud_text),
                hud_rec.len(),
                hud_rec.join(",\n")
            )
            .as_bytes(),
        )?;
        write(
            "map-manifest.json",
            format!(
                "{{\n  \"schema\": 1,\n  \"made_by\": \"Overbright's launcher (hud_mod)\",\n  \"variant\": \"Default\",\n  \"source\": {},\n  \"script_sha256\": \"{}\",\n  \"overrides_sha256\": \"{}\",\n  \"counts\": {{\"map_png_files\": {}, \"metadata_rows\": {}, \"metadata_tracks\": {tracks}, \"total_png_files\": {}}},\n  \"textures\": [\n{}\n  ],\n  \"metadata\": [\n{}\n  ]\n}}\n",
                json_str(&map.path().display().to_string()),
                sha256_hex(&map_text),
                sha256_hex(tsv.as_bytes()),
                map_rec.len(),
                ms.rows.len(),
                names.len(),
                map_rec.join(",\n"),
                rows_json.join(",\n")
            )
            .as_bytes(),
        )?;
        for (src, from, to) in [
            (hud, "ReadMe.txt", "ReadMe.txt"),
            (hud, "Changelog.txt", "Changelog.txt"),
            (map, "readme.txt", "detailed-map-readme.txt"),
        ] {
            if let Ok(b) = src.read(from) {
                write(to, &b)?;
            }
        }
        write(
            MARKER,
            format!(
                "Made by Overbright's launcher from the HUD mod's downloads (the mods belong to their authors).\nhud={}\nmap={}\n",
                hud.path().display(),
                map.path().display()
            )
            .as_bytes(),
        )?;
        Ok(Summary {
            dir: stage.clone(),
            hud_sources,
            hud_images: hud_rec.len(),
            map_images: map_rec.len(),
            rows: ms.rows.len(),
            tracks,
            check: Default::default(),
        })
    })();
    let mut summary = match result {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&stage);
            return Err(e);
        }
    };
    // Into place: an older pack moved aside first, removed after.
    let dest = packs.join(PACK_DIR);
    let old = packs.join(".hud-mod-old");
    let _ = std::fs::remove_dir_all(&old);
    let swap = (|| -> Result<()> {
        if dest.exists() {
            ensure!(
                dest.join(MARKER).is_file(),
                "{} exists and was not made by Overbright: it is left alone",
                dest.display()
            );
            std::fs::rename(&dest, &old)?;
        }
        if let Err(e) = std::fs::rename(&stage, &dest) {
            let _ = std::fs::rename(&old, &dest);
            return Err(e.into());
        }
        Ok(())
    })();
    if let Err(e) = swap {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(e.into());
    }
    let _ = std::fs::remove_dir_all(&old);
    summary.dir = dest.clone();
    summary.check = crate::hd_ui_pack::inspect(&dest);
    Ok(summary)
}

/// A finished conversion's or a refusal's lines in plain words: (ok,
/// line); ok None is a warning.
pub fn result_lines(r: &std::result::Result<Summary, Problem>, parts: &Parts) -> Vec<(Option<bool>, String)> {
    let what = |s: &Option<Source>| {
        s.as_ref().and_then(|s| s.path().file_name()).map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    };
    match r {
        Ok(s) if s.check.complete => vec![(
            Some(true),
            format!(
                "HUD mod set up: {} HUD images and {} maps converted, the maps calibrated for {} tracks. The check \
                 finds it complete.",
                s.hud_images, s.map_images, s.tracks
            ),
        )],
        Ok(s) => vec![(
            None,
            format!(
                "HUD mod converted ({} HUD images, {} maps), but it is not the version Overbright knows ({} and {}): \
                 the game keeps its own HUD with it.",
                s.hud_images, s.map_images, KNOWN_HUD_IMAGES, KNOWN_MAP_IMAGES
            ),
        )],
        Err(Problem::NotTheMod(why)) => {
            vec![(Some(false), format!("This is not the HUD mod: {why}. Nothing was changed."))]
        }
        Err(Problem::Missing(files)) => {
            let shown: Vec<&str> = files.iter().take(4).map(String::as_str).collect();
            let more =
                if files.len() > shown.len() { format!(", … ({} in all)", files.len()) } else { String::new() };
            vec![(
                Some(false),
                format!(
                    "HUD mod not set up: files are missing or damaged in {} / {}: {}{more}. Download the mod again and \
                     extract all of it. Nothing was changed.",
                    what(&parts.hud),
                    what(&parts.map),
                    shown.join(", ")
                ),
            )]
        }
        Err(Problem::Other(e)) => vec![(Some(false), format!("HUD mod not set up: {e:#}. Nothing was changed."))],
    }
}

/// The line while only one part is known (or none), for the player.
pub fn waiting_line(parts: &Parts) -> Option<String> {
    match (&parts.hud, &parts.map) {
        (Some(h), None) => Some(format!(
            "Found the HUD part ({}). Now add the map part: its .zip, or the folder it was extracted to.",
            h.path().display()
        )),
        (None, Some(m)) => Some(format!(
            "Found the map part ({}). Now add the HUD part: the folder its .7z was extracted to.",
            m.path().display()
        )),
        _ => None,
    }
}

/// Removes a converted pack this module made (never anything else).
pub fn remove(dir: &Path) -> Result<()> {
    ensure!(dir.join(MARKER).is_file(), "{} was not made by Overbright's launcher", dir.display());
    std::fs::remove_dir_all(dir).with_context(|| dir.display().to_string())
}

#[cfg(test)]
pub mod tests;

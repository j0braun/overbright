//! TexWizard texture packs (docs/texture-packs.md, "TexWizard packs"), read
//! as data. TexWizard (github.com/R-033/texwizard, GPL-3.0) is a script for
//! the original game: at start it reads each pack's `meta.json`, has the
//! game load the pack's `textures.bin` with its global files, and answers
//! every texture lookup for an original's hash with the replacement's.
//! Our code is our own, written from TexWizard's public source and wiki
//! (THIRD-PARTY-NOTICES.md):
//!
//! * A pack is a folder holding `meta.json` and `textures.bin`.
//! * `meta.json` (parsed by TexWizard with jsoncpp's default settings:
//!   comments and trailing commas allowed): `{"textures": [[ORIGINAL, NEW],
//!   …]}`; each name is a texture name, hashed with the game's string hash
//!   (`bin_hash`, SPEED2.EXE 0x43DB50), or `0x` and its hash in hex. A
//!   later pair for the same original wins. TexWizard Packer also writes
//!   `"links": [{"path": FILE, "textures": [...]}]` (where the originals
//!   are; not used by the game).
//! * `textures.bin` is a game chunk file (made with the Binary tool or
//!   TexWizard Packer) holding texture packs (chunk 0xB3300000), the same
//!   format as the game's own (`texture::parse_texture_pack`).
//! * Where the packs are: `scripts/TexWizard.json` in the game folder,
//!   `{"packs": [FOLDER, …], "autoScan": true}` (folders relative to the
//!   game folder), then, unless `autoScan` is false, every folder holding
//!   a `meta.json` under `TexturePacks/` (recursively). A later pack's pair
//!   wins over an earlier one's.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use super::reader::Limits;
use super::texture::Texture;

/// The most bytes one pack's `textures.bin` may have.
pub const MAX_BIN: usize = 1 << 30;

// ------------------------------------------------------------------ JSON

/// A JSON value, as far as TexWizard's files need one.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// The member `key` of an object (the last one when repeated, as
    /// jsoncpp keeps it).
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(m) => m.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_array(&self) -> &[Json] {
        match self {
            Json::Array(a) => a,
            _ => &[],
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }
}

/// Parses JSON as jsoncpp's default reader takes it: a byte-order mark,
/// `//` and `/* */` comments and trailing commas are allowed, text after
/// the value is ignored. Depth is held to 64.
pub fn parse_json(text: &str) -> Result<Json> {
    let mut p = Parser { b: text.strip_prefix('\u{feff}').unwrap_or(text).as_bytes(), i: 0 };
    p.value(0)
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn skip(&mut self) {
        loop {
            while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if self.b[self.i..].starts_with(b"//") {
                while self.i < self.b.len() && self.b[self.i] != b'\n' {
                    self.i += 1;
                }
            } else if self.b[self.i..].starts_with(b"/*") {
                match self.b[self.i + 2..].windows(2).position(|w| w == b"*/") {
                    Some(n) => self.i += n + 4,
                    None => self.i = self.b.len(),
                }
            } else {
                return;
            }
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip();
        self.b.get(self.i).copied()
    }

    fn value(&mut self, depth: usize) -> Result<Json> {
        ensure!(depth < 64, "JSON nested too deep");
        match self.peek().context("JSON: unexpected end")? {
            b'{' => {
                self.i += 1;
                let mut m = Vec::new();
                loop {
                    match self.peek() {
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Object(m));
                        }
                        Some(b'"') => {
                            let k = self.string()?;
                            ensure!(self.peek() == Some(b':'), "JSON: ':' expected at byte {}", self.i);
                            self.i += 1;
                            let v = self.value(depth + 1)?;
                            m.push((k, v));
                            match self.peek() {
                                Some(b',') => self.i += 1,
                                Some(b'}') => {}
                                _ => bail!("JSON: ',' or '}}' expected at byte {}", self.i),
                            }
                        }
                        _ => bail!("JSON: a member name expected at byte {}", self.i),
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut a = Vec::new();
                loop {
                    if self.peek() == Some(b']') {
                        self.i += 1;
                        return Ok(Json::Array(a));
                    }
                    a.push(self.value(depth + 1)?);
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {}
                        _ => bail!("JSON: ',' or ']' expected at byte {}", self.i),
                    }
                }
            }
            b'"' => Ok(Json::String(self.string()?)),
            _ => {
                let start = self.i;
                while self.i < self.b.len()
                    && (self.b[self.i].is_ascii_alphanumeric() || b"+-.".contains(&self.b[self.i]))
                {
                    self.i += 1;
                }
                let word = std::str::from_utf8(&self.b[start..self.i]).unwrap_or("");
                Ok(match word {
                    "null" => Json::Null,
                    "true" => Json::Bool(true),
                    "false" => Json::Bool(false),
                    w => {
                        Json::Number(w.parse().ok().with_context(|| format!("JSON: unexpected {w:?} at byte {start}"))?)
                    }
                })
            }
        }
    }

    fn string(&mut self) -> Result<String> {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = *self.b.get(self.i).context("JSON: unterminated string")?;
            self.i += 1;
            match c {
                b'"' => return Ok(String::from_utf8_lossy(&out).into_owned()),
                b'\\' => {
                    let e = *self.b.get(self.i).context("JSON: unterminated string")?;
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'u' => {
                            let hex = self.b.get(self.i..self.i + 4).context("JSON: short \\u escape")?;
                            let v = u32::from_str_radix(std::str::from_utf8(hex).unwrap_or("x"), 16)
                                .ok()
                                .context("JSON: bad \\u escape")?;
                            self.i += 4;
                            let ch = char::from_u32(v).unwrap_or('\u{fffd}');
                            out.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
                        }
                        other => out.push(other),
                    }
                }
                c => out.push(c),
            }
        }
    }
}

// ------------------------------------------------------------------ meta

/// A name of `meta.json` as TexWizard hashes it: `0x` and hex digits (read
/// up to the first other character), else the game's string hash of the
/// name as written. None: `0x` without a hex digit (TexWizard would use an
/// undefined value).
pub fn name_hash(name: &str) -> Option<u32> {
    match name.strip_prefix("0x") {
        Some(hex) => {
            let digits: &str = &hex[..hex.find(|c: char| !c.is_ascii_hexdigit()).unwrap_or(hex.len())];
            u32::from_str_radix(digits, 16).ok()
        }
        None => Some(super::hash::bin_hash(name)),
    }
}

/// One `[ORIGINAL, NEW]` pair of `meta.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    pub original: String,
    pub original_hash: u32,
    pub new: String,
    pub new_hash: u32,
}

/// `meta.json`'s pairs, one per original (a later pair for the same
/// original replaces the earlier, in the earlier's place), and the count
/// of pairs that could not be read (not two strings, a bad `0x` name).
pub fn parse_meta(text: &str) -> Result<(Vec<Pair>, usize)> {
    let root = parse_json(text).context("meta.json")?;
    let textures = root.get("textures").context("meta.json has no \"textures\" list")?;
    ensure!(matches!(textures, Json::Array(_)), "meta.json: \"textures\" is not a list");
    let mut pairs: Vec<Pair> = Vec::new();
    let mut at: HashMap<u32, usize> = HashMap::new();
    let mut bad = 0;
    for t in textures.as_array() {
        let a = t.as_array();
        let (Some(o), Some(n)) = (a.first().and_then(Json::as_str), a.get(1).and_then(Json::as_str)) else {
            bad += 1;
            continue;
        };
        let (Some(oh), Some(nh)) = (name_hash(o), name_hash(n)) else {
            bad += 1;
            continue;
        };
        let p = Pair { original: o.to_string(), original_hash: oh, new: n.to_string(), new_hash: nh };
        match at.get(&oh) {
            Some(&i) => pairs[i] = p,
            None => {
                at.insert(oh, pairs.len());
                pairs.push(p);
            }
        }
    }
    Ok((pairs, bad))
}

// ------------------------------------------------------------------ packs

/// A TexWizard pack read: its pairs and the textures of its `textures.bin`.
pub struct Pack {
    /// The pack folder's name.
    pub name: String,
    pub pairs: Vec<Pair>,
    /// Pairs `meta.json` holds that could not be read.
    pub bad_pairs: usize,
    pub textures: Vec<Texture>,
    /// Bytes of `textures.bin`.
    pub bin_bytes: u64,
}

impl Pack {
    /// The pack's texture with `hash` (the first when several have it).
    pub fn texture(&self, hash: u32) -> Option<&Texture> {
        self.textures.iter().find(|t| t.hash == hash)
    }
}

/// Every texture of the texture packs (chunk 0xB3300000) in a chunk file
/// (JDLZ-compressed as a whole, or not).
pub fn read_bin(raw: &[u8]) -> Result<Vec<Texture>> {
    let unpacked;
    let data = if raw.starts_with(super::jdlz::MAGIC) {
        unpacked = super::jdlz::decompress(raw).context("textures.bin: JDLZ")?;
        &unpacked[..]
    } else {
        raw
    };
    let mut out = Vec::new();
    let mut err = None;
    super::bundle::walk(super::bundle::chunks(data, 0), &mut |c| {
        if c.id == super::bundle::id::TEXTURE_PACK {
            match super::texture::parse_texture_pack(c) {
                Ok(ts) => out.extend(ts),
                Err(e) => err = Some(e),
            }
        }
    });
    if out.is_empty() {
        match err {
            Some(e) => return Err(e.context("textures.bin: a texture pack could not be read")),
            None => bail!("textures.bin holds no texture pack (not a TexWizard pack?)"),
        }
    }
    Ok(out)
}

/// A pack from its two files' bytes.
pub fn read_pack(name: &str, meta: &[u8], bin: &[u8]) -> Result<Pack> {
    let (pairs, bad_pairs) = parse_meta(&String::from_utf8_lossy(meta))?;
    let textures = read_bin(bin)?;
    Ok(Pack { name: name.to_string(), pairs, bad_pairs, textures, bin_bytes: bin.len() as u64 })
}

/// A folder holding `meta.json` and `textures.bin` (either case).
pub fn is_pack_dir(dir: &Path) -> bool {
    file_in(dir, "meta.json").is_some() && file_in(dir, "textures.bin").is_some()
}

/// The file `name` in `dir`, compared without case (packs made on Windows).
fn file_in(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if exact.is_file() {
        return Some(exact);
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_file() && p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name)))
}

/// Reads the pack in `dir`.
pub fn read_dir(dir: &Path) -> Result<Pack> {
    let meta =
        file_in(dir, "meta.json").with_context(|| format!("{}: no meta.json (not a TexWizard pack)", dir.display()))?;
    let bin =
        file_in(dir, "textures.bin").with_context(|| format!("{}: no textures.bin beside meta.json", dir.display()))?;
    let len = std::fs::metadata(&bin).map_or(0, |m| m.len()) as usize;
    Limits::check("TexWizard textures.bin size", len, MAX_BIN)?;
    let name = dir.file_name().map_or_else(|| dir.display().to_string(), |n| n.to_string_lossy().into_owned());
    read_pack(
        &name,
        &std::fs::read(&meta).with_context(|| meta.display().to_string())?,
        &std::fs::read(&bin).with_context(|| bin.display().to_string())?,
    )
}

/// The pack folders in a ZIP archive (`…/meta.json` with a `textures.bin`
/// beside it), as `(folder in the archive, its name)`.
pub fn zip_packs(zip: &super::zip::Zip) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = zip
        .entries
        .iter()
        .filter_map(|e| {
            let (dir, file) = e.name.rsplit_once('/').unwrap_or(("", e.name.as_str()));
            if !file.eq_ignore_ascii_case("meta.json") {
                return None;
            }
            let bin = if dir.is_empty() { "textures.bin".to_string() } else { format!("{dir}/textures.bin") };
            zip.find(&bin)?;
            Some((dir.to_string(), dir.rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or("").to_string()))
        })
        .collect();
    out.sort();
    out
}

/// Reads the pack in folder `dir` of a ZIP archive (`name` when the pack
/// is at the archive's top).
pub fn read_zip_pack(zip: &super::zip::Zip, dir: &str, name: &str) -> Result<Pack> {
    let at = |f: &str| if dir.is_empty() { f.to_string() } else { format!("{dir}/{f}") };
    let meta = zip.find(&at("meta.json")).context("no meta.json")?;
    let bin = zip.find(&at("textures.bin")).context("no textures.bin beside meta.json")?;
    read_pack(name, &zip.read(meta)?, &zip.read(bin)?)
}

/// How deep [`find_packs`] looks below a folder that is not a game folder.
const SCAN_DEPTH: usize = 3;

fn scan(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if is_pack_dir(dir) {
        out.push(dir.to_path_buf());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut subs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    subs.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    for s in subs {
        scan(&s, depth - 1, out);
    }
}

/// The game folder's `scripts/TexWizard.json`: its packs (folders relative
/// to the game folder, in order) and `autoScan` (true when not given).
pub fn read_config(root: &Path) -> Option<(Vec<String>, bool)> {
    let text = std::fs::read_to_string(file_in(&file_in_dir(root, "scripts")?, "TexWizard.json")?).ok()?;
    let j = parse_json(&text).ok()?;
    let packs =
        j.get("packs").map_or(&[][..], Json::as_array).iter().filter_map(Json::as_str).map(str::to_string).collect();
    let auto = !matches!(j.get("autoScan"), Some(Json::Bool(false)));
    Some((packs, auto))
}

fn file_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if exact.is_dir() {
        return Some(exact);
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name)))
}

/// A path of TexWizard.json (`\` or `/`, relative to the game folder) on
/// this system.
fn relative(root: &Path, rel: &str) -> PathBuf {
    let mut p = root.to_path_buf();
    for part in rel.split(['\\', '/']).filter(|s| !s.is_empty() && *s != ".") {
        p = match file_in_dir(&p, part) {
            Some(d) => d,
            None => p.join(part),
        };
    }
    p
}

/// The TexWizard packs in or below `dir`, most important first (the order
/// Overbright's list wants: the first wins):
/// * `dir` is a pack: itself;
/// * `dir` is a game folder with TexWizard (`scripts/TexWizard.json` or a
///   `TexturePacks` folder): the packs TexWizard loads, in its order
///   reversed (TexWizard's later pack wins);
/// * a folder named `TexturePacks`: its packs, as TexWizard scans it;
/// * else the packs up to three folders down, by name.
pub fn find_packs(dir: &Path) -> Vec<PathBuf> {
    if is_pack_dir(dir) {
        return vec![dir.to_path_buf()];
    }
    let config = read_config(dir);
    let packs_dir = file_in_dir(dir, "TexturePacks");
    if config.is_some() || packs_dir.is_some() {
        let (listed, auto) = config.unwrap_or((Vec::new(), true));
        let mut order: Vec<PathBuf> = Vec::new();
        let key = |p: &Path| p.to_string_lossy().replace('\\', "/").to_lowercase();
        for rel in listed {
            let p = relative(dir, &rel);
            if is_pack_dir(&p) && !order.iter().any(|q| key(q) == key(&p)) {
                order.push(p);
            }
        }
        if auto && let Some(d) = packs_dir {
            let mut found = Vec::new();
            scan(&d, 16, &mut found);
            for p in found {
                if !order.iter().any(|q| key(q) == key(&p)) {
                    order.push(p);
                }
            }
        }
        order.reverse();
        return order;
    }
    let depth = if dir.file_name().is_some_and(|n| n.eq_ignore_ascii_case("TexturePacks")) { 16 } else { SCAN_DEPTH };
    let mut found = Vec::new();
    scan(dir, depth, &mut found);
    // TexWizard's scan loads a later folder's pairs over an earlier's.
    if depth == 16 {
        found.reverse();
    }
    found
}

// ------------------------------------------------------------------ tests

/// A texture pack chunk (0xB3300000) holding `textures`, the way the game's
/// files and Binary lay one out, for tests (the library's too).
#[doc(hidden)]
pub fn tpk_bytes(textures: &[Texture]) -> Vec<u8> {
    use super::bundle::id;
    use super::texture::PixelFormat;
    fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend((body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }
    let mut pixels: Vec<u8> = Vec::new();
    let mut entries: Vec<u8> = Vec::new();
    for t in textures {
        let at = pixels.len();
        pixels.extend_from_slice(&t.data);
        let mut e = vec![0u8; 0x7C];
        let n = t.name.as_bytes();
        e[0x0C..0x0C + n.len().min(0x17)].copy_from_slice(&n[..n.len().min(0x17)]);
        e[0x24..0x28].copy_from_slice(&t.hash.to_le_bytes());
        e[0x30..0x34].copy_from_slice(&(at as u32).to_le_bytes());
        e[0x38..0x3C].copy_from_slice(&(t.data.len() as u32).to_le_bytes());
        e[0x44..0x46].copy_from_slice(&(t.width as u16).to_le_bytes());
        e[0x46..0x48].copy_from_slice(&(t.height as u16).to_le_bytes());
        e[0x4A] = match t.format {
            PixelFormat::Dxt1 => 0x22,
            PixelFormat::Dxt3 => 0x24,
            PixelFormat::Dxt5 => 0x26,
            PixelFormat::P8 => 0x08,
            _ => 0x20,
        };
        e[0x4E] = t.mip_count as u8;
        entries.extend(e);
        while pixels.len() % 128 != 0 {
            pixels.push(0);
        }
    }
    let header = chunk(id::TPK_HEADER, &chunk(id::TPK_TEXTURES, &entries));
    // The data chunk's body starts 128-aligned in the file: 8 (pack
    // header) + header + 8 (data block header) + 8 (data header) + pad.
    let before = 8 + header.len() + 16;
    let pad = (128 - before % 128) % 128;
    let mut data = vec![0u8; pad];
    data.extend(pixels);
    let block = chunk(id::TPK_DATA_BLOCK, &chunk(id::TPK_DATA, &data));
    chunk(id::TEXTURE_PACK, &[header, block].concat())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::hash::bin_hash;

    /// A fresh scratch folder named for `name`.
    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ob-texture-mods-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A game texture of `w` × `h` RGBA8 texels of one colour.
    fn texture(name: &str, w: u32, h: u32, rgba: [u8; 4]) -> crate::formats::texture::Texture {
        use crate::formats::texture::{Blend, PixelFormat, Texture};
        Texture {
            name: name.into(),
            hash: bin_hash(name),
            width: w,
            height: h,
            format: PixelFormat::Rgba8,
            mip_count: 1,
            data: rgba.repeat((w * h) as usize).into(),
            palette: Vec::new(),
            alpha_test: false,
            blend: Blend::None,
            rendering_order: 0,
            late_buckets: false,
            scroll: Default::default(),
        }
    }

    #[test]
    fn json_as_jsoncpp_reads_it() {
        let j = parse_json(
            "\u{feff}// a pack\n{ \"textures\": [ [\"A\", \"0x1F\"], /* x */ ], \"n\": -1.5e2, \"t\": true, }trailing",
        )
        .unwrap();
        assert_eq!(j.get("textures").unwrap().as_array()[0].as_array()[1].as_str(), Some("0x1F"));
        assert_eq!(j.get("n"), Some(&Json::Number(-150.0)));
        assert_eq!(j.get("t"), Some(&Json::Bool(true)));
        assert_eq!(parse_json(r#""a\"b\\cA\/""#).unwrap(), Json::String("a\"b\\cA/".into()));
        assert!(parse_json("{\"a\" 1}").is_err());
        assert!(parse_json("[1, 2").is_err());
        assert!(parse_json(&"[".repeat(100)).is_err(), "depth held");
    }

    #[test]
    fn names_hash_as_texwizard_does() {
        assert_eq!(name_hash("TACH_FILL_CUSTOM_00"), Some(bin_hash("TACH_FILL_CUSTOM_00")));
        assert_eq!(name_hash("0x1A2b3C4d"), Some(0x1A2B_3C4D));
        // Read up to the first character that is no hex digit.
        assert_eq!(name_hash("0x12zz"), Some(0x12));
        assert_eq!(name_hash("0xzz"), None);
        // Only a lower-case "0x" is a hash; the name is hashed as written.
        assert_eq!(name_hash("0X12"), Some(bin_hash("0X12")));
    }

    #[test]
    fn meta_pairs_later_wins() {
        let (pairs, bad) = parse_meta(
            r#"{ "textures": [ ["ROAD", "_ROAD"], ["WALL", "0x0000BEEF"], ["ROAD", "_ROAD2"], ["only one"], [1, 2] ],
                 "links": [ { "path": "TRACKS\\L4RA.BUN", "textures": ["ROAD"] } ] }"#,
        )
        .unwrap();
        assert_eq!(bad, 2);
        assert_eq!(pairs.len(), 2);
        assert_eq!((pairs[0].original.as_str(), pairs[0].new.as_str()), ("ROAD", "_ROAD2"));
        assert_eq!(pairs[0].new_hash, bin_hash("_ROAD2"));
        assert_eq!(pairs[1].new_hash, 0xBEEF);
        assert!(parse_meta("{}").is_err());
        assert!(parse_meta("not json").is_err());
    }

    #[test]
    fn a_written_pack_reads_back() {
        let t = texture("_ROAD", 4, 4, [1, 2, 3, 255]);
        let mut t = t;
        t.format = super::super::texture::PixelFormat::Bgra8;
        let bin = tpk_bytes(std::slice::from_ref(&t));
        let back = read_bin(&bin).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!((back[0].name.as_str(), back[0].hash, back[0].width), ("_ROAD", bin_hash("_ROAD"), 4));
        assert_eq!(&back[0].data[..], &t.data[..]);
        assert!(read_bin(b"not a chunk file").is_err());
        let p = read_pack("Pack", br#"{"textures":[["ROAD","_ROAD"]]}"#, &bin).unwrap();
        assert_eq!(p.texture(bin_hash("_ROAD")).map(|t| t.width), Some(4));
    }

    #[test]
    fn packs_are_found_in_texwizards_order() {
        let d = scratch("texwizard-find");
        let mk = |rel: &str| {
            let p = d.join(rel);
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(p.join("meta.json"), "{\"textures\":[]}").unwrap();
            std::fs::write(p.join("textures.bin"), b"").unwrap();
            p
        };
        let listed = mk("Mods/Listed");
        let a = mk("TexturePacks/A");
        let b = mk("TexturePacks/sub/B");
        std::fs::create_dir_all(d.join("scripts")).unwrap();
        std::fs::write(
            d.join("scripts/TexWizard.json"),
            "{ \"packs\": [ \"Mods\\\\Listed\", \"TexturePacks\\\\A\" ] }",
        )
        .unwrap();
        // Loaded: Listed, A (listed), then B (scanned); the last wins, so
        // the list wants B, A, Listed.
        assert_eq!(find_packs(&d), [b.clone(), a.clone(), listed.clone()]);
        std::fs::write(d.join("scripts/TexWizard.json"), "{ \"packs\": [ \"Mods/Listed\" ], \"autoScan\": false }")
            .unwrap();
        assert_eq!(find_packs(&d), [listed.clone()]);
        assert_eq!(find_packs(&a), [a.clone()], "a pack folder is itself");
        assert_eq!(find_packs(&d.join("TexturePacks")), [b, a], "TexturePacks scanned as TexWizard does");
        assert_eq!(find_packs(&d.join("Mods")), [listed], "a plain folder: the packs below it");
        let _ = std::fs::remove_dir_all(d);
    }
}

//! Bundles (`.obz`) and pack files (`.obpk`) (layout.md §4): ZIP archives of a restricted profile (APPNOTE.TXT
//! 6.3.10) that any ZIP tool opens and Overbright memory-maps.
//!
//! The reader reads the central directory once and checks the whole profile (`LAY.1`); a stored entry is a slice
//! of the mapped file (zero copy), a deflated one is inflated on use with its CRC-32 checked. The writer is
//! deterministic: entries' data in the order they are added (the unit's load order), the central directory
//! sorted by logical path, every stored entry's data aligned to 64 bytes through the alignment extra field
//! (header id `0xD935`, as Android's zipalign writes it), every time 1980-01-01 00:00.

use std::borrow::Cow;

use crate::digest::{Alg, Digest};
use crate::names;
use crate::{Refusal, Rule};

/// The alignment of stored entries' data.
pub const ALIGN: u64 = 64;
/// The alignment extra field's header id.
pub const ALIGN_FIELD: u16 = 0xD935;
/// The most entries a bundle holds.
pub const MAX_ENTRIES: usize = 65_535;
/// The largest stored entry.
pub const MAX_STORED: u64 = 1 << 30;
/// A bundle is smaller than this.
pub const MAX_BUNDLE: u64 = 1 << 32;
/// Every entry's DOS date (1980-01-01) and time (00:00).
pub const DOS_DATE: u16 = 0x0021;
pub const DOS_TIME: u16 = 0;
/// The manifest's logical path, the first data of a `.obpk`.
pub const MANIFEST: &str = "pack.toml";
/// Extensions of payload entries a base set's bundle stores, never deflates (outside `residual/`).
pub const PAYLOAD_EXTENSIONS: &[&str] = &["bin", "ktx2", "png", "eagl", "ivf"];

const LOCAL_SIG: u32 = 0x0403_4B50;
const CENTRAL_SIG: u32 = 0x0201_4B50;
const END_SIG: u32 = 0x0605_4B50;
const ZIP64_LOCATOR_SIG: u32 = 0x0706_4B50;
const UTF8_FLAG: u16 = 1 << 11;

/// What a file is: a bundle written by the importer or Studio (stored entries MUST be aligned), or a pack file
/// (`.obpk`, `pack.toml` first; a third party's misaligned stored entries are accepted and read as copies).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bundle,
    PackFile,
}

/// How an entry's data is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Stored,
    Deflated,
}

impl Method {
    fn code(self) -> u16 {
        match self {
            Method::Stored => 0,
            Method::Deflated => 8,
        }
    }
}

/// One entry of a bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The logical path.
    pub path: String,
    pub method: Method,
    pub crc: u32,
    /// Bytes stored in the bundle.
    pub stored_size: u64,
    /// Bytes of the entry's content.
    pub size: u64,
    /// Where the local header starts.
    pub header_offset: u64,
    /// Where the data starts.
    pub data_offset: u64,
}

impl Entry {
    /// Whether the entry can be used in place (stored and aligned).
    pub fn mapped(&self) -> bool {
        self.method == Method::Stored && self.data_offset.is_multiple_of(ALIGN)
    }
}

fn lay1(msg: impl Into<String>) -> Refusal {
    Refusal::new(Rule::Lay1, msg)
}

fn u16_at(b: &[u8], o: u64) -> Result<u16, Refusal> {
    let o = o as usize;
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| lay1("truncated").at_offset(o as u64))
}

fn u32_at(b: &[u8], o: u64) -> Result<u32, Refusal> {
    let o = o as usize;
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| lay1("truncated").at_offset(o as u64))
}

/// The extra fields of a header: checks their framing and returns the alignment field's value, if any. Other
/// tools' fields are ignored.
fn alignment_field(extra: &[u8]) -> Result<Option<u16>, Refusal> {
    let mut at = 0;
    let mut align = None;
    while at < extra.len() {
        if at + 4 > extra.len() {
            return Err(lay1("a truncated extra field"));
        }
        let id = u16::from_le_bytes([extra[at], extra[at + 1]]);
        let len = u16::from_le_bytes([extra[at + 2], extra[at + 3]]) as usize;
        let data = extra.get(at + 4..at + 4 + len).ok_or_else(|| lay1("an extra field runs past its header"))?;
        if id == 0x0001 {
            return Err(lay1("a 64-bit ZIP extension"));
        }
        if id == ALIGN_FIELD {
            if len < 2 || data[2..].iter().any(|&b| b != 0) {
                return Err(lay1("a malformed alignment field"));
            }
            align = Some(u16::from_le_bytes([data[0], data[1]]));
        }
        at += 4 + len;
    }
    Ok(align)
}

/// The bytes behind a bundle: a memory map or a buffer.
pub enum Backing {
    Mapped(memmap2::Mmap),
    Owned(Vec<u8>),
}

impl std::ops::Deref for Backing {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Backing::Mapped(m) => m,
            Backing::Owned(v) => v,
        }
    }
}

/// An open bundle: its entries, sorted by logical path.
pub struct Bundle {
    data: Backing,
    kind: Kind,
    entries: Vec<Entry>,
}

impl Bundle {
    /// Maps the file at `path` and reads its directory.
    pub fn open(path: &std::path::Path, kind: Kind) -> Result<Bundle, Refusal> {
        let io = |e: std::io::Error| lay1(format!("cannot read {}: {e}", path.display()));
        let f = std::fs::File::open(path).map_err(io)?;
        // SAFETY: the map is only read. A file changed meanwhile gives wrong bytes, which the verification of the
        // bundle (its digest, every entry's CRC) catches.
        let map = unsafe { memmap2::Mmap::map(&f) }.map_err(io)?;
        Bundle::new(Backing::Mapped(map), kind)
    }

    /// Reads a bundle held in memory.
    pub fn from_bytes(bytes: Vec<u8>, kind: Kind) -> Result<Bundle, Refusal> {
        Bundle::new(Backing::Owned(bytes), kind)
    }

    fn new(data: Backing, kind: Kind) -> Result<Bundle, Refusal> {
        let entries = directory(&data, kind)?;
        Ok(Bundle { data, kind, entries })
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The whole file.
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    /// Every entry, sorted by logical path.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The entry at a logical path (binary search; case is significant).
    pub fn get(&self, path: &str) -> Option<&Entry> {
        self.entries.binary_search_by(|e| e.path.as_bytes().cmp(path.as_bytes())).ok().map(|i| &self.entries[i])
    }

    /// An entry's content: a slice of the bundle for a stored, aligned entry (not re-checked: [`Bundle::verify`]
    /// checks every CRC once); a copy for a misaligned stored entry of a pack file; inflated, its CRC-32 and size
    /// checked, for a deflated one.
    pub fn read(&self, e: &Entry) -> Result<Cow<'_, [u8]>, Refusal> {
        let raw = &self.data[e.data_offset as usize..(e.data_offset + e.stored_size) as usize];
        match e.method {
            Method::Stored if e.mapped() => Ok(Cow::Borrowed(raw)),
            Method::Stored => Ok(Cow::Owned(raw.to_vec())),
            Method::Deflated => {
                let out = miniz_oxide::inflate::decompress_to_vec_with_limit(raw, e.size as usize)
                    .map_err(|err| lay1(format!("deflate data does not inflate: {err:?}")).at_path(&e.path))?;
                if out.len() as u64 != e.size || ob_formats::formats::tpf::zip_crc(&out) != e.crc {
                    return Err(lay1("CRC or size mismatch").at_path(&e.path));
                }
                Ok(Cow::Owned(out))
            }
        }
    }

    /// Checks every entry's CRC-32 (stored ones in place, deflated ones inflated).
    pub fn verify(&self) -> Result<(), Refusal> {
        for e in &self.entries {
            let data = self.read(e)?;
            if e.method == Method::Stored && ob_formats::formats::tpf::zip_crc(&data) != e.crc {
                return Err(lay1("CRC mismatch").at_path(&e.path));
            }
        }
        Ok(())
    }

    /// The bundle's `xxh3-128` digest (what `import.toml` and `[bundles]` list).
    pub fn digest(&self) -> Digest {
        Alg::Xxh3_128.of(&self.data)
    }

    /// The base-set rule (layout.md §4, "Methods"): payload entries outside `residual/` are stored.
    pub fn check_base_set_payloads(&self) -> Result<(), Refusal> {
        for e in &self.entries {
            let ext = e.path.rsplit_once('.').map_or("", |(_, x)| x);
            if !e.path.starts_with("residual/") && PAYLOAD_EXTENSIONS.contains(&ext) && e.method != Method::Stored {
                return Err(lay1("a payload entry of a base set is deflated").at_path(&e.path));
            }
        }
        Ok(())
    }
}

/// Reads and checks the central directory and every local header.
fn directory(b: &[u8], kind: Kind) -> Result<Vec<Entry>, Refusal> {
    let len = b.len() as u64;
    if len >= MAX_BUNDLE {
        return Err(lay1("a bundle of 4 GiB or more"));
    }
    if len < 22 {
        return Err(lay1("not a ZIP archive"));
    }
    // No archive comment: the end record is the file's last 22 bytes.
    let end = len - 22;
    if u32_at(b, end)? != END_SIG {
        return Err(lay1("no end record in the last 22 bytes (an archive comment, or not a ZIP archive)"));
    }
    if end >= 20 && u32_at(b, end - 20)? == ZIP64_LOCATOR_SIG {
        return Err(lay1("a 64-bit ZIP extension"));
    }
    let (disk, cd_disk) = (u16_at(b, end + 4)?, u16_at(b, end + 6)?);
    let (here, count) = (u16_at(b, end + 8)?, u16_at(b, end + 10)?);
    let (cd_size, cd_offset) = (u64::from(u32_at(b, end + 12)?), u64::from(u32_at(b, end + 16)?));
    if disk != 0 || cd_disk != 0 || here != count {
        return Err(lay1("a multi-disk archive"));
    }
    if count == 0xFFFF || cd_offset == 0xFFFF_FFFF || cd_size == 0xFFFF_FFFF {
        return Err(lay1("a 64-bit ZIP extension"));
    }
    if cd_offset + cd_size != end {
        return Err(lay1("the central directory does not end at the end record").at_offset(cd_offset));
    }
    let mut entries = Vec::with_capacity(count as usize);
    let mut at = cd_offset;
    for _ in 0..count {
        let e = central_entry(b, at, cd_offset, kind)?;
        at = e.1;
        entries.push(e.0);
    }
    if at != end {
        return Err(lay1("the central directory holds more than its entries").at_offset(at));
    }
    for w in entries.windows(2) {
        if w[0].path.as_bytes() >= w[1].path.as_bytes() {
            let why = if w[0].path == w[1].path {
                "a duplicate entry"
            } else {
                "the central directory is not sorted by path"
            };
            return Err(lay1(why).at_path(&w[1].path));
        }
    }
    names::case_unique(entries.iter().map(|e| e.path.as_str())).map_err(|r| lay1(r.message))?;
    // No two entries' bytes overlap, and none starts before the previous one ends.
    let mut by_offset: Vec<&Entry> = entries.iter().collect();
    by_offset.sort_by_key(|e| e.header_offset);
    for w in by_offset.windows(2) {
        if w[1].header_offset < w[0].data_offset + w[0].stored_size {
            return Err(lay1("overlapping entries").at_path(&w[1].path));
        }
    }
    if kind == Kind::PackFile && by_offset.first().map(|e| e.path.as_str()) != Some(MANIFEST) {
        return Err(lay1("a pack file whose first data is not pack.toml"));
    }
    Ok(entries)
}

/// One central directory header at `at`, checked against its local header; returns the entry and where the next
/// header starts.
fn central_entry(b: &[u8], at: u64, cd_offset: u64, kind: Kind) -> Result<(Entry, u64), Refusal> {
    if u32_at(b, at)? != CENTRAL_SIG {
        return Err(lay1("a broken central directory").at_offset(at));
    }
    let flags = u16_at(b, at + 8)?;
    let method = u16_at(b, at + 10)?;
    let (time, date) = (u16_at(b, at + 12)?, u16_at(b, at + 14)?);
    let crc = u32_at(b, at + 16)?;
    let (csize, size) = (u64::from(u32_at(b, at + 20)?), u64::from(u32_at(b, at + 24)?));
    let (n, m, k) = (u16_at(b, at + 28)? as u64, u16_at(b, at + 30)? as u64, u16_at(b, at + 32)? as u64);
    let disk = u16_at(b, at + 34)?;
    let header = u64::from(u32_at(b, at + 42)?);
    let name_bytes =
        b.get((at + 46) as usize..(at + 46 + n) as usize).ok_or_else(|| lay1("truncated").at_offset(at))?;
    let extra =
        b.get((at + 46 + n) as usize..(at + 46 + n + m) as usize).ok_or_else(|| lay1("truncated").at_offset(at))?;
    let next = at + 46 + n + m + k;
    if next > cd_offset + (b.len() as u64 - 22 - cd_offset) {
        return Err(lay1("truncated").at_offset(at));
    }
    let path = std::str::from_utf8(name_bytes).map_err(|_| lay1("a name that is not UTF-8").at_offset(at))?.to_string();
    let fail = |msg: &str| Err(lay1(msg.to_string()).at_path(&path));
    if flags & 0x41 != 0 {
        return fail("an encrypted entry");
    }
    if flags & 0x08 != 0 {
        return fail("a data descriptor");
    }
    if flags & UTF8_FLAG == 0 {
        return fail("the UTF-8 flag is not set");
    }
    let method = match method {
        0 => Method::Stored,
        8 => Method::Deflated,
        m => return fail(&format!("method {m} (only stored and deflate)")),
    };
    if (time, date) != (DOS_TIME, DOS_DATE) {
        return fail("a time other than 1980-01-01 00:00");
    }
    if disk != 0 || header == 0xFFFF_FFFF || csize == 0xFFFF_FFFF || size == 0xFFFF_FFFF {
        return fail("a 64-bit ZIP extension or another disk");
    }
    if path.ends_with('/') {
        return fail("a directory entry");
    }
    names::logical_path(&path, false).map_err(|r| lay1(r.message).at_path(&path))?;
    if method == Method::Stored && csize != size {
        return fail("a stored entry whose two sizes differ");
    }
    if method == Method::Stored && size > MAX_STORED {
        return fail("a stored entry over 1 GiB");
    }
    let central_align = alignment_field(extra).map_err(|r| r.at_path(&path))?;
    // The local header repeats the central one.
    if u32_at(b, header)? != LOCAL_SIG {
        return fail("no local header where the directory says");
    }
    let same = u16_at(b, header + 6)? == flags
        && u16_at(b, header + 8)? == method.code()
        && u16_at(b, header + 10)? == time
        && u16_at(b, header + 12)? == date
        && u32_at(b, header + 14)? == crc
        && u64::from(u32_at(b, header + 18)?) == csize
        && u64::from(u32_at(b, header + 22)?) == size
        && u16_at(b, header + 26)? as u64 == n
        && b.get((header + 30) as usize..(header + 30 + n) as usize) == Some(name_bytes);
    if !same {
        return fail("the local header differs from the central directory");
    }
    let local_m = u16_at(b, header + 28)? as u64;
    let local_extra = b
        .get((header + 30 + n) as usize..(header + 30 + n + local_m) as usize)
        .ok_or_else(|| lay1("truncated").at_path(&path))?;
    let data_offset = header + 30 + n + local_m;
    if data_offset + csize > cd_offset {
        return fail("the entry's data runs past the central directory");
    }
    if method == Method::Stored && !data_offset.is_multiple_of(ALIGN) && kind == Kind::Bundle {
        return fail("a stored entry not aligned to 64 bytes");
    }
    if kind == Kind::Bundle {
        // Written identically in both headers, so each entry's data offset follows from the directory alone.
        if local_extra != extra {
            return fail("the local and central extra fields differ");
        }
        if method == Method::Stored && central_align != Some(ALIGN as u16) {
            return fail("a stored entry without the 64-byte alignment field");
        }
    }
    let entry = Entry { path, method, crc, stored_size: csize, size, header_offset: header, data_offset };
    Ok((entry, next))
}

// ---- writing ----------------------------------------------------------------------------------------------------

struct Pending<'a> {
    path: String,
    data: Cow<'a, [u8]>,
    method: Method,
}

/// A deterministic bundle writer: the same entries added in the same order always give the same bytes.
pub struct Writer<'a> {
    kind: Kind,
    entries: Vec<Pending<'a>>,
}

impl<'a> Writer<'a> {
    pub fn new(kind: Kind) -> Writer<'a> {
        Writer { kind, entries: Vec::new() }
    }

    /// Adds an entry; its data follows the entries added before (a `.obpk`'s `pack.toml` always comes first).
    /// Refused: a bad logical path, a duplicate or case-only duplicate, a stored entry over 1 GiB.
    pub fn add(&mut self, path: &str, data: impl Into<Cow<'a, [u8]>>, method: Method) -> Result<(), Refusal> {
        names::logical_path(path, false).map_err(|r| lay1(r.message).at_path(path))?;
        if self.entries.iter().any(|e| e.path.eq_ignore_ascii_case(path)) {
            return Err(lay1("a duplicate entry").at_path(path));
        }
        let data = data.into();
        if method == Method::Stored && data.len() as u64 > MAX_STORED {
            return Err(lay1("a stored entry over 1 GiB").at_path(path));
        }
        if self.entries.len() == MAX_ENTRIES {
            return Err(lay1("more than 65,535 entries").at_path(path));
        }
        self.entries.push(Pending { path: path.to_string(), data, method });
        Ok(())
    }

    /// The bundle's bytes.
    pub fn finish(self) -> Result<Vec<u8>, Refusal> {
        let mut out = Vec::new();
        self.write_to(&mut out)?;
        Ok(out)
    }

    /// Writes the bundle to `w`.
    pub fn write_to(mut self, w: &mut impl std::io::Write) -> Result<(), Refusal> {
        if self.kind == Kind::PackFile {
            let Some(i) = self.entries.iter().position(|e| e.path == MANIFEST) else {
                return Err(lay1("a pack file without pack.toml"));
            };
            let manifest = self.entries.remove(i);
            self.entries.insert(0, manifest);
        }
        let io = |e: std::io::Error| lay1(format!("write failed: {e}"));
        let mut pos: u64 = 0;
        // (path, central header) per entry.
        let mut central: Vec<(String, Vec<u8>)> = Vec::with_capacity(self.entries.len());
        for e in &self.entries {
            let crc = ob_formats::formats::tpf::zip_crc(&e.data);
            let packed: Cow<[u8]> = match e.method {
                Method::Stored => Cow::Borrowed(&e.data),
                Method::Deflated => Cow::Owned(miniz_oxide::deflate::compress_to_vec(&e.data, 6)),
            };
            let name = e.path.as_bytes();
            let extra = match e.method {
                Method::Stored => {
                    let unpadded = pos + 30 + name.len() as u64 + 6;
                    let pad = ((ALIGN - unpadded % ALIGN) % ALIGN) as usize;
                    let mut x = Vec::with_capacity(6 + pad);
                    x.extend_from_slice(&ALIGN_FIELD.to_le_bytes());
                    x.extend_from_slice(&((2 + pad) as u16).to_le_bytes());
                    x.extend_from_slice(&(ALIGN as u16).to_le_bytes());
                    x.resize(6 + pad, 0);
                    x
                }
                Method::Deflated => Vec::new(),
            };
            let version: u16 = if e.method == Method::Stored { 10 } else { 20 };
            let fixed = |h: &mut Vec<u8>| {
                h.extend_from_slice(&version.to_le_bytes());
                h.extend_from_slice(&UTF8_FLAG.to_le_bytes());
                h.extend_from_slice(&e.method.code().to_le_bytes());
                h.extend_from_slice(&DOS_TIME.to_le_bytes());
                h.extend_from_slice(&DOS_DATE.to_le_bytes());
                h.extend_from_slice(&crc.to_le_bytes());
                h.extend_from_slice(&(packed.len() as u32).to_le_bytes());
                h.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
                h.extend_from_slice(&(name.len() as u16).to_le_bytes());
                h.extend_from_slice(&(extra.len() as u16).to_le_bytes());
            };
            let mut local = LOCAL_SIG.to_le_bytes().to_vec();
            fixed(&mut local);
            local.extend_from_slice(name);
            local.extend_from_slice(&extra);
            let mut c = CENTRAL_SIG.to_le_bytes().to_vec();
            c.extend_from_slice(&version.to_le_bytes()); // made by: MS-DOS attributes, the same version
            fixed(&mut c);
            c.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // comment length, disk, internal + external attributes
            c.extend_from_slice(&(pos as u32).to_le_bytes());
            c.extend_from_slice(name);
            c.extend_from_slice(&extra);
            w.write_all(&local).map_err(io)?;
            w.write_all(&packed).map_err(io)?;
            pos += local.len() as u64 + packed.len() as u64;
            central.push((e.path.clone(), c));
            if pos >= MAX_BUNDLE {
                return Err(lay1("a bundle of 4 GiB or more"));
            }
        }
        central.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        let cd_offset = pos;
        for (_, c) in &central {
            w.write_all(c).map_err(io)?;
            pos += c.len() as u64;
        }
        let mut end = END_SIG.to_le_bytes().to_vec();
        end.extend_from_slice(&[0, 0, 0, 0]);
        end.extend_from_slice(&(central.len() as u16).to_le_bytes());
        end.extend_from_slice(&(central.len() as u16).to_le_bytes());
        end.extend_from_slice(&((pos - cd_offset) as u32).to_le_bytes());
        end.extend_from_slice(&(cd_offset as u32).to_le_bytes());
        end.extend_from_slice(&[0, 0]);
        if pos + 22 >= MAX_BUNDLE {
            return Err(lay1("a bundle of 4 GiB or more"));
        }
        w.write_all(&end).map_err(io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(kind: Kind) -> Vec<u8> {
        let mut w = Writer::new(kind);
        // Load order differs from path order on purpose.
        w.add("world/L4RA/sections/1427.bin", vec![7u8; 1000], Method::Stored).unwrap();
        w.add("world/L4RA/region.toml", b"schema = \"ob.region/1.0\"\n".repeat(20), Method::Deflated).unwrap();
        w.add("pack.toml", b"format = 2\n".to_vec(), Method::Deflated).unwrap();
        w.add("a.bin", vec![1u8; 3], Method::Stored).unwrap();
        w.finish().unwrap()
    }

    #[test]
    fn writes_deterministic_aligned_sorted() {
        let bytes = sample(Kind::Bundle);
        assert_eq!(bytes, sample(Kind::Bundle));
        let b = Bundle::from_bytes(bytes, Kind::Bundle).unwrap();
        let paths: Vec<&str> = b.entries().iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["a.bin", "pack.toml", "world/L4RA/region.toml", "world/L4RA/sections/1427.bin"]);
        // Data in the order added.
        let mut by_data: Vec<&Entry> = b.entries().iter().collect();
        by_data.sort_by_key(|e| e.data_offset);
        assert_eq!(by_data[0].path, "world/L4RA/sections/1427.bin");
        for e in b.entries().iter().filter(|e| e.method == Method::Stored) {
            assert_eq!(e.data_offset % ALIGN, 0);
            assert!(matches!(b.read(e).unwrap(), Cow::Borrowed(_)));
        }
        let region = b.get("world/L4RA/region.toml").unwrap();
        assert_eq!(&*b.read(region).unwrap(), b"schema = \"ob.region/1.0\"\n".repeat(20).as_slice());
        assert!(region.stored_size < region.size);
        b.verify().unwrap();
        b.check_base_set_payloads().unwrap();
        assert!(b.get("World/L4RA/region.toml").is_none());
        // The existing plain ZIP reader opens it as any ZIP tool would.
        let plain = ob_formats::formats::zip::directory(b.bytes()).unwrap();
        assert_eq!(plain.len(), 4);
        assert_eq!(
            ob_formats::formats::zip::read_entry(b.bytes(), &plain[2]).unwrap(),
            b.read(region).unwrap().to_vec()
        );
    }

    #[test]
    fn pack_file_puts_the_manifest_first() {
        let b = Bundle::from_bytes(sample(Kind::PackFile), Kind::PackFile).unwrap();
        let first = b.entries().iter().min_by_key(|e| e.header_offset).unwrap();
        assert_eq!((first.path.as_str(), first.header_offset), ("pack.toml", 0));
        // Without pack.toml first a pack file is refused; as a bundle the same bytes are fine.
        let bundle = sample(Kind::Bundle);
        assert_eq!(Bundle::from_bytes(bundle.clone(), Kind::PackFile).err().unwrap().rule, Rule::Lay1);
        let mut w = Writer::new(Kind::PackFile);
        w.add("x.toml", b"".to_vec(), Method::Stored).unwrap();
        assert!(w.finish().is_err());
    }

    #[test]
    fn writer_refusals() {
        let mut w = Writer::new(Kind::Bundle);
        w.add("a/b.toml", b"".to_vec(), Method::Stored).unwrap();
        assert!(w.add("a/B.toml", b"".to_vec(), Method::Stored).is_err());
        assert!(w.add("../x", b"".to_vec(), Method::Stored).is_err());
        assert!(w.add(".studio/j.toml", b"".to_vec(), Method::Stored).is_err());
    }

    /// Changes `bytes` at `at` and expects the bundle to be refused under LAY.1.
    fn refused(mut bytes: Vec<u8>, patch: impl FnOnce(&mut Vec<u8>), kind: Kind) -> String {
        patch(&mut bytes);
        let e = Bundle::from_bytes(bytes, kind).err().expect("refused");
        assert_eq!(e.rule, Rule::Lay1, "{e}");
        e.message
    }

    fn cd(b: &[u8]) -> usize {
        let end = b.len() - 22;
        u32::from_le_bytes(b[end + 16..end + 20].try_into().unwrap()) as usize
    }

    #[test]
    fn profile_breaches() {
        let ok = sample(Kind::Bundle);
        let c = cd(&ok);
        // An archive comment.
        refused(ok.clone(), |b| b.extend_from_slice(b"x"), Kind::Bundle);
        // Encryption, a data descriptor, no UTF-8 flag (central flags at +8).
        for flags in [0x0801u16, 0x0808, 0x0000] {
            refused(ok.clone(), |b| b[c + 8..c + 10].copy_from_slice(&flags.to_le_bytes()), Kind::Bundle);
        }
        // An unknown method; another time.
        refused(ok.clone(), |b| b[c + 10..c + 12].copy_from_slice(&14u16.to_le_bytes()), Kind::Bundle);
        refused(ok.clone(), |b| b[c + 12..c + 14].copy_from_slice(&1u16.to_le_bytes()), Kind::Bundle);
        // A bad name (`a.bin` → `.\0bin` would be invalid UTF-8-free but a bad path: `a/bin` stays valid, so use `.`).
        let n = c + 46;
        assert_eq!(&ok[n..n + 5], b"a.bin");
        refused(ok.clone(), |b| b[n] = b'.', Kind::Bundle);
        // The central directory out of order: swap the first byte of two names breaks sorting or the local match.
        refused(ok.clone(), |b| b[n] = b'z', Kind::Bundle);
        // A 64-bit marker in the end record.
        refused(
            ok.clone(),
            |b| {
                let end = b.len() - 22;
                b[end + 16..end + 20].copy_from_slice(&u32::MAX.to_le_bytes());
            },
            Kind::Bundle,
        );
        // A stored entry's data moved off the 64-byte grid: its alignment field one byte shorter.
        let msg = refused(
            ok,
            |b| {
                // The first entry's local header (offset 0) is the stored 1427.bin; drop one pad byte by growing the
                // name length is not possible, so rewrite the alignment value instead.
                let n = u16::from_le_bytes([b[26], b[27]]) as usize;
                b[30 + n + 4] = 32;
            },
            Kind::Bundle,
        );
        assert!(msg.contains("extra") || msg.contains("alignment"), "{msg}");
    }

    #[test]
    fn misaligned_pack_file_entries_are_copied() {
        // A plain ZIP as another tool writes it: no alignment field, so the stored entry is misaligned.
        let mut plain = ob_formats::formats::zip::build(&[("pack.toml", b"format = 2\n", 0), ("x.bin", &[5u8; 10], 0)]);
        // That writer sets neither the UTF-8 flag nor the profile's time; set them as a profile tool would.
        let fix = |b: &mut Vec<u8>| {
            let mut i = 0;
            while i + 4 <= b.len() {
                let sig = u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
                if sig == LOCAL_SIG {
                    b[i + 6..i + 8].copy_from_slice(&UTF8_FLAG.to_le_bytes());
                    b[i + 10..i + 14].copy_from_slice(&[0, 0, 0x21, 0]);
                } else if sig == CENTRAL_SIG {
                    b[i + 8..i + 10].copy_from_slice(&UTF8_FLAG.to_le_bytes());
                    b[i + 12..i + 16].copy_from_slice(&[0, 0, 0x21, 0]);
                }
                i += 1;
            }
        };
        fix(&mut plain);
        let b = Bundle::from_bytes(plain.clone(), Kind::PackFile).unwrap();
        let x = b.get("x.bin").unwrap();
        assert!(!x.mapped());
        assert_eq!(&*b.read(x).unwrap(), &[5u8; 10]);
        assert!(matches!(b.read(x).unwrap(), Cow::Owned(_)));
        b.verify().unwrap();
        assert_eq!(Bundle::from_bytes(plain, Kind::Bundle).err().unwrap().rule, Rule::Lay1);
    }

    #[test]
    fn base_set_payloads_are_stored() {
        let mut w = Writer::new(Kind::Bundle);
        w.add("residual/GLOBAL/GlobalB.lzc.kept.bin", vec![0u8; 100], Method::Deflated).unwrap();
        w.add("global/ai/tuning.bin", vec![0u8; 100], Method::Deflated).unwrap();
        let b = Bundle::from_bytes(w.finish().unwrap(), Kind::Bundle).unwrap();
        let e = b.check_base_set_payloads().unwrap_err();
        assert_eq!(e.path.as_deref(), Some("global/ai/tuning.bin"));
    }

    #[test]
    fn corrupt_deflate_is_caught() {
        let mut bytes = sample(Kind::Bundle);
        let b = Bundle::from_bytes(bytes.clone(), Kind::Bundle).unwrap();
        let e = b.get("world/L4RA/region.toml").unwrap().clone();
        bytes[e.data_offset as usize + 2] ^= 0x55;
        let b = Bundle::from_bytes(bytes, Kind::Bundle).unwrap();
        assert_eq!(b.verify().unwrap_err().rule, Rule::Lay1);
    }
}

//! EA's "big file" archives (`SDATA/sdat.viv`, a "BIG4"), read in place as
//! SPEED2.EXE's file library (realcore `big_vfs.cpp`) reads them.
//!
//! * **Mount** (0x486510, the audio system's start-up): `sdata/sdat.viv`
//!   is looked for under the install folder, else under the CD path
//!   (0x458890). The big-file device is made (0x6F6750, one archive slot
//!   of 0x120 bytes and 12 open-file slots of 0x20), its name is put at
//!   the *front* of the file search path (0x6F6E7A with 1: the device
//!   name, ';', then the old path), and the archive is mounted (0x6F67D7:
//!   the first 0x40 bytes, the header size from them (0x6FA93B), the whole
//!   header read and kept, the archive appended to the device's list,
//!   0x6F805C).
//! * **Header kinds** (0x6FA8D0): big-endian 0xC0FB → 1 (header size =
//!   u16 at +2 plus 4, entries {3-byte offset, 3-byte size, name});
//!   "BIGF" or "Shp…" → 2, "BIG?" → 3 (header size = u32 BE at +0xC,
//!   entries from +0x10). Kind 3 takes its field width from the fourth
//!   character ('4' → 4 bytes, capped at 4). Each entry: offset (BE),
//!   size (BE), NUL-terminated name.
//! * **Lookup** (0x6F69E6 → 0x6FAA2E): an optional "bf:" and one leading
//!   '/' or '\' are dropped, "archive|name" picks one archive, otherwise
//!   the archives are tried in mount order; the entries are scanned in
//!   order and compared case-insensitively (0x6FA8A0 → tolower 0x75DED8),
//!   with no separator folding.
//! * **Order against loose files**: names without a device prefix go
//!   through the search path (0x6F78DC → the default device), which the
//!   mount starts with the big-file device, so a file in the archive wins
//!   over a loose file of the same name; loose files are found after it
// prov: inferred formats.viv.module
//!   (**inferred**: the default device's walk of the path list was not
//!   decompiled).

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::reader::Reader;

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub offset: u64,
    pub size: u64,
}

#[derive(Clone, Debug)]
pub struct Viv {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
}

fn be(b: &[u8]) -> u64 {
    b.iter().fold(0u64, |a, &x| (a << 8) | x as u64)
}

/// The header kind (0x6FA8D0).
pub fn header_kind(h: &[u8]) -> u32 {
    if h.len() < 4 {
        return 0;
    }
    if h[0] == 0xC0 && h[1] == 0xFB {
        1
    } else if &h[..4] == b"BIGF" || &h[..3] == b"Shp" {
        2
    } else if &h[..3] == b"BIG" {
        3
    } else {
        0
    }
}

impl Viv {
    /// Parse a header already in memory (0x6FA93B / 0x6FAA2E). `None`
    /// for an unknown kind or a header cut off inside an entry.
    pub fn parse_header(path: PathBuf, h: &[u8]) -> Option<Viv> {
        let kind = header_kind(h);
        let r = Reader::new(h);
        let (o, end, width_off, width_size) = match kind {
            1 => (6usize, be(r.bytes_at(2, 2).ok()?) as usize + 4, 3usize, 3usize),
            2 | 3 => {
                let w = if kind == 3 { (r.u8_at(3).ok()?.wrapping_sub(b'0') as usize).min(4) } else { 4 };
                (0x10usize, be(r.bytes_at(0xC, 4).ok()?) as usize, 4usize, w)
            }
            _ => return None,
        };
        // The directory: the header up to its declared end.
        let mut dir = Reader::new(r.bytes_at(0, end.min(h.len())).ok()?);
        dir.seek(o.min(dir.len())).ok()?;
        let mut entries = Vec::new();
        while dir.remaining() > width_off + width_size {
            let offset = be(dir.bytes(width_off).ok()?);
            let size = be(dir.bytes(width_size).ok()?);
            let name = String::from_utf8_lossy(dir.cstr().ok()?).into_owned();
            if name.is_empty() {
                break;
            }
            entries.push(Entry { name, offset, size });
        }
        Some(Viv { path, entries })
    }

    /// Mount an archive: read its first 0x40 bytes, then the whole header.
    pub fn open(path: &Path) -> Option<Viv> {
        let mut f = crate::game_fs::open_reader(path).ok()?;
        let mut first = [0u8; 0x40];
        f.read_exact(&mut first).ok()?;
        let size = match header_kind(&first) {
            1 => be(&first[2..4]) as usize + 4,
            2 | 3 => be(&first[0xC..0x10]) as usize,
            _ => return None,
        };
        // A header longer than the file cannot be read whole (read_exact
        // fails below); refuse it before reserving its size.
        if size as u64 > f.seek(SeekFrom::End(0)).ok()? {
            return None;
        }
        let mut h = vec![0u8; size.max(0x40)];
        f.seek(SeekFrom::Start(0)).ok()?;
        f.read_exact(&mut h).ok()?;
        Viv::parse_header(path.to_path_buf(), &h)
    }

    /// The game's mount: `sdata/sdat.viv` under the install folder.
    pub fn mount_game(root: &Path) -> Option<Viv> {
        let p = find_ci(root, "sdata/sdat.viv")?;
        Viv::open(&p)
    }

    /// The entry for a name (0x6F69E6's rules: "bf:" and one leading
    /// slash dropped, compared case-insensitively).
    pub fn find(&self, name: &str) -> Option<&Entry> {
        let mut n = name.strip_prefix("bf:").unwrap_or(name);
        if n.starts_with('/') || n.starts_with('\\') {
            n = &n[1..];
        }
        if let Some((_, rest)) = n.split_once('|') {
            n = rest;
        }
        self.entries.iter().find(|e| e.name.eq_ignore_ascii_case(n))
    }

    /// Read `len` bytes at `at` within an entry (clamped to the entry).
    pub fn read_range(&self, e: &Entry, at: u64, len: usize) -> Option<Vec<u8>> {
        let at = at.min(e.size);
        let len = (len as u64).min(e.size - at) as usize;
        let mut f = crate::game_fs::open_reader(&self.path).ok()?;
        // An entry past the file's end cannot be read whole (read_exact
        // fails below); refuse it before reserving its size.
        if e.offset.saturating_add(at).saturating_add(len as u64) > f.seek(SeekFrom::End(0)).ok()? {
            return None;
        }
        f.seek(SeekFrom::Start(e.offset + at)).ok()?;
        let mut v = vec![0u8; len];
        f.read_exact(&mut v).ok()?;
        Some(v)
    }

    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        let e = self.find(name)?;
        self.read_range(e, 0, e.size as usize)
    }
}

/// A path under `root` matched component by component without case
/// ([`crate::game_fs::find`], which this names for older callers).
pub fn find_ci(root: &Path, rel: &str) -> Option<PathBuf> {
    crate::game_fs::find(root, rel)
}

/// The game's file lookup for a name without a device: the mounted
/// archive first (it heads the search path), then the loose file.
pub fn read_game_file(viv: Option<&Viv>, root: &Path, name: &str) -> Option<Vec<u8>> {
    if let Some(v) = viv.and_then(|v| v.read(name)) {
        return Some(v);
    }
    crate::game_fs::read(root, name).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn big4_header_entries() {
        let mut h = b"BIG4".to_vec();
        h.extend_from_slice(&0u32.to_le_bytes());
        h.extend_from_slice(&1u32.to_be_bytes());
        h.extend_from_slice(&0x30u32.to_be_bytes());
        h.extend_from_slice(&0x40u32.to_be_bytes());
        h.extend_from_slice(&5u32.to_be_bytes());
        h.extend_from_slice(b"a/B.txt\0");
        h.resize(0x40, 0);
        let v = Viv::parse_header(PathBuf::new(), &h).unwrap();
        assert_eq!(v.entries.len(), 1);
        assert_eq!((v.entries[0].offset, v.entries[0].size), (0x40, 5));
        assert!(v.find("bf:/A/b.TXT").is_some());
        assert!(v.find("a\\b.txt").is_none());
    }
}

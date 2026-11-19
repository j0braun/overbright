//! Plain ZIP archives as mod authors ship them: the central directory read
//! once, each entry decompressed only when it is asked for (stored, deflate,
//! or LZMA, the method 7-Zip writes when "LZMA" is chosen for a `.zip`).
//! Encrypted entries, multi-disk archives and the 64-bit extension are refused. The file
//! is memory-mapped and only read.
//!
//! LZMA (method 14, APPNOTE.TXT 5.8.8): a 4-byte header (the encoder's
//! version, then the properties' size, 5) and the 5 properties bytes come
//! before the raw stream; decoded with `lzma-rs` (MIT) for the entry's
//! known size. TexMod's masked, encrypted `.tpf` stays `formats::tpf`.
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use super::reader::Limits;

/// One file of the archive (folders are left out).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The name as stored, `/`-separated.
    pub name: String,
    method: u16,
    flags: u16,
    crc: u32,
    csize: usize,
    pub size: usize,
    local: usize,
}

/// An open archive.
pub struct Zip {
    data: memmap2::Mmap,
    pub entries: Vec<Entry>,
}

fn u16_at(b: &[u8], o: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(b.get(o..o + 2).context("truncated ZIP")?.try_into()?))
}
fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(b.get(o..o + 4).context("truncated ZIP")?.try_into()?))
}

impl Zip {
    /// Opens `path` and reads its directory (a file that is no ZIP fails).
    pub fn open(path: &Path) -> Result<Zip> {
        let f = std::fs::File::open(path).with_context(|| path.display().to_string())?;
        // SAFETY: the map is only read; a file changed meanwhile gives
        // wrong bytes, which the CRC check of each entry catches.
        let data = unsafe { memmap2::Mmap::map(&f) }.with_context(|| path.display().to_string())?;
        let entries = directory(&data)?;
        Ok(Zip { data, entries })
    }

    /// The entry named `name`, compared without case and with either
    /// separator.
    pub fn find(&self, name: &str) -> Option<&Entry> {
        let want = name.replace('\\', "/");
        self.entries.iter().find(|e| e.name.eq_ignore_ascii_case(&want))
    }

    /// The entry's bytes (its CRC checked).
    pub fn read(&self, e: &Entry) -> Result<Vec<u8>> {
        read_entry(&self.data, e)
    }
}

/// The central directory of the archive `b`.
pub fn directory(b: &[u8]) -> Result<Vec<Entry>> {
    ensure!(b.starts_with(b"PK\x03\x04") || b.starts_with(b"PK\x05\x06"), "not a ZIP archive");
    // The end record is within the last 64 KiB + 22 bytes (its comment is
    // at most 65,535 bytes).
    let from = b.len().saturating_sub(22 + 0xFFFF);
    let eocd = (from..b.len().saturating_sub(21))
        .rev()
        .find(|&i| b[i..].starts_with(b"PK\x05\x06"))
        .context("no ZIP directory (a damaged or incomplete download?)")?;
    ensure!(u16_at(b, eocd + 4)? == 0 && u16_at(b, eocd + 6)? == 0, "multi-part ZIP archives are not read");
    let count = u16_at(b, eocd + 10)? as usize;
    let cd = u32_at(b, eocd + 16)? as usize;
    ensure!(cd != 0xFFFF_FFFF && count != 0xFFFF, "64-bit ZIP archives are not read");
    let mut out = Vec::with_capacity(count.min(b.len() / 46));
    let mut p = cd;
    for _ in 0..count {
        ensure!(b.get(p..p + 4) == Some(b"PK\x01\x02"), "bad ZIP directory entry at {p:#x}");
        let flags = u16_at(b, p + 8)?;
        let method = u16_at(b, p + 10)?;
        let crc = u32_at(b, p + 16)?;
        let csize = u32_at(b, p + 20)? as usize;
        let size = u32_at(b, p + 24)? as usize;
        let (nlen, xlen, klen) =
            (u16_at(b, p + 28)? as usize, u16_at(b, p + 30)? as usize, u16_at(b, p + 32)? as usize);
        let local = u32_at(b, p + 42)? as usize;
        let name =
            String::from_utf8_lossy(b.get(p + 46..p + 46 + nlen).context("truncated ZIP name")?).replace('\\', "/");
        p += 46 + nlen + xlen + klen;
        if name.ends_with('/') {
            continue;
        }
        if !super::tpf::safe_name(&name) {
            log::warn!("zip: entry {name:?} left out (a path outside the archive)");
            continue;
        }
        out.push(Entry { name, method, flags, crc, csize, size, local });
    }
    Ok(out)
}

/// One entry of the archive `b` decompressed (CRC and size checked).
pub fn read_entry(b: &[u8], e: &Entry) -> Result<Vec<u8>> {
    let name = &e.name;
    ensure!(e.flags & 1 == 0, "{name}: encrypted ZIP entries are not read");
    Limits::check("ZIP entry size", e.size, Limits::DEFAULT.max_alloc)?;
    ensure!(b.get(e.local..e.local + 4) == Some(b"PK\x03\x04"), "{name}: bad local header");
    let start = e.local + 30 + u16_at(b, e.local + 26)? as usize + u16_at(b, e.local + 28)? as usize;
    let body = b.get(start..start + e.csize).with_context(|| format!("{name}: truncated data"))?;
    let data = match e.method {
        0 => body.to_vec(),
        8 => miniz_oxide::inflate::decompress_to_vec_with_limit(body, e.size.max(1))
            .map_err(|err| anyhow::anyhow!("{name}: inflate failed: {err:?}"))?,
        14 => {
            ensure!(body.len() >= 9 && u16_at(body, 2)? == 5, "{name}: bad LZMA header");
            let mut out = Vec::with_capacity(e.size);
            let opts = lzma_rs::decompress::Options {
                unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(Some(e.size as u64)),
                memlimit: Some(e.size.max(1 << 20) + (64 << 20)),
                allow_incomplete: false,
            };
            lzma_rs::lzma_decompress_with_options(&mut &body[4..], &mut out, &opts)
                .map_err(|err| anyhow::anyhow!("{name}: LZMA failed: {err:?}"))?;
            out
        }
        m => bail!("{name}: ZIP method {m} is not read (stored, deflate and LZMA are)"),
    };
    ensure!(
        data.len() == e.size && super::tpf::zip_crc(&data) == e.crc,
        "{name}: CRC or size mismatch (a damaged download?)"
    );
    Ok(data)
}

/// A ZIP archive of `files` (name, bytes, method 0 or 8), for tests (the
/// library's too).
#[doc(hidden)]
pub fn build(files: &[(&str, &[u8], u16)]) -> Vec<u8> {
    let mut zip = Vec::new();
    let mut cd = Vec::new();
    for (name, data, method) in files {
        let body = match method {
            8 => miniz_oxide::deflate::compress_to_vec(data, 6),
            _ => data.to_vec(),
        };
        let crc = super::tpf::zip_crc(data);
        let local = zip.len() as u32;
        let head = |v: &mut Vec<u8>| {
            v.extend_from_slice(&method.to_le_bytes());
            v.extend_from_slice(&[0; 4]);
            v.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&(body.len() as u32).to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(name.len() as u16).to_le_bytes());
            v.extend_from_slice(&[0, 0]);
        };
        zip.extend_from_slice(b"PK\x03\x04\x14\x00\x00\x00");
        head(&mut zip);
        zip.extend_from_slice(name.as_bytes());
        zip.extend_from_slice(&body);
        cd.extend_from_slice(b"PK\x01\x02\x14\x00\x14\x00\x00\x00");
        head(&mut cd);
        cd.extend_from_slice(&[0; 10]);
        cd.extend_from_slice(&local.to_le_bytes());
        cd.extend_from_slice(name.as_bytes());
    }
    let at = zip.len() as u32;
    zip.extend_from_slice(&cd);
    zip.extend_from_slice(b"PK\x05\x06\x00\x00\x00\x00");
    zip.extend_from_slice(&(files.len() as u16).to_le_bytes());
    zip.extend_from_slice(&(files.len() as u16).to_le_bytes());
    zip.extend_from_slice(&(cd.len() as u32).to_le_bytes());
    zip.extend_from_slice(&at.to_le_bytes());
    zip.extend_from_slice(&[0, 0]);
    zip
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_and_deflated_entries_round_trip() {
        let big = vec![7u8; 5000];
        let zip = build(&[("Binary/script.end", b"[VERSN2]\n", 0), ("Binary/Map/A.dds", &big, 8), ("dir/", b"", 0)]);
        let d = directory(&zip).unwrap();
        assert_eq!(d.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Binary/script.end", "Binary/Map/A.dds"]);
        assert_eq!(read_entry(&zip, &d[0]).unwrap(), b"[VERSN2]\n");
        assert_eq!(read_entry(&zip, &d[1]).unwrap(), big);
    }

    #[test]
    fn lzma_entries_are_read() {
        // An LZMA entry as 7-Zip writes it: version, properties size (5),
        // properties, raw stream. lzma-rs writes `.lzma` (properties, an
        // 8-byte size, stream): its size field is cut out.
        let data: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8 ^ (i / 97) as u8).collect();
        let mut lz = Vec::new();
        lzma_rs::lzma_compress(&mut &data[..], &mut lz).unwrap();
        let mut body = vec![9, 20, 5, 0];
        body.extend_from_slice(&lz[..5]);
        body.extend_from_slice(&lz[13..]);
        let mut zip = build(&[("Map/B.dds", &body, 0)]);
        // Patch the method (14) and the sizes into both headers.
        let d = directory(&zip).unwrap();
        let local = d[0].local;
        let cd = zip.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
        for (at, crc_at) in [(local + 8, local + 14), (cd + 10, cd + 16)] {
            zip[at..at + 2].copy_from_slice(&14u16.to_le_bytes());
            zip[crc_at..crc_at + 4].copy_from_slice(&crate::formats::tpf::zip_crc(&data).to_le_bytes());
            zip[crc_at + 8..crc_at + 12].copy_from_slice(&(data.len() as u32).to_le_bytes());
        }
        let d = directory(&zip).unwrap();
        assert_eq!(read_entry(&zip, &d[0]).unwrap(), data);
        // A changed byte is caught.
        let n = zip.len();
        let mut bad = zip.clone();
        bad[local + 30 + 9 + 40] ^= 0x55;
        assert!(read_entry(&bad, &d[0]).is_err());
        assert_eq!(n, bad.len());
    }

    #[test]
    fn not_a_zip_and_unsafe_names() {
        assert!(directory(b"7z\xBC\xAF\x27\x1C rest").is_err());
        let zip = build(&[("../evil.dds", b"x", 0), ("ok.txt", b"y", 0)]);
        let d = directory(&zip).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].name, "ok.txt");
    }
}

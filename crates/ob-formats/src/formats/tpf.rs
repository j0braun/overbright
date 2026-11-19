//! TexMod packages (`.tpf`) and TexMod's texture hash (
//! docs/modding/texmod-import.md). A `.tpf` is a ZIP archive masked with the dword
//! 0x3FA43FA4 and encrypted with traditional PKWARE (ZipCrypto) under a
//! fixed 42-byte password; its entries are named `<EXE>_0x<HASH>.<ext>`,
//! with an optional `texmod.def` of `0xHASH|file` lines. TexMod keys a
//! replacement by a CRC-32 (reflected polynomial 0xEDB88320, initial
//! 0xFFFFFFFF, **no** final XOR) over `width * height * bpp(format)` bytes
//! of the locked level-0 surface (pitch ignored).
//!
//! The format is documented by the clean-room `retex` project
//! (github.com/Depth118/retex, docs/texmod-format.md, MIT OR Apache-2.0): XOR mask,
//! password, naming, CRC variant and the per-format bpp table. This file is
//! our own implementation from that description (no code copied).
use anyhow::{Context, Result, bail, ensure};

use super::reader::{Limits, Reader};

/// The dword every 4 bytes of a `.tpf` are XORed with (little-endian).
pub const XOR_MASK: u32 = 0x3FA4_3FA4;

/// TexMod's fixed ZipCrypto password.
pub const PASSWORD: [u8; 42] = [
    0x73, 0x2A, 0x63, 0x7D, 0x5F, 0x0A, 0xA6, 0xBD, 0x7D, 0x65, 0x7E, 0x67, 0x61, 0x2A, 0x7F, 0x7F, 0x74, 0x61, 0x67, 0x5B, 0x60, 0x70, 0x45,
    0x74, 0x5C, 0x22, 0x74, 0x5D, 0x6E, 0x6A, 0x73, 0x41, 0x77, 0x6E, 0x46, 0x47, 0x77, 0x49, 0x0C, 0x4B, 0x46, 0x6F,
];

fn crc_table() -> &'static [u32; 256] {
    static T: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        std::array::from_fn(|n| {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            c
        })
    })
}

fn crc_update(mut c: u32, bytes: &[u8]) -> u32 {
    let t = crc_table();
    for &b in bytes {
        c = t[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c
}

/// TexMod's checksum: CRC-32 without the final XOR.
pub fn texmod_crc(bytes: &[u8]) -> u32 {
    crc_update(0xFFFF_FFFF, bytes)
}

/// Standard CRC-32 (ZIP).
pub fn zip_crc(bytes: &[u8]) -> u32 {
    !crc_update(0xFFFF_FFFF, bytes)
}

/// Undo the dword mask in place (a trailing partial dword is masked with
/// the mask's leading bytes).
pub fn unmask(bytes: &mut [u8]) {
    let m = XOR_MASK.to_le_bytes();
    for (i, b) in bytes.iter_mut().enumerate() {
        *b ^= m[i % 4];
    }
}

struct ZipCrypto([u32; 3]);

impl ZipCrypto {
    fn new(password: &[u8]) -> Self {
        let mut z = ZipCrypto([0x1234_5678, 0x2345_6789, 0x3456_7890]);
        for &b in password {
            z.update(b);
        }
        z
    }
    fn update(&mut self, b: u8) {
        let k = &mut self.0;
        k[0] = crc_table()[((k[0] ^ b as u32) & 0xFF) as usize] ^ (k[0] >> 8);
        k[1] = k[1].wrapping_add(k[0] & 0xFF).wrapping_mul(134_775_813).wrapping_add(1);
        k[2] = crc_table()[((k[2] ^ (k[1] >> 24)) & 0xFF) as usize] ^ (k[2] >> 8);
    }
    fn decrypt(&mut self, c: u8) -> u8 {
        let t = (self.0[2] | 2) as u16;
        let p = c ^ (t.wrapping_mul(t ^ 1) >> 8) as u8;
        self.update(p);
        p
    }
}

/// One file of a package.
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
}

/// A decoded package: its files and the `texmod.def` map (hash → file).
pub struct Package {
    pub entries: Vec<Entry>,
    pub comment: String,
}

/// The hash in an entry name `<EXE>_0x<8 hex>.<ext>` (or a bare `0x…`).
pub fn name_hash(name: &str) -> Option<u32> {
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    let i = stem.rfind("0x").or_else(|| stem.rfind("0X"))?;
    let hex = &stem[i + 2..];
    (hex.len() == 8).then(|| u32::from_str_radix(hex, 16).ok()).flatten()
}

/// `texmod.def`: `0xHASH|file` lines (other lines ignored).
pub fn parse_def(text: &str) -> Vec<(u32, String)> {
    text.lines()
        .filter_map(|l| {
            let (h, f) = l.trim().split_once('|')?;
            let h = h.trim().trim_start_matches("0x").trim_start_matches("0X");
            Some((u32::from_str_radix(h, 16).ok()?, f.trim().to_string()))
        })
        .collect()
}

fn u16_at(b: &Reader<'_>, o: usize) -> Result<u16> {
    b.u16_at(o).context("truncated ZIP")
}
fn u32_at(b: &Reader<'_>, o: usize) -> Result<u32> {
    b.u32_at(o).context("truncated ZIP")
}

/// All entries of one package together (each is held to `Limits::max_alloc`).
pub const MAX_TOTAL: usize = 1 << 30;

/// A name that stays inside the package: not absolute, no drive, no `..`
/// component (either separator), no NUL.
pub fn safe_name(name: &str) -> bool {
    !(name.starts_with(['/', '\\']) || name.contains(['\0', ':']) || name.split(['/', '\\']).any(|c| c == ".."))
}

/// Read a `.tpf` (masked) or an already unmasked ZIP. Entries are read
/// through the central directory; encrypted ones with TexMod's password.
pub fn read(raw: &[u8]) -> Result<Package> {
    let mut bytes = raw.to_vec();
    if !bytes.starts_with(b"PK\x03\x04") {
        unmask(&mut bytes);
    }
    ensure!(bytes.starts_with(b"PK\x03\x04"), "not a TexMod package (no ZIP header after unmasking)");
    // The end-of-central-directory record, searched from the end (the
    // comment may follow it, and TexMod's tail bytes may be garbled).
    let eocd = (0..bytes.len().saturating_sub(21)).rev().find(|&i| bytes[i..].starts_with(b"PK\x05\x06")).context("no ZIP directory")?;
    let bytes = &Reader::new(&bytes);
    let count = u16_at(bytes, eocd + 10)? as usize;
    let cd = u32_at(bytes, eocd + 16)? as usize;
    let clen = u16_at(bytes, eocd + 20)? as usize;
    // The comment, cut at the end of the file.
    let at = (eocd + 22).min(bytes.len());
    let comment = String::from_utf8_lossy(bytes.bytes_at(at, clen.min(bytes.len() - at))?).into_owned();
    // A directory entry is at least 46 bytes: a count the file cannot hold
    // reserves nothing.
    let mut entries = Vec::with_capacity(count.min(bytes.len() / 46));
    let mut p = cd;
    let mut total = 0usize;
    for _ in 0..count {
        ensure!(bytes.bytes_at(p, 4).ok() == Some(b"PK\x01\x02"), "bad ZIP directory entry at {p:#x}");
        let flags = u16_at(bytes, p + 8)?;
        let method = u16_at(bytes, p + 10)?;
        let mtime = u16_at(bytes, p + 12)?;
        let crc = u32_at(bytes, p + 16)?;
        let csize = u32_at(bytes, p + 20)? as usize;
        let usize_ = u32_at(bytes, p + 24)? as usize;
        let (nlen, xlen, klen) = (u16_at(bytes, p + 28)? as usize, u16_at(bytes, p + 30)? as usize, u16_at(bytes, p + 32)? as usize);
        let local = u32_at(bytes, p + 42)? as usize;
        let name = String::from_utf8_lossy(bytes.bytes_at(p + 46, nlen).ok().context("truncated name")?).into_owned();
        p += 46 + nlen + xlen + klen;
        // One entry inflates to at most this (a declared size is the
        // inflate limit; a ZIP bomb could declare 4 GB).
        Limits::check("ZIP entry size", usize_, Limits::DEFAULT.max_alloc)?;
        ensure!(bytes.bytes_at(local, 4).ok() == Some(b"PK\x03\x04"), "bad local header for {name}");
        let start = local + 30 + u16_at(bytes, local + 26)? as usize + u16_at(bytes, local + 28)? as usize;
        let mut body = bytes.bytes_at(start, csize).ok().with_context(|| format!("truncated data for {name}"))?.to_vec();
        if flags & 1 != 0 {
            ensure!(body.len() >= 12, "short encrypted entry {name}");
            let mut z = ZipCrypto::new(&PASSWORD);
            for b in body.iter_mut() {
                *b = z.decrypt(*b);
            }
            // The header's last byte checks the password (CRC high byte, or
            // the DOS time's high byte with a data descriptor).
            let check = if flags & 8 != 0 { (mtime >> 8) as u8 } else { (crc >> 24) as u8 };
            ensure!(body[11] == check, "{name}: wrong password (not a TexMod package?)");
            body.drain(..12);
        }
        let data = match method {
            0 => body,
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(&body, usize_.max(1)).map_err(|e| anyhow::anyhow!("{name}: inflate failed: {e:?}"))?,
            m => bail!("{name}: unsupported ZIP method {m}"),
        };
        ensure!(data.len() == usize_ && zip_crc(&data) == crc, "{name}: CRC or size mismatch");
        if name.ends_with('/') {
            continue;
        }
        // All the entries together stay within the limit
        // (a ZIP bomb: many entries that each inflate to the entry limit),
        // and an entry named to leave its folder is dropped.
        total += data.len();
        Limits::check("ZIP total size", total, MAX_TOTAL)?;
        if !safe_name(&name) {
            log::warn!("tpf: entry {name:?} dropped (path outside the package)");
            continue;
        }
        entries.push(Entry { name, data });
    }
    Ok(Package { entries, comment })
}

/// Bytes per pixel TexMod assumes for a D3D9 format (its float table;
/// unknown formats, e.g. D3DFMT_P8, count 0.5).
pub fn bpp(format: crate::formats::texture::PixelFormat) -> f32 {
    use crate::formats::texture::PixelFormat::*;
    match format {
        Dxt1 => 0.5,
        Dxt3 | Dxt5 | Bc7 => 1.0,
        Bgra8 | Rgba8 => 4.0,
        P8 => 0.5,
    }
}

/// TexMod's hash of `bytes` as the level-0 surface of a `w`×`h` texture
/// whose locked data starts with `bytes` (`len = w*h*bpp`, truncated like
/// TexMod's float-to-int; `None` when `bytes` is shorter than that, i.e.
/// the hash would read past the level, which needs the real surface).
pub fn surface_hash(bytes: &[u8], w: u32, h: u32, bpp: f32) -> Option<u32> {
    let len = (w as f32 * h as f32 * bpp) as usize;
    bytes.get(..len).map(texmod_crc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_variant_matches_retex_vector() {
        // retex docs/texmod-format.md: bytes(i % 256) for 1 KiB -> 0x48F4B3D9,
        // the value TexMod's own CRC routine returns.
        let v: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        assert_eq!(texmod_crc(&v), 0x48F4_B3D9);
        assert_eq!(zip_crc(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn names_and_def() {
        assert_eq!(name_hash("SPEED2.EXE_0x018FDB67.dds"), Some(0x018F_DB67));
        assert_eq!(name_hash("texmod.def"), None);
        assert_eq!(parse_def("0x018FDB67|SPEED2.EXE_0x018FDB67.dds\r\nnoise\n"), vec![(0x018F_DB67, "SPEED2.EXE_0x018FDB67.dds".into())]);
    }

    /// A stored, encrypted, masked one-file archive round-trips.
    #[test]
    fn reads_a_masked_encrypted_zip() {
        let data = b"hello texmod".to_vec();
        let crc = zip_crc(&data);
        // Encrypt with a 12-byte header whose last byte is the CRC's high byte.
        let mut z = ZipCrypto::new(&PASSWORD);
        let mut enc = Vec::new();
        let mut header = [0u8; 12];
        header[11] = (crc >> 24) as u8;
        for &p in header.iter().chain(data.iter()) {
            let t = (z.0[2] | 2) as u16;
            enc.push(p ^ (t.wrapping_mul(t ^ 1) >> 8) as u8);
            z.update(p);
        }
        let name = b"GAME.EXE_0x12345678.dds";
        let mut zip = Vec::new();
        let local = |zip: &mut Vec<u8>| {
            zip.extend_from_slice(b"PK\x03\x04");
            zip.extend_from_slice(&[20, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
            zip.extend_from_slice(&crc.to_le_bytes());
            zip.extend_from_slice(&(enc.len() as u32).to_le_bytes());
            zip.extend_from_slice(&(data.len() as u32).to_le_bytes());
            zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
            zip.extend_from_slice(&[0, 0]);
            zip.extend_from_slice(name);
            zip.extend_from_slice(&enc);
        };
        local(&mut zip);
        let cd = zip.len();
        zip.extend_from_slice(b"PK\x01\x02");
        zip.extend_from_slice(&[20, 0, 20, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
        zip.extend_from_slice(&crc.to_le_bytes());
        zip.extend_from_slice(&(enc.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(data.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&[0; 12]);
        zip.extend_from_slice(&0u32.to_le_bytes());
        zip.extend_from_slice(name);
        let cdlen = zip.len() - cd;
        zip.extend_from_slice(b"PK\x05\x06");
        zip.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
        zip.extend_from_slice(&(cdlen as u32).to_le_bytes());
        zip.extend_from_slice(&(cd as u32).to_le_bytes());
        zip.extend_from_slice(&[0, 0]);
        unmask(&mut zip); // masking is its own inverse
        let p = read(&zip).unwrap();
        assert_eq!(p.entries.len(), 1);
        assert_eq!(p.entries[0].data, data);
        assert_eq!(name_hash(&p.entries[0].name), Some(0x1234_5678));
    }
}

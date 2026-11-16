//! The language string table (LANGUAGES/<language>.bin, chunk 0x39000).
//!
//! ```text
//! u32 header size (16)   u32 count   u32 table offset   u32 text offset
//! u32 256, then 256 × u16: the character map (a non-zero entry is the
//!   code point a byte shows as: English maps 0x81 ’, 0x82 ™, 0x83 –,
//!   0x84 ”, 0x86 …, 0x87 “, 0x88 „; zero keeps the byte; 1..0x7F
//!   starts a double-byte pair, see [`decode`]; bytes under 0x80 are
//!   never mapped)
//! table: count × { u32 hash, u32 offset into the text }, sorted by hash
//! text:  NUL-terminated Latin-1 strings
//! ```
//!
//! The hash is `bin_hash` of the label (e.g. "DEFAULT_STRING").

use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::formats::bundle::chunks;

/// A little-endian u32 of the table, 0 past its end (fuzz: counts and
/// offsets come from the file).
fn read_u32(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map_or(0, |w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}

pub struct Strings {
    pub map: HashMap<u32, String>,
}

impl Strings {
    pub fn parse(file: &[u8]) -> Result<Strings> {
        let chunk = chunks(file, 0).find(|c| c.id == 0x39000).context("no string table")?;
        let b = chunk.body;
        // At most as many entries as the table has room for.
        let count = (read_u32(b, 4) as usize).min(b.len().saturating_sub(read_u32(b, 8) as usize) / 8);
        let table = read_u32(b, 8) as usize;
        let text = read_u32(b, 12) as usize;
        // The character map after the header (Wine, menus15: Options' "EA
        // GAMES\x82 TRAX" shows "EA GAMES™ TRAX"). The chunk handler 0x5125B0
        // keeps it at 0x8383DC (the u32 256, then the u16 entries) and the
        // FE texts convert through 0x50D3C0 ([`decode`]).
        let hdr = read_u32(b, 0) as usize;
        let map = (b.len() >= hdr + 4 + 512 && read_u32(b, hdr) == 256).then(|| &b[hdr..]);
        let mut out = HashMap::with_capacity(count);
        for i in 0..count {
            let hash = read_u32(b, table + i * 8);
            let at = text + read_u32(b, table + i * 8 + 4) as usize;
            out.insert(hash, decode(map, &b[at.min(b.len())..]));
        }
        let map = out;
        Ok(Strings { map })
    }

    pub fn get(&self, hash: u32) -> Option<&str> {
        self.map.get(&hash).map(String::as_str)
    }

    /// The text for a label name, or the name itself.
    pub fn label(&self, name: &str) -> String {
        self.get(crate::formats::hash::bin_hash(name)).map_or_else(|| name.to_string(), str::to_string)
    }
}

/// 0x50D3C0, the language text converter (ECX = the map: u32 256, then
/// u16 entries). Per byte `c`: under 0x80 it is itself; else `v =
/// entry[c]`: 0 keeps the byte, ≥ 0x80 is the character, and 1..0x7F is a
/// double-byte lead: the next byte `d` is always taken, and under 0x80
/// gives '_' (the error flag), else the u16 at map + 2·((v << 7) + d) −
/// 0xFC (entry v·128 + d − 0x80), '_' when that is 0. The text ends at the
/// first character 0 written — a '_' from a lead before the NUL runs on.
/// Without a map (0x43E000) every byte is itself. The global 0x838530 that
/// makes every byte itself is taken as clear (its writer is not traced).
pub(crate) fn decode(map: Option<&[u8]>, text: &[u8]) -> String {
    let entry = |m: &[u8], k: usize| m.get(4 + 2 * k..6 + 2 * k).map_or(0, |w| u16::from_le_bytes([w[0], w[1]]));
    let mut out = String::new();
    let mut i = 0;
    while let Some(&c) = text.get(i) {
        i += 1;
        let ch: u32 = match map {
            Some(m) if c >= 0x80 => {
                let v = entry(m, usize::from(c));
                if v == 0 {
                    u32::from(c)
                } else if v >= 0x80 {
                    u32::from(v)
                } else {
                    let d = text.get(i).copied().unwrap_or(0);
                    i += 1;
                    if d < 0x80 {
                        u32::from(b'_')
                    } else {
                        match entry(m, (usize::from(v) << 7) + usize::from(d) - 0x80) {
                            0 => u32::from(b'_'),
                            w => u32::from(w),
                        }
                    }
                }
            }
            _ => u32::from(c),
        };
        if ch == 0 {
            break;
        }
        out.push(char::from_u32(ch).unwrap_or('_'));
    }
    out
}

#[cfg(test)]
mod menu_text_tests {
    /// The long dialog texts the menus show (R8) are read from the player's
    /// language file by hash, like every other menu text: each is there,
    /// pinned by the SHA-256 of its English text (the source carries none).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn dialog_texts_come_from_the_language_file() {
        const TEXTS: [(u32, &str); 10] = [
            (0x3F92_0248, "8fda98ff80e551468d44bd1dde6166398318372d6a5875acae8cbabdd2c7a0f3"),
            (0x60D2_4D65, "0f2b3fd80ee0a3ef315614b92180823ae9d18482d8b6385a3ca00d940989fbaf"),
            (0x20B3_0CD8, "8e9f0d577970b93e3cf9448ce39357e04e89302b9c0c340df00ec2b98ab8acdf"),
            (0xD649_BE08, "131ad2811235d1665933eab9c8f1025022a7699c11fadee4bd8d693fbddaaf14"),
            (0xA9E9_66D7, "2aa0216ddab2b0a0aa791c50809b66ddbb919dbc60131a349753cd5359e869fa"),
            (0x7185_1654, "f34b868ffa038e36e1597d2dcbd375e3168939b2d322a4a702c42e5f52e11c4c"),
            (0xD366_20FE, "27e56232b2d45da720c73d13bc554bcecb5af3f8b0d22994d4563e1958b717e4"),
            (0x9038_54F0, "3220505fcce82b663635bff8f7b73179cfd4379a72d290c5a5d02a17c8a4b2e1"),
            (0xD1AC_60BE, "7eef067c9064b86420cfb9c9817129d1e70d4aa14f35a94c4d9c3bdb80748d15"),
            (0x2A9B_A6F9, "59bb3c8d661e5a401940d900f563a912f187a15ba8e52f84055101fff1fb17e1"),
        ];
        let root = crate::testkit::game_root();
        let file = crate::game_fs::read(&root, "LANGUAGES/English.bin").expect("LANGUAGES/English.bin");
        let strings = super::Strings::parse(&file).expect("the English strings");
        for (hash, sha) in TEXTS {
            let text = strings.get(hash).unwrap_or_else(|| panic!("text {hash:#010x} not in English.bin"));
            assert_eq!(crate::fingerprint::sha256_hex(text.as_bytes()), sha, "text {hash:#010x}");
        }
    }
}

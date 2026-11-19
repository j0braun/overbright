//! Digests (conventions.md §7): `"<alg>:<lower-case hex>"`, the grammar of OCI descriptors. `sha256` for
//! everything compared between machines, `xxh3-128` for local content addressing; digests of lists hash a
//! defined byte sequence (a domain tag, then per entry its path, a NUL and its raw digest, sorted by path).

use sha2::Digest as _;

use crate::{Refusal, Rule};

/// A digest algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Alg {
    Sha256,
    Xxh3_128,
}

impl Alg {
    pub const fn name(self) -> &'static str {
        match self {
            Alg::Sha256 => "sha256",
            Alg::Xxh3_128 => "xxh3-128",
        }
    }

    /// The raw digest's length in bytes.
    pub const fn digest_len(self) -> usize {
        match self {
            Alg::Sha256 => 32,
            Alg::Xxh3_128 => 16,
        }
    }

    /// The digest of `bytes`.
    pub fn of(self, bytes: &[u8]) -> Digest {
        match self {
            Alg::Sha256 => Digest::Sha256(sha2::Sha256::digest(bytes).into()),
            // The 128-bit value is written high 64 bits first.
            Alg::Xxh3_128 => Digest::Xxh3_128(xxhash_rust::xxh3::xxh3_128(bytes).to_be_bytes()),
        }
    }
}

/// A digest value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Digest {
    Sha256([u8; 32]),
    Xxh3_128([u8; 16]),
}

impl Digest {
    pub fn alg(&self) -> Alg {
        match self {
            Digest::Sha256(_) => Alg::Sha256,
            Digest::Xxh3_128(_) => Alg::Xxh3_128,
        }
    }

    /// The raw digest bytes (what lists hash).
    pub fn bytes(&self) -> &[u8] {
        match self {
            Digest::Sha256(b) => b,
            Digest::Xxh3_128(b) => b,
        }
    }

    /// Reads `"<alg>:<hex>"`; an unknown algorithm, a wrong length or upper-case hex is `CONV.8`.
    pub fn parse(s: &str) -> Result<Digest, Refusal> {
        let bad = |why: &str| Err(Refusal::new(Rule::Conv8, format!("{s:?} is not a digest: {why}")));
        let Some((alg, hex)) = s.split_once(':') else { return bad("no `<alg>:`") };
        let alg = match alg {
            "sha256" => Alg::Sha256,
            "xxh3-128" => Alg::Xxh3_128,
            _ => return bad("unknown algorithm"),
        };
        if hex.len() != alg.digest_len() * 2 || !hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return bad(&format!("{} lower-case hex digits expected", alg.digest_len() * 2));
        }
        let raw: Vec<u8> = (0..alg.digest_len())
            .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("checked hex"))
            .collect();
        Ok(match alg {
            Alg::Sha256 => Digest::Sha256(raw.try_into().expect("32 bytes")),
            Alg::Xxh3_128 => Digest::Xxh3_128(raw.try_into().expect("16 bytes")),
        })
    }
}

impl std::fmt::Display for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:", self.alg().name())?;
        self.bytes().iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

/// The domain tag of a pack's content digest (manifest.md §4).
pub const PACK_CONTENT: &[u8] = b"ob-pack-content/1\0";
/// The domain tag of a cooked unit's inputs (cooked.md).
pub const COOK_INPUTS: &[u8] = b"ob-cook-inputs/1\0";

/// The digest of a list: `tag`, then for each entry, sorted by path (byte order), the path's UTF-8 bytes, a NUL
/// and the entry's raw digest. Every entry's digest must be of `alg`.
pub fn list(alg: Alg, tag: &[u8], entries: &[(&str, Digest)]) -> Digest {
    let mut sorted: Vec<&(&str, Digest)> = entries.iter().collect();
    sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut bytes = tag.to_vec();
    for (path, d) in sorted {
        assert_eq!(d.alg(), alg, "a list digest's entries use its own algorithm");
        bytes.extend_from_slice(path.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(d.bytes());
    }
    alg.of(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values() {
        assert_eq!(
            Alg::Sha256.of(b"abc").to_string(),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // XXH3-128 of the empty input (the reference implementation's test vector), high half first.
        assert_eq!(Alg::Xxh3_128.of(b"").to_string(), "xxh3-128:99aa06d3014798d86001c324468d497f");
    }

    #[test]
    fn parse_round_trip() {
        // manifest.md §1's example.
        let s = "xxh3-128:5c1f0e6a9d3b4f2c8e7a1b0d9c6f3e21";
        assert_eq!(Digest::parse(s).unwrap().to_string(), s);
        let d = Alg::Sha256.of(b"x");
        assert_eq!(Digest::parse(&d.to_string()).unwrap(), d);
        for bad in ["md5:00", "sha256:ABC", "xxh3-128:5C1F0E6A9D3B4F2C8E7A1B0D9C6F3E21", "xxh3-128:00", "sha256"] {
            assert_eq!(Digest::parse(bad).unwrap_err().rule, Rule::Conv8, "{bad}");
        }
    }

    #[test]
    fn list_digest_is_order_independent_and_path_sensitive() {
        let a = Alg::Xxh3_128.of(b"a");
        let b = Alg::Xxh3_128.of(b"b");
        let one = list(Alg::Xxh3_128, PACK_CONTENT, &[("x/a", a), ("x/b", b)]);
        assert_eq!(one, list(Alg::Xxh3_128, PACK_CONTENT, &[("x/b", b), ("x/a", a)]));
        assert_ne!(one, list(Alg::Xxh3_128, PACK_CONTENT, &[("x/a", b), ("x/b", a)]));
        assert_ne!(one, list(Alg::Xxh3_128, COOK_INPUTS, &[("x/a", a), ("x/b", b)]));
        // The bytes are exactly tag, path, NUL, raw digest.
        let mut bytes = PACK_CONTENT.to_vec();
        bytes.extend_from_slice(b"x/a\0");
        bytes.extend_from_slice(a.bytes());
        assert_eq!(list(Alg::Xxh3_128, PACK_CONTENT, &[("x/a", a)]), Alg::Xxh3_128.of(&bytes));
    }
}

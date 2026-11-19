//! The string hash the engine uses to name everything (solids, textures,
//! materials). Verified against the data: `bin_hash("PAN_STADIUM_Z_E96")`
//! equals the hash stored in that solid's header (0x3BE35F00).

pub const fn bin_hash(name: &str) -> u32 {
    let b = name.as_bytes();
    let mut h = 0xFFFF_FFFF_u32;
    let mut i = 0;
    while i < b.len() {
        h = h.wrapping_mul(33).wrapping_add(b[i] as u32);
        i += 1;
    }
    h
}

/// Continue a hash with more text: `bin_hash_from(bin_hash(a), b)` is
/// `bin_hash(a + b)` (0x43DB80 appends "_TITLE" to a cover's name so).
pub fn bin_hash_from(seed: u32, more: &str) -> u32 {
    more.bytes().fold(seed, |h, c| h.wrapping_mul(33).wrapping_add(c as u32))
}

/// The header name field holds 27 characters; longer names are cut off but
/// the stored hash is of the full name. Recover up to `max_extra` missing
/// characters by trying every completion from `A-Z 0-9 _`.
pub fn complete_truncated_name(truncated: &str, hash: u32, max_extra: usize) -> Option<String> {
    if bin_hash(truncated) == hash {
        return Some(truncated.to_string());
    }
    // The same solids are parsed again by every load that reads their file
    // (each car load reads the shared part files): the answer is kept.
    type Memo = std::collections::HashMap<(String, u32, usize), Option<String>>;
    static MEMO: std::sync::OnceLock<std::sync::Mutex<Memo>> = std::sync::OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    let key = (truncated.to_string(), hash, max_extra);
    if let Some(found) = memo.lock().unwrap().get(&key) {
        return found.clone();
    }
    let found = complete_uncached(truncated, hash, max_extra);
    memo.lock().unwrap().insert(key, found.clone());
    found
}

fn complete_uncached(truncated: &str, hash: u32, max_extra: usize) -> Option<String> {
    const ALPHABET: &[u8] = b"_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    fn search(h: u32, target: u32, depth: usize, suffix: &mut Vec<u8>) -> bool {
        if depth == 0 {
            return false;
        }
        // The last character is solved, not searched: 33 is odd, so
        // `h * 33 + c == target` has one `c` (the same first match the
        // full walk finds, in 36x fewer steps).
        if depth == 1 {
            let c = target.wrapping_sub(h.wrapping_mul(33));
            if c < 256 && ALPHABET.contains(&(c as u8)) {
                suffix.push(c as u8);
                return true;
            }
            return false;
        }
        // The last two likewise: `c1 * 33 + c2` must be the rest, which
        // for most `h` is out of the alphabet's reach at once. (No shorter
        // suffix matches here: the shorter depths were all tried first.)
        if depth == 2 {
            let rest = target.wrapping_sub(h.wrapping_mul(33 * 33));
            if !(b'0' as u32 * 34..=b'_' as u32 * 34).contains(&rest) {
                return false;
            }
            for &c1 in ALPHABET {
                let c2 = rest.wrapping_sub(c1 as u32 * 33);
                if c2 < 256 && ALPHABET.contains(&(c2 as u8)) {
                    suffix.extend([c1, c2 as u8]);
                    return true;
                }
            }
            return false;
        }
        for &c in ALPHABET {
            let next = h.wrapping_mul(33).wrapping_add(c as u32);
            suffix.push(c);
            if next == target || search(next, target, depth - 1, suffix) {
                return true;
            }
            suffix.pop();
        }
        false
    }
    let mut suffix = Vec::new();
    for depth in 1..=max_extra {
        suffix.clear();
        if search(bin_hash(truncated), hash, depth, &mut suffix) {
            return Some(format!("{truncated}{}", String::from_utf8_lossy(&suffix)));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_value_stored_in_game_data() {
        assert_eq!(bin_hash("PAN_STADIUM_Z_E96"), 0x3BE3_5F00);
    }

    #[test]
    fn completes_truncated_names() {
        for full in ["PAN_STADIUM_Z_E96", "AB_9", "X_Z", "Q0", "ABC_DEFG"] {
            for cut in 1..=4.min(full.len()) {
                let t = &full[..full.len() - cut];
                let got = complete_truncated_name(t, bin_hash(full), 4).unwrap();
                assert_eq!(bin_hash(&got), bin_hash(full));
                assert!(got.len() <= full.len());
            }
        }
        assert_eq!(complete_truncated_name("ABC", bin_hash("ABCD") ^ 0x8000_0000, 2), None);
    }

    /// The solved search finds what the plain walk over every completion
    /// finds (the first in alphabet order at the least length).
    #[test]
    fn completion_matches_full_walk() {
        const ALPHABET: &[u8] = b"_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        fn walk(h: u32, target: u32, depth: usize, suffix: &mut Vec<u8>) -> bool {
            if depth == 0 {
                return false;
            }
            for &c in ALPHABET {
                let next = h.wrapping_mul(33).wrapping_add(c as u32);
                suffix.push(c);
                if next == target || walk(next, target, depth - 1, suffix) {
                    return true;
                }
                suffix.pop();
            }
            false
        }
        let reference = |t: &str, hash: u32, max: usize| {
            (1..=max).find_map(|d| {
                let mut suffix = Vec::new();
                walk(bin_hash(t), hash, d, &mut suffix).then(|| format!("{t}{}", String::from_utf8_lossy(&suffix)))
            })
        };
        let mut x = 0x1234_5678u32;
        for i in 0..60u32 {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let base = format!("SOLID_{i}_");
            let suffix: String = (0..1 + i as usize % 3).map(|k| ALPHABET[((x >> (k * 5)) % 36) as usize] as char).collect();
            for target in [bin_hash(&format!("{base}{suffix}")), x, bin_hash(&format!("{base}{suffix}")).wrapping_add(33)] {
                assert_eq!(complete_uncached(&base, target, 3), reference(&base, target, 3), "{base} {target:#x}");
            }
        }
    }
}

/// The front end's object-name hash (0x505450): `bin_hash` of the name in
/// upper case, so "LapNumber" and "LAPNUMBER" name the same object.
pub fn fe_hash(name: &str) -> u32 {
    // `bin_hash(&name.to_ascii_uppercase())` without the copy (the HUD
    // hashes its object names every frame).
    name.bytes().fold(0xFFFF_FFFF_u32, |h, c| h.wrapping_mul(33).wrapping_add(c.to_ascii_uppercase() as u32))
}

/// `bin_hash(&name.to_uppercase())` (Unicode upper case, as the texture and
/// font lookups by file name take it) without the copy for ASCII names.
pub fn upper_hash(name: &str) -> u32 {
    if name.is_ascii() {
        fe_hash(name)
    } else {
        bin_hash(&name.to_uppercase())
    }
}

#[cfg(test)]
mod upper_tests {
    use super::*;

    #[test]
    fn fe_and_upper_hash_match_the_copying_forms() {
        for n in ["LapNumber", "3rdPersonSpeed", "tach_fill", "a_b-c.9", "", "Straße", "ÄÖÜ_hud", "MiXeD_cAsE_123"] {
            assert_eq!(fe_hash(n), bin_hash(&n.to_ascii_uppercase()), "{n}");
            assert_eq!(upper_hash(n), bin_hash(&n.to_uppercase()), "{n}");
        }
    }
}

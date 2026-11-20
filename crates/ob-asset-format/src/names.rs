//! Names, keys and references (conventions.md §6): pack ids, logical paths, game names, 32-bit hash keys,
//! `nameref` values, references between items and the tools' `ob://` addresses.

use crate::{Refusal, Rule};

/// Whether `id` is a pack id: `[a-z0-9][a-z0-9_-]{0,63}` (§6.1).
pub fn is_pack_id(id: &str) -> bool {
    let b = id.as_bytes();
    (1..=64).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}

/// A pack id, or a refusal under `rule` (`CONV.8`, or the page's own rule such as `MAN.2`).
pub fn pack_id(id: &str, rule: Rule) -> Result<&str, Refusal> {
    if is_pack_id(id) {
        Ok(id)
    } else {
        Err(Refusal::new(rule, format!("{id:?} is not a pack id ([a-z0-9][a-z0-9_-]{{0,63}})")))
    }
}

/// The longest logical path, in bytes.
pub const MAX_PATH_BYTES: usize = 240;
/// The longest component of a logical path, in characters.
pub const MAX_COMPONENT: usize = 100;
/// The authoring folder a pack folder may hold (never in a bundle).
pub const STUDIO_DIR: &str = ".studio";

/// Checks a logical path (§6.2): `/`-separated components of `[A-Za-z0-9._-]`, each 1–100 characters, not
/// `.` or `..`, none starting with `.` except a first component `.studio` when `allow_studio`; at most 240
/// bytes; no leading or trailing `/`. Refused as `CONV.13`.
pub fn logical_path(path: &str, allow_studio: bool) -> Result<(), Refusal> {
    let bad = |why: &str| Err(Refusal::new(Rule::Conv13, format!("{path:?} is not a logical path: {why}")));
    if path.is_empty() {
        return bad("empty");
    }
    if path.len() > MAX_PATH_BYTES {
        return bad("longer than 240 bytes");
    }
    for (i, c) in path.split('/').enumerate() {
        if c.is_empty() {
            return bad("an empty component (a leading, trailing or doubled `/`)");
        }
        if c.len() > MAX_COMPONENT {
            return bad("a component longer than 100 characters");
        }
        if let Some(ch) = c.chars().find(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))) {
            return bad(&format!("the character {ch:?}"));
        }
        if c.starts_with('.') && !(i == 0 && allow_studio && c == STUDIO_DIR) {
            return bad("a component starting with `.`");
        }
    }
    Ok(())
}

/// Checks that no two of `paths` differ only in ASCII case (`CONV.13`): they would clash on Windows and macOS.
/// The refusal names both paths.
pub fn case_unique<'a>(paths: impl IntoIterator<Item = &'a str>) -> Result<(), Refusal> {
    let mut seen = std::collections::HashMap::new();
    for p in paths {
        if let Some(prev) = seen.insert(p.to_ascii_lowercase(), p)
            && prev != p
        {
            return Err(Refusal::new(Rule::Conv13, format!("{prev:?} and {p:?} differ only in case")));
        }
    }
    Ok(())
}

/// Whether `name` is a game-style name (§3.2 `name`, §6.3): 1–63 printable ASCII characters, no leading or
/// trailing space.
pub fn is_name(name: &str) -> bool {
    let b = name.as_bytes();
    (1..=63).contains(&b.len()) && b.iter().all(|c| (0x20..=0x7E).contains(c)) && b[0] != b' ' && b[b.len() - 1] != b' '
}

/// A 32-bit hash key in its text form, `0x` + 8 upper-case hex digits (§6.4).
pub fn format_hash32(h: u32) -> String {
    format!("0x{h:08X}")
}

/// Reads a `hash32` string (`CONV.8` when malformed).
pub fn parse_hash32(s: &str) -> Result<u32, Refusal> {
    let hex = s
        .strip_prefix("0x")
        .filter(|h| h.len() == 8 && h.bytes().all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)));
    match hex {
        Some(h) => Ok(u32::from_str_radix(h, 16).expect("checked hex")),
        None => Err(Refusal::new(Rule::Conv8, format!("{s:?} is not a hash (0x + 8 upper-case hex digits)"))),
    }
}

/// The original's name hash (§6.4, `bin_hash`).
pub fn name_hash(name: &str) -> u32 {
    ob_formats::formats::hash::bin_hash(name)
}

/// A `nameref` value (§6.3): a game name, or the hash of a name that is not known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameRef {
    Name(String),
    Hash(u32),
}

impl NameRef {
    /// Reads a `nameref`: a `0x` prefix makes it a hash (game names never start with `0x`).
    pub fn parse(s: &str) -> Result<NameRef, Refusal> {
        if s.starts_with("0x") {
            parse_hash32(s).map(NameRef::Hash)
        } else if is_name(s) {
            Ok(NameRef::Name(s.to_string()))
        } else {
            Err(Refusal::new(Rule::Conv8, format!("{s:?} is neither a name nor a hash")))
        }
    }

    /// The hash the engine looks the item up by.
    pub fn hash(&self) -> u32 {
        match self {
            NameRef::Name(n) => name_hash(n),
            NameRef::Hash(h) => *h,
        }
    }
}

/// Every short kind name a reference may start with (README.md §3: the kind registry and its additional short
/// names). `docs::check` fails when the README names one this list lacks.
pub const KINDS: &[&str] = &[
    "texture",
    "texture-anim",
    "solid",
    "anim",
    "skeleton",
    "part-pivots",
    "camera-track",
    "scene",
    "region",
    "section",
    "scenery",
    "placement",
    "light",
    "prop-kind",
    "scenery-group",
    "scenery-link",
    "fog-map",
    "collision",
    "barrier",
    "volume",
    "road",
    "route",
    "marker",
    "route-trigger",
    "ai-tuning",
    "race",
    "run",
    "car",
    "preset",
    "part",
    "part-cost",
    "perf-package",
    "perf-part",
    "tuning-unlock",
    "car-unit",
    "event",
    "stage",
    "sponsor",
    "track",
    "track-map",
    "fe-package",
    "fe-object",
    "font",
    "language",
    "string",
    "credits",
    "legal-text",
    "memcard-text",
    "subtitles",
    "sound-bank",
    "sound",
    "sound-event",
    "engine-recording",
    "speech-line",
    "speech-event",
    "music-stream",
    "music-event",
    "mix-map",
    "reverb-preset",
    "movie",
    "movie-entry",
    "table",
    "solid-list",
    "anim-bank",
    "camera-group",
    "flare",
    "prop",
    "emitter",
    "anim-entity",
    "anim-instance",
    "horizon-fog",
    "collision-face",
    "route-barrier",
    "checkpoints",
    "road-node",
    "route-record",
    "trick",
    "cash-pickup",
    "car-unlock",
    "fe-platform",
    "fe-envmap",
    "speech-bank",
    "music-map",
];

/// Whether `kind` has the shape of a short kind name (`[a-z][a-z0-9-]*`).
fn kind_shape(kind: &str) -> bool {
    let b = kind.as_bytes();
    !b.is_empty()
        && b[0].is_ascii_lowercase()
        && b.iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

/// A reference to an item, `<kind>:<key>` (§6.6). The key may itself hold `:` (`race:docks-remix:sprint`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Reference {
    pub kind: String,
    pub key: String,
}

impl Reference {
    /// Reads a reference; the kind must be one of [`KINDS`] (`CONV.8` otherwise). A key is 1–200 characters
    /// without white space.
    pub fn parse(s: &str) -> Result<Reference, Refusal> {
        let bad = |why: &str| Err(Refusal::new(Rule::Conv8, format!("{s:?} is not a reference: {why}")));
        let Some((kind, key)) = s.split_once(':') else { return bad("no `<kind>:`") };
        if !kind_shape(kind) || !KINDS.contains(&kind) {
            return bad(&format!("unknown kind {kind:?}"));
        }
        if key.is_empty() || key.chars().count() > 200 || key.chars().any(char::is_whitespace) {
            return bad("the key is empty, too long or holds white space");
        }
        Ok(Reference { kind: kind.to_string(), key: key.to_string() })
    }
}

impl std::fmt::Display for Reference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.kind, self.key)
    }
}

/// A tools' address, `ob://<pack id>/<kind>/<key>[#<field path>][?guard=<value>]` (§6.6). Never stored in data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    pub pack: String,
    pub item: Reference,
    pub field: Option<String>,
    pub guard: Option<String>,
}

impl Address {
    pub fn parse(s: &str) -> Result<Address, Refusal> {
        let bad = |why: &str| Err(Refusal::new(Rule::Conv8, format!("{s:?} is not an ob:// address: {why}")));
        let Some(rest) = s.strip_prefix("ob://") else { return bad("no `ob://`") };
        let (rest, guard) = match rest.split_once('?') {
            Some((r, q)) => match q.strip_prefix("guard=") {
                Some(g) => (r, Some(g.to_string())),
                None => return bad("the only query is `guard=`"),
            },
            None => (rest, None),
        };
        let (rest, field) = match rest.split_once('#') {
            Some((r, f)) if !f.is_empty() => (r, Some(f.to_string())),
            Some(_) => return bad("an empty field path"),
            None => (rest, None),
        };
        let Some((pack, rest)) = rest.split_once('/') else { return bad("no kind") };
        if !is_pack_id(pack) {
            return bad("bad pack id");
        }
        let Some((kind, key)) = rest.split_once('/') else { return bad("no key") };
        let item = Reference::parse(&format!("{kind}:{key}"))?;
        Ok(Address { pack: pack.to_string(), item, field, guard })
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ob://{}/{}/{}", self.pack, self.item.kind, self.item.key)?;
        if let Some(field) = &self.field {
            write!(f, "#{field}")?;
        }
        if let Some(g) = &self.guard {
            write!(f, "?guard={g}")?;
        }
        Ok(())
    }
}

/// Whether `name` is an extension name, `[A-Z0-9]+_[a-z0-9_]+` (§9).
pub fn is_extension_name(name: &str) -> bool {
    let Some((prefix, rest)) = name.split_once('_') else { return false };
    !prefix.is_empty()
        && prefix.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        && !rest.is_empty()
        && rest.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_ids() {
        for ok in ["docks-remix", "nfsu2-pc12-eu", "a", "0", "hud_mod"] {
            assert!(is_pack_id(ok), "{ok}");
        }
        for bad in ["", "-a", "_a", "Docks", "a b", "a.b", &"a".repeat(65)] {
            assert!(!is_pack_id(bad), "{bad}");
        }
    }

    #[test]
    fn logical_paths() {
        // conventions.md §2, layout.md §7.
        for ok in [
            "world/L4RA/sections/1427.toml",
            "global/textures/GlobalB/66994F4A.ktx2",
            "residual/GLOBAL/GlobalB.lzc.map.toml",
            "world/_any/textures/images/0x5FB8BCD1.png",
            "cars/PEUGOT/meshes/GEOMETRY.BIN.0.gltf",
        ] {
            logical_path(ok, false).unwrap();
        }
        logical_path(".studio/journal.toml", true).unwrap();
        for bad in [
            "",
            "/abs",
            "a//b",
            "a/",
            "../x",
            "a/./b",
            "a/../b",
            ".studio/x",
            "a/.hidden",
            "sp ace",
            "a\\b",
            "ü",
            &"a".repeat(101),
            &format!("{}/x", "a/".repeat(120)),
        ] {
            assert_eq!(logical_path(bad, false).unwrap_err().rule, Rule::Conv13, "{bad}");
        }
        assert!(logical_path("a/.studio/x", true).is_err());
        case_unique(["a/B.toml", "a/c.toml"]).unwrap();
        assert_eq!(case_unique(["a/B.toml", "a/b.toml"]).unwrap_err().rule, Rule::Conv13);
    }

    #[test]
    fn names_and_hashes() {
        assert!(is_name("XO_CONE_01"));
        assert!(is_name("A'B&C-D.E"));
        assert!(!is_name(" A") && !is_name("A ") && !is_name("") && !is_name(&"A".repeat(64)) && !is_name("Ä"));
        assert_eq!(format_hash32(0x3BE3_5F00), "0x3BE35F00");
        assert_eq!(parse_hash32("0x3BE35F00").unwrap(), 0x3BE3_5F00);
        for bad in ["0x3be35f00", "3BE35F00", "0x3BE35F0", "0x3BE35F000"] {
            assert_eq!(parse_hash32(bad).unwrap_err().rule, Rule::Conv8);
        }
        // conventions.md §6.3's nameref example: a name and its hash.
        let name = NameRef::parse("XO_CONE_01").unwrap();
        let hash = NameRef::parse("0x608A650A").unwrap();
        assert_eq!(name.hash(), 0x608A_650A);
        assert_eq!(hash, NameRef::Hash(0x608A_650A));
    }

    #[test]
    fn references_and_addresses() {
        // conventions.md §6.6's examples.
        for r in [
            "texture:0x3BE35F00",
            "solid:XO_CONE_01",
            "car:PEUGOT",
            "solid:world/L4RA/1427.0/XO_CONE_01",
            "placement:L4RA/1407/12",
            "race:docks-remix:sprint",
        ] {
            assert_eq!(Reference::parse(r).unwrap().to_string(), r);
        }
        assert_eq!(Reference::parse("race:docks-remix:sprint").unwrap().key, "docks-remix:sprint");
        for bad in ["texture", "nokind:x", "car:", "car:a b", "Car:X"] {
            assert_eq!(Reference::parse(bad).unwrap_err().rule, Rule::Conv8, "{bad}");
        }
        let a = Address::parse("ob://docks-remix/placement/L4RA/1407/12#position").unwrap();
        assert_eq!(a.pack, "docks-remix");
        assert_eq!(a.item.to_string(), "placement:L4RA/1407/12");
        assert_eq!(a.field.as_deref(), Some("position"));
        assert_eq!(a.to_string(), "ob://docks-remix/placement/L4RA/1407/12#position");
        let g = Address::parse("ob://docks-remix/car/PEUGOT#mass_kg?guard=1200.0").unwrap();
        assert_eq!(g.guard.as_deref(), Some("1200.0"));
        assert!(Address::parse("ob://Docks/car/X").is_err());
        assert!(Address::parse("ob://docks/car").is_err());
    }

    #[test]
    fn extension_names() {
        assert!(is_extension_name("OB_solid_list"));
        assert!(is_extension_name("ACME2_wet_roads"));
        assert!(!is_extension_name("ob_x") && !is_extension_name("OB_") && !is_extension_name("OB_X"));
    }
}

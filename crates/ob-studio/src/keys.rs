//! The keys of the placement and race kinds, and the placement guard (docs/modding/asset-format/world.md §2, §4.3;
//! roads-ai.md §3.7; layering.md §3.4).
//!
//! An original placement keeps its key `(region, scenery section, index)` for good; a placement a pack adds is
//! keyed by the pack (`<pack>:<id>`), and so is every race. A `merge` of an original placement carries a guard: the
//! base set's drawn solid and position, so a pack made for another base set is refused (`WLD.11`) instead of moving
//! the wrong prop.

use glam::Vec3;
use ob_asset_format::names::is_pack_id;
use ob_asset_format::{Refusal, Rule};

/// How far a guard's position may lie from the base set's placement, in metres.
pub const GUARD_TOLERANCE_M: f32 = 0.05;

fn is_region(s: &str) -> bool {
    (2..=8).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn is_local_id(s: &str, max: usize) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= max
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_' || *c == b'-')
}

fn number(s: &str) -> Option<u32> {
    (1..=5).contains(&s.len()).then_some(())?;
    s.bytes().all(|b| b.is_ascii_digit()).then(|| s.parse().ok())?
}

fn bad(what: &str, text: &str) -> Refusal {
    Refusal::new(Rule::Conv8, format!("{text:?} is not a {what} key"))
}

/// Which placement within a region.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PlacementId {
    /// An original placement: its scenery section and its index there.
    Original { scenery: u32, index: u32 },
    /// A placement the pack `pack` added, named `id` by it.
    Added { pack: String, id: String },
}

impl PlacementId {
    /// The form `placements.toml` writes in an entry's `key` (`1407/12`, `docks-remix:a12`).
    pub fn parse(text: &str) -> Result<PlacementId, Refusal> {
        if let Some((pack, id)) = text.split_once(':') {
            if is_pack_id(pack) && is_local_id(id, 32) {
                return Ok(PlacementId::Added { pack: pack.into(), id: id.into() });
            }
        } else if let Some((s, i)) = text.split_once('/')
            && let (Some(scenery), Some(index)) = (number(s), number(i))
        {
            return Ok(PlacementId::Original { scenery, index });
        }
        Err(bad("placement", text))
    }

    /// The entry's `key`.
    pub fn entry_key(&self) -> String {
        match self {
            PlacementId::Original { scenery, index } => format!("{scenery}/{index}"),
            PlacementId::Added { pack, id } => format!("{pack}:{id}"),
        }
    }
}

/// A placement's full key (`placement:L4RA/1407/12`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlacementKey {
    pub region: String,
    pub id: PlacementId,
}

impl PlacementKey {
    pub fn parse(text: &str) -> Result<PlacementKey, Refusal> {
        let rest = text.strip_prefix("placement:").ok_or_else(|| bad("placement", text))?;
        let (region, id) = rest.split_once('/').ok_or_else(|| bad("placement", text))?;
        if !is_region(region) {
            return Err(bad("placement", text));
        }
        Ok(PlacementKey { region: region.into(), id: PlacementId::parse(id).map_err(|_| bad("placement", text))? })
    }

    /// Whether only the pack `pack` (and packs that depend on it) may name it.
    pub fn owned_by(&self, pack: &str) -> bool {
        matches!(&self.id, PlacementId::Added { pack: p, .. } if p == pack)
    }
}

impl std::fmt::Display for PlacementKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "placement:{}/{}", self.region, self.id.entry_key())
    }
}

/// A race's key (`race:docks-remix:sprint`): races are always a pack's own.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RaceKey {
    pub pack: String,
    pub id: String,
}

impl RaceKey {
    pub fn parse(text: &str) -> Result<RaceKey, Refusal> {
        let rest = text.strip_prefix("race:").ok_or_else(|| bad("race", text))?;
        let (pack, id) = rest.split_once(':').ok_or_else(|| bad("race", text))?;
        // The race file's `id`: 1 to 40 of a-z, 0-9, `_` and `-`.
        let id_ok = (1..=40).contains(&id.len())
            && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
        if !is_pack_id(pack) || !id_ok {
            return Err(bad("race", text));
        }
        Ok(RaceKey { pack: pack.into(), id: id.into() })
    }
}

impl std::fmt::Display for RaceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "race:{}:{}", self.pack, self.id)
    }
}

/// A merge's guard: what the pack's author saw at the key.
#[derive(Clone, Debug, PartialEq)]
pub struct Guard {
    /// The drawn solid's name (`XO_CONE_01`, without `solid:`).
    pub solid: String,
    pub position: Vec3,
}

/// Checks a guard against the base set's placement of that key (its drawn solid and position, before any layer):
/// the solid names must match (no case) and the positions lie within [`GUARD_TOLERANCE_M`], the distance computed in
/// binary32. A mismatch is `WLD.11`, which refuses the whole pack.
pub fn check_guard(key: &PlacementKey, guard: &Guard, base_solid: &str, base_position: Vec3) -> Result<(), Refusal> {
    let d = guard.position - base_position;
    let dist = (d.x * d.x + d.y * d.y + d.z * d.z).sqrt();
    let why = if !guard.solid.eq_ignore_ascii_case(base_solid) {
        format!("{key}: the guard names {}, the base set has {base_solid} there", guard.solid)
    } else if dist.is_nan() || dist > GUARD_TOLERANCE_M {
        format!("{key}: the guard's position is {dist:.3} m from the base set's")
    } else {
        return Ok(());
    };
    Err(Refusal::new(Rule::Wld11, why).in_field("guard"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_keys_read_and_write_both_forms() {
        for text in ["placement:L4RA/1407/12", "placement:L4RA/docks-remix:a12", "placement:L4RH/2600/0"] {
            assert_eq!(PlacementKey::parse(text).unwrap().to_string(), text);
        }
        let k = PlacementKey::parse("placement:L4RA/docks-remix:a12").unwrap();
        assert!(k.owned_by("docks-remix") && !k.owned_by("other"));
        assert!(!PlacementKey::parse("placement:L4RA/1407/12").unwrap().owned_by("docks-remix"));
        assert_eq!(PlacementId::parse("1407/12").unwrap(), PlacementId::Original { scenery: 1407, index: 12 });
    }

    #[test]
    fn bad_keys_are_refused_with_the_syntax_rule() {
        for text in [
            "L4RA/1407/12",
            "placement:L4RA/1407",
            "placement:L4RA/123456/1",
            "placement:L4RA/1407/-1",
            "placement:X/1/2",
            "placement:L4RA/Docks:a1",
            "placement:L4RA/docks:",
            "placement:L4RA/docks:Big",
        ] {
            assert_eq!(PlacementKey::parse(text).unwrap_err().rule, Rule::Conv8, "{text}");
        }
        for text in ["race:docks", "race:Docks:a", "race:docks:", "race:docks:UPPER", "docks:sprint"] {
            assert_eq!(RaceKey::parse(text).unwrap_err().rule, Rule::Conv8, "{text}");
        }
        assert_eq!(RaceKey::parse("race:docks-remix:sprint").unwrap().to_string(), "race:docks-remix:sprint");
    }

    #[test]
    fn a_guard_holds_within_five_centimetres_of_the_same_solid() {
        let key = PlacementKey::parse("placement:L4RA/1407/12").unwrap();
        let at = Vec3::new(-215.25, -1037.5, 146.0);
        let g = |solid: &str, d: Vec3| Guard { solid: solid.into(), position: at + d };
        assert!(check_guard(&key, &g("XO_CONE_01", Vec3::ZERO), "XO_CONE_01", at).is_ok());
        assert!(check_guard(&key, &g("xo_cone_01", Vec3::new(0.03, 0.03, 0.0)), "XO_CONE_01", at).is_ok());
        for (solid, d) in [("XO_CONE_01", Vec3::new(0.04, 0.04, 0.0)), ("XC_BARRIER_03", Vec3::ZERO)] {
            let r = check_guard(&key, &g(solid, d), "XO_CONE_01", at).unwrap_err();
            assert_eq!(r.rule, Rule::Wld11);
        }
        assert!(check_guard(&key, &g("XO_CONE_01", Vec3::splat(f32::NAN)), "XO_CONE_01", at).is_err());
    }
}

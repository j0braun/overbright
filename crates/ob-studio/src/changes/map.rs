//! A map mod's documents as the layering tables (placement and race items) that [`super`] compares: each prop
//! edit an entry of the `placement` kind, keyed by the original placement it changes (or as the mod's own
//! addition), each race an addition of the `race` kind. The mods enabled in the player's game are the layers in
//! their order (`enabled.txt`: applied in that order, the last the strongest).
//!
//! What the map mod knows of the original: an edited placement's solid and position (its `at`). Its heading
//! comes from the game's files, which this does not read: a heading's original is unknown here.

use ob_asset_format::layering::{KindRules, Layer};
use ob_map::tools::map::format::{MapMod, PropEdit};

use super::{Clash, Row, Tables, compare, this_mod};

/// The kinds' short names.
pub const PLACEMENT: &str = "placement";
pub const RACE: &str = "race";

/// The rules both kinds merge with here: lists given whole.
pub const RULES: KindRules = KindRules::ALL;

pub use nfsu2::studio_api::mod_packs::{added_key, heading, layers, placement_key, race_key};

/// The player's game as layering tables: the enabled mods bottom to top (weakest first).
#[derive(Clone, Debug, Default)]
pub struct Game {
    pub base: Vec<(String, toml::Table)>,
    pub placements: Vec<Layer>,
    pub races: Vec<Layer>,
}

impl Game {
    /// The game made of `mods` (pack id, mod), weakest first.
    pub fn new(mods: &[(String, MapMod)]) -> Game {
        let mut g = Game::default();
        for (pack, m) in mods {
            let (base, places, races) = layers(m, pack);
            for (k, t) in base {
                if !g.base.iter().any(|(x, _)| *x == k) {
                    g.base.push((k, t));
                }
            }
            g.placements.push(places);
            g.races.push(races);
        }
        g
    }

    fn tables(&self) -> [Tables<'_>; 2] {
        [
            Tables { kind: PLACEMENT, base: &self.base, layers: &self.placements, rules: &RULES },
            Tables { kind: RACE, base: &[], layers: &self.races, rules: &RULES },
        ]
    }

    /// Every value `yours` gives, placements first.
    pub fn this_mod(&self, yours: &str) -> Vec<Row> {
        self.tables().iter().flat_map(|t| this_mod(t, yours)).collect()
    }

    /// The values `yours` gives that another mod gives too.
    pub fn clashes(&self, yours: &str) -> Vec<Clash> {
        self.tables().iter().flat_map(|t| super::clashes(t, yours)).collect()
    }
}

/// One state of a mod as items with their values (an edit's own fields over the original's; `disabled` when
/// hidden), by kind.
fn items(map: &MapMod, pack: &str) -> [Vec<(String, toml::Table)>; 2] {
    let (base, places, races) = layers(map, pack);
    let flat = |l: Layer, base: &[(String, toml::Table)]| -> Vec<(String, toml::Table)> {
        l.entries
            .into_iter()
            .map(|e| {
                let mut t = base.iter().find(|(k, _)| *k == e.key).map(|(_, t)| t.clone()).unwrap_or_default();
                t.extend(e.fields);
                if e.disabled == Some(true) {
                    t.insert("disabled".into(), toml::Value::Boolean(true));
                }
                (e.key, t)
            })
            .collect()
    };
    [flat(places, &base), flat(races, &[])]
}

/// Two states of one mod (before → after), value by value: `original` is the value before.
pub fn compare_maps(before: &MapMod, after: &MapMod, pack: &str) -> Vec<Row> {
    let ([pa, ra], [pb, rb]) = (items(before, pack), items(after, pack));
    let mut rows = compare(PLACEMENT, &pa, &pb);
    rows.extend(compare(RACE, &ra, &rb));
    rows
}

/// Whether the original of `field` of a placement is known here (a heading's is in the game's files).
pub fn original_known(kind: &str, field: &str) -> bool {
    !(kind == PLACEMENT && field == "heading")
}

/// `map` with the value at `field` of item `key` given back: a moved prop's position back to its original
/// place; its heading (whose original is in the game's files), a hidden prop's `disabled`, an added prop or a
/// race: the edit taken out. None when the mod has no such item.
pub fn reset(map: &MapMod, pack: &str, kind: &str, key: &str, field: &str) -> Option<MapMod> {
    let mut m = map.clone();
    if kind == RACE {
        let i = m.races.iter().position(|r| race_key(pack, &r.id) == key)?;
        m.races.remove(i);
        return Some(m);
    }
    let i = m.props.iter().position(|p| match p {
        PropEdit::Move { target, .. } | PropEdit::Delete { target } => placement_key(target) == key,
        PropEdit::Add { id, region, .. } => added_key(region, pack, id) == key,
    })?;
    match &mut m.props[i] {
        PropEdit::Move { target, position, .. } if field == "position" => *position = target.at,
        _ => {
            m.props.remove(i);
        }
    }
    Some(m)
}

/// `map` with the whole object `key` given back: a moved or hidden prop is as the original again, an added prop or a
/// race is gone. None when the mod has no such object.
pub fn reset_object(map: &MapMod, pack: &str, kind: &str, key: &str) -> Option<MapMod> {
    let mut m = map.clone();
    if kind == RACE {
        let i = m.races.iter().position(|r| race_key(pack, &r.id) == key)?;
        m.races.remove(i);
        return Some(m);
    }
    let i = m.props.iter().position(|p| match p {
        PropEdit::Move { target, .. } | PropEdit::Delete { target } => placement_key(target) == key,
        PropEdit::Add { id, region, .. } => added_key(region, pack, id) == key,
    })?;
    m.props.remove(i);
    Some(m)
}

/// Several values given back in one step (a multi-selection's Reset); None when one of them is not there.
pub fn reset_all(map: &MapMod, pack: &str, picks: &[(String, String, String)]) -> Option<MapMod> {
    let mut m = map.clone();
    for (kind, key, field) in picks {
        // A value of an item an earlier pick took out whole is reset already.
        if let Some(n) = reset(&m, pack, kind, key, field) {
            m = n;
        }
    }
    (m != *map).then_some(m)
}

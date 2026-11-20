//! A map mod's documents as the layering tables (placement and race items) that [`super`] compares: each prop
//! edit an entry of the `placement` kind, keyed by the original placement it changes (or as the mod's own
//! addition), each race an addition of the `race` kind. The mods enabled in the player's game are the layers in
//! their order (`enabled.txt`: applied in that order, the last the strongest).
//!
//! What the map mod knows of the original: an edited placement's solid and position (its `at`). Its heading
//! comes from the game's files, which this does not read: a heading's original is unknown here.

use ob_asset_format::layering::{Entry, KindRules, Layer, Op};
use ob_map::tools::map::format::{CustomRace, MapMod, PropEdit, PropRef};

use super::{Clash, Row, Tables, compare, this_mod};

/// The kinds' short names.
pub const PLACEMENT: &str = "placement";
pub const RACE: &str = "race";

/// The rules both kinds merge with here: lists given whole.
pub const RULES: KindRules = KindRules::ALL;

fn round(x: f32, step: f64) -> f64 {
    (x as f64 / step).round() * step
}

fn v3(v: glam::Vec3) -> toml::Value {
    toml::Value::Array([v.x, v.y, v.z].iter().map(|c| toml::Value::Float(round(*c, 0.001))).collect())
}

/// The heading around +z from +x in degrees (the rotation's rows are the local axes; its x axis is forward).
pub fn heading(rotation: glam::Mat3) -> f64 {
    let f = rotation.row(0);
    round(f.y.atan2(f.x).to_degrees(), 0.01)
}

/// An original placement's item key: `<region>/<section>/<solid>/<x>:<y>:<z>/<nth>`.
pub fn placement_key(t: &PropRef) -> String {
    format!("{}/{}/{}/{}:{}:{}/{}", t.region, t.section, t.solid, t.at.x, t.at.y, t.at.z, t.nth)
}

/// An added placement's item key: `<region>/<pack>:<id>`.
pub fn added_key(region: &str, pack: &str, id: &str) -> String {
    format!("{region}/{pack}:{id}")
}

/// A race's item key: `<pack>:<id>`.
pub fn race_key(pack: &str, id: &str) -> String {
    format!("{pack}:{id}")
}

fn table(pairs: Vec<(&str, toml::Value)>) -> toml::Table {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn original(t: &PropRef) -> (String, toml::Table) {
    (placement_key(t), table(vec![("solid", toml::Value::String(t.solid.clone())), ("position", v3(t.at))]))
}

fn race_fields(r: &CustomRace) -> toml::Table {
    let pts = |v: &[glam::Vec2]| {
        toml::Value::Array(
            v.iter()
                .map(|p| {
                    toml::Value::Array(vec![
                        toml::Value::Float(round(p.x, 0.001)),
                        toml::Value::Float(round(p.y, 0.001)),
                    ])
                })
                .collect(),
        )
    };
    table(vec![
        ("name", toml::Value::String(r.name.clone())),
        ("type", toml::Value::String(r.kind.name().into())),
        ("laps", toml::Value::Integer(r.laps as i64)),
        ("track", toml::Value::Integer(r.track as i64)),
        ("backward", toml::Value::Boolean(r.backward)),
        ("checkpoints", pts(&r.checkpoints)),
        ("grid", toml::Value::Array(r.grid.iter().map(|s| v3(s.position)).collect())),
    ])
}

/// One mod's layers (placements, races) and the originals its edits name.
pub fn layers(map: &MapMod, pack: &str) -> (Vec<(String, toml::Table)>, Layer, Layer) {
    let mut base = Vec::new();
    let mut places = Vec::new();
    for p in &map.props {
        match p {
            PropEdit::Move { target, position, rotation } => {
                base.push(original(target));
                let fields =
                    table(vec![("position", v3(*position)), ("heading", toml::Value::Float(heading(*rotation)))]);
                places.push(Entry { key: placement_key(target), op: Some(Op::Merge), disabled: None, fields });
            }
            PropEdit::Delete { target } => {
                base.push(original(target));
                places.push(Entry {
                    key: placement_key(target),
                    op: Some(Op::Merge),
                    disabled: Some(true),
                    fields: toml::Table::new(),
                });
            }
            PropEdit::Add { id, region, solid, position, rotation } => {
                let fields = table(vec![
                    ("solid", toml::Value::String(solid.clone())),
                    ("position", v3(*position)),
                    ("heading", toml::Value::Float(heading(*rotation))),
                ]);
                places.push(Entry { key: added_key(region, pack, id), op: Some(Op::Add), disabled: None, fields });
            }
        }
    }
    let races = map
        .races
        .iter()
        .map(|r| Entry { key: race_key(pack, &r.id), op: Some(Op::Add), disabled: None, fields: race_fields(r) })
        .collect();
    let layer = |entries| Layer { pack: pack.into(), dependencies: Vec::new(), entries };
    (base, layer(places), layer(races))
}

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

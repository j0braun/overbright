//! A map mod as layer entries (layering.md §3): each prop edit an entry of the `placement` kind, keyed by the
//! original placement it changes (or as the mod's own addition), each race an addition of the `race` kind. Studio's
//! change lists and the game's clash report (`mod_packs`) read the same entries.

use ob_asset_format::layering::{Entry, Layer, Op};
use ob_map::tools::map::format::{CustomRace, MapMod, PropEdit, PropRef};

pub fn round(x: f32, step: f64) -> f64 {
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

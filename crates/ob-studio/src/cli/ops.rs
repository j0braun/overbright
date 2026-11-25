//! A command's arguments made into the map edit it requests: the edit's JSON (the same JSON the map editor's
//! requests, an operations file's lines and an agent's requests are), read as an [`Op`].

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

use ob_map::tools::map::format::MapMod;
use ob_map::tools::map::ops::Op;

/// Why no edit was made from the arguments: a usage error (exit 2).
pub type Usage = String;

const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn op(tag: &str, fields: &BTreeMap<String, Value>, take: &[&str]) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("op".into(), Value::String(tag.into()));
    for k in take {
        if let Some(v) = fields.get(*k) {
            o.insert((*k).into(), v.clone());
        }
    }
    Value::Object(o)
}

/// An upright rotation turned to `heading` degrees (around +z from +x, the game's convention; the Properties
/// panel's Heading), as the edit's rows (its local x, y and z axes).
fn upright(heading: f32) -> [[f32; 3]; 3] {
    let (s, c) = heading.to_radians().sin_cos();
    [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]]
}

/// The rotation of `props move` / `props add`: `--rotation` as given, or `--heading` in degrees as an upright
/// rotation; not both. `props move` then turns the prop's own rotation in the world to the heading, so its tilt is
/// kept as the Properties panel's Heading keeps it ([`super::try_edit`]).
fn rotation_arg(name: &str, a: &BTreeMap<String, Value>) -> Result<Option<Value>, Usage> {
    match (a.get("rotation"), a.get("heading")) {
        (Some(_), Some(_)) => Err(format!("`{name}`: give --rotation or --heading, not both")),
        (Some(r), None) => Ok(Some(r.clone())),
        (None, Some(h)) => {
            let deg = h.as_f64().filter(|d| d.is_finite());
            let deg = deg.ok_or_else(|| format!("`{name}` --heading: expected degrees, got {h}"))?;
            Ok(Some(json!(upright(deg as f32))))
        }
        (None, None) => Ok(None),
    }
}

fn point(a: &BTreeMap<String, Value>) -> Value {
    json!({ "part": a["part"], "index": a.get("index").cloned().unwrap_or(json!(0)) })
}

/// The edit's JSON for command `name` with arguments `a` (`map`: the mod as it is, for the arguments left out
/// that keep what it has).
pub fn request(name: &str, a: &BTreeMap<String, Value>, map: &MapMod, dir: &Path) -> Result<Value, Usage> {
    Ok(match name {
        "props.move" => {
            let mut v = op("place", a, &["key", "position"]);
            let r = rotation_arg("props move", a)?.ok_or("`props move` needs --rotation or --heading")?;
            v.as_object_mut().unwrap().insert("rotation".into(), r);
            v
        }
        "props.delete" => op("delete", a, &["key"]),
        "props.reset" => op("reset", a, &["key"]),
        "props.drop" => op("drop_to_ground", a, &["key"]),
        "props.duplicate" => op("duplicate", a, &["key", "position"]),
        "props.add" => {
            let mut v = op("add", a, &["solid", "position", "id", "ground"]);
            let r = rotation_arg("props add", a)?.unwrap_or(json!(IDENTITY));
            v.as_object_mut().unwrap().insert("rotation".into(), r);
            v
        }
        "race.create" => {
            let mut v = op("new_race", a, &["track", "kind", "id"]);
            v.as_object_mut().unwrap().entry("kind").or_insert(json!("sprint"));
            v
        }
        "race.delete" => op("delete_race", a, &["race"]),
        "race.set" => {
            let race = &a["race"];
            // The id last: the edits before it name the race by its old id.
            let each = [
                ("name", "rename_race"),
                ("track", "set_track"),
                ("kind", "set_kind"),
                ("laps", "set_laps"),
                ("backward", "set_direction"),
                ("id", "set_race_id"),
            ];
            let ops: Vec<Value> = each
                .iter()
                .filter_map(|(k, tag)| {
                    let v = a.get(*k)?;
                    Some(json!({ "op": tag, "race": race, *k: v }))
                })
                .collect();
            match ops.len() {
                0 => {
                    return Err(
                        "`race set` needs at least one of --name, --id, --track, --kind, --laps, --backward".into()
                    );
                }
                1 => ops.into_iter().next().unwrap(),
                _ => {
                    json!({ "op": "group", "label": format!("Change race {}", race.as_str().unwrap_or("")), "ops": ops })
                }
            }
        }
        "race.point.add" => op("add_point", a, &["race", "part", "index", "position", "heading"]),
        "race.point.move" => match (a.get("later"), a.get("position")) {
            (Some(_), Some(_)) => return Err("`race point move`: give --position or --later, not both".into()),
            (Some(later), None) => {
                if a["part"] != "checkpoint" {
                    return Err("`race point move --later`: only a checkpoint moves in the route".into());
                }
                json!({ "op": "move_checkpoint", "race": a["race"], "index": a.get("index").cloned().unwrap_or(json!(0)), "later": later })
            }
            (None, Some(_)) => {
                let mut v = op("move_point", a, &["race", "position", "heading"]);
                v.as_object_mut().unwrap().insert("point".into(), point(a));
                v
            }
            (None, None) => return Err("`race point move` needs --position (or --later)".into()),
        },
        "race.point.delete" => json!({ "op": "delete_point", "race": a["race"], "point": point(a) }),
        "race.put" => op("put_race", a, &["race", "to"]),
        "pack.details" => {
            let keep = |k: &str, now: &str| a.get(k).cloned().unwrap_or(json!(now));
            json!({ "op": "details", "name": keep("name", &map.name), "author": keep("author", &map.author), "description": keep("description", &map.description) })
        }
        "pack.put" => {
            let file = dir.join(a["file"].as_str().unwrap_or_default());
            let text = std::fs::read_to_string(&file).map_err(|e| format!("--file {}: {e}", file.display()))?;
            let m = MapMod::parse(&text).map_err(|e| format!("--file {}: {e}", file.display()))?;
            json!({ "op": "put_mod", "map": serde_json::to_value(&m).map_err(|e| e.to_string())? })
        }
        "edit.group" => op("group", a, &["label", "ops"]),
        "ops.apply" => {
            let file = dir.join(a["file"].as_str().unwrap_or_default());
            let text = std::fs::read_to_string(&file).map_err(|e| format!("--file {}: {e}", file.display()))?;
            let ops = Op::read_lines(&text).map_err(|(n, e)| format!("{}:{n}: not an edit: {e}", file.display()))?;
            if ops.is_empty() {
                return Err(format!("{}: no edits in the file", file.display()));
            }
            let label = match a.get("label").and_then(Value::as_str) {
                Some(l) => l.to_string(),
                None if ops.len() == 1 => ops[0].label(map),
                None => format!("Apply {}", file.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()),
            };
            if ops.len() == 1 && a.get("label").is_none() {
                serde_json::to_value(&ops[0]).map_err(|e| e.to_string())?
            } else {
                json!({ "op": "group", "label": label, "ops": ops })
            }
        }
        _ => return Err(format!("`{name}` is not an edit")),
    })
}

/// The edit the JSON `v` is (a usage error when it is not one).
pub fn read(v: &Value) -> Result<Op, Usage> {
    serde_json::from_value(v.clone()).map_err(|e| format!("not an edit: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn heading_is_degrees_as_the_properties_panel_shows_it() {
        let map = MapMod::default();
        let dir = Path::new("");
        let a = args(&[("solid", json!("BOX")), ("position", json!([1.0, 2.0, 3.0])), ("heading", json!(90.0))]);
        let v = request("props.add", &a, &map, dir).unwrap();
        let Op::Add { rotation, .. } = read(&v).unwrap() else { panic!("{v}") };
        // The heading is where the local x axis points: 90° is +y.
        let x = rotation.row(0);
        assert!((x.y.atan2(x.x).to_degrees() - 90.0).abs() < 1e-4, "{rotation}");
        assert!((rotation.row(2) - glam::Vec3::Z).length() < 1e-6, "upright");
        // No heading: the identity, as before.
        let mut b = a.clone();
        b.remove("heading");
        let Op::Add { rotation, .. } = read(&request("props.add", &b, &map, dir).unwrap()).unwrap() else { panic!() };
        assert_eq!(rotation, glam::Mat3::IDENTITY);
        // Move takes one of the two, not both, not neither.
        let m = args(&[("key", json!({"added": "a1"})), ("position", json!([0.0, 0.0, 0.0])), ("heading", json!(-45.0))]);
        let Op::Place { rotation, .. } = read(&request("props.move", &m, &map, dir).unwrap()).unwrap() else { panic!() };
        let x = rotation.row(0);
        assert!((x.y.atan2(x.x).to_degrees() + 45.0).abs() < 1e-4);
        let mut both = m.clone();
        both.insert("rotation".into(), json!(upright(0.0)));
        assert!(request("props.move", &both, &map, dir).unwrap_err().contains("not both"));
        let mut neither = m.clone();
        neither.remove("heading");
        assert!(request("props.move", &neither, &map, dir).unwrap_err().contains("--heading"));
    }
}

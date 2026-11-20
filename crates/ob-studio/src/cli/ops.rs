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

fn point(a: &BTreeMap<String, Value>) -> Value {
    json!({ "part": a["part"], "index": a.get("index").cloned().unwrap_or(json!(0)) })
}

/// The edit's JSON for command `name` with arguments `a` (`map`: the mod as it is, for the arguments left out
/// that keep what it has).
pub fn request(name: &str, a: &BTreeMap<String, Value>, map: &MapMod, dir: &Path) -> Result<Value, Usage> {
    Ok(match name {
        "props.move" => op("place", a, &["key", "position", "rotation"]),
        "props.delete" => op("delete", a, &["key"]),
        "props.reset" => op("reset", a, &["key"]),
        "props.drop" => op("drop_to_ground", a, &["key"]),
        "props.duplicate" => op("duplicate", a, &["key", "position"]),
        "props.add" => {
            let mut v = op("add", a, &["solid", "position", "rotation", "id", "ground"]);
            v.as_object_mut().unwrap().entry("rotation").or_insert(json!(IDENTITY));
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

//! The Data workspace on the command line: `data kinds` and `data show` read the grid's table
//! ([`crate::data::Grid`]), `data set` writes one cell as the same edit the grid makes ([`crate::data::op_for`])
//! through the ordinary edit path, so it is one undo step with a request, a diff and a dry run like any edit.

use serde_json::{Value, json};

use super::{Done, FAILED, Mod, OK, USAGE, args, fail, open_read};
use crate::changes::From;
use crate::data::{self, Column, Grid, KINDS};

pub(super) fn run(name: &str, line: &args::Line, m: &Mod) -> Done {
    match open_read(m) {
        Ok((_, map)) => match name {
            "data.kinds" => kinds(&map, &m.folder),
            _ => show(line, &map, &m.folder),
        },
        Err(d) => d,
    }
}

fn from_word(f: &From) -> String {
    match f {
        From::Original => "original".into(),
        From::Yours => "yours".into(),
        From::Other(n) => format!("mod {n}"),
    }
}

fn ty_word(c: &Column) -> &'static str {
    match c.ty {
        data::Ty::Text => "text",
        data::Ty::Int { .. } => "int",
        data::Ty::Bool => "bool",
        data::Ty::Enum(_) => "enum",
        data::Ty::Vec3 => "vec3",
        data::Ty::Degrees => "degrees",
        data::Ty::Count(_) => "count",
    }
}

fn column_json(c: &Column) -> Value {
    json!({ "field": c.field, "label": c.label, "type": ty_word(c), "editable": c.editable })
}

fn kinds(map: &ob_map::tools::map::format::MapMod, pack: &str) -> Done {
    let list: Vec<Value> = KINDS
        .iter()
        .map(|k| {
            let rows = Grid::build(map, pack, k).map_or(0, |g| g.counts().0);
            json!({ "kind": k, "rows": rows, "columns": data::columns(k).iter().map(column_json).collect::<Vec<_>>() })
        })
        .collect();
    let text = list
        .iter()
        .map(|k| {
            let cols: Vec<&str> = k["columns"].as_array().into_iter().flatten().filter_map(|c| c["field"].as_str()).collect();
            format!("{:<10} {:>5} rows  {}", k["kind"].as_str().unwrap_or(""), k["rows"], cols.join(", "))
        })
        .collect();
    Done { code: OK, text, doc: json!({ "kinds": list }) }
}

fn show(line: &args::Line, map: &ob_map::tools::map::format::MapMod, pack: &str) -> Done {
    let text = |k: &str| line.args.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let kind = text("kind");
    let Some(mut g) = Grid::build(map, pack, &kind) else {
        let mut m = format!("--kind: no kind `{kind}`: use {}", KINDS.join(" or "));
        if let Some(n) = crate::browse::did_you_mean(&kind, KINDS.iter().copied()) {
            m.push_str(&format!(". Did you mean `{n}`?"));
        }
        let mut d = fail(USAGE, m);
        d.doc["error"]["reason"] = json!("bad_kind");
        return d;
    };
    let column = |word: &str| {
        g.columns.iter().position(|c| c.field.eq_ignore_ascii_case(word) || c.label.eq_ignore_ascii_case(word))
    };
    let bad_column = |word: &str, flag: &str| {
        let names: Vec<&str> = g.columns.iter().map(|c| c.field).collect();
        let mut d = fail(USAGE, format!("--{flag}: no column `{word}`: use {}", names.join(", ")));
        d.doc["error"]["reason"] = json!("bad_column");
        d
    };
    let search = text("search");
    let filter = text("filter");
    let sort = text("sort");
    let mut filters = Vec::new();
    if !filter.is_empty() {
        let Some((c, t)) = filter.split_once('=') else {
            return fail(USAGE, "--filter: expected `column=text`");
        };
        match column(c.trim()) {
            Some(i) => filters.push((i, t.to_string())),
            None => return bad_column(c.trim(), "filter"),
        }
    }
    let sort_col = if sort.is_empty() {
        None
    } else {
        match column(&sort) {
            Some(i) => Some(i),
            None => return bad_column(&sort, "sort"),
        }
    };
    g.set_search(&search);
    for (c, t) in &filters {
        g.set_filter(*c, t);
    }
    if let Some(c) = sort_col {
        g.sort_by(c);
        if line.args.get("desc").and_then(Value::as_bool).unwrap_or(false) {
            g.sort_by(c);
        }
    }
    let (all, shown) = g.counts();
    let mut rows = Vec::new();
    let mut lines = Vec::new();
    for i in 0..shown {
        let key = g.key(i).unwrap_or_default().to_string();
        let mut cells = serde_json::Map::new();
        let mut parts = Vec::new();
        for (c, col) in g.columns.iter().enumerate() {
            let Some(cell) = g.cell(i, c) else { continue };
            parts.push(cell.text.clone());
            cells.insert(
                col.field.to_string(),
                json!({ "text": cell.text, "from": from_word(&cell.from), "yours": cell.yours, "original": cell.original }),
            );
        }
        lines.push(format!("{key}  {}", parts.join(" | ")));
        rows.push(json!({ "key": key, "cells": cells }));
    }
    if rows.is_empty() {
        lines.push(format!("No {kind} rows{}", if all > 0 { " match" } else { "" }));
    }
    Done {
        code: OK,
        text: lines,
        doc: json!({
            "kind": g.kind, "total": all, "count": shown,
            "columns": g.columns.iter().map(column_json).collect::<Vec<_>>(), "rows": rows,
        }),
    }
}

/// The edit `data set` runs: the grid's op for the cell, as the request JSON the edit path takes.
pub(super) fn request(
    line: &args::Line,
    map: &ob_map::tools::map::format::MapMod,
    pack: &str,
) -> Result<Value, Done> {
    let t = |k: &str| line.args.get(k).and_then(Value::as_str).unwrap_or_default();
    if !KINDS.contains(&t("kind")) {
        let mut d = fail(USAGE, format!("--kind: no kind `{}`: use {}", t("kind"), KINDS.join(" or ")));
        d.doc["error"]["reason"] = json!("bad_kind");
        return Err(d);
    }
    let op = data::op_for(map, pack, t("kind"), t("row"), t("field"), t("value")).map_err(|e| {
        let mut d = fail(FAILED, e.clone());
        d.doc["error"]["reason"] = json!("bad_value");
        d.doc["error"]["detail"] = json!(e);
        d
    })?;
    serde_json::to_value(&op).map_err(|e| fail(FAILED, e.to_string()))
}

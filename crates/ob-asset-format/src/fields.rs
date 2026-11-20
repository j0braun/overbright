//! Field tables (conventions.md §3.2, §12; versioning.md §6): one description per field of a TOML file kind,
//! used three ways, so the three cannot drift apart:
//!
//! - [`check_table`] checks a document against it: unknown keys (`CONV.4`), missing required fields (`CONV.5`),
//!   types, ranges and enums (`CONV.6`), name and digest syntax (`CONV.8`), limits (`CONV.12`), fields newer than
//!   the declared schema minor (`VER.6`);
//! - [`doc_table`] writes the page's field table;
//! - [`schema`] writes the kind's JSON Schema.

use crate::json::Json;
use crate::{Refusal, Rule};

/// A string check with its JSON Schema form.
#[derive(Clone, Copy, Debug)]
pub struct Pattern {
    /// The JSON Schema `pattern` (ECMA-262 regular expression).
    pub json: &'static str,
    /// The same check in code.
    pub check: fn(&str) -> bool,
    /// The rule a mismatch breaks.
    pub rule: Rule,
}

/// A field's type.
#[derive(Clone, Copy, Debug)]
pub enum Ty {
    Bool,
    /// An integer within `min..=max`.
    Int {
        min: i64,
        max: i64,
    },
    /// Exactly this integer.
    Const(i64),
    /// A float read as binary32 (conventions.md §3.4).
    F32,
    /// A string of `min..=max` characters, optionally matching a pattern.
    Str {
        min: usize,
        max: usize,
        pattern: Option<Pattern>,
    },
    /// A float read as binary32 within `min..=max` (a number outside it is refused; an exact-bits escape, when
    /// `bits`, is not range-checked, as the schema has it).
    F32In {
        min: i64,
        max: i64,
        bits: bool,
    },
    /// A shared definition of `schemas/common.schema.json` (`$defs/<name>`), checked as `ty`.
    Def(&'static str, &'static Ty),
    /// A definition of the kind's own schema (`#/$defs/<name>`, written into its `$defs`), checked as `ty`.
    Local(&'static str, &'static Ty),
    /// A reference to an item of the kind with this short name (`solid:XO_CONE_01`), checked and written as `ty`.
    Ref(&'static str, &'static Ty),
    /// One of two types (`oneOf`): a keyed list element's `index`, a base position or a pack's `<pack>:<n>`.
    Either(&'static Ty, &'static Ty),
    Enum(&'static [&'static str]),
    Array {
        item: &'static Ty,
        min: usize,
        max: Option<usize>,
        unique: bool,
    },
    /// A table whose keys are data (`map<K,V>`): keys checked as `key` (a string type), values as `value`.
    Map {
        key: &'static Ty,
        value: &'static Ty,
    },
    /// A sub-table with its own fields.
    Table(&'static [Field]),
    /// `[extras]`: free content for tools (conventions.md §9).
    Extras,
}

/// Whether a field must be present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Req {
    Yes,
    /// Required, with the page's own "Req." text.
    YesAs(&'static str),
    No,
    /// Required under a condition the kind's own code checks; the text is the page's "Req." cell.
    When(&'static str),
    /// Required in an entry that adds the item (layering.md §3.1).
    OnAdd,
    /// Required in an entry that merges into an item a lower layer has.
    OnMerge,
}

/// Which entries of a layered kind may give a field (layering.md §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Only {
    /// Any entry (and a file kind that is not layered).
    Any,
    /// Only an entry that adds the item (what the item is: a placement's solid, a race's id).
    Add,
    /// Only an entry that merges into an item a lower layer has (the key naming it, its guard).
    Merge,
}

impl Req {
    pub fn required(self) -> bool {
        matches!(self, Req::Yes | Req::YesAs(_))
    }
}

/// One field of a TOML file kind.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    /// The TOML key.
    pub key: &'static str,
    pub ty: Ty,
    pub req: Req,
    /// The minor schema version that introduced the field (`x-since`; 0 for the first version).
    pub since: u32,
    /// The page's row: the name cell (`` `[game]` ``, `` `game.bases` ``), or "" for a field the page does not
    /// list (an accepted spelling).
    pub doc_name: &'static str,
    /// The page's type cell.
    pub doc_type: &'static str,
    /// The page's default cell.
    pub doc_default: &'static str,
    /// The page's meaning cell (Markdown).
    pub doc_meaning: &'static str,
    /// For a sub-table: whether its fields' rows follow in the same page table.
    pub rows_inline: bool,
    /// Which entries may give it.
    pub only: Only,
    /// A merge may change it on an item the entry does not add (Studio edits it there); keys, guards and the
    /// entry's own `op` are not changes.
    pub merge: bool,
    /// Studio's inspector row; "" for a field the inspector does not show (keys, guards).
    pub label: &'static str,
}

impl Field {
    /// A field with no page row of its own (filled in with the struct-update syntax).
    pub const BLANK: Field = Field {
        key: "",
        ty: Ty::Bool,
        req: Req::No,
        since: 0,
        doc_name: "",
        doc_type: "",
        doc_default: "",
        doc_meaning: "",
        rows_inline: false,
        only: Only::Any,
        merge: false,
        label: "",
    };
}

// ---- checking -------------------------------------------------------------------------------------------------

fn type_name(v: &toml::Value) -> &'static str {
    v.type_str()
}

fn wrong(expected: &str, v: &toml::Value) -> Refusal {
    Refusal::new(Rule::Conv6, format!("{expected} expected, found {}", type_name(v)))
}

/// Checks one value against a type.
pub fn check_value(v: &toml::Value, ty: &Ty) -> Result<(), Refusal> {
    match ty {
        Ty::Bool => v.as_bool().map(|_| ()).ok_or_else(|| wrong("a boolean", v)),
        Ty::Int { min, max } => match v.as_integer() {
            Some(i) if (*min..=*max).contains(&i) => Ok(()),
            Some(i) => Err(Refusal::new(Rule::Conv6, format!("{i} is outside {min}..={max}"))),
            None => Err(wrong("an integer", v)),
        },
        Ty::Const(c) => match v.as_integer() {
            Some(i) if i == *c => Ok(()),
            _ => Err(Refusal::new(Rule::Conv6, format!("must be {c}"))),
        },
        Ty::F32 => crate::float::read_f32(v, crate::float::Allow::default()).map(|_| ()),
        Ty::F32In { min, max, bits } => {
            if v.is_str() && !bits {
                return Err(wrong("a number", v));
            }
            let x = crate::float::read_f32(v, crate::float::Allow::default())?;
            if !v.is_str() && !(*min as f32..=*max as f32).contains(&x) {
                return Err(Refusal::new(Rule::Conv6, format!("{x} is outside {min}..={max}")));
            }
            Ok(())
        }
        Ty::Str { min, max, pattern } => {
            let s = v.as_str().ok_or_else(|| wrong("a string", v))?;
            let n = s.chars().count();
            if n < *min {
                return Err(Refusal::new(Rule::Conv6, format!("shorter than {min} characters")));
            }
            if n > *max {
                return Err(Refusal::new(Rule::Conv12, format!("longer than {max} characters")));
            }
            if let Some(p) = pattern
                && !(p.check)(s)
            {
                return Err(Refusal::new(p.rule, format!("{s:?} does not match {}", p.json)));
            }
            Ok(())
        }
        Ty::Def(_, inner) | Ty::Local(_, inner) | Ty::Ref(_, inner) => check_value(v, inner),
        Ty::Either(a, b) => check_value(v, a).or_else(|e| check_value(v, b).map_err(|_| e)),
        Ty::Enum(values) => match v.as_str() {
            Some(s) if values.contains(&s) => Ok(()),
            Some(s) => Err(Refusal::new(Rule::Conv6, format!("{s:?} is not one of {}", values.join(", ")))),
            None => Err(wrong("a string", v)),
        },
        Ty::Array { item, min, max, unique } => {
            let a = v.as_array().ok_or_else(|| wrong("an array", v))?;
            if a.len() < *min {
                return Err(Refusal::new(Rule::Conv6, format!("fewer than {min} elements")));
            }
            if let Some(max) = max
                && a.len() > *max
            {
                return Err(Refusal::new(Rule::Conv12, format!("more than {max} elements")));
            }
            for (i, e) in a.iter().enumerate() {
                check_value(e, item).map_err(|r| r.in_field(&format!("[{i}]")))?;
                if *unique && a[..i].contains(e) {
                    return Err(Refusal::new(Rule::Conv6, "an element is repeated").in_field(&format!("[{i}]")));
                }
            }
            Ok(())
        }
        Ty::Map { key, value } => {
            let t = v.as_table().ok_or_else(|| wrong("a table", v))?;
            for (k, e) in t {
                check_value(&toml::Value::String(k.clone()), key).map_err(|r| r.in_field(k))?;
                check_value(e, value).map_err(|r| r.in_field(k))?;
            }
            Ok(())
        }
        Ty::Table(fields) => check_table(v.as_table().ok_or_else(|| wrong("a table", v))?, fields, None),
        Ty::Extras => v.as_table().map(|_| ()).ok_or_else(|| wrong("a table", v)),
    }
}

/// Checks a table against its fields, in the order of conventions.md §11: fields newer than `declared_minor`
/// (`VER.6`, when the file declares a schema minor), unknown keys (`CONV.4`), missing required fields
/// (`CONV.5`), then each field's type. A refusal names the field.
pub fn check_table(t: &toml::Table, fields: &[Field], declared_minor: Option<u32>) -> Result<(), Refusal> {
    let find = |k: &str| fields.iter().find(|f| f.key == k);
    if let Some(minor) = declared_minor {
        for k in t.keys() {
            if let Some(f) = find(k).filter(|f| f.since > minor) {
                return Err(Refusal::new(
                    Rule::Ver6,
                    format!("introduced in minor version {}, but the file declares {minor}", f.since),
                )
                .in_field(k));
            }
        }
    }
    if let Some(k) = t.keys().find(|k| find(k).is_none()) {
        return Err(Refusal::new(Rule::Conv4, "unknown key").in_field(k));
    }
    if let Some(f) = fields.iter().find(|f| f.req.required() && !t.contains_key(f.key)) {
        return Err(Refusal::new(Rule::Conv5, "missing required field").in_field(f.key));
    }
    for (k, v) in t {
        check_value(v, &find(k).expect("checked above").ty).map_err(|r| r.in_field(k))?;
    }
    Ok(())
}

// ---- the page's table -------------------------------------------------------------------------------------------

fn req_cell(r: Req) -> &'static str {
    match r {
        Req::Yes => "yes",
        Req::YesAs(s) => s,
        Req::No => "no",
        Req::When(s) => s,
        Req::OnAdd => "add",
        Req::OnMerge => "merge",
    }
}

/// The page's field table: `| Field | Type | Req. | Default | Meaning |`, one row per listed field, the rows of
/// inline sub-tables after their table's row.
pub fn doc_table(fields: &[Field]) -> String {
    let mut out = String::from("| Field | Type | Req. | Default | Meaning |\n|---|---|---|---|---|\n");
    fn rows(out: &mut String, fields: &[Field]) {
        for f in fields.iter().filter(|f| !f.doc_name.is_empty()) {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                f.doc_name,
                f.doc_type,
                req_cell(f.req),
                f.doc_default,
                f.doc_meaning
            ));
            if let (Ty::Table(sub), true) = (f.ty, f.rows_inline) {
                rows(out, sub);
            }
        }
    }
    rows(&mut out, fields);
    out
}

// ---- the JSON Schema --------------------------------------------------------------------------------------------

/// The JSON Schema of a type (references into `common.schema.json` for shared definitions).
pub fn schema(ty: &Ty) -> Json {
    match ty {
        Ty::Bool => Json::obj([("type", Json::str("boolean"))]),
        Ty::Int { min, max } => {
            Json::obj([("type", Json::str("integer")), ("minimum", Json::Int(*min)), ("maximum", Json::Int(*max))])
        }
        Ty::Const(c) => Json::obj([("const", Json::Int(*c))]),
        Ty::F32 => common("f32"),
        Ty::F32In { min, max, bits } => {
            let n =
                Json::obj([("type", Json::str("number")), ("minimum", Json::Int(*min)), ("maximum", Json::Int(*max))]);
            if *bits { Json::obj([("oneOf", Json::Array(vec![n, common("floatBits")]))]) } else { n }
        }
        Ty::Str { min, max, pattern } => {
            let mut j = Json::obj([("type", Json::str("string"))]);
            if *min > 0 {
                j.push("minLength", Json::Int(*min as i64));
            }
            if *max < usize::MAX {
                j.push("maxLength", Json::Int(*max as i64));
            }
            if let Some(p) = pattern {
                j.push("pattern", Json::str(p.json));
            }
            j
        }
        Ty::Def(name, _) => common(name),
        Ty::Local(name, _) => Json::obj([("$ref", Json::str(format!("#/$defs/{name}")))]),
        Ty::Ref(_, inner) => schema(inner),
        Ty::Either(a, b) => Json::obj([("oneOf", Json::Array(vec![schema(a), schema(b)]))]),
        Ty::Enum([one]) => Json::obj([("const", Json::str(*one))]),
        Ty::Enum(values) => Json::obj([("enum", Json::strs(values.iter()))]),
        Ty::Array { item, min, max, unique } => {
            let mut j = Json::obj([("type", Json::str("array"))]);
            if *min > 0 {
                j.push("minItems", Json::Int(*min as i64));
            }
            if let Some(max) = max {
                j.push("maxItems", Json::Int(*max as i64));
            }
            if *unique {
                j.push("uniqueItems", Json::Bool(true));
            }
            j.push("items", schema(item));
            j
        }
        Ty::Map { key, value } => Json::obj([
            ("type", Json::str("object")),
            ("propertyNames", schema(key)),
            ("additionalProperties", schema(value)),
        ]),
        Ty::Table(fields) => table_schema(fields),
        Ty::Extras => common("extras"),
    }
}

fn common(name: &str) -> Json {
    Json::obj([("$ref", Json::str(format!("common.schema.json#/$defs/{name}")))])
}

/// The JSON Schema of a table: its properties (with `x-since` for fields newer than the first version), the
/// required ones, no others.
pub fn table_schema(fields: &[Field]) -> Json {
    let props = fields
        .iter()
        .map(|f| {
            let mut s = schema(&f.ty);
            if f.since > 0 {
                s.push("x-since", Json::Int(i64::from(f.since)));
            }
            (f.key, s)
        })
        .collect::<Vec<_>>();
    let required: Vec<&str> = fields.iter().filter(|f| f.req.required()).map(|f| f.key).collect();
    let mut j = Json::obj([("type", Json::str("object")), ("properties", Json::obj(props))]);
    if !required.is_empty() {
        j.push("required", Json::strs(required));
    }
    j.push("additionalProperties", Json::Bool(false));
    j
}

// ---- layered kinds: the entry's own definitions and its add / merge rule (layering.md §3.1) ---------------------

/// The kind's own definitions ([`Ty::Local`]) the fields use, each once, a definition after those it uses: the
/// schema's `$defs`.
pub fn local_defs(fields: &[Field]) -> Vec<(&'static str, Json)> {
    fn walk(ty: &Ty, out: &mut Vec<(&'static str, Json)>) {
        match ty {
            Ty::Local(name, inner) => {
                walk(inner, out);
                if !out.iter().any(|(n, _)| n == name) {
                    out.push((name, schema(inner)));
                }
            }
            Ty::Def(_, inner) | Ty::Ref(_, inner) => walk(inner, out),
            Ty::Either(a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Ty::Array { item, .. } => walk(item, out),
            Ty::Map { key, value } => {
                walk(key, out);
                walk(value, out);
            }
            Ty::Table(fields) => fields.iter().for_each(|f| walk(&f.ty, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    fields.iter().for_each(|f| walk(&f.ty, &mut out));
    out
}

fn required_all<'a>(keys: impl IntoIterator<Item = &'a str>) -> Json {
    Json::obj([("required", Json::strs(keys))])
}

fn none_of(fields: &[Field], only: Only) -> Json {
    let keys = fields.iter().filter(|f| f.only == only).map(|f| Json::obj([("required", Json::strs([f.key]))]));
    Json::obj([("not", Json::obj([("anyOf", Json::Array(keys.collect()))]))])
}

/// Whether an entry adds its item: `op = "add"`, or no `op` and the pack's own local id in `id_key` (layering.md
/// §3.1, pack-scoped keys default to `add`).
pub fn is_addition(t: &toml::Table, id_key: &str) -> bool {
    match t.get("op").and_then(|o| o.as_str()) {
        Some(op) => op == "add",
        None => t.contains_key(id_key),
    }
}

/// The schema's add / merge rule of a layered entry as `if` / `then` / `else`: an addition ([`is_addition`]) needs
/// its [`Req::OnAdd`] fields and has none of the [`Only::Merge`] ones; a merge needs its [`Req::OnMerge`] fields
/// and has none of the [`Only::Add`] ones.
pub fn entry_rule(fields: &[Field], id_key: &str) -> Vec<(&'static str, Json)> {
    let op_add = Json::obj([
        ("properties", Json::obj([("op", Json::obj([("const", Json::str("add"))]))])),
        ("required", Json::strs(["op"])),
    ]);
    let own_id = Json::obj([("not", required_all(["op"])), ("required", Json::strs([id_key]))]);
    let test = Json::obj([
        (
            "description",
            Json::str(
                "An addition: op = add, or no op and this pack's own local id (layering.md 3.1, pack-scoped keys \
                 default to add)",
            ),
        ),
        ("anyOf", Json::Array(vec![op_add, own_id])),
    ]);
    let need = |r: Req| fields.iter().filter(move |f| f.req == r).map(|f| f.key);
    let mut then = required_all(need(Req::OnAdd));
    if let (Json::Object(t), Json::Object(n)) = (&mut then, none_of(fields, Only::Merge)) {
        t.extend(n);
    }
    let mut other = required_all(need(Req::OnMerge));
    if let (Json::Object(t), Json::Object(n)) = (&mut other, none_of(fields, Only::Add)) {
        t.extend(n);
    }
    vec![("if", test), ("then", then), ("else", other)]
}

/// Checks a layered entry: its table ([`check_table`]), then the add / merge rule of [`entry_rule`] (a field the
/// entry may not give is `CONV.4`, a missing one `CONV.5`).
pub fn check_entry(
    t: &toml::Table,
    fields: &[Field],
    id_key: &str,
    declared_minor: Option<u32>,
) -> Result<(), Refusal> {
    check_table(t, fields, declared_minor)?;
    let add = is_addition(t, id_key);
    let (not_here, need, what) =
        if add { (Only::Merge, Req::OnAdd, "an addition") } else { (Only::Add, Req::OnMerge, "a merge") };
    if let Some(f) = fields.iter().find(|f| f.only == not_here && t.contains_key(f.key)) {
        return Err(Refusal::new(Rule::Conv4, format!("not allowed in {what}")).in_field(f.key));
    }
    if let Some(f) = fields.iter().find(|f| f.req == need && !t.contains_key(f.key)) {
        return Err(Refusal::new(Rule::Conv5, format!("missing in {what}")).in_field(f.key));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUB: &[Field] = &[Field { key: "x", ty: Ty::Int { min: 0, max: 9 }, req: Req::Yes, ..Field::BLANK }];
    const FIELDS: &[Field] = &[
        Field { key: "a", ty: Ty::Str { min: 1, max: 4, pattern: None }, req: Req::Yes, ..Field::BLANK },
        Field { key: "b", ty: Ty::Enum(&["p", "q"]), since: 2, ..Field::BLANK },
        Field {
            key: "c",
            ty: Ty::Array { item: &Ty::Table(SUB), min: 0, max: Some(2), unique: false },
            ..Field::BLANK
        },
        Field { key: "extras", ty: Ty::Extras, ..Field::BLANK },
    ];

    fn check(text: &str, minor: Option<u32>) -> Result<(), Refusal> {
        check_table(&text.parse::<toml::Table>().unwrap(), FIELDS, minor)
    }

    #[test]
    fn checks_in_order() {
        check("a = \"ok\"\nb = \"p\"\n[[c]]\nx = 1\n[extras]\nanything = 1", Some(2)).unwrap();
        let rule = |t: &str, m| check(t, m).unwrap_err().rule;
        assert_eq!(rule("a = \"ok\"\nb = \"p\"", Some(1)), Rule::Ver6);
        // A newer field is reported before an unknown key.
        assert_eq!(rule("b = \"p\"\nzz = 1", Some(1)), Rule::Ver6);
        assert_eq!(rule("a = \"ok\"\nzz = 1", None), Rule::Conv4);
        assert_eq!(rule("b = \"p\"", None), Rule::Conv5);
        assert_eq!(rule("a = 1", None), Rule::Conv6);
        assert_eq!(rule("a = \"\"", None), Rule::Conv6);
        assert_eq!(rule("a = \"toolong\"", None), Rule::Conv12);
        assert_eq!(rule("a = \"ok\"\nb = \"r\"", None), Rule::Conv6);
        let e = check("a = \"ok\"\n[[c]]\nx = 10", None).unwrap_err();
        assert_eq!((e.rule, e.field.as_deref()), (Rule::Conv6, Some("c[0].x")));
        assert_eq!(rule("a = \"ok\"\n[[c]]\nx = 1\n[[c]]\nx = 1\n[[c]]\nx = 1", None), Rule::Conv12);
    }

    #[test]
    fn schema_shape() {
        let s = table_schema(FIELDS).canonical();
        assert!(s.contains("\"b\":{\"enum\":[\"p\",\"q\"],\"x-since\":2}"), "{s}");
        assert!(s.contains("\"required\":[\"a\"]"), "{s}");
    }
}

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
#[derive(Clone, Copy)]
pub struct Pattern {
    /// The JSON Schema `pattern` (ECMA-262 regular expression).
    pub json: &'static str,
    /// The same check in code.
    pub check: fn(&str) -> bool,
    /// The rule a mismatch breaks.
    pub rule: Rule,
}

/// A field's type.
#[derive(Clone, Copy)]
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
    /// A shared definition of `schemas/common.schema.json` (`$defs/<name>`), checked as `ty`.
    Def(&'static str, &'static Ty),
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
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Req {
    Yes,
    /// Required, with the page's own "Req." text.
    YesAs(&'static str),
    No,
    /// Required under a condition the kind's own code checks; the text is the page's "Req." cell.
    When(&'static str),
}

impl Req {
    pub fn required(self) -> bool {
        matches!(self, Req::Yes | Req::YesAs(_))
    }
}

/// One field of a TOML file kind.
#[derive(Clone, Copy)]
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
        Ty::Def(_, inner) => check_value(v, inner),
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

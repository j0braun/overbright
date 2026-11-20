//! The field tables of the layered kinds Studio edits first: `ob.placements` (world.md §3.9, §4) and `ob.race`
//! (roads-ai.md §3.7). One table per kind gives the file check ([`check_placements`], [`check_race`]), the JSON
//! Schema (`schemas/placements.schema.json`, `schemas/race.schema.json`, written by [`crate::docs`]) and Studio's
//! inspector rows (each field's `label` and `merge`). [`run`]: the run file (`ob.run`), a kind of line records
//! whose one table of records does the same.

pub mod run;

use crate::fields::{self, Field, Only, Pattern, Req, Ty};
use crate::json::Json;
use crate::{Refusal, Rule};

// ---- string checks ------------------------------------------------------------------------------------------------

fn word(s: &str, first: fn(u8) -> bool, rest: fn(u8) -> bool, len: std::ops::RangeInclusive<usize>) -> bool {
    let b = s.as_bytes();
    len.contains(&b.len()) && b.first().is_some_and(|&c| first(c)) && b.iter().all(|&c| rest(c))
}

fn lower_digit(c: u8) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

fn id_char(c: u8) -> bool {
    lower_digit(c) || c == b'_' || c == b'-'
}

fn digits(s: &str, len: std::ops::RangeInclusive<usize>) -> bool {
    len.contains(&s.len()) && s.bytes().all(|c| c.is_ascii_digit())
}

/// `<prefix>1.<minor>`.
fn schema_id(s: &str, prefix: &str) -> bool {
    s.strip_prefix(prefix)
        .and_then(|r| r.strip_prefix("1."))
        .is_some_and(|m| !m.is_empty() && digits(m, 1..=usize::MAX))
}

fn is_placements_schema(s: &str) -> bool {
    schema_id(s, "ob.placements/")
}

fn is_race_schema(s: &str) -> bool {
    schema_id(s, "ob.race/")
}

/// A pack's local id of an added item: `[a-z0-9][a-z0-9_-]{0,31}`.
fn is_local_id(s: &str) -> bool {
    word(s, lower_digit, id_char, 1..=32)
}

/// `<pack>:<rest>` with a pack id and `rest` as `tail` checks it.
fn pack_scoped(s: &str, tail: fn(&str) -> bool) -> bool {
    s.split_once(':').is_some_and(|(p, r)| crate::names::is_pack_id(p) && tail(r))
}

/// A placement's key in its region: `<scenery>/<index>` of an original, `<pack>:<id>` of an added one.
fn is_placement_key(s: &str) -> bool {
    s.split_once('/').is_some_and(|(a, b)| digits(a, 1..=5) && digits(b, 1..=5)) || pack_scoped(s, is_local_id)
}

fn is_solid_ref(s: &str) -> bool {
    s.strip_prefix("solid:").is_some_and(|n| {
        word(n, |c| c.is_ascii_alphanumeric() || c == b'_', |c| c.is_ascii_alphanumeric() || c == b'_', 1..=63)
            || n.strip_prefix("0x")
                .is_some_and(|h| h.len() == 8 && h.bytes().all(|c| matches!(c, b'0'..=b'9' | b'A'..=b'F')))
    })
}

fn is_prop_kind_ref(s: &str) -> bool {
    s.strip_prefix("prop-kind:").and_then(|r| r.split_once('/')).is_some_and(|(region, name)| {
        word(region, |c| c.is_ascii_alphanumeric(), |c| c.is_ascii_alphanumeric(), 2..=8)
            && word(
                name,
                |c| c.is_ascii_alphanumeric() || c == b'_',
                |c| c.is_ascii_alphanumeric() || c == b'_',
                1..=23,
            )
    })
}

/// A race's id: `[a-z0-9_-]{1,40}` (the file name).
fn is_race_id(s: &str) -> bool {
    word(s, id_char, id_char, 1..=40)
}

/// Another pack's race a merge names: `<pack>:<id>`.
fn is_race_key(s: &str) -> bool {
    pack_scoped(s, is_race_id)
}

fn no_control(s: &str) -> bool {
    !s.chars().any(|c| c <= '\u{1F}' || c == '\u{7F}')
}

const fn pat(json: &'static str, check: fn(&str) -> bool, rule: Rule) -> Option<Pattern> {
    Some(Pattern { json, check, rule })
}

const fn text(pattern: Option<Pattern>) -> Ty {
    Ty::Str { min: 0, max: usize::MAX, pattern }
}

// ---- shared types -------------------------------------------------------------------------------------------------

const U16: Ty = Ty::Def("u16", &Ty::Int { min: 0, max: 65535 });
const U32: Ty = Ty::Def("u32", &Ty::Int { min: 0, max: 4_294_967_295 });
const F32: Ty = Ty::Def("f32", &Ty::F32);
const VEC3: Ty = Ty::Def("vec3", &Ty::Array { item: &F32, min: 3, max: Some(3), unique: false });
const MAT3: Ty = Ty::Def("mat3", &Ty::Array { item: &F32, min: 9, max: Some(9), unique: false });
const EXT: Ty = Ty::Def("ext", &Ty::Extras);
/// `MAX_COORD`: a coordinate's limit in metres.
const MAX_COORD: i64 = 100_000;

const fn field(key: &'static str, ty: Ty, req: Req, only: Only, merge: bool, label: &'static str) -> Field {
    Field { key, ty, req, only, merge, label, ..Field::BLANK }
}

// ---- ob.placements ------------------------------------------------------------------------------------------------

const SOLID_REF: Ty = Ty::Local(
    "solidRef",
    &Ty::Ref("solid", &text(pat("^solid:([A-Za-z0-9_]{1,63}|0x[0-9A-F]{8})$", is_solid_ref, Rule::Conv8))),
);
const PLACEMENT_COORD: Ty = Ty::Local(
    "coord",
    &Ty::Array {
        item: &Ty::F32In { min: -MAX_COORD, max: MAX_COORD, bits: true },
        min: 3,
        max: Some(3),
        unique: false,
    },
);

/// A merge's guard: what the original placement it names was (world.md §4.2).
pub const GUARD: &[Field] = &[
    field("solid", SOLID_REF, Req::Yes, Only::Any, false, ""),
    field("position", VEC3, Req::Yes, Only::Any, false, ""),
];

/// One `[[placement]]` entry.
pub const PLACEMENT: &[Field] = &[
    field("op", Ty::Enum(&["merge", "add"]), Req::No, Only::Any, false, ""),
    field(
        "key",
        text(pat(
            "^([0-9]{1,5}/[0-9]{1,5}|[a-z0-9][a-z0-9_-]{0,63}:[a-z0-9][a-z0-9_-]{0,31})$",
            is_placement_key,
            Rule::Conv8,
        )),
        Req::OnMerge,
        Only::Merge,
        false,
        "",
    ),
    field("guard", Ty::Table(GUARD), Req::When("a merge of an original placement"), Only::Merge, false, ""),
    field("id", text(pat("^[a-z0-9][a-z0-9_-]{0,31}$", is_local_id, Rule::Conv8)), Req::OnAdd, Only::Add, false, ""),
    field("scenery", U32, Req::OnAdd, Only::Add, false, "Scenery section"),
    field("solid", SOLID_REF, Req::OnAdd, Only::Add, false, "Object"),
    field("lod_b", SOLID_REF, Req::No, Only::Add, false, "Middle detail"),
    field("lod_c", SOLID_REF, Req::No, Only::Add, false, "Low detail"),
    field("radius_m", F32, Req::No, Only::Add, false, "Draw radius"),
    field("flags", U16, Req::No, Only::Add, false, "Flags"),
    field(
        "prop",
        Ty::Ref(
            "prop-kind",
            &text(pat("^prop-kind:[A-Za-z0-9]{2,8}/[A-Za-z0-9_]{1,23}$", is_prop_kind_ref, Rule::Conv8)),
        ),
        Req::No,
        Only::Add,
        false,
        "Knock-over prop",
    ),
    field("position", PLACEMENT_COORD, Req::OnAdd, Only::Any, true, "Position"),
    field("rotation", MAT3, Req::No, Only::Any, true, "Rotation"),
    field("disabled", Ty::Bool, Req::No, Only::Any, true, "Hidden"),
    field("extras", Ty::Extras, Req::No, Only::Any, false, ""),
];

/// The most entries one placements file holds.
pub const MAX_PLACEMENTS: usize = 50_000;

/// The file `world/<REGION>/placements.toml`.
pub const PLACEMENTS_FILE: &[Field] = &[
    field(
        "schema",
        text(pat(r"^ob\.placements/1\.[0-9]+$", is_placements_schema, Rule::Conv6)),
        Req::Yes,
        Only::Any,
        false,
        "",
    ),
    field(
        "placement",
        Ty::Array {
            item: &Ty::Local("entry", &Ty::Table(PLACEMENT)),
            min: 0,
            max: Some(MAX_PLACEMENTS),
            unique: false,
        },
        Req::Yes,
        Only::Any,
        false,
        "",
    ),
    field("extras", Ty::Extras, Req::No, Only::Any, false, ""),
    field("ext", EXT, Req::No, Only::Any, false, ""),
];

/// Checks a placements file: its fields, then each entry's add / merge rule (a refusal names `placement[i]`).
pub fn check_placements(doc: &toml::Table) -> Result<(), Refusal> {
    fields::check_table(doc, PLACEMENTS_FILE, None)?;
    for (i, e) in doc["placement"].as_array().into_iter().flatten().enumerate() {
        let t = e.as_table().expect("checked above");
        fields::check_entry(t, PLACEMENT, "id", None).map_err(|r| r.in_field(&format!("placement[{i}]")))?;
    }
    Ok(())
}

fn schema_head(id: &str, title: &str, description: &str) -> Vec<(String, Json)> {
    let mut description = description.to_string();
    if !description.is_empty() {
        description.push(' ');
    }
    description += "Generated from the code (crates/ob-asset-format/src/kinds.rs); do not edit.";
    vec![
        ("$schema".into(), Json::str("https://json-schema.org/draft/2020-12/schema")),
        ("$id".into(), Json::str(id)),
        ("title".into(), Json::str(title)),
        ("description".into(), Json::str(description)),
    ]
}

fn object(mut head: Vec<(String, Json)>, body: Json, more: Vec<(&'static str, Json)>) -> Json {
    let Json::Object(body) = body else { unreachable!("a table's schema is an object") };
    head.extend(body);
    head.extend(more.into_iter().map(|(k, v)| (k.to_string(), v)));
    Json::Object(head)
}

/// The JSON Schema `schemas/placements.schema.json`.
pub fn placements_schema() -> Json {
    let head = schema_head(
        "placements.schema.json",
        "world/<REGION>/placements.toml (ob.placements)",
        "world.md 3.9 and 4: a mod pack's moved, disabled and added placements in one region. Mod packs only \
         (WLD.10). An entry with id and no op is an addition, one with key and no op a merge. Guards, keys, \
         dependencies and clashes are checked against the layer stack (WLD.11..15, MRG.*, ORD.2); one stale entry \
         refuses the whole pack.",
    );
    let defs = fields::local_defs(PLACEMENTS_FILE).into_iter().map(|(name, j)| {
        if name == "entry" { (name, object(Vec::new(), j, fields::entry_rule(PLACEMENT, "id"))) } else { (name, j) }
    });
    object(head, fields::table_schema(PLACEMENTS_FILE), vec![("$defs", Json::obj(defs))])
}

// ---- ob.race ------------------------------------------------------------------------------------------------------

const RACE_COORD: Ty = Ty::Local("coord", &Ty::F32In { min: -MAX_COORD, max: MAX_COORD, bits: false });

/// A spot: a place and a heading (degrees around +z from +x).
pub const SPOT: &[Field] = &[
    field(
        "position",
        Ty::Array { item: &RACE_COORD, min: 3, max: Some(3), unique: false },
        Req::Yes,
        Only::Any,
        true,
        "Position",
    ),
    field("heading_deg", RACE_COORD, Req::Yes, Only::Any, true, "Heading"),
];

const SPOT_TY: Ty = Ty::Local("spot", &Ty::Table(SPOT));

/// A grid slot's key (layering.md §3.2, the grid is keyed by position): a base slot's `index`, or `<pack>:<n>`, the
/// n-th slot that pack added.
fn is_slot_key(s: &str) -> bool {
    pack_scoped(s, |n| digits(n, 1..=2))
}

/// One grid slot: a spot; in a merge of another pack's race, a slot named by `index` (`<pack>:<n>`) with only the fields it
/// changes, or one more slot (`op = "add"`), or a slot hidden (`disabled`).
pub const GRID_SLOT: &[Field] = &[
    field("op", Ty::Enum(&["merge", "add"]), Req::No, Only::Any, false, ""),
    field(
        "index",
        Ty::Either(
            &Ty::Int { min: 0, max: MAX_GRID as i64 - 1 },
            &text(pat("^[a-z0-9][a-z0-9_-]{0,63}:[0-9]{1,2}$", is_slot_key, Rule::Conv8)),
        ),
        Req::No,
        Only::Any,
        false,
        "",
    ),
    field(
        "position",
        Ty::Array { item: &RACE_COORD, min: 3, max: Some(3), unique: false },
        Req::When("a new slot"),
        Only::Any,
        true,
        "Position",
    ),
    field("heading_deg", RACE_COORD, Req::When("a new slot"), Only::Any, true, "Heading"),
    field("disabled", Ty::Bool, Req::No, Only::Any, true, "Off"),
];

/// The most checkpoints and grid slots of a race.
pub const MAX_CHECKPOINTS: usize = 64;
pub const MAX_GRID: usize = 8;
/// The race modes; `sprint` and `drift` run one lap.
pub const MODES: &[&str] = &["circuit", "sprint", "streetx", "drift"];

/// The file `races/<id>.toml`: one race. A pack adds its own race (`id`); a pack that depends on another merges
/// into that pack's race or disables it (`key`, layering.md §3.1).
pub const RACE: &[Field] = &[
    field("schema", text(pat(r"^ob\.race/1\.[0-9]+$", is_race_schema, Rule::Conv6)), Req::Yes, Only::Any, false, ""),
    field("op", Ty::Enum(&["merge", "add"]), Req::No, Only::Any, false, ""),
    field(
        "key",
        text(pat("^[a-z0-9][a-z0-9_-]{0,63}:[a-z0-9_-]{1,40}$", is_race_key, Rule::Conv8)),
        Req::OnMerge,
        Only::Merge,
        false,
        "",
    ),
    field("disabled", Ty::Bool, Req::No, Only::Any, true, "Off"),
    field("id", text(pat("^[a-z0-9_-]{1,40}$", is_race_id, Rule::Conv8)), Req::OnAdd, Only::Add, false, "Id"),
    field(
        "name",
        Ty::Str { min: 0, max: 200, pattern: pat(r"^[^\u0000-\u001F\u007F]*$", no_control, Rule::Conv6) },
        Req::No,
        Only::Any,
        true,
        "Name",
    ),
    field("track", U16, Req::OnAdd, Only::Any, true, "Base track"),
    field("backward", Ty::Bool, Req::No, Only::Any, true, "Backward"),
    field("mode", Ty::Enum(MODES), Req::OnAdd, Only::Any, true, "Mode"),
    field("laps", Ty::Int { min: 1, max: 20 }, Req::No, Only::Any, true, "Laps"),
    field(
        "checkpoints",
        Ty::Array {
            item: &Ty::Local("xy", &Ty::Array { item: &RACE_COORD, min: 2, max: Some(2), unique: false }),
            min: 0,
            max: Some(MAX_CHECKPOINTS),
            unique: false,
        },
        Req::No,
        Only::Any,
        true,
        "Checkpoints",
    ),
    field(
        "grid",
        Ty::Array { item: &Ty::Local("gridSlot", &Ty::Table(GRID_SLOT)), min: 0, max: Some(MAX_GRID), unique: false },
        Req::No,
        Only::Any,
        true,
        "Grid",
    ),
    field("start", SPOT_TY, Req::No, Only::Any, true, "Start line"),
    field("finish", SPOT_TY, Req::No, Only::Any, true, "Finish line"),
    field("extras", Ty::Extras, Req::No, Only::Any, false, ""),
    field("ext", EXT, Req::No, Only::Any, false, ""),
];

/// Checks a race file: its fields, the add / merge rule, a new grid slot whole (in an addition every slot is new;
/// in a merge, one without `index` or with `op = "add"`), and one lap for `sprint` and `drift`.
pub fn check_race(doc: &toml::Table) -> Result<(), Refusal> {
    fields::check_entry(doc, RACE, "id", None)?;
    let addition = fields::is_addition(doc, "id");
    for (i, slot) in doc.get("grid").and_then(|g| g.as_array()).into_iter().flatten().enumerate() {
        let s = slot.as_table().expect("checked above");
        let new = s.get("op").and_then(|o| o.as_str()) == Some("add") || !s.contains_key("index");
        let at = |r: Refusal| r.in_field(&format!("grid[{i}]"));
        if addition && (s.contains_key("index") || s.contains_key("op")) {
            return Err(at(Refusal::new(Rule::Conv4, "a race's own grid has no slot keys")));
        }
        if let Some(f) = ["position", "heading_deg"].into_iter().find(|f| new && !s.contains_key(*f)) {
            return Err(at(Refusal::new(Rule::Conv5, "missing in a new slot").in_field(f)));
        }
    }
    let one_lap = doc.get("mode").and_then(|m| m.as_str()).is_some_and(|m| m == "sprint" || m == "drift");
    if one_lap && doc.get("laps").is_some_and(|l| l.as_integer() != Some(1)) {
        return Err(Refusal::new(Rule::Conv6, "a sprint or drift race runs one lap").in_field("laps"));
    }
    Ok(())
}

/// The JSON Schema `schemas/race.schema.json`.
pub fn race_schema() -> Json {
    let head = schema_head(
        "race.schema.json",
        "ob.race: a custom race of a mod pack (roads-ai.md 3.7)",
        "A pack adds its own race (id; no op: an addition); a pack that lists the race's pack in [dependencies] \
         merges into it or disables it (key = \"<pack>:<id>\"; layering.md 3.1).",
    );
    let one_lap = Json::obj([
        (
            "if",
            Json::obj([
                ("properties", Json::obj([("mode", Json::obj([("enum", Json::strs(["sprint", "drift"]))]))])),
                ("required", Json::strs(["mode"])),
            ]),
        ),
        ("then", Json::obj([("properties", Json::obj([("laps", Json::obj([("const", Json::Int(1))]))]))])),
    ]);
    let mut more = fields::entry_rule(RACE, "id");
    more.push(("allOf", Json::Array(vec![one_lap])));
    more.push(("$defs", Json::obj(fields::local_defs(RACE))));
    let coord_note = "finite and within +-100000 (MAX_COORD); exact-bits escapes are not used here";
    if let Some((_, Json::Object(defs))) = more.last_mut()
        && let Some((_, Json::Object(coord))) = defs.iter_mut().find(|(k, _)| k == "coord")
    {
        coord.insert(0, ("description".into(), Json::str(coord_note)));
    }
    object(head, fields::table_schema(RACE), more)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> toml::Table {
        text.parse().unwrap()
    }

    fn example(rel: &str) -> toml::Table {
        let path = format!("{}/../../docs/modding/asset-format/examples/{rel}", env!("CARGO_MANIFEST_DIR"));
        doc(&std::fs::read_to_string(path).unwrap())
    }

    #[test]
    fn the_examples_pass() {
        check_placements(&example("world/placements.toml")).unwrap();
        check_race(&example("roads-ai/race-docks-sprint.toml")).unwrap();
    }

    #[test]
    fn placement_entries_follow_the_add_and_merge_rule() {
        let one =
            |entry: &str| check_placements(&doc(&format!("schema = \"ob.placements/1.0\"\n[[placement]]\n{entry}")));
        let rule = |entry: &str| one(entry).unwrap_err();
        one("key = \"1407/0\"\nposition = [1.0, 2.0, 3.0]").unwrap();
        one("id = \"a1\"\nscenery = 1\nsolid = \"solid:XO_CONE_01\"\nposition = [1.0, 2.0, \"bits:0x3F800000\"]")
            .unwrap();
        // An addition without its solid; a merge giving one; an addition with a key.
        let r = rule("id = \"a1\"\nscenery = 1\nposition = [1.0, 2.0, 3.0]");
        assert_eq!((r.rule, r.field.as_deref()), (Rule::Conv5, Some("placement[0].solid")));
        assert_eq!(rule("key = \"1407/0\"\nsolid = \"solid:X\"").rule, Rule::Conv4);
        assert_eq!(rule("op = \"add\"\nkey = \"1407/0\"\nid = \"a\"\nscenery = 1\nsolid = \"solid:X\"\nposition = [0.0, 0.0, 0.0]").rule, Rule::Conv4);
        assert_eq!(rule("position = [1.0, 2.0, 3.0]").rule, Rule::Conv5, "a merge without its key");
        assert_eq!(rule("key = \"1407/0\"\nposition = [1.0, 2.0, 300000.0]").rule, Rule::Conv6);
        assert_eq!(rule("key = \"14070000/0\"").rule, Rule::Conv8);
    }

    #[test]
    fn a_race_is_added_by_its_pack_and_merged_or_disabled_by_a_dependent() {
        let r = |text: &str| check_race(&doc(&format!("schema = \"ob.race/1.0\"\n{text}")));
        r("id = \"sprint\"\ntrack = 4001\nmode = \"sprint\"").unwrap();
        r("key = \"docks-remix:sprint\"\nname = \"Docks sprint, longer\"\ncheckpoints = [[1.0, 2.0]]").unwrap();
        r("op = \"merge\"\nkey = \"docks-remix:sprint\"\ndisabled = true").unwrap();
        assert_eq!(
            r("id = \"sprint\"\nmode = \"sprint\"").unwrap_err().rule,
            Rule::Conv5,
            "an addition needs its track"
        );
        assert_eq!(r("key = \"docks-remix:sprint\"\nid = \"other\"").unwrap_err().rule, Rule::Conv4);
        assert_eq!(r("op = \"replace\"\nkey = \"docks-remix:sprint\"").unwrap_err().rule, Rule::Conv6);
        assert_eq!(r("id = \"s\"\ntrack = 1\nmode = \"drift\"\nlaps = 3").unwrap_err().rule, Rule::Conv6);
        // The grid of another pack's race, slot by slot.
        r("key = \"docks-remix:sprint\"\ngrid = [{ index = 1, heading_deg = 40.0 }, { index = \"addon:0\", disabled = true }, { op = \"add\", position = [0.0, 0.0, 0.0], heading_deg = 1.0 }]").unwrap();
        assert_eq!(
            r("key = \"docks-remix:sprint\"\ngrid = [{ op = \"add\", position = [0.0, 0.0, 0.0] }]").unwrap_err().rule,
            Rule::Conv5
        );
        assert_eq!(
            r("key = \"docks-remix:sprint\"\ngrid = [{ index = 8, heading_deg = 1.0 }]").unwrap_err().rule,
            Rule::Conv6
        );
        assert_eq!(
            r("id = \"s\"\ntrack = 1\nmode = \"sprint\"\ngrid = [{ index = 0, heading_deg = 1.0 }]").unwrap_err().rule,
            Rule::Conv4
        );
        assert_eq!(r("id = \"s\"\ntrack = 1\nmode = \"sprint\"\nstart = { position = [0.0, 0.0, \"bits:0x00000000\"], heading_deg = 0.0 }").unwrap_err().rule, Rule::Conv6, "no exact-bits escapes in a race");
    }
}

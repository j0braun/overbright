//! `ob.run` (runs.md): the run file, one file for simulations, recordings and replays. Plain text, one record per
//! line, the record's tag first; [`RUN`] is the kind's one table: each record's fields in order. It gives the file
//! check ([`check_run`], each line read into a table of its named fields and checked by [`fields::check_table`]),
//! the JSON Schema `schemas/run.schema.json` ([`run_schema`]: each line as that table), the page's record table
//! ([`record_table`]) and Studio's record registry. The writer and the typed reader are the map data's
//! (`ob_map::tools::map::sim::format`); its tests keep them on this table.
//!
//! Unlike a TOML kind, a reader skips what it does not know (a record of an unknown tag, the words after the fields
//! it knows, an unknown `key=value` of the header), so a newer minor version still reads; only a newer major is
//! refused (`VER.2`).

use super::{digits, pat, text, word};
use crate::fields::{self, Field, Req, Ty};
use crate::json::Json;
use crate::text::SchemaId;
use crate::{Refusal, Rule};

/// The kind's name in a schema id, and the major and highest minor version this reader knows.
pub const KIND: &str = "run";
pub const MAJOR: u32 = 1;
pub const MINOR: u32 = 0;
/// The schema id a writer of this version puts in the header (`schema=`).
pub const SCHEMA: &str = "ob.run/1.0";

/// One record of the run file.
#[derive(Clone, Copy, Debug)]
pub struct Record {
    /// The line's first word.
    pub tag: &'static str,
    /// What the record is (the page's row).
    pub meaning: &'static str,
    /// Its fields in the order they are written.
    pub fields: &'static [Field],
    /// The number of fields written as plain words before `key=value` pairs (the header), or all of them.
    pub named_from: Option<usize>,
    /// The last field takes the rest of the line: a text (a name), or a list of tables split at `|` (a tick's
    /// cars).
    pub rest: bool,
}

const fn rf(key: &'static str, ty: Ty, req: Req, doc_type: &'static str, meaning: &'static str) -> Field {
    Field { key, ty, req, doc_type, doc_meaning: meaning, ..Field::BLANK }
}

const U8: Ty = Ty::Int { min: 0, max: 255 };
const U16: Ty = Ty::Int { min: 0, max: 65535 };
const U32: Ty = Ty::Int { min: 0, max: 4_294_967_295 };
const I32: Ty = Ty::Int { min: -2_147_483_648, max: 2_147_483_647 };
const TICK: Ty = Ty::Int { min: 0, max: i64::MAX };
/// A number written as a decimal (no exact-bits escape in a run file), finite and within ±1e9.
const F32: Ty = Ty::Local("num", &Ty::F32In { min: -1_000_000_000, max: 1_000_000_000, bits: false });
const NONE: Ty = Ty::Enum(&["-"]);
const F32_OR_NONE: Ty = Ty::Either(&F32, &NONE);
const CAR_KIND: Ty = Ty::Enum(&["P", "R", "T", "X"]);

fn is_run_schema(s: &str) -> bool {
    super::schema_id(s, "ob.run/")
}

fn is_seed(s: &str) -> bool {
    digits(s, 1..=20) && s.parse::<u64>().is_ok()
}

fn is_race_or_none(s: &str) -> bool {
    s == "-" || super::is_race_id(s)
}

fn is_model(s: &str) -> bool {
    word(s, |c| c.is_ascii_graphic() && c != b'|', |c| c.is_ascii_graphic() && c != b'|', 1..=64)
}

fn is_part(s: &str) -> bool {
    s.split_once(':')
        .is_some_and(|(a, b)| a.len() == 2 && b.len() == 8 && a.bytes().chain(b.bytes()).all(|c| c.is_ascii_hexdigit()))
}

fn is_event_word(s: &str) -> bool {
    word(s, |c| c.is_ascii_lowercase(), |c| c.is_ascii_lowercase() || c == b'-', 1..=32)
}

fn is_hash(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

fn one_line(s: &str) -> bool {
    !s.chars().any(|c| c.is_control() || c == '|')
}

const HEADER: &[Field] = &[
    rf("version", Ty::Int { min: 1, max: 1 }, Req::Yes, "u32", "The major version (`VER.2` when unknown)."),
    rf(
        "schema",
        text(pat(r"^ob\.run/1\.[0-9]+$", is_run_schema, Rule::Conv3)),
        Req::No,
        "schema id",
        "`ob.run/1.<minor>`; streams written before the kind had none (read as 1.0).",
    ),
    rf("mode", Ty::Enum(&["race", "world"]), Req::No, "enum", "A custom race, or free roam from a spot."),
    rf("seed", text(pat("^[0-9]{1,20}$", is_seed, Rule::Conv6)), Req::No, "u64", "The run's random seed."),
    rf(
        "race",
        text(pat("^(-|[a-z0-9_-]{1,40})$", is_race_or_none, Rule::Conv8)),
        Req::No,
        "race id",
        "The custom race's id (`-` in free roam).",
    ),
    rf("track", U16, Req::No, "u16", "The base track."),
    rf("laps", U32, Req::No, "u32", "The race's laps."),
    rf("checkpoints", U32, Req::No, "u32", "The race's checkpoints."),
];

const CAR: &[Field] = &[
    rf("id", U32, Req::Yes, "u32", "The car: the player's slot 0, opponents 1.., traffic 100.., street racers 200.."),
    rf("kind", CAR_KIND, Req::Yes, "enum", "Player's slot, racer, traffic, street racer."),
    rf("x", F32, Req::Yes, "f32 m", "Position (world metres, to 1 cm)."),
    rf("y", F32, Req::Yes, "f32 m", ""),
    rf("z", F32, Req::Yes, "f32 m", ""),
    rf("qx", F32, Req::Yes, "f32", "Rotation, a unit quaternion (to 1e-4)."),
    rf("qy", F32, Req::Yes, "f32", ""),
    rf("qz", F32, Req::Yes, "f32", ""),
    rf("qw", F32, Req::Yes, "f32", ""),
    rf("speed", F32, Req::Yes, "f32 m/s", "Speed."),
    rf("lap", I32, Req::Yes, "i32", "The lap under way, from 0."),
    rf("checkpoint", U32, Req::Yes, "u32", "The race's checkpoints passed in this lap."),
    rf("place", U32, Req::Yes, "u32", "1-based; 0 outside a race."),
    rf(
        "steer0",
        F32,
        Req::No,
        "f32 rad",
        "The front wheels' steer, left and right (the wheels and lights follow together).",
    ),
    rf("steer1", F32, Req::No, "f32 rad", ""),
    rf("roll0", F32, Req::No, "f32 rad", "Each wheel's roll about its axle, 0..2π, table order."),
    rf("roll1", F32, Req::No, "f32 rad", ""),
    rf("roll2", F32, Req::No, "f32 rad", ""),
    rf("roll3", F32, Req::No, "f32 rad", ""),
    rf("lift0", F32, Req::No, "f32 m", "Each wheel's lift: the hub's offset from rest."),
    rf("lift1", F32, Req::No, "f32 m", ""),
    rf("lift2", F32, Req::No, "f32 m", ""),
    rf("lift3", F32, Req::No, "f32 m", ""),
    rf("lights", U8, Req::No, "u8", "Bit 1 headlights, bit 2 brake lights."),
];

/// The record of one car at a tick (a part of [`RUN`]'s `F`).
pub const RUN_CAR: &[Field] = CAR;

/// The run file's records, in the order a writer first writes them.
pub const RUN: &[Record] = &[
    Record {
        tag: "OBSIM",
        meaning: "The header, the file's first line: its version, then `key=value` pairs.",
        fields: HEADER,
        named_from: Some(1),
        rest: false,
    },
    Record {
        tag: "C",
        meaning: "A car's first appearance.",
        fields: &[
            rf("id", U32, Req::Yes, "u32", "The car."),
            rf("kind", CAR_KIND, Req::Yes, "enum", "Its role."),
            rf(
                "name",
                Ty::Str { min: 0, max: 200, pattern: pat(r"^[^\u0000-\u001F\u007F|]*$", one_line, Rule::Conv6) },
                Req::No,
                "text",
                "Its name (the rest of the line).",
            ),
        ],
        named_from: None,
        rest: true,
    },
    Record {
        tag: "L",
        meaning: "A car's look: before its first tick and again when it changes (a traffic slot taking another car).",
        fields: &[
            rf("id", U32, Req::Yes, "u32", "The car."),
            rf(
                "model",
                text(pat(r"^[!-{}~]{1,64}$", is_model, Rule::Conv8)),
                Req::Yes,
                "name",
                "The model's folder (`CARS/<model>`).",
            ),
            rf("camber", F32, Req::Yes, "f32 rad", "The front wheels' x-turn."),
            rf(
                "parts",
                Ty::Array {
                    item: &text(pat("^[0-9A-Fa-f]{2}:[0-9A-Fa-f]{8}$", is_part, Rule::Conv8)),
                    min: 0,
                    max: Some(256),
                    unique: false,
                },
                Req::Yes,
                "list (`,`; `-` none)",
                "The fitted visual parts: slot and part hash in hex, in slot order.",
            ),
        ],
        named_from: None,
        rest: false,
    },
    Record {
        tag: "F",
        meaning: "One tick (the game's 60 Hz update) and every car in it.",
        fields: &[
            rf("tick", TICK, Req::Yes, "u64", "The tick, from 0."),
            rf(
                "time",
                F32,
                Req::Yes,
                "f32 s",
                "The race clock (negative before the green), or since the start in free roam.",
            ),
            rf(
                "cars",
                Ty::Array { item: &Ty::Local("car", &Ty::Table(CAR)), min: 0, max: Some(1024), unique: false },
                Req::No,
                "list (`|`)",
                "Each car (the rest of the line), its fields as below.",
            ),
        ],
        named_from: None,
        rest: true,
    },
    Record {
        tag: "E",
        meaning: "An event of the tick before it.",
        fields: &[
            rf("tick", TICK, Req::Yes, "u64", "The tick."),
            rf("time", F32, Req::Yes, "f32 s", "Its time."),
            rf(
                "kind",
                text(pat("^[a-z][a-z-]{0,31}$", is_event_word, Rule::Conv6)),
                Req::Yes,
                "word",
                "`start`, `checkpoint`, `lap`, `finish`, `crash`, `stuck`, `wrong-way`; a reader skips an event of a \
                 word it does not know.",
            ),
            rf("car", U32, Req::Yes, "u32", "The car."),
            rf(
                "checkpoint",
                U32,
                Req::Yes,
                "u32",
                "The checkpoint passed, or for a problem the next one (1-based; 0 none).",
            ),
            rf(
                "value",
                F32,
                Req::Yes,
                "f32",
                "A lap's or the finish's time (s), the speed a crash lost (m/s), how long stuck (s).",
            ),
        ],
        named_from: None,
        rest: false,
    },
    Record {
        tag: "H",
        meaning: "The state hash, every 60 ticks and after the last tick: the same seed gives the same hashes, so a \
                  replay or a second run shows the tick where it parted.",
        fields: &[
            rf("tick", TICK, Req::Yes, "u64", "The last tick it covers."),
            rf(
                "hash",
                text(pat("^[0-9a-f]{16}$", is_hash, Rule::Conv8)),
                Req::Yes,
                "hex u64",
                "FNV-1a 64 over every tick so far: per car its id and the exact bits of its position, rotation, \
                 velocity and spin (runs.md 3.2).",
            ),
        ],
        named_from: None,
        rest: false,
    },
    Record {
        tag: "R",
        meaning: "A racer's result at the end.",
        fields: &[
            rf("car", U32, Req::Yes, "u32", "The racer."),
            rf("place", U32, Req::Yes, "u32", "1-based; 0 did not finish."),
            rf("time", F32_OR_NONE, Req::Yes, "f32 s or `-`", "The race time."),
            rf("best", F32_OR_NONE, Req::Yes, "f32 s or `-`", "The best lap."),
            rf(
                "laps",
                Ty::Array { item: &F32, min: 0, max: Some(1000), unique: false },
                Req::No,
                "list (`,`; `-` none)",
                "Each lap's time.",
            ),
        ],
        named_from: None,
        rest: false,
    },
    Record {
        tag: "END",
        meaning: "The end, the file's last line.",
        fields: &[rf(
            "why",
            Ty::Str { min: 1, max: 400, pattern: pat(r"^[^\u0000-\u001F\u007F|]*$", one_line, Rule::Conv6) },
            Req::Yes,
            "text",
            "`finish`, `quit`, `limit` or `error <why>` (the rest of the line).",
        )],
        named_from: None,
        rest: true,
    },
];

/// The record of a tag.
pub fn record(tag: &str) -> Option<&'static Record> {
    RUN.iter().find(|r| r.tag == tag)
}

/// A word as the value its type reads (a word that does not read as its type stays a string, which the check
/// then refuses with the type's rule).
fn word_value(w: &str, ty: &Ty) -> toml::Value {
    match ty {
        Ty::Def(_, inner) | Ty::Local(_, inner) | Ty::Ref(_, inner) => word_value(w, inner),
        Ty::Int { .. } | Ty::Const(_) => w.parse::<i64>().map_or_else(|_| w.into(), toml::Value::Integer),
        Ty::F32 | Ty::F32In { .. } => w.parse::<f64>().map_or_else(|_| w.into(), toml::Value::Float),
        Ty::Either(a, b) => {
            let v = word_value(w, a);
            if fields::check_value(&v, a).is_ok() { v } else { word_value(w, b) }
        }
        Ty::Array { .. } if w == "-" => toml::Value::Array(Vec::new()),
        Ty::Array { item, .. } => toml::Value::Array(w.split(',').map(|p| word_value(p, item)).collect()),
        _ => w.into(),
    }
}

/// The table of one list element's (or one record's) words, in field order; the words after the known fields are
/// skipped.
fn words_table<'a>(words: impl Iterator<Item = &'a str>, fields: &[Field]) -> toml::Table {
    fields.iter().zip(words).map(|(f, w)| (f.key.to_string(), word_value(w, &f.ty))).collect()
}

/// The rest of the line as its field's value: a list of tables split at `|`, or the text.
fn rest_value(rest: &str, ty: &Ty) -> Option<toml::Value> {
    let rest = rest.trim();
    match ty {
        Ty::Array { item: Ty::Local(_, Ty::Table(sub)) | Ty::Table(sub), .. } => (!rest.is_empty()).then(|| {
            toml::Value::Array(
                rest.split('|').map(|e| toml::Value::Table(words_table(e.split_whitespace(), sub))).collect(),
            )
        }),
        _ => (!rest.is_empty()).then(|| rest.into()),
    }
}

/// One line read into its record and the table of its named fields; `None`: a record this reader does not know
/// (skipped). The table is not checked yet ([`check_line`]).
pub fn read_line(line: &str) -> Option<(&'static Record, toml::Table)> {
    let line = line.trim_end_matches(['\r', '\n']);
    let (tag, rest) = line.split_once(' ').unwrap_or((line, ""));
    let rec = record(tag)?;
    let t = if let Some(n) = rec.named_from {
        let mut w = rest.split_whitespace();
        let mut t = words_table(w.by_ref().take(n), &rec.fields[..n]);
        for (k, v) in w.filter_map(|kv| kv.split_once('=')) {
            if let Some(f) = rec.fields[n..].iter().find(|f| f.key == k) {
                t.insert(k.to_string(), word_value(v, &f.ty));
            }
        }
        t
    } else if rec.rest {
        let (head, last) = rec.fields.split_at(rec.fields.len() - 1);
        let mut w = rest.splitn(head.len() + 1, ' ');
        let mut t = words_table(w.by_ref().take(head.len()), head);
        if let Some(v) = w.next().and_then(|r| rest_value(r, &last[0].ty)) {
            t.insert(last[0].key.to_string(), v);
        }
        t
    } else {
        words_table(rest.split_whitespace(), rec.fields)
    };
    Some((rec, t))
}

/// Checks one line read by [`read_line`] against its record (a refusal names the record's field).
pub fn check_line(rec: &Record, t: &toml::Table, minor: Option<u32>) -> Result<(), Refusal> {
    fields::check_table(t, rec.fields, minor).map_err(|r| {
        let at = r.field.clone().unwrap_or_default();
        Refusal {
            message: format!("{} record: {}", rec.tag, r.message),
            field: Some(at).filter(|a| !a.is_empty()),
            ..r
        }
    })
}

/// What a checked run file holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunSummary {
    /// The header's schema id (`ob.run/1.0` for a file from before the kind).
    pub schema: String,
    /// Lines of each known record, in [`RUN`]'s order.
    pub records: Vec<(&'static str, u64)>,
    /// Lines skipped: records this reader does not know.
    pub skipped: u64,
    /// Whether the file ends with `END` (a run still being written does not).
    pub ended: bool,
}

/// Checks a run file: the header first (its major `VER.2` when unknown, its schema `CONV.3` when malformed), then
/// every known record against [`RUN`]; unknown records and trailing words are skipped. A refusal names the line.
pub fn check_run(text: &str) -> Result<RunSummary, Refusal> {
    let mut sum = RunSummary { records: RUN.iter().map(|r| (r.tag, 0)).collect(), ..Default::default() };
    let mut minor = None;
    for (i, line) in text.lines().enumerate() {
        let n = i as u32 + 1;
        let at = |r: Refusal| r.at_line(n);
        let Some((rec, t)) = read_line(line) else {
            if i == 0 {
                return Err(at(Refusal::new(Rule::Conv3, "the first line is not the run file's header (`OBSIM`)")));
            }
            sum.skipped += 1;
            continue;
        };
        if i == 0 {
            if rec.tag != "OBSIM" {
                return Err(at(Refusal::new(Rule::Conv3, "the first line is not the run file's header (`OBSIM`)")));
            }
            if let Some(v) = t.get("version").and_then(|v| v.as_integer())
                && v != i64::from(MAJOR)
            {
                return Err(at(Refusal::new(Rule::Ver2, format!("major version {v}; this reader reads {MAJOR}"))
                    .in_field("version")));
            }
            let id = match t.get("schema").and_then(|s| s.as_str()) {
                Some(s) => SchemaId::parse(s).map_err(at)?,
                None => SchemaId { kind: KIND.into(), major: MAJOR, minor: 0 },
            };
            if id.kind != KIND || id.major != MAJOR {
                return Err(at(
                    Refusal::new(Rule::Ver2, format!("{id}: this reader reads ob.{KIND}/{MAJOR}.x")).in_field("schema")
                ));
            }
            // A newer minor reads: what it adds is skipped (runs.md 9).
            minor = Some(id.minor.min(MINOR));
            sum.schema = id.to_string();
        } else if rec.tag == "OBSIM" {
            return Err(at(Refusal::new(Rule::Conv6, "a second header")));
        } else if sum.ended {
            return Err(at(Refusal::new(Rule::Conv6, "a record after `END`")));
        }
        check_line(rec, &t, minor).map_err(at)?;
        if let Some(c) = sum.records.iter_mut().find(|c| c.0 == rec.tag) {
            c.1 += 1;
        }
        sum.ended |= rec.tag == "END";
    }
    if minor.is_none() {
        return Err(Refusal::new(Rule::Conv3, "an empty file (no header)").at_line(1));
    }
    Ok(sum)
}

// ---- the page and the schema --------------------------------------------------------------------------------------

/// The page's record table (`<!-- generated:records run -->`): every record's fields in order, the car's after
/// them.
pub fn record_table() -> String {
    let mut out = String::from("| Record | Field | Type | Req. | Meaning |\n|---|---|---|---|---|\n");
    let mut rows = |what: String, fields: &[Field], named_from: Option<usize>| {
        for (i, f) in fields.iter().enumerate() {
            let name =
                if named_from.is_some_and(|n| i >= n) { format!("`{}=`", f.key) } else { format!("`{}`", f.key) };
            let req = if f.req.required() { "yes" } else { "no" };
            let first = if i == 0 { what.as_str() } else { "" };
            out.push_str(&format!("| {first} | {name} | {} | {req} | {} |\n", f.doc_type, f.doc_meaning));
        }
    };
    for r in RUN {
        rows(format!("`{}`: {}", r.tag, r.meaning), r.fields, r.named_from);
    }
    rows("a car of `F`".into(), CAR, None);
    out
}

/// The JSON Schema `schemas/run.schema.json`: one line of a run file read as the table of its named fields, with
/// `record` its tag.
pub fn run_schema() -> Json {
    let head = super::schema_head(
        "run.schema.json",
        "A run file's record (ob.run)",
        "runs.md: a run file is text, one record per line; each line read as an object of its named fields in order \
         (record = the line's first word) is one of these. A reader skips records it does not know and the words \
         after the fields it knows.",
    );
    let one = |r: &Record| {
        let Json::Object(mut body) = fields::table_schema(r.fields) else { unreachable!("a table's schema") };
        let props = body.iter_mut().find(|(k, _)| k == "properties").expect("properties");
        if let Json::Object(p) = &mut props.1 {
            p.insert(0, ("record".into(), Json::obj([("const", Json::str(r.tag))])));
        }
        if let Some((_, Json::Array(req))) = body.iter_mut().find(|(k, _)| k == "required") {
            req.insert(0, Json::str("record"));
        } else {
            body.push(("required".into(), Json::strs(["record"])));
        }
        body.insert(0, ("description".into(), Json::str(r.meaning)));
        Json::Object(body)
    };
    let mut defs: Vec<(String, Json)> = RUN.iter().map(|r| (format!("record{}", r.tag), one(r))).collect();
    defs.extend(
        fields::local_defs(&RUN.iter().flat_map(|r| r.fields.iter().copied()).collect::<Vec<_>>())
            .into_iter()
            .map(|(n, j)| (n.to_string(), j)),
    );
    let any =
        Json::Array(RUN.iter().map(|r| Json::obj([("$ref", Json::str(format!("#/$defs/record{}", r.tag)))])).collect());
    super::object(head, Json::obj([("oneOf", any)]), vec![("$defs", Json::Object(defs))])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> String {
        let path = format!("{}/../../docs/modding/asset-format/examples/runs/race.obrun", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn the_example_passes() {
        let s = check_run(&example()).unwrap();
        assert_eq!(s.schema, SCHEMA);
        assert!(s.ended);
        assert!(s.records.iter().all(|r| r.1 > 0), "the example shows every record: {:?}", s.records);
    }

    #[test]
    fn lines_read_into_their_fields() {
        let (r, t) = read_line("F 12 -2.483 0 P -310.25 88.12 4.50 0 0 0.7071 0.7071 0.00 0 0 1|1 R 1 2 3 0 0 0 1 5 0 0 2 0.1 0.1 0 1 2 3 0 0 0 0 3 9 9").unwrap();
        assert_eq!(r.tag, "F");
        let cars = t["cars"].as_array().unwrap();
        assert_eq!(cars.len(), 2);
        assert_eq!(cars[0]["kind"].as_str(), Some("P"));
        assert_eq!(cars[1]["lights"].as_integer(), Some(3), "the words after the known fields are skipped");
        check_line(r, &t, Some(0)).unwrap();
        let (r, t) = read_line("OBSIM 1 schema=ob.run/1.0 mode=world seed=18446744073709551615 later=x").unwrap();
        assert_eq!((r.tag, t["seed"].as_str(), t.get("later")), ("OBSIM", Some("18446744073709551615"), None));
        let (_, t) = read_line("C 4 R Mazda RX-8 (blue)").unwrap();
        assert_eq!(t["name"].as_str(), Some("Mazda RX-8 (blue)"));
        let (_, t) = read_line("R 3 0 - - -").unwrap();
        assert_eq!((t["time"].as_str(), t["laps"].as_array().map(Vec::len)), (Some("-"), Some(0)));
        assert!(read_line("Z new record").is_none());
    }

    #[test]
    fn bad_files_are_refused_with_their_line() {
        let ok = "OBSIM 1 schema=ob.run/1.0 mode=race seed=7\nF 0 -1.000\n";
        check_run(ok).unwrap();
        let refuse = |extra: &str| check_run(&format!("{ok}{extra}")).unwrap_err();
        let r = refuse("F 1 x\n");
        assert_eq!((r.rule, r.line, r.field.as_deref()), (Rule::Conv6, Some(3), Some("time")));
        let r = refuse("F 1 0.0 3 Q 1 2 3 0 0 0 1 5 0 0 1\n");
        assert_eq!((r.rule, r.field.as_deref()), (Rule::Conv6, Some("cars[0].kind")));
        assert_eq!(refuse("F 1 0.0 3 P 1 2\n").rule, Rule::Conv5, "a car without its place");
        assert_eq!(refuse("L 1 COROLLA 0 1d:xyz\n").rule, Rule::Conv8);
        assert_eq!(refuse("H 60 12\n").rule, Rule::Conv8);
        assert_eq!(refuse("END finish\nF 2 0.0\n").rule, Rule::Conv6);
        assert_eq!(refuse("OBSIM 1\n").rule, Rule::Conv6);
        // Skipped: a newer record, a newer event word, a newer minor.
        let s = check_run(&format!("{ok}Z anything\nE 1 0.5 jumped 2 0 1.0\nEND finish\n")).unwrap();
        assert_eq!((s.skipped, s.ended), (1, true));
        check_run("OBSIM 1 schema=ob.run/1.7\n").unwrap();
        // A newer major, a header missing, a stream from before the kind.
        assert_eq!(check_run("OBSIM 2 mode=race\n").unwrap_err().rule, Rule::Ver2);
        assert_eq!(check_run("OBSIM 1 schema=ob.run/2.0\n").unwrap_err().rule, Rule::Ver2);
        assert_eq!(check_run("F 0 0.0\n").unwrap_err().rule, Rule::Conv3);
        assert_eq!(check_run("").unwrap_err().rule, Rule::Conv3);
        assert_eq!(check_run("OBSIM 1 mode=race\n").unwrap().schema, "ob.run/1.0");
    }
}

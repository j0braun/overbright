//! `pack.toml` (manifest.md): format 2 for base sets and mod packs, format 1 (today's texture packs) read by
//! the legacy rules of §6, the pack's content digest (§4), and a pack's `migrations.toml` (versioning.md §4.3).

use std::collections::BTreeMap;

use crate::digest::{self, Alg, Digest};
use crate::fields::{self, Field, Pattern, Req, Ty};
use crate::json::Json;
use crate::names;
use crate::text::{self, toml_inline, toml_key, toml_string};
use crate::{Refusal, Rule};

/// The manifest's file name at a pack's root.
pub const FILE: &str = "pack.toml";
/// The newest manifest format this reader reads.
pub const FORMAT: u32 = 2;
/// The largest `pack.toml` (`MAN.8`).
pub const MAX_BYTES: usize = 64 * 1024;

// ---- the shared definitions the field tables use (schemas/common.schema.json) ------------------------------------

fn is_semver(s: &str) -> bool {
    let (core, pre) = match s.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (s, None),
    };
    let nums: Vec<&str> = core.split('.').collect();
    nums.len() == 3
        && nums.iter().all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        && pre.is_none_or(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-'))
        && semver::Version::parse(s).is_ok()
}

/// A version requirement as the format writes it (`>=0.4`, `>=1.2,<2`, `*`): the pattern of
/// `common.schema.json#/$defs/versionReq`, and readable by Cargo's rules.
pub fn is_version_req(s: &str) -> bool {
    fn num(s: &str) -> bool {
        let parts: Vec<&str> = s.split('.').collect();
        parts.len() <= 3 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    }
    fn clause(c: &str, first: bool) -> bool {
        let ops: &[&str] = if first { &[">=", "<=", ">", "<", "=", "^", "~"] } else { &[">=", "<=", ">", "<", "="] };
        let rest = ops.iter().find_map(|op| c.strip_prefix(op)).unwrap_or(if first { c } else { "" });
        num(rest)
    }
    if s == "*" {
        return true;
    }
    let mut parts = s.split(',');
    let first = parts.next().unwrap_or("");
    clause(first, true)
        && parts.all(|p| clause(p.trim_matches(|c: char| c.is_ascii_whitespace()), false))
        && semver::VersionReq::parse(s).is_ok()
}

fn is_https(s: &str) -> bool {
    s.strip_prefix("https://")
        .is_some_and(|r| (1..=292).contains(&r.chars().count()) && !r.chars().any(char::is_whitespace))
}

fn is_origin(s: &str) -> bool {
    matches!(s, "own-work" | "from-game-files" | "imported")
        || s.strip_prefix("port:").is_some_and(|g| {
            (1..=32).contains(&g.len()) && g.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

fn is_base_pattern(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=64).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit() || b[0] == b'*')
        && b.iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-' | b'*'))
}

fn is_edition(s: &str) -> bool {
    (1..=32).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn is_language(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=32).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b.iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}

fn is_profile(s: &str) -> bool {
    s.split_once('/').is_some_and(|(n, v)| {
        !n.is_empty()
            && n.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !v.is_empty()
            && v.bytes().all(|b| b.is_ascii_digit())
    })
}

fn is_race_id(s: &str) -> bool {
    (1..=40).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn is_bundle_name(s: &str) -> bool {
    s.strip_suffix(".obz").is_some_and(|stem| {
        let b = stem.as_bytes();
        (1..=96).contains(&b.len())
            && b[0] != b'.'
            && b.iter().all(|&c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
    })
}

fn is_logical_path(s: &str) -> bool {
    names::logical_path(s, false).is_ok()
}

fn is_digest(s: &str) -> bool {
    Digest::parse(s).is_ok()
}

const fn pat(json: &'static str, check: fn(&str) -> bool, rule: Rule) -> Option<Pattern> {
    Some(Pattern { json, check, rule })
}

const fn string(max: usize, pattern: Option<Pattern>) -> Ty {
    Ty::Str { min: 1, max, pattern }
}

pub const PACK_ID: Ty =
    Ty::Def("packId", &string(64, pat("^[a-z0-9][a-z0-9_-]{0,63}$", names::is_pack_id, Rule::Conv8)));
pub const SEMVER: Ty =
    Ty::Def("semver", &string(usize::MAX, pat("^[0-9]+\\.[0-9]+\\.[0-9]+(-[0-9A-Za-z.-]+)?$", is_semver, Rule::Conv6)));
pub const VERSION_REQ: Ty = Ty::Def("versionReq", &string(usize::MAX, pat("versionReq", is_version_req, Rule::Man7)));
pub const LOGICAL_PATH: Ty = Ty::Def("logicalPath", &string(240, pat("logicalPath", is_logical_path, Rule::Conv13)));
pub const DIGEST: Ty = Ty::Def("digest", &string(usize::MAX, pat("digest", is_digest, Rule::Conv8)));
pub const NAME: Ty = Ty::Def("name", &string(63, pat("name", names::is_name, Rule::Conv8)));
pub const I32: Ty = Ty::Def("i32", &Ty::Int { min: i32::MIN as i64, max: i32::MAX as i64 });
const EXTENSION: Ty =
    Ty::Str { min: 1, max: usize::MAX, pattern: pat("^[A-Z0-9]+_[a-z0-9_]+$", names::is_extension_name, Rule::Conv8) };
const EXTENSIONS: Ty = Ty::Array { item: &EXTENSION, min: 0, max: None, unique: true };
const PACK_REFS: Ty = Ty::Map { key: &PACK_ID, value: &VERSION_REQ };

/// The kind families a pack may list in `contents`.
pub const CONTENTS: &[&str] = &[
    "textures",
    "meshes",
    "anim",
    "world",
    "collision",
    "routes",
    "races",
    "cars",
    "career",
    "ui",
    "text",
    "audio",
    "movies",
    "tables",
];

const GAME_FIELDS: &[Field] = &[
    Field {
        key: "bases",
        ty: Ty::Array {
            item: &string(64, pat("^[a-z0-9*][a-z0-9_*-]{0,63}$", is_base_pattern, Rule::Conv8)),
            min: 0,
            max: None,
            unique: false,
        },
        doc_name: "`game.bases`",
        doc_type: "array<string>",
        doc_default: "`[\"*\"]`",
        doc_meaning: "Base set ids it supports, `*` wildcards allowed (`nfsu2-pc12-*`). The game refuses a pack whose list has no match for the base set in use, or that names a base set when no imported copy is in use, and says which it was made for; a pack that is on but refused is not mounted at all. Any reference or guard that does not match refuses it too (conventions §11: the whole pack, with the stale items listed).",
        ..Field::BLANK
    },
    Field {
        key: "fingerprint",
        ty: Ty::Array { item: &Ty::Str { min: 0, max: usize::MAX, pattern: None }, min: 0, max: None, unique: false },
        doc_name: "`game.fingerprint`",
        doc_type: "array<sha256>",
        doc_default: "–",
        doc_meaning: "Digests that identify the copy of the game the pack was made for (`sha256:<hex>`, the import record's `source.identity`; Overbright's exporter writes it beside `bases`). A non-empty list the base set in use is not on is refused like a base set mismatch. Format-1 files may list digests of key original files, which are read and shown.",
        ..Field::BLANK
    },
];

const TESTED_FIELDS: &[Field] = &[
    Field {
        key: "race",
        ty: string(40, pat("^[a-z0-9_-]{1,40}$", is_race_id, Rule::Conv8)),
        req: Req::Yes,
        ..Field::BLANK
    },
    Field { key: "race_digest", ty: DIGEST, req: Req::Yes, ..Field::BLANK },
    Field { key: "car", ty: NAME, ..Field::BLANK },
    Field { key: "time_s", ty: Ty::F32, ..Field::BLANK },
    Field { key: "overbright", ty: SEMVER, req: Req::Yes, ..Field::BLANK },
];

/// The `[base]` table's fields (manifest.md §3, second table).
pub const BASE_FIELDS: &[Field] = &[
    Field {
        key: "edition",
        ty: string(32, pat("^[a-z0-9-]{1,32}$", is_edition, Rule::Conv8)),
        req: Req::Yes,
        doc_name: "`edition`",
        doc_type: "string `[a-z0-9-]{1,32}`",
        doc_default: "–",
        doc_meaning: "The importer's edition key ([import.md](import.md) §3), `custom` for an unknown or modified install, or the community set's own name.",
        ..Field::BLANK
    },
    Field {
        key: "region",
        ty: Ty::Enum(&["na", "eu"]),
        req: Req::Yes,
        doc_name: "`region`",
        doc_type: "enum `na`, `eu`",
        doc_default: "–",
        doc_meaning: "The original's region behaviour the set follows (crates/ob-core/src/region.rs: NA = region 0, every European release = 6).",
        ..Field::BLANK
    },
    Field {
        key: "languages",
        ty: Ty::Array {
            item: &string(32, pat("^[a-z][a-z0-9_-]{0,31}$", is_language, Rule::Conv8)),
            min: 1,
            max: None,
            unique: true,
        },
        req: Req::Yes,
        doc_name: "`languages`",
        doc_type: "array<string>",
        doc_default: "–",
        doc_meaning: "The language keys a player can choose: the `text/<language>/` units with role `player` that hold a string table ([text.md](text.md) §2).",
        ..Field::BLANK
    },
    Field {
        key: "profile",
        ty: string(usize::MAX, pat("^[a-z0-9-]+/[0-9]+$", is_profile, Rule::Conv8)),
        req: Req::Yes,
        doc_name: "`profile`",
        doc_type: "string `<name>/<major>`",
        doc_default: "–",
        doc_meaning: "The **required-items profile** the set satisfies (§5).",
        ..Field::BLANK
    },
    Field {
        key: "import",
        ty: LOGICAL_PATH,
        req: Req::When("for imported sets"),
        doc_name: "`import`",
        doc_type: "logicalPath",
        doc_default: "–",
        doc_meaning: "The import record, normally `import.toml`.",
        ..Field::BLANK
    },
    Field {
        key: "marker",
        ty: Ty::Str { min: 0, max: 80, pattern: None },
        doc_name: "`marker`",
        doc_type: "string ≤ 80",
        doc_default: "–",
        doc_meaning: "A short line the launcher shows for community sets (\"Community city set 0.3\").",
        ..Field::BLANK
    },
    Field {
        key: "storage",
        ty: Ty::Enum(&["owned", "linked"]),
        doc_name: "`storage`",
        doc_type: "enum `owned`, `linked`",
        doc_default: "`owned`",
        doc_meaning: "Whether stage-0 files are copies (`owned`) or references to the game folder (`linked`, developer builds only; [import.md](import.md) §4.2).",
        ..Field::BLANK
    },
];

/// The manifest's fields (manifest.md §3, first table).
pub const FIELDS: &[Field] = &[
    Field {
        key: "format",
        ty: Ty::Const(2),
        req: Req::YesAs("yes in format 2"),
        doc_name: "`format`",
        doc_type: "u32",
        doc_default: "–",
        doc_meaning: "2. A reader refuses a higher format (`MAN.1`). A file without `format` is read as a format-1 manifest by the legacy rules of §6, not as a format-2 file missing a field.",
        ..Field::BLANK
    },
    Field {
        key: "kind",
        ty: Ty::Enum(&["base", "mod"]),
        req: Req::Yes,
        doc_name: "`kind`",
        doc_type: "enum `base`, `mod`",
        doc_default: "–",
        doc_meaning: "What the pack is ([layering.md](layering.md) §1).",
        ..Field::BLANK
    },
    Field {
        key: "id",
        ty: PACK_ID,
        req: Req::Yes,
        doc_name: "`id`",
        doc_type: "packId",
        doc_default: "–",
        doc_meaning: "Unique id (conventions §6.1). For a folder pack it SHOULD equal the folder name; for `<id>-<version>.obpk` it MUST equal the file name's id part (`MAN.2`).",
        ..Field::BLANK
    },
    Field {
        key: "name",
        ty: string(80, None),
        req: Req::Yes,
        doc_name: "`name`",
        doc_type: "string ≤ 80",
        doc_default: "–",
        doc_meaning: "Display name.",
        ..Field::BLANK
    },
    Field {
        key: "version",
        ty: SEMVER,
        req: Req::Yes,
        doc_name: "`version`",
        doc_type: "semver",
        doc_default: "–",
        doc_meaning: "The pack's own version (https://semver.org/spec/v2.0.0.html).",
        ..Field::BLANK
    },
    Field {
        key: "authors",
        ty: Ty::Array { item: &Ty::Str { min: 0, max: 80, pattern: None }, min: 0, max: Some(32), unique: false },
        doc_name: "`authors`",
        doc_type: "array<string ≤ 80>, ≤ 32",
        doc_default: "`[]`",
        doc_meaning: "Format 1's `author` is read as one entry.",
        ..Field::BLANK
    },
    Field {
        key: "licence",
        ty: string(200, None),
        req: Req::When("yes for `share = \"public\"`"),
        doc_name: "`licence`",
        doc_type: "string ≤ 200",
        doc_default: "–",
        doc_meaning: "An SPDX licence expression (https://spdx.org/licenses/); `LicenseRef-…` for others. `license` is accepted as a spelling.",
        ..Field::BLANK
    },
    Field { key: "license", ty: string(200, None), ..Field::BLANK },
    Field {
        key: "overbright",
        ty: VERSION_REQ,
        req: Req::Yes,
        doc_name: "`overbright`",
        doc_type: "versionReq",
        doc_default: "–",
        doc_meaning: "The Overbright versions that can load it; a reader outside the range refuses with \"made for Overbright <range>\" (`MAN.3`).",
        ..Field::BLANK
    },
    Field {
        key: "description",
        ty: Ty::Str { min: 0, max: 2000, pattern: None },
        doc_name: "`description`",
        doc_type: "string ≤ 2000",
        doc_default: "`\"\"`",
        doc_meaning: "Plain text, shown as text, never as markup.",
        ..Field::BLANK
    },
    Field {
        key: "homepage",
        ty: Ty::Str { min: 0, max: usize::MAX, pattern: pat("^https://[^\\s]{1,292}$", is_https, Rule::Conv6) },
        doc_name: "`homepage`",
        doc_type: "string (https URL) ≤ 300",
        doc_default: "–",
        doc_meaning: "A link shown in the launcher; never fetched by Overbright.",
        ..Field::BLANK
    },
    Field {
        key: "origin",
        ty: Ty::Str {
            min: 0,
            max: usize::MAX,
            pattern: pat("^(own-work|from-game-files|imported|port:[a-z0-9-]{1,32})$", is_origin, Rule::Conv6),
        },
        req: Req::Yes,
        doc_name: "`origin`",
        doc_type: "enum",
        doc_default: "–",
        doc_meaning: "`own-work`; `from-game-files` (computed from the player's game data: upscaled textures, copied records); `port:<game>` (from another game the player owns); `imported` (a base set made by the importer).",
        ..Field::BLANK
    },
    Field {
        key: "share",
        ty: Ty::Enum(&["public", "local-only"]),
        req: Req::Yes,
        doc_name: "`share`",
        doc_type: "enum `public`, `local-only`",
        doc_default: "–",
        doc_meaning: "`local-only` is forced for `from-game-files`, `port:*` and `imported` (`MAN.4`); Studio's exporter refuses to mark them public.",
        ..Field::BLANK
    },
    Field {
        key: "contents",
        ty: Ty::Array { item: &Ty::Enum(CONTENTS), min: 0, max: None, unique: true },
        doc_name: "`contents`",
        doc_type: "array<enum>",
        doc_default: "computed",
        doc_meaning: "The kind families the pack holds (`textures`, `meshes`, `world`, `collision`, `routes`, `races`, `cars`, `career`, `ui`, `text`, `audio`, `movies`, `tables`, `anim`); shown as chips in the launcher. Writers compute it; a reader warns when it differs from the files.",
        ..Field::BLANK
    },
    Field {
        key: "affects_play",
        ty: Ty::Bool,
        doc_name: "`affects_play`",
        doc_type: "bool",
        doc_default: "computed",
        doc_meaning: "`true` when any file can change simulation, AI or career (world placements with collision, routes, races, cars, career, tables). Saves and replays record such packs ([layering.md](layering.md); the save sidecar, [layering.md](layering.md) §8). Writers compute it; a reader recomputes and uses the stricter value.",
        ..Field::BLANK
    },
    Field {
        key: "priority",
        ty: I32,
        doc_name: "`priority`",
        doc_type: "i32",
        doc_default: "0",
        doc_meaning: "Format 1 only: where a new pack is first placed among unlisted packs. Ignored once the pack is in `order.txt`.",
        ..Field::BLANK
    },
    Field {
        key: "extensions_used",
        ty: EXTENSIONS,
        doc_name: "`extensions_used`",
        doc_type: "array<string>",
        doc_default: "`[]`",
        doc_meaning: "Extensions the pack's files use (conventions §9).",
        ..Field::BLANK
    },
    Field {
        key: "extensions_required",
        ty: EXTENSIONS,
        doc_name: "`extensions_required`",
        doc_type: "array<string>",
        doc_default: "`[]`",
        doc_meaning: "Subset of `extensions_used` a reader MUST know (`CONV.14`).",
        ..Field::BLANK
    },
    Field {
        key: "extensions_safe_to_copy",
        ty: EXTENSIONS,
        doc_name: "`extensions_safe_to_copy`",
        doc_type: "array<string>",
        doc_default: "`[]`",
        doc_meaning: "Subset of `extensions_used` whose data a tool that does not know them may keep when it rewrites an item (conventions §9); an extension not listed is not safe to copy.",
        ..Field::BLANK
    },
    Field {
        key: "game",
        ty: Ty::Table(GAME_FIELDS),
        doc_name: "`[game]`",
        doc_type: "table",
        doc_default: "–",
        doc_meaning: "What the pack was made against.",
        rows_inline: true,
        ..Field::BLANK
    },
    Field {
        key: "dependencies",
        ty: PACK_REFS,
        doc_name: "`[dependencies]`",
        doc_type: "table packId → versionReq",
        doc_default: "–",
        doc_meaning: "Required packs, enabled and below this one ([layering.md](layering.md) §2, `ORD.2`).",
        ..Field::BLANK
    },
    Field {
        key: "optional",
        ty: PACK_REFS,
        doc_name: "`[optional]`",
        doc_type: "table packId → versionReq",
        doc_default: "–",
        doc_meaning: "If present and enabled, kept below this one; never required.",
        ..Field::BLANK
    },
    Field {
        key: "conflicts",
        ty: PACK_REFS,
        doc_name: "`[conflicts]`",
        doc_type: "table packId → versionReq",
        doc_default: "–",
        doc_meaning: "Never enabled together (`ORD.3`).",
        ..Field::BLANK
    },
    Field {
        key: "tested",
        ty: Ty::Array { item: &Ty::Table(TESTED_FIELDS), min: 0, max: Some(256), unique: false },
        doc_name: "`[[tested]]`",
        doc_type: "array of tables",
        doc_default: "–",
        doc_meaning: "One entry per tested race, written by Studio after a finished test run: `race` (the race id), `race_digest` (the race digest of [roads-ai.md](roads-ai.md) §3.7, followed by the content digests of the pack's play-affecting files other than `races/` files, in path order, all hashed together, so editing one race never clears another's mark), `car` (name), `time_s` (f32), `overbright` (semver). An entry whose `race_digest` no longer matches is stale and is removed on the next save; other races keep their marks.",
        ..Field::BLANK
    },
    Field {
        key: "base",
        ty: Ty::Table(BASE_FIELDS),
        req: Req::When("iff `kind = \"base\"`"),
        doc_name: "`[base]`",
        doc_type: "table",
        doc_default: "–",
        doc_meaning: "Base-set fields (below). Forbidden in mod packs (`MAN.5`).",
        ..Field::BLANK
    },
    Field {
        key: "bundles",
        ty: Ty::Map {
            key: &string(100, pat("^[A-Za-z0-9_-][A-Za-z0-9._-]{0,99}\\.obz$", is_bundle_name, Rule::Conv13)),
            value: &DIGEST,
        },
        doc_name: "`[bundles]`",
        doc_type: "table file name → digest",
        doc_default: "–",
        doc_meaning: "Bundles of a pack stored as several `.obz` files ([layout.md](layout.md) §4–5) and their `xxh3-128` digests. Imported base sets list them in `import.toml` instead.",
        ..Field::BLANK
    },
    Field {
        key: "extras",
        ty: Ty::Extras,
        doc_name: "`[extras]`",
        doc_type: "table",
        doc_default: "–",
        doc_meaning: "Tools' data (conventions §9).",
        ..Field::BLANK
    },
];

/// The JSON Schema `schemas/pack.schema.json`, from [`FIELDS`] plus the manifest's conditional rules.
pub fn schema() -> Json {
    let body = fields::table_schema(FIELDS);
    let Json::Object(body) = body else { unreachable!() };
    let mut j = Json::obj([
        ("$schema", Json::str("https://json-schema.org/draft/2020-12/schema")),
        ("$id", Json::str("pack.schema.json")),
        ("title", Json::str("pack.toml, format 2")),
        (
            "description",
            Json::str(
                "manifest.md. Shape only; cross-file rules (MAN.*, ORD.*) are on the pages. Format-1 manifests are read by \
                 the legacy rules of manifest.md section 6 and are not validated by this schema. Generated from the code \
                 (crates/ob-asset-format/src/manifest.rs); do not edit.",
            ),
        ),
    ]);
    if let Json::Object(o) = &mut j {
        o.extend(body);
    }
    let kind_base = Json::obj([("properties", Json::obj([("kind", Json::obj([("const", Json::str("base"))]))]))]);
    let share_public = Json::obj([("properties", Json::obj([("share", Json::obj([("const", Json::str("public"))]))]))]);
    let req = |k: &str| Json::obj([("required", Json::strs([k]))]);
    j.push(
        "allOf",
        Json::Array(vec![
            Json::obj([("if", kind_base), ("then", req("base")), ("else", Json::obj([("not", req("base"))]))]),
            Json::obj([
                ("if", share_public),
                (
                    "then",
                    Json::obj([
                        ("anyOf", Json::Array(vec![req("licence"), req("license")])),
                        ("properties", Json::obj([("origin", Json::obj([("const", Json::str("own-work"))]))])),
                    ]),
                ),
            ]),
        ]),
    );
    j
}

// ---- the model --------------------------------------------------------------------------------------------------

/// What a pack is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackKind {
    Base,
    Mod,
}

/// Where a pack's content comes from (`origin`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    OwnWork,
    FromGameFiles,
    Imported,
    Port(String),
}

impl Origin {
    fn parse(s: &str) -> Origin {
        match s {
            "own-work" => Origin::OwnWork,
            "from-game-files" => Origin::FromGameFiles,
            "imported" => Origin::Imported,
            other => Origin::Port(other.trim_start_matches("port:").to_string()),
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            Origin::OwnWork => "own-work".into(),
            Origin::FromGameFiles => "from-game-files".into(),
            Origin::Imported => "imported".into(),
            Origin::Port(g) => format!("port:{g}"),
        }
    }

    /// Whether this origin forces `share = "local-only"` (`MAN.4`).
    pub fn forces_local(&self) -> bool {
        !matches!(self, Origin::OwnWork)
    }
}

/// Whether a pack may be shared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Share {
    Public,
    LocalOnly,
}

/// How a format-1 pack's images are kept for the GPU (`compress`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Compress {
    #[default]
    Bc7,
    None,
}

/// `[base]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
    pub edition: String,
    /// `"na"` or `"eu"`.
    pub region: String,
    pub languages: Vec<String>,
    pub profile: String,
    pub import: Option<String>,
    pub marker: Option<String>,
    /// `linked` (developer builds only) instead of `owned`.
    pub linked: bool,
}

/// One `[[tested]]` entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Tested {
    pub race: String,
    pub race_digest: Digest,
    pub car: Option<String>,
    pub time_s: Option<f32>,
    pub overbright: String,
}

/// What a format-1 manifest held beyond format 2's fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Legacy {
    /// `version` as written (any text, not semver).
    pub version: String,
    pub compress: Compress,
    pub source: String,
}

/// A pack's manifest.
#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    /// 1 for a legacy texture pack, 2 otherwise.
    pub format: u32,
    pub kind: PackKind,
    pub id: String,
    pub name: String,
    /// The pack's version (`0.0.0` for a legacy pack, whose version text is in [`Legacy::version`]).
    pub version: semver::Version,
    pub authors: Vec<String>,
    pub licence: Option<String>,
    pub overbright: semver::VersionReq,
    /// The `overbright` requirement as written.
    pub overbright_text: String,
    pub description: String,
    pub homepage: Option<String>,
    pub origin: Origin,
    pub share: Share,
    pub contents: Option<Vec<String>>,
    pub affects_play: Option<bool>,
    pub priority: i32,
    pub extensions_used: Vec<String>,
    pub extensions_required: Vec<String>,
    pub extensions_safe_to_copy: Vec<String>,
    pub game_bases: Option<Vec<String>>,
    pub game_fingerprint: Option<Vec<String>>,
    pub dependencies: BTreeMap<String, String>,
    pub optional: BTreeMap<String, String>,
    pub conflicts: BTreeMap<String, String>,
    pub tested: Vec<Tested>,
    pub base: Option<Base>,
    pub bundles: BTreeMap<String, Digest>,
    pub extras: Option<toml::Table>,
    /// Set for format-1 manifests.
    pub legacy: Option<Legacy>,
}

/// Where a manifest was found, for the checks that compare it with its container's name.
#[derive(Clone, Copy, Debug)]
pub enum Container<'a> {
    /// A pack folder with this name.
    Folder(&'a str),
    /// A pack file with this file name (`<id>-<version>.obpk`).
    PackFile(&'a str),
    /// Not known (a test, a stream).
    Unknown,
}

fn strs(v: Option<&toml::Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

fn s(t: &toml::Table, k: &str) -> Option<String> {
    t.get(k).and_then(|v| v.as_str()).map(str::to_string)
}

fn map_of(t: &toml::Table, k: &str) -> BTreeMap<String, String> {
    t.get(k)
        .and_then(|v| v.as_table())
        .map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect())
        .unwrap_or_default()
}

impl Manifest {
    /// Reads `pack.toml`'s bytes. A file without `format`, or with `format = 1`, is a legacy texture pack
    /// (§6; its id is the container's name). Returns the manifest and the warnings a reader shows without
    /// refusing (a folder named unlike the id).
    pub fn parse(bytes: &[u8], container: Container) -> Result<(Manifest, Vec<String>), Refusal> {
        let at = |r: Refusal| r.at_path(FILE);
        let text = text::decode(bytes, MAX_BYTES, Rule::Man8).map_err(at)?;
        let doc = text::parse_toml(text).map_err(at)?;
        let format = match doc.get("format") {
            None => 1,
            Some(toml::Value::Integer(f)) if (1..=i64::from(FORMAT)).contains(f) => *f as u32,
            Some(toml::Value::Integer(f)) if *f > i64::from(FORMAT) => {
                return Err(at(Refusal::new(
                    Rule::Man1,
                    format!("format {f} is newer than this Overbright reads ({FORMAT})"),
                )
                .in_field("format")));
            }
            Some(_) => return Err(at(Refusal::new(Rule::Man1, "`format` is not a format number").in_field("format"))),
        };
        let m = if format == 1 { Manifest::legacy(&doc, container) } else { Manifest::format2(&doc, container) };
        m.map_err(at)
    }

    fn legacy(doc: &toml::Table, container: Container) -> Result<(Manifest, Vec<String>), Refusal> {
        let id = match container {
            Container::Folder(name) => name.to_string(),
            Container::PackFile(file) => file.trim_end_matches(".obpk").to_string(),
            Container::Unknown => String::new(),
        };
        // Format 1's fields keep their meaning (src/obpack/manifest.rs); other keys stay ignored.
        let clean = |v: Option<String>, max: usize| {
            v.unwrap_or_default().chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
        };
        let name = clean(s(doc, "name"), 80);
        if name.is_empty() {
            return Err(Refusal::new(Rule::Conv5, "missing required field").in_field("name"));
        }
        let priority = match doc.get("priority") {
            None => 0,
            Some(toml::Value::Integer(p)) if p.abs() <= 1_000_000 => *p as i32,
            Some(_) => return Err(Refusal::new(Rule::Conv6, "outside -1000000..1000000").in_field("priority")),
        };
        let compress = match s(doc, "compress").map(|c| c.to_ascii_lowercase()).as_deref() {
            None | Some("bc7") | Some("auto") => Compress::Bc7,
            Some("none") => Compress::None,
            Some(other) => {
                return Err(Refusal::new(Rule::Conv6, format!("{other:?} (bc7 or none)")).in_field("compress"));
            }
        };
        let mut authors = Vec::new();
        match (s(doc, "author"), doc.get("authors")) {
            (Some(a), _) => authors.push(clean(Some(a), 200)),
            (None, Some(_)) => authors = strs(doc.get("authors")).into_iter().map(|a| clean(Some(a), 80)).collect(),
            (None, None) => {}
        }
        let source = clean(s(doc, "source"), 40);
        let origin = if matches!(source.to_ascii_lowercase().as_str(), "texmod" | "texwizard" | "upscale") {
            Origin::FromGameFiles
        } else {
            Origin::OwnWork
        };
        let licence = Some(clean(s(doc, "licence").or_else(|| s(doc, "license")), 200)).filter(|l| !l.is_empty());
        // `share` follows `origin`: what was made from game files stays local; a legacy pack of one's own work is
        // public only with a licence (MAN.4's rule).
        let share = if origin.forces_local() || licence.is_none() { Share::LocalOnly } else { Share::Public };
        let manifest = Manifest {
            format: 1,
            kind: PackKind::Mod,
            id,
            name,
            version: semver::Version::new(0, 0, 0),
            authors,
            licence,
            overbright: semver::VersionReq::STAR,
            overbright_text: "*".into(),
            description: clean(s(doc, "description"), 1000),
            homepage: None,
            origin,
            share,
            contents: Some(vec!["textures".into()]),
            affects_play: Some(false),
            priority,
            extensions_used: Vec::new(),
            extensions_required: Vec::new(),
            extensions_safe_to_copy: Vec::new(),
            game_bases: None,
            game_fingerprint: None,
            dependencies: BTreeMap::new(),
            optional: BTreeMap::new(),
            conflicts: BTreeMap::new(),
            tested: Vec::new(),
            base: None,
            bundles: BTreeMap::new(),
            extras: None,
            legacy: Some(Legacy { version: clean(s(doc, "version"), 40), compress, source }),
        };
        Ok((manifest, Vec::new()))
    }

    fn format2(doc: &toml::Table, container: Container) -> Result<(Manifest, Vec<String>), Refusal> {
        // Rules a page names come before the generic shape rules (conventions.md §12).
        if let Some(id) = doc.get("id").and_then(|v| v.as_str()) {
            names::pack_id(id, Rule::Man2).map_err(|r| r.in_field("id"))?;
        }
        if doc.contains_key("licence") && doc.contains_key("license") {
            return Err(Refusal::new(Rule::Conv4, "both spellings `licence` and `license`").in_field("license"));
        }
        fields::check_table(doc, FIELDS, None)?;
        let id = s(doc, "id").expect("checked");
        let version = semver::Version::parse(&s(doc, "version").expect("checked")).expect("checked");
        let kind = if s(doc, "kind").as_deref() == Some("base") { PackKind::Base } else { PackKind::Mod };
        let origin = Origin::parse(&s(doc, "origin").expect("checked"));
        let share = if s(doc, "share").as_deref() == Some("public") { Share::Public } else { Share::LocalOnly };
        let licence = s(doc, "licence").or_else(|| s(doc, "license"));
        let mut warnings = Vec::new();
        match container {
            Container::PackFile(file) if file != format!("{id}-{version}.obpk") => {
                return Err(Refusal::new(Rule::Man2, format!("the file name {file:?} is not \"{id}-{version}.obpk\""))
                    .in_field("id"));
            }
            Container::Folder(name) if name != id => {
                warnings.push(format!("pack folder {name:?} holds the pack {id:?}"))
            }
            _ => {}
        }
        if share == Share::Public && origin.forces_local() {
            return Err(Refusal::new(Rule::Man4, format!("origin {} keeps a pack local-only", origin.as_str()))
                .in_field("share"));
        }
        if share == Share::Public && licence.is_none() {
            return Err(Refusal::new(Rule::Man4, "a public pack needs a licence").in_field("licence"));
        }
        match (kind, doc.contains_key("base")) {
            (PackKind::Mod, true) => return Err(Refusal::new(Rule::Man5, "`[base]` in a mod pack").in_field("base")),
            (PackKind::Base, false) => {
                return Err(Refusal::new(Rule::Man5, "a base set without `[base]`").in_field("base"));
            }
            _ => {}
        }
        let (dependencies, optional, conflicts) =
            (map_of(doc, "dependencies"), map_of(doc, "optional"), map_of(doc, "conflicts"));
        for (table, m) in [("dependencies", &dependencies), ("optional", &optional), ("conflicts", &conflicts)] {
            if m.contains_key(&id) {
                return Err(Refusal::new(Rule::Man7, "names the pack itself").in_field(&format!("{table}.{id}")));
            }
        }
        let used = strs(doc.get("extensions_used"));
        let required = strs(doc.get("extensions_required"));
        let safe = strs(doc.get("extensions_safe_to_copy"));
        if let Some(x) = required.iter().find(|x| !used.contains(x)) {
            return Err(Refusal::new(Rule::Man9, format!("{x} is required but not in `extensions_used`"))
                .in_field("extensions_required"));
        }
        if let Some(x) = safe.iter().find(|x| !used.contains(x)) {
            return Err(Refusal::new(Rule::Conv6, format!("{x} is not in `extensions_used`"))
                .in_field("extensions_safe_to_copy"));
        }
        let overbright_text = s(doc, "overbright").expect("checked");
        let base = doc.get("base").and_then(|b| b.as_table()).map(|b| Base {
            edition: s(b, "edition").expect("checked"),
            region: s(b, "region").expect("checked"),
            languages: strs(b.get("languages")),
            profile: s(b, "profile").expect("checked"),
            import: s(b, "import"),
            marker: s(b, "marker"),
            linked: s(b, "storage").as_deref() == Some("linked"),
        });
        let tested = doc
            .get("tested")
            .and_then(|t| t.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.as_table())
                    .map(|e| Tested {
                        race: s(e, "race").expect("checked"),
                        race_digest: Digest::parse(&s(e, "race_digest").expect("checked")).expect("checked"),
                        car: s(e, "car"),
                        time_s: e
                            .get("time_s")
                            .map(|v| crate::float::read_f32(v, Default::default()).expect("checked")),
                        overbright: s(e, "overbright").expect("checked"),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let game = doc.get("game").and_then(|g| g.as_table());
        let manifest = Manifest {
            format: 2,
            kind,
            id,
            name: s(doc, "name").expect("checked"),
            version,
            authors: strs(doc.get("authors")),
            licence,
            overbright: semver::VersionReq::parse(&overbright_text).expect("checked"),
            overbright_text,
            description: s(doc, "description").unwrap_or_default(),
            homepage: s(doc, "homepage"),
            origin,
            share,
            contents: doc.get("contents").map(|v| strs(Some(v))),
            affects_play: doc.get("affects_play").and_then(|v| v.as_bool()),
            priority: doc.get("priority").and_then(|v| v.as_integer()).unwrap_or(0) as i32,
            extensions_used: used,
            extensions_required: required,
            extensions_safe_to_copy: safe,
            game_bases: game.and_then(|g| g.get("bases")).map(|v| strs(Some(v))),
            game_fingerprint: game.and_then(|g| g.get("fingerprint")).map(|v| strs(Some(v))),
            dependencies,
            optional,
            conflicts,
            tested,
            base,
            bundles: doc
                .get("bundles")
                .and_then(|b| b.as_table())
                .map(|b| {
                    b.iter()
                        .map(|(k, v)| (k.clone(), Digest::parse(v.as_str().expect("checked")).expect("checked")))
                        .collect()
                })
                .unwrap_or_default(),
            extras: doc.get("extras").and_then(|e| e.as_table()).cloned(),
            legacy: None,
        };
        Ok((manifest, warnings))
    }

    /// Whether this Overbright may load the pack (`MAN.3`).
    pub fn check_engine(&self, running: &semver::Version) -> Result<(), Refusal> {
        if self.overbright.matches(running) {
            Ok(())
        } else {
            Err(Refusal::new(Rule::Man3, format!("made for Overbright {}", self.overbright_text))
                .in_pack(&self.id)
                .in_field("overbright"))
        }
    }

    /// Whether every required extension is one the reader knows (`CONV.14`).
    pub fn check_extensions(&self, known: &[&str]) -> Result<(), Refusal> {
        match self.extensions_required.iter().find(|x| !known.contains(&x.as_str())) {
            Some(x) => Err(Refusal::new(Rule::Conv14, format!("requires the unknown extension {x}")).in_pack(&self.id)),
            None => Ok(()),
        }
    }

    /// The manifest as `pack.toml` text in the canonical layout (format 2): top-level fields in the order of
    /// [`FIELDS`], then the tables. The manifest examples of the specification read and write back byte for byte.
    pub fn to_toml(&self) -> String {
        assert_eq!(self.format, 2, "only format 2 is written");
        let mut out = String::new();
        let mut line = |k: &str, v: String| out.push_str(&format!("{k} = {v}\n"));
        let list = |v: &[String]| format!("[{}]", v.iter().map(|s| toml_string(s)).collect::<Vec<_>>().join(", "));
        line("format", "2".into());
        line("kind", toml_string(if self.kind == PackKind::Base { "base" } else { "mod" }));
        line("id", toml_string(&self.id));
        line("name", toml_string(&self.name));
        line("version", toml_string(&self.version.to_string()));
        if !self.authors.is_empty() {
            line("authors", list(&self.authors));
        }
        if let Some(l) = &self.licence {
            line("licence", toml_string(l));
        }
        line("overbright", toml_string(&self.overbright_text));
        if !self.description.is_empty() {
            line("description", toml_string(&self.description));
        }
        if let Some(h) = &self.homepage {
            line("homepage", toml_string(h));
        }
        line("origin", toml_string(&self.origin.as_str()));
        line("share", toml_string(if self.share == Share::Public { "public" } else { "local-only" }));
        if let Some(c) = &self.contents {
            line("contents", list(c));
        }
        if let Some(a) = self.affects_play {
            line("affects_play", a.to_string());
        }
        if self.priority != 0 {
            line("priority", self.priority.to_string());
        }
        for (k, v) in [
            ("extensions_used", &self.extensions_used),
            ("extensions_required", &self.extensions_required),
            ("extensions_safe_to_copy", &self.extensions_safe_to_copy),
        ] {
            if !v.is_empty() {
                line(k, list(v));
            }
        }
        let section = |out: &mut String, header: &str, rows: Vec<(String, String)>| {
            out.push_str(&format!("\n{header}\n"));
            for (k, v) in rows {
                out.push_str(&format!("{k} = {v}\n"));
            }
        };
        if self.game_bases.is_some() || self.game_fingerprint.is_some() {
            let mut rows = Vec::new();
            if let Some(b) = &self.game_bases {
                rows.push(("bases".into(), list(b)));
            }
            if let Some(f) = &self.game_fingerprint {
                rows.push(("fingerprint".into(), list(f)));
            }
            section(&mut out, "[game]", rows);
        }
        for (name, m) in
            [("dependencies", &self.dependencies), ("optional", &self.optional), ("conflicts", &self.conflicts)]
        {
            if !m.is_empty() {
                section(&mut out, &format!("[{name}]"), m.iter().map(|(k, v)| (toml_key(k), toml_string(v))).collect());
            }
        }
        for t in &self.tested {
            let mut rows = vec![
                ("race".into(), toml_string(&t.race)),
                ("race_digest".into(), toml_string(&t.race_digest.to_string())),
            ];
            if let Some(c) = &t.car {
                rows.push(("car".into(), toml_string(c)));
            }
            if let Some(x) = t.time_s {
                rows.push(("time_s".into(), crate::float::write_f32(x)));
            }
            rows.push(("overbright".into(), toml_string(&t.overbright)));
            section(&mut out, "[[tested]]", rows);
        }
        if let Some(b) = &self.base {
            let mut rows = vec![
                ("edition".into(), toml_string(&b.edition)),
                ("region".into(), toml_string(&b.region)),
                ("languages".into(), list(&b.languages)),
                ("profile".into(), toml_string(&b.profile)),
            ];
            if let Some(i) = &b.import {
                rows.push(("import".into(), toml_string(i)));
            }
            if let Some(m) = &b.marker {
                rows.push(("marker".into(), toml_string(m)));
            }
            if b.linked {
                rows.push(("storage".into(), toml_string("linked")));
            }
            section(&mut out, "[base]", rows);
        }
        if !self.bundles.is_empty() {
            section(
                &mut out,
                "[bundles]",
                self.bundles.iter().map(|(k, v)| (toml_key(k), toml_string(&v.to_string()))).collect(),
            );
        }
        if let Some(e) = &self.extras {
            section(&mut out, "[extras]", e.iter().map(|(k, v)| (toml_key(k), toml_inline(v))).collect());
        }
        out
    }
}

/// The digest `pack.toml` takes part with in the content digest (§4): of the canonical JSON form (RFC 8785) of its
/// TOML document with its `tested` array removed, so marking a race as tested does not change the pack's identity.
pub fn manifest_digest(alg: Alg, bytes: &[u8]) -> Result<Digest, Refusal> {
    let text = text::decode(bytes, MAX_BYTES, Rule::Man8).map_err(|r| r.at_path(FILE))?;
    let mut doc = text::parse_toml(text).map_err(|r| r.at_path(FILE))?;
    doc.remove("tested");
    Ok(alg.of(Json::from_table(&doc).canonical().as_bytes()))
}

/// A pack's content digest (§4): over its files sorted by logical path, `.studio/` excluded, each as its path and
/// digest (of `alg`), `pack.toml` by [`manifest_digest`]. `files` holds every other file's digest; `manifest` is
/// `pack.toml`'s bytes.
pub fn content_digest(alg: Alg, manifest: &[u8], files: &[(&str, Digest)]) -> Result<Digest, Refusal> {
    let m = manifest_digest(alg, manifest)?;
    let mut entries: Vec<(&str, Digest)> =
        files.iter().filter(|(p, _)| *p != FILE && !p.starts_with(".studio/")).copied().collect();
    entries.push((FILE, m));
    Ok(digest::list(alg, digest::PACK_CONTENT, &entries))
}

// ---- migrations.toml ------------------------------------------------------------------------------------------

/// A pack's own key renames (`migrations.toml`, `ob.pack-migrations/1.0`).
pub const MIGRATIONS_FILE: &str = "migrations.toml";
pub const MIGRATIONS_SUPPORTED: &[text::Supported] = &[text::Supported { kind: "pack-migrations", major: 1, minor: 0 }];

fn is_kind_name(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b[0].is_ascii_lowercase()
        && b.iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

const RENAME_FIELDS: &[Field] = &[
    Field {
        key: "kind",
        ty: Ty::Str { min: 0, max: usize::MAX, pattern: pat("^[a-z][a-z0-9-]*$", is_kind_name, Rule::Conv8) },
        req: Req::Yes,
        ..Field::BLANK
    },
    Field { key: "from", ty: string(200, None), req: Req::Yes, ..Field::BLANK },
    Field { key: "to", ty: string(200, None), req: Req::Yes, ..Field::BLANK },
    Field { key: "since", ty: SEMVER, req: Req::Yes, ..Field::BLANK },
];

/// The fields of `migrations.toml`.
pub const MIGRATIONS_FIELDS: &[Field] = &[
    Field {
        key: "schema",
        ty: Ty::Str {
            min: 0,
            max: usize::MAX,
            pattern: pat("^ob\\.pack-migrations/1\\.[0-9]+$", |s| s.starts_with("ob.pack-migrations/1."), Rule::Conv3),
        },
        req: Req::Yes,
        ..Field::BLANK
    },
    Field {
        key: "rename",
        ty: Ty::Array { item: &Ty::Table(RENAME_FIELDS), min: 1, max: None, unique: false },
        req: Req::Yes,
        ..Field::BLANK
    },
    Field { key: "extras", ty: Ty::Extras, ..Field::BLANK },
];

/// The JSON Schema `schemas/pack-migrations.schema.json`.
pub fn migrations_schema() -> Json {
    let Json::Object(body) = fields::table_schema(MIGRATIONS_FIELDS) else { unreachable!() };
    let mut j = Json::obj([
        ("$schema", Json::str("https://json-schema.org/draft/2020-12/schema")),
        ("$id", Json::str("pack-migrations.schema.json")),
        ("title", Json::str("A pack's own key renames (ob.pack-migrations/1.0)")),
        (
            "description",
            Json::str(
                "versioning.md 4.3: renames of keys the pack itself added, so saves made with an older version of the pack \
                 still resolve. File: migrations.toml at the pack's root. Generated from the code \
                 (crates/ob-asset-format/src/manifest.rs); do not edit.",
            ),
        ),
    ]);
    if let Json::Object(o) = &mut j {
        o.extend(body);
    }
    j
}

/// One rename.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rename {
    pub kind: String,
    pub from: String,
    pub to: String,
    pub since: semver::Version,
}

/// Reads `migrations.toml`.
pub fn parse_migrations(bytes: &[u8]) -> Result<Vec<Rename>, Refusal> {
    let at = |r: Refusal| r.at_path(MIGRATIONS_FILE);
    let text = text::decode(bytes, text::MAX_TOML_BYTES, Rule::Conv12).map_err(at)?;
    let doc = text::parse_toml(text).map_err(at)?;
    let id = text::check_schema(&doc, MIGRATIONS_SUPPORTED).map_err(at)?;
    fields::check_table(&doc, MIGRATIONS_FIELDS, Some(id.minor)).map_err(at)?;
    Ok(doc["rename"]
        .as_array()
        .expect("checked")
        .iter()
        .map(|r| {
            let r = r.as_table().expect("checked");
            Rename {
                kind: s(r, "kind").expect("checked"),
                from: s(r, "from").expect("checked"),
                to: s(r, "to").expect("checked"),
                since: semver::Version::parse(&s(r, "since").expect("checked")).expect("checked"),
            }
        })
        .collect())
}

/// Applies a rename table to the keys of one kind, all renames at once (so two names may swap); refuses a
/// result in which two items share a key (`VER.5`).
pub fn apply_renames(renames: &[Rename], kind: &str, keys: &[String]) -> Result<Vec<String>, Refusal> {
    let out: Vec<String> = keys
        .iter()
        .map(|k| renames.iter().find(|r| r.kind == kind && &r.from == k).map_or_else(|| k.clone(), |r| r.to.clone()))
        .collect();
    let mut seen = std::collections::HashSet::new();
    match out.iter().find(|k| !seen.insert(k.as_str())) {
        Some(k) => Err(Refusal::new(Rule::Ver5, format!("two {kind} items end up with the key {k:?}"))),
        None => Ok(out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOD: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/modding/asset-format/examples/manifest/mod/pack.toml"));
    const BASE: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/modding/asset-format/examples/manifest/base/pack.toml"));

    fn parse(text: &str) -> Result<Manifest, Refusal> {
        Manifest::parse(text.as_bytes(), Container::Unknown).map(|(m, _)| m)
    }

    fn rule(text: &str) -> Rule {
        parse(text).unwrap_err().rule
    }

    #[test]
    fn examples_read_and_write_back() {
        let m = parse(MOD).unwrap();
        assert_eq!((m.kind, m.id.as_str(), m.version.to_string()), (PackKind::Mod, "docks-remix", "1.2.0".into()));
        assert_eq!(m.dependencies["cone-pack"], ">=1.0");
        assert_eq!(m.tested[0].time_s, Some(83.41));
        assert_eq!(m.to_toml(), MOD);
        let b = parse(BASE).unwrap();
        assert_eq!(b.base.as_ref().unwrap().profile, "nfsu2/1");
        assert_eq!(b.share, Share::LocalOnly);
        assert_eq!(b.to_toml(), BASE);
        m.check_engine(&semver::Version::new(0, 5, 0)).unwrap();
        assert_eq!(m.check_engine(&semver::Version::new(0, 4, 9)).unwrap_err().rule, Rule::Man3);
    }

    #[test]
    fn refusals() {
        let with = |from: &str, to: &str| MOD.replacen(from, to, 1);
        assert_eq!(rule(&with("format = 2", "format = 3")), Rule::Man1);
        assert_eq!(rule(&with("format = 2", "format = \"2\"")), Rule::Man1);
        assert_eq!(rule(&with("id = \"docks-remix\"", "id = \"Docks\"")), Rule::Man2);
        assert_eq!(rule(&with("share = \"public\"", "share = \"public\"\norigin2 = 1")), Rule::Conv4);
        assert_eq!(rule(&with("origin = \"own-work\"", "origin = \"from-game-files\"")), Rule::Man4);
        assert_eq!(rule(&with("licence = \"CC-BY-4.0\"\n", "")), Rule::Man4);
        assert_eq!(rule(&with("licence = \"CC-BY-4.0\"", "licence = \"x\"\nlicense = \"y\"")), Rule::Conv4);
        assert_eq!(
            rule(&with(
                "affects_play = true",
                "affects_play = true\n\n[base]\nedition = \"x\"\nregion = \"eu\"\nlanguages = [\"english\"]\nprofile = \"nfsu2/1\""
            )),
            Rule::Man5
        );
        assert_eq!(
            rule(
                &BASE
                    .replacen("\n[base]", "\n[extras]", 1)
                    .replace("edition", "a")
                    .replace("region", "b")
                    .replace("languages", "c")
                    .replace("profile", "d")
                    .replace("import =", "e =")
            ),
            Rule::Man5
        );
        assert_eq!(rule(&with("cone-pack = \">=1.0\"", "docks-remix = \"*\"")), Rule::Man7);
        assert_eq!(rule(&with("cone-pack = \">=1.0\"", "cone-pack = \"at least 1\"")), Rule::Man7);
        assert_eq!(
            rule(&with("affects_play = true", "affects_play = true\nextensions_required = [\"ACME_x\"]")),
            Rule::Man9
        );
        assert_eq!(rule(&with("name = \"Docks remix\"\n", "")), Rule::Conv5);
        assert_eq!(rule(&with("version = \"1.2.0\"", "version = \"1.2\"")), Rule::Conv6);
        assert_eq!(rule(&with("homepage = \"https://", "homepage = \"http://")), Rule::Conv6);
        assert_eq!(rule(&with("race_digest = \"xxh3-128:", "race_digest = \"md5:")), Rule::Conv8);
        assert_eq!(rule(&format!("{MOD}#{}", "x".repeat(MAX_BYTES))), Rule::Man8);
        assert_eq!(rule("\u{feff}format = 2"), Rule::Conv1);
        assert_eq!(rule("format = = 2"), Rule::Conv2);
        // The file name of a pack file carries the id and version.
        let pf = |f| Manifest::parse(MOD.as_bytes(), Container::PackFile(f));
        assert!(pf("docks-remix-1.2.0.obpk").is_ok());
        assert_eq!(pf("docks-1.2.0.obpk").unwrap_err().rule, Rule::Man2);
        let (_, warn) = Manifest::parse(MOD.as_bytes(), Container::Folder("docks")).unwrap();
        assert_eq!(warn.len(), 1);
        let ext = parse(&with(
            "affects_play = true",
            "affects_play = true\nextensions_used = [\"ACME_x\"]\nextensions_required = [\"ACME_x\"]",
        ))
        .unwrap();
        assert_eq!(ext.check_extensions(&[]).unwrap_err().rule, Rule::Conv14);
        ext.check_extensions(&["ACME_x"]).unwrap();
    }

    #[test]
    fn legacy_manifests() {
        // Today's texture-pack manifests (docs/modding/texture-packs.md), read by manifest.md §6.
        let (m, _) = Manifest::parse(
            b"# Overbright texture pack\nformat = 1\nname = \"HD HUD\"\nauthor = \"someone\"\nlicence = \"CC0-1.0\"\npriority = 5\nversion = \"v2 beta\"\nsource = \"texmod\"\nunknown = 1\n",
            Container::Folder("hd-hud"),
        )
        .unwrap();
        assert_eq!((m.format, m.kind, m.id.as_str(), m.priority), (1, PackKind::Mod, "hd-hud", 5));
        assert_eq!(m.authors, ["someone"]);
        assert_eq!((m.origin.clone(), m.share), (Origin::FromGameFiles, Share::LocalOnly));
        assert_eq!(m.legacy.as_ref().unwrap().version, "v2 beta");
        assert!(m.overbright.matches(&semver::Version::new(9, 9, 9)));
        let (own, _) =
            Manifest::parse(b"name = \"Mine\"\nlicense = \"MIT\"\ncompress = \"none\"", Container::Folder("mine"))
                .unwrap();
        assert_eq!(
            (own.origin, own.share, own.legacy.unwrap().compress),
            (Origin::OwnWork, Share::Public, Compress::None)
        );
        assert_eq!(Manifest::parse(b"format = 1\n", Container::Folder("x")).unwrap_err().rule, Rule::Conv5);
        assert_eq!(
            Manifest::parse(b"name = \"a\"\npriority = 2000000", Container::Folder("x")).unwrap_err().rule,
            Rule::Conv6
        );
    }

    #[test]
    fn content_digest_ignores_tested_and_storage() {
        let files =
            [("world/L4RA/placements.toml", Alg::Xxh3_128.of(b"p")), (".studio/journal.toml", Alg::Xxh3_128.of(b"j"))];
        let a = content_digest(Alg::Xxh3_128, MOD.as_bytes(), &files).unwrap();
        let untested = MOD.split("\n[[tested]]").next().unwrap().to_string() + "\n";
        assert_eq!(a, content_digest(Alg::Xxh3_128, untested.as_bytes(), &files[..1]).unwrap());
        // Key order and layout of pack.toml do not matter, its values do.
        let reordered =
            MOD.replacen("kind = \"mod\"\nid = \"docks-remix\"", "id = \"docks-remix\"\nkind   =   'mod'", 1);
        assert_eq!(a, content_digest(Alg::Xxh3_128, reordered.as_bytes(), &files).unwrap());
        assert_ne!(a, content_digest(Alg::Xxh3_128, MOD.replace("1.2.0", "1.2.1").as_bytes(), &files).unwrap());
        assert_ne!(
            a,
            content_digest(Alg::Xxh3_128, MOD.as_bytes(), &[("world/L4RA/placements.toml", Alg::Xxh3_128.of(b"q"))])
                .unwrap()
        );
        let s = content_digest(Alg::Sha256, MOD.as_bytes(), &[("world/L4RA/placements.toml", Alg::Sha256.of(b"p"))])
            .unwrap();
        assert_eq!(s.alg(), Alg::Sha256);
    }

    #[test]
    fn migrations() {
        let ex = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/modding/asset-format/examples/versioning/migrations.toml"
        ));
        let r = parse_migrations(ex).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].to, "docks-remix:docks-sprint");
        let keys = ["L4RA/docks-remix:a12".to_string(), "L4RA/docks-remix:a13".to_string()];
        assert_eq!(apply_renames(&r, "placement", &keys).unwrap()[0], "L4RA/docks-remix:crane-1");
        // A swap is fine; two keys onto one is VER.5.
        let swap = [
            Rename { kind: "race".into(), from: "a".into(), to: "b".into(), since: semver::Version::new(1, 0, 0) },
            Rename { kind: "race".into(), from: "b".into(), to: "a".into(), since: semver::Version::new(1, 0, 0) },
        ];
        assert_eq!(apply_renames(&swap, "race", &["a".into(), "b".into()]).unwrap(), ["b", "a"]);
        assert_eq!(apply_renames(&swap[..1], "race", &["a".into(), "b".into()]).unwrap_err().rule, Rule::Ver5);
        let bad = |t: &str| parse_migrations(t.as_bytes()).unwrap_err().rule;
        assert_eq!(
            bad(
                "schema = \"ob.pack-migrations/1.1\"\n[[rename]]\nkind = \"race\"\nfrom = \"a\"\nto = \"b\"\nsince = \"1.0.0\""
            ),
            Rule::Ver1
        );
        assert_eq!(
            bad(
                "schema = \"ob.pack-migrations/2.0\"\n[[rename]]\nkind = \"race\"\nfrom = \"a\"\nto = \"b\"\nsince = \"1.0.0\""
            ),
            Rule::Ver2
        );
        assert_eq!(
            bad("schema = \"ob.pack-migrations/1.0\"\n[[rename]]\nkind = \"race\"\nfrom = \"a\"\nto = \"b\""),
            Rule::Conv5
        );
        assert_eq!(bad("schema = \"ob.pack-migrations/1.0\"\nrename = []"), Rule::Conv6);
    }
}

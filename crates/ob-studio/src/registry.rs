//! The registries: every kind of content, its fields and every command, declared once. The declarations give the
//! schema export (`ob://schema/commands` and the kinds, [`export`]), and later the tables, inspector rows,
//! command-line verbs and docs; a test keeps each kind's fields in step with its file's schema in the asset format
//! (docs/modding/asset-format/schemas/, generated from the same tables), and the export in step with
//! `crates/ob-studio/schema/studio.json`.

pub use ob_asset_format::fields::{Field, Req, Ty};
use ob_asset_format::json::Json;
use ob_asset_format::kinds;
use ob_asset_format::layering::{KindRules, ListKey};
use ob_map::tools::map::ops::Op;

/// Studio's name of a field's type (the export's `type`, the inspector's editor): `bool`, `u8`/`u16`/`u32`, `f32`,
/// `vec2`/`vec3`/`mat3`, `text`, `enum`, `ref <kind>`, `key` (a keyed list element's), `table`, `list of <type>`,
/// `extras`.
pub fn type_name(ty: &Ty) -> String {
    match ty {
        Ty::Bool => "bool".into(),
        Ty::Int { min, max } | Ty::Def(_, Ty::Int { min, max }) if *min >= 0 => if *max <= 255 {
            "u8"
        } else if *max <= 65535 {
            "u16"
        } else {
            "u32"
        }
        .into(),
        Ty::Int { .. } | Ty::Const(_) => "int".into(),
        Ty::F32 | Ty::F32In { .. } => "f32".into(),
        Ty::Str { .. } => "text".into(),
        Ty::Enum(_) => "enum".into(),
        Ty::Ref(kind, _) => format!("ref {kind}"),
        Ty::Def("json", _) => "json".into(),
        Ty::Def(_, inner) | Ty::Local(_, inner) => type_name(inner),
        Ty::Array { item, min, max: Some(max), .. }
            if min == max && matches!(*min, 2 | 3 | 9) && type_name(item) == "f32" =>
        {
            match min {
                2 => "vec2",
                3 => "vec3",
                _ => "mat3",
            }
            .into()
        }
        Ty::Array { item, .. } => format!("list of {}", type_name(item)),
        Ty::Map { .. } => "map".into(),
        Ty::Either(..) => "key".into(),
        Ty::Table(_) => "table".into(),
        Ty::Extras => "extras".into(),
    }
}

/// The sub-fields of a table field or of a list of tables.
fn sub_fields(ty: &Ty) -> Option<&'static [Field]> {
    match ty {
        Ty::Table(sub) => Some(sub),
        Ty::Def(_, inner) | Ty::Local(_, inner) | Ty::Ref(_, inner) => sub_fields(inner),
        Ty::Array { item, .. } => sub_fields(item),
        _ => None,
    }
}

/// The words of an enum field.
pub fn enum_words(ty: &Ty) -> Option<&'static [&'static str]> {
    match ty {
        Ty::Enum(words) => Some(words),
        Ty::Def(_, inner) | Ty::Local(_, inner) => enum_words(inner),
        _ => None,
    }
}

const fn f(key: &'static str, ty: Ty, req: Req, label: &'static str) -> Field {
    Field { key, ty, req, label, ..Field::BLANK }
}

/// How an item of a kind is named (the asset format's kind registry, docs/modding/asset-format/README.md §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyRule {
    /// The key's form, as the asset format writes it.
    pub form: &'static str,
    /// An example.
    pub example: &'static str,
}

/// One kind of content.
#[derive(Clone, Copy, Debug)]
pub struct Kind {
    /// The short name references use (`placement`).
    pub short: &'static str,
    /// The file kind's schema id (`ob.placements/1.0`).
    pub schema: &'static str,
    /// The asset format's schema file of that file kind (docs/modding/asset-format/schemas/).
    pub schema_file: &'static str,
    /// The table an entry sits in (`placement` for `[[placement]]`), or "" when the file is one item.
    pub entries: &'static str,
    /// The keys of original items and of items a pack adds.
    pub keys: [KeyRule; 2],
    /// The entry's fields.
    pub fields: &'static [Field],
    /// What packs may do with the kind's items.
    pub rules: KindRules,
}

/// The kind of a command (who may run it, and how).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
    /// Reads only.
    Read,
    /// Changes the open pack, as one undo step.
    Edit,
    /// A job with progress and a result; not undoable.
    Run,
    /// Cannot be undone; asks first.
    Destructive,
}

impl CommandKind {
    /// Its word in the schema export and the help.
    pub fn word(self) -> &'static str {
        match self {
            CommandKind::Read => "read",
            CommandKind::Edit => "edit",
            CommandKind::Run => "run",
            CommandKind::Destructive => "destructive",
        }
    }
}

/// One command: a stable name (`props.move`), its kind and its arguments.
#[derive(Clone, Copy, Debug)]
pub struct Command {
    pub name: &'static str,
    pub kind: CommandKind,
    /// One line for help texts and tool lists.
    pub summary: &'static str,
    pub args: &'static [Field],
}

// ---- the kinds ----------------------------------------------------------------------------------------------------
// Their fields are the asset format's tables (ob_asset_format::kinds): the same table checks the file, writes its
// schema and gives the inspector's rows.

/// A scenery placement (docs/modding/asset-format/world.md §3.9).
pub const PLACEMENT: Kind = Kind {
    short: "placement",
    schema: "ob.placements/1.0",
    schema_file: "placements.schema.json",
    entries: "placement",
    keys: [
        KeyRule { form: "placement:<REGION>/<scenery>/<index>", example: "placement:L4RA/1407/12" },
        KeyRule { form: "placement:<REGION>/<pack>:<id>", example: "placement:L4RA/docks-remix:a12" },
    ],
    fields: kinds::PLACEMENT,
    rules: KindRules { add: true, replace: false, disable: true, keyed_lists: &[] },
};

/// A custom race of a mod pack (docs/modding/asset-format/roads-ai.md §3.7).
pub const RACE: Kind = Kind {
    short: "race",
    schema: "ob.race/1.0",
    schema_file: "race.schema.json",
    entries: "",
    keys: [
        KeyRule { form: "(races are never original)", example: "" },
        KeyRule { form: "race:<pack>:<id>", example: "race:docks-remix:sprint" },
    ],
    fields: kinds::RACE,
    // A race is its pack's own (`race:<pack>:<id>`): the pack adds it; a pack that depends on it may merge into it
    // or disable it (layering.md §3.1), its grid merged slot by slot.
    rules: KindRules { add: true, replace: false, disable: true, keyed_lists: &[("grid", ListKey::Index)] },
};

/// Every kind Studio edits.
pub const KINDS: &[Kind] = &[PLACEMENT, RACE];

/// A kind of line records Studio reads (not pack content, not layered): its records are the asset format's table,
/// the same one that checks the file and writes its schema.
#[derive(Clone, Copy, Debug)]
pub struct RecordKind {
    pub short: &'static str,
    /// The schema id a writer of this version puts in the file.
    pub schema: &'static str,
    pub schema_file: &'static str,
    pub records: &'static [kinds::run::Record],
}

/// The run file (docs/modding/asset-format/runs.md): simulations, recordings and replays.
pub const RUN: RecordKind =
    RecordKind { short: "run", schema: kinds::run::SCHEMA, schema_file: "run.schema.json", records: kinds::run::RUN };

/// Every record kind Studio reads.
pub const RECORD_KINDS: &[RecordKind] = &[RUN];

/// The kind with this short name.
pub fn kind(short: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|k| k.short == short)
}

// ---- the commands -------------------------------------------------------------------------------------------------

const TEXT: Ty = Ty::Str { min: 0, max: usize::MAX, pattern: None };
const F32: Ty = Ty::Def("f32", &Ty::F32);
const VEC3: Ty = Ty::Def("vec3", &Ty::Array { item: &F32, min: 3, max: Some(3), unique: false });
const MAT3: Ty = Ty::Def("mat3", &Ty::Array { item: &F32, min: 9, max: Some(9), unique: false });
const U8: Ty = Ty::Def("u8", &Ty::Int { min: 0, max: 255 });
const OUTLINE: Ty = Ty::Array { item: &VEC3, min: 3, max: Some(32), unique: false };
const U16: Ty = Ty::Def("u16", &Ty::Int { min: 0, max: 65535 });
const U32: Ty = Ty::Def("u32", &Ty::Int { min: 0, max: u32::MAX as i64 });
const SOLID: Ty = Ty::Ref("solid", &TEXT);
const RACE_KIND: Ty = Ty::Enum(&["circuit", "sprint", "streetx", "drift"]);
const PART: Ty = Ty::Enum(&["checkpoint", "grid", "start", "finish"]);
/// A JSON value of the shape the command's summary names (a whole race, a list of edits).
const JSON: Ty = Ty::Def("json", &Ty::Extras);
const KEY: Field = f("key", TEXT, Req::Yes, "Object");
const RACE_ID: Field = f("race", TEXT, Req::Yes, "Race");
const AT: Field = f("position", VEC3, Req::Yes, "Position");
const HEADING: Field = f("heading", F32, Req::No, "Heading");
const POINT_PART: Field = f("part", PART, Req::Yes, "Part");
const POINT_INDEX: Field = f("index", U32, Req::No, "Number");
const RUN_NAME: Field = f("run", TEXT, Req::Yes, "The run's name (`runs list`)");
const FILE: Field = f("file", TEXT, Req::Yes, "File");
/// A game texture (`world:0x1A2B3C4D`, Studio Looks).
const TEXTURE: Field = f("texture", TEXT, Req::Yes, "Texture");

/// Every command, by stable name. Each map edit ([`Op`]) has one ([`command_of`]); the command-line tool has one
/// verb per command (`props move` runs `props.move`) and one flag per argument.
pub const COMMANDS: &[Command] = &[
    Command {
        name: "props.move",
        kind: CommandKind::Edit,
        summary: "Move a prop",
        args: &[KEY, AT, f("rotation", MAT3, Req::No, "Rotation"), HEADING],
    },
    Command {
        name: "props.delete",
        kind: CommandKind::Edit,
        summary: "Hide an original prop or remove an added one",
        args: &[KEY],
    },
    Command {
        name: "props.reset",
        kind: CommandKind::Edit,
        summary: "Put an original prop back as the original has it",
        args: &[KEY],
    },
    Command {
        name: "props.add",
        kind: CommandKind::Edit,
        summary: "Add a prop of one of the region's objects",
        args: &[
            f("solid", SOLID, Req::Yes, "Object"),
            AT,
            f("rotation", MAT3, Req::No, "Rotation"),
            HEADING,
            f("id", TEXT, Req::No, "Id"),
            f("ground", Ty::Bool, Req::No, "On the ground"),
        ],
    },
    Command {
        name: "props.duplicate",
        kind: CommandKind::Edit,
        summary: "Copy a prop to a new place",
        args: &[KEY, AT],
    },
    Command {
        name: "props.drop",
        kind: CommandKind::Edit,
        summary: "Stand a prop on the floor under it",
        args: &[KEY],
    },
    Command {
        name: "collision.remove",
        kind: CommandKind::Edit,
        summary: "Take out a wall of the original's collision, or a drawn wall or ground",
        args: &[f("target", JSON, Req::Yes, "Collision")],
    },
    Command {
        name: "collision.wall",
        kind: CommandKind::Edit,
        summary: "Draw a wall of a height between two points",
        args: &[
            f("from", VEC3, Req::Yes, "From"),
            f("to", VEC3, Req::Yes, "To"),
            f("height", F32, Req::Yes, "Height"),
            f("surface", U8, Req::Yes, "Surface"),
            f("id", TEXT, Req::No, "Id"),
        ],
    },
    Command {
        name: "collision.ground",
        kind: CommandKind::Edit,
        summary: "Draw ground over an outline",
        args: &[
            f("outline", OUTLINE, Req::Yes, "Outline"),
            f("surface", U8, Req::Yes, "Surface"),
            f("id", TEXT, Req::No, "Id"),
        ],
    },
    Command {
        name: "race.create",
        kind: CommandKind::Edit,
        summary: "Make a race on a base track",
        args: &[
            f("track", U16, Req::Yes, "Base track"),
            f("kind", RACE_KIND, Req::No, "Mode"),
            f("id", TEXT, Req::No, "Id"),
        ],
    },
    Command { name: "race.delete", kind: CommandKind::Edit, summary: "Delete a race", args: &[RACE_ID] },
    Command {
        name: "race.set",
        kind: CommandKind::Edit,
        summary: "Change a race's name, id, track, mode, laps or direction",
        args: &[
            RACE_ID,
            f("name", TEXT, Req::No, "Name"),
            f("id", TEXT, Req::No, "Id"),
            f("track", U16, Req::No, "Base track"),
            f("kind", RACE_KIND, Req::No, "Mode"),
            f("laps", U32, Req::No, "Laps"),
            f("backward", Ty::Bool, Req::No, "Backward"),
        ],
    },
    Command {
        name: "race.point.add",
        kind: CommandKind::Edit,
        summary: "Add a checkpoint or grid slot, or place the start or finish line",
        args: &[RACE_ID, POINT_PART, POINT_INDEX, AT, HEADING],
    },
    Command {
        name: "race.point.move",
        kind: CommandKind::Edit,
        summary: "Move a race point, or a checkpoint one place later or earlier in the route",
        args: &[
            RACE_ID,
            POINT_PART,
            POINT_INDEX,
            f("position", VEC3, Req::No, "Position"),
            HEADING,
            f("later", Ty::Bool, Req::No, "Later in the route"),
        ],
    },
    Command {
        name: "race.point.delete",
        kind: CommandKind::Edit,
        summary: "Delete a race point",
        args: &[RACE_ID, POINT_PART, POINT_INDEX],
    },
    Command {
        name: "race.put",
        kind: CommandKind::Edit,
        summary: "Put a whole race in place (the race as JSON)",
        args: &[RACE_ID, f("to", JSON, Req::Yes, "Race")],
    },
    Command {
        name: "pack.create",
        kind: CommandKind::Run,
        summary: "Make a new map mod, nothing changed yet or from a template (its folder from its name unless given)",
        args: &[
            f("name", TEXT, Req::Yes, "Name"),
            f("folder", TEXT, Req::No, "Folder"),
            f("template", TEXT, Req::No, "Start from: empty (default), sprint, circuit or sample"),
            f("maps", TEXT, Req::No, "Mods folder (default: the game's map mods folder)"),
        ],
    },
    Command {
        name: "pack.details",
        kind: CommandKind::Edit,
        summary: "Change the pack's name, author and description",
        args: &[
            f("name", TEXT, Req::No, "Name"),
            f("author", TEXT, Req::No, "Author"),
            f("description", TEXT, Req::No, "Description"),
        ],
    },
    Command {
        name: "pack.put",
        kind: CommandKind::Edit,
        summary: "Put the whole map in place (from a map file)",
        args: &[FILE],
    },
    Command {
        name: "pack.export",
        kind: CommandKind::Run,
        summary: "Export the mod as one file to share: a .zip (or a folder), without its work files",
        args: &[
            f("to", TEXT, Req::Yes, "Export to"),
            f("derived-ok", Ty::Bool, Req::No, "Share textures made from the original game's pixels"),
        ],
    },
    Command {
        name: "modpack.export",
        kind: CommandKind::Run,
        summary: "Export the mod as a mod pack file (.obpk): its placements, races and textures as the asset format's own files",
        args: &[
            f("to", TEXT, Req::Yes, "Export to: a .obpk file, or a folder"),
            f("version", TEXT, Req::No, "Pack version (default 1.0.0)"),
            f("licence", TEXT, Req::No, "SPDX licence; without one the pack stays on this computer"),
            f("derived-ok", Ty::Bool, Req::No, "Share textures made from the original game's pixels"),
        ],
    },
    Command {
        name: "modpack.convert",
        kind: CommandKind::Run,
        summary: "Convert a plain map mod or a texture pack (a folder or .zip) to a mod pack file (.obpk)",
        args: &[
            f("from", TEXT, Req::Yes, "The map mod or texture pack: a folder or .zip"),
            f("to", TEXT, Req::Yes, "Write to: a .obpk file, or a folder"),
            f("id", TEXT, Req::No, "Pack id (default: made from the name)"),
            f("version", TEXT, Req::No, "Pack version (default 1.0.0)"),
            f("licence", TEXT, Req::No, "SPDX licence; without one the pack stays on this computer"),
            f("derived-ok", Ty::Bool, Req::No, "Convert textures made from the original game's pixels"),
        ],
    },
    Command {
        name: "modpack.report",
        kind: CommandKind::Read,
        summary: "What a mod pack changes compared with the game, in words: its props, races, textures, credits and clashes",
        args: &[
            f("pack", TEXT, Req::Yes, "A .obpk file or pack folder, or the id of an installed pack"),
            f("to", TEXT, Req::No, "Also write the report to this text file"),
        ],
    },
    Command {
        name: "pack.repair",
        kind: CommandKind::Edit,
        summary: "Repair a map file that no longer reads, where a safe repair exists (as one undo step)",
        args: &[],
    },
    Command {
        name: "edit.group",
        kind: CommandKind::Edit,
        summary: "Several edits as one undo step (the edits as a JSON list)",
        args: &[f("label", TEXT, Req::Yes, "Name"), f("ops", JSON, Req::Yes, "Edits")],
    },
    Command {
        name: "ops.apply",
        kind: CommandKind::Edit,
        summary: "Apply an operations file (one edit per line) as one undo step",
        args: &[FILE, f("label", TEXT, Req::No, "Name")],
    },
    Command {
        name: "looks.open",
        kind: CommandKind::Run,
        summary: "Open a texture in your own paint program: its work file, from the game's pixels or blank",
        args: &[TEXTURE, f("blank", Ty::Bool, Req::No, "Repaint from blank")],
    },
    Command {
        name: "looks.import",
        kind: CommandKind::Edit,
        summary: "Take a texture's work file in now, as one step",
        args: &[TEXTURE],
    },
    Command {
        name: "looks.watch",
        kind: CommandKind::Run,
        summary: "Watch the work files and take each save in as one step",
        args: &[f("seconds", F32, Req::No, "For how long (default: until Ctrl+C)")],
    },
    Command {
        name: "looks.list",
        kind: CommandKind::Read,
        summary: "The mod's textures and work files, with where their pixels started",
        args: &[],
    },
    Command {
        name: "props.solids",
        kind: CommandKind::Read,
        summary: "List the region's placeable solids (the prop browser's list), by name, with where they are placed",
        args: &[
            f("filter", TEXT, Req::No, "Words the name has (all of them, any case)"),
            f(
                "category",
                TEXT,
                Req::No,
                "Only one category: knock-over, barriers, signs, street, trees or all (default all)",
            ),
        ],
    },
    Command {
        name: "race.tracks",
        kind: CommandKind::Read,
        summary: "List the region's base tracks a race can be on (number and name)",
        args: &[],
    },
    Command {
        name: "looks.find",
        kind: CommandKind::Read,
        summary: "Find the region's world textures by name or hash, with where each is used (words after the command or --text)",
        args: &[f("text", TEXT, Req::Yes, "Words of the name, or the hash (0x1A2B3C4D)")],
    },
    Command {
        name: "game.find",
        kind: CommandKind::Read,
        summary: "Show which game folder the tool uses and why (--game, NFSU2_ROOT or the launcher)",
        args: &[],
    },
    Command {
        name: "collision.show",
        kind: CommandKind::Read,
        summary: "What the collision layer shows in a mode (off, invisible barriers or all collision), counted as the World view draws it; with --mod the mod's removed and drawn collision applies",
        args: &[f("mode", TEXT, Req::No, "off, invisible or all (default all)")],
    },
    Command {
        name: "runs.list",
        kind: CommandKind::Read,
        summary: "The mod's kept runs (simulations and recordings), newest first, as the Runs workspace lists them",
        args: &[],
    },
    Command {
        name: "runs.show",
        kind: CommandKind::Read,
        summary: "Open a run: its race, seed, how it ended, the results and the problems it found",
        args: &[RUN_NAME],
    },
    Command {
        name: "runs.telemetry",
        kind: CommandKind::Read,
        summary: "One car's telemetry in a run: speed over time, distance from the racing line, its events, and the gear, rpm and pedals of a run recorded with the drive channels",
        args: &[RUN_NAME, f("car", U32, Req::No, "Car number (default: the player's slot, 0)")],
    },
    Command {
        name: "runs.compare",
        kind: CommandKind::Read,
        summary: "Compare two runs: where they part and when a car is over a metre from where the first has it",
        args: &[RUN_NAME, f("with", TEXT, Req::Yes, "The run to compare it with"), f("car", U32, Req::No, "Car number (default 0)")],
    },
    Command {
        name: "runs.rename",
        kind: CommandKind::Run,
        summary: "Rename a run (never over another run)",
        args: &[RUN_NAME, f("to", TEXT, Req::Yes, "The new name")],
    },
    Command {
        name: "runs.delete",
        kind: CommandKind::Destructive,
        summary: "Delete a run file",
        args: &[RUN_NAME],
    },
    Command {
        name: "data.kinds",
        kind: CommandKind::Read,
        summary: "The kinds of records the Data grid shows, with their columns and how many rows the mod's view has",
        args: &[],
    },
    Command {
        name: "data.show",
        kind: CommandKind::Read,
        summary: "One kind's table as the Data grid shows it, with where each value comes from",
        args: &[
            f("kind", TEXT, Req::Yes, "The kind: placement or race"),
            f("search", TEXT, Req::No, "Keep rows where any cell has all these words"),
            f("filter", TEXT, Req::No, "Keep rows whose column has the text: `column=text`"),
            f("sort", TEXT, Req::No, "Sort by this column (smallest first)"),
            f("desc", Ty::Bool, Req::No, "Largest first"),
        ],
    },
    Command {
        name: "data.set",
        kind: CommandKind::Edit,
        summary: "Write one cell of the Data grid as one undo step (the cell as the grid takes it typed)",
        args: &[
            f("kind", TEXT, Req::Yes, "The kind: placement or race"),
            f("row", TEXT, Req::Yes, "The row's key, as `data show` gives it"),
            f("field", TEXT, Req::Yes, "The column's field"),
            f("value", TEXT, Req::Yes, "The value, as typed into the cell"),
        ],
    },
    Command {
        name: "browser.search",
        kind: CommandKind::Read,
        summary: "Search the region's models and textures by name, hash or `section:N`, as the Browser workspace does",
        args: &[
            f("text", TEXT, Req::No, "Words of the name, a hash (0x1A2B3C4D) or section:N"),
            f("kind", TEXT, Req::No, "all, models, textures, sounds or strings (default all)"),
            f("category", TEXT, Req::No, "Models only: knock-over, barriers, signs, street, trees or all"),
            f("textures", TEXT, Req::No, "Textures only: any, in-use, unused, animated or large"),
        ],
    },
    Command {
        name: "browser.sounds",
        kind: CommandKind::Read,
        summary: "List and search the game's sounds (sound bank events) by bank or name, as the Browser's Sounds chip does",
        args: &[f("text", TEXT, Req::No, "Words of the bank's path or the sound's name (all must be in it)")],
    },
    Command {
        name: "browser.strings",
        kind: CommandKind::Read,
        summary: "List and search a language file's lines by words, 0x hash or label, as the Browser's Strings chip does",
        args: &[
            f("text", TEXT, Req::No, "Words of the text, a hash (0x1A2B3C4D) or a label (IND_GRAPHICS_BODY)"),
            f("language", TEXT, Req::No, "The language file (default English)"),
        ],
    },
    Command {
        name: "browser.wav",
        kind: CommandKind::Run,
        summary: "Write a sound as a WAV file (marked as from the game files, local-only)",
        args: &[
            f("text", TEXT, Req::Yes, "The sound's name (all words, any case), with words of its bank when two share a name"),
            f("to", TEXT, Req::No, "The folder to write (default the mod's .editor/reference; never inside the game's folder)"),
        ],
    },
    Command {
        name: "browser.mesh",
        kind: CommandKind::Read,
        summary: "A model's geometry in numbers (vertices, triangles, size, parts), as the Browser's model viewer shows it",
        args: &[f("text", TEXT, Req::Yes, "The model's name (all words, any case) or hash")],
    },
    Command {
        name: "browser.area",
        kind: CommandKind::Run,
        summary: "Write the area around a point (the camera) as one glTF reference (marked as from the game files, local-only)",
        args: &[
            f("at", TEXT, Req::Yes, "The centre as x,y,z in the game's metres (Z up), as the camera's place shows"),
            f("radius", F32, Req::No, "Metres around it, 1 to 300 (default 60)"),
            f("to", TEXT, Req::No, "The folder to write (default the mod's .editor/reference; never inside the game's folder)"),
        ],
    },
    Command {
        name: "browser.used",
        kind: CommandKind::Read,
        summary: "Where a model or texture is used: its placements, sections, textures or models (the Browser's used by)",
        args: &[f("text", TEXT, Req::Yes, "Its name (all words, any case) or hash")],
    },
    Command {
        name: "browser.export",
        kind: CommandKind::Run,
        summary: "Write a model as a glTF reference for an outside 3D program (marked as from the game files, local-only)",
        args: &[
            f("text", TEXT, Req::Yes, "The model's name (all words, any case) or hash"),
            f("to", TEXT, Req::No, "The folder to write (default the mod's .editor/reference; never inside the game's folder)"),
        ],
    },
    Command {
        name: "browser.import",
        kind: CommandKind::Run,
        summary: "Bring an edited glTF back as a mod model, mapped onto the original model's parts; refuses what cannot be mapped",
        args: &[
            f("file", TEXT, Req::Yes, "The edited .gltf (with its .bin beside it)"),
            f("model", TEXT, Req::No, "The model it is for (default the one the file says it was exported from)"),
            f("own", Ty::Bool, Req::No, "The model is entirely your own work: shared with the mod, without the game's pictures"),
        ],
    },
    Command { name: "checks.run", kind: CommandKind::Read, summary: "Run the map checks on the pack", args: &[] },
    Command {
        name: "history.show",
        kind: CommandKind::Read,
        summary: "The pack's kept steps, with their authors",
        args: &[],
    },
    Command { name: "schema.export", kind: CommandKind::Read, summary: "The kinds' and commands' schemas", args: &[] },
];

/// The command a map edit runs as.
pub fn command_of(op: &Op) -> &'static str {
    // Exhaustive on purpose: a new edit does not build until it has a command.
    match op {
        Op::Place { .. } => "props.move",
        Op::Delete { .. } => "props.delete",
        Op::Reset { .. } => "props.reset",
        Op::Add { .. } => "props.add",
        Op::Duplicate { .. } => "props.duplicate",
        Op::DropToGround { .. } => "props.drop",
        Op::RemoveCollision { .. } => "collision.remove",
        Op::DrawWall { .. } => "collision.wall",
        Op::DrawGround { .. } => "collision.ground",
        Op::NewRace { .. } => "race.create",
        Op::DeleteRace { .. } => "race.delete",
        Op::RenameRace { .. }
        | Op::SetRaceId { .. }
        | Op::SetTrack { .. }
        | Op::SetKind { .. }
        | Op::SetLaps { .. }
        | Op::SetDirection { .. } => "race.set",
        Op::AddPoint { .. } => "race.point.add",
        Op::MovePoint { .. } | Op::MoveCheckpoint { .. } => "race.point.move",
        Op::DeletePoint { .. } => "race.point.delete",
        Op::PutRace { .. } => "race.put",
        Op::Details { .. } => "pack.details",
        Op::PutMod { .. } => "pack.put",
        Op::Group { .. } => "edit.group",
    }
}

/// The command with this name.
pub fn command(name: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.name == name)
}

// ---- the schema export --------------------------------------------------------------------------------------------

fn req_word(r: Req) -> &'static str {
    match r {
        Req::Yes | Req::YesAs(_) => "yes",
        Req::OnAdd => "add",
        Req::OnMerge => "merge",
        Req::When(_) => "when",
        Req::No => "no",
    }
}

fn fields_json(fields: &[Field]) -> Json {
    Json::Array(
        fields
            .iter()
            .map(|x| {
                let mut o = Json::obj([
                    ("key", Json::str(x.key)),
                    ("type", Json::str(type_name(&x.ty))),
                    ("required", Json::str(req_word(x.req))),
                    ("merge", Json::Bool(x.merge)),
                ]);
                if !x.label.is_empty() {
                    o.push("label", Json::str(x.label));
                }
                if let Some(words) = enum_words(&x.ty) {
                    o.push("enum", Json::strs(words));
                }
                if let Some(sub) = sub_fields(&x.ty) {
                    o.push("fields", fields_json(sub));
                }
                o
            })
            .collect(),
    )
}

/// The registries as one JSON document (`"format": 1`): the kinds with their keys, fields and rules, the record
/// kinds with their records' fields, and the commands with their kinds and arguments.
pub fn export() -> Json {
    let kinds = KINDS
        .iter()
        .map(|k| {
            Json::obj([
                ("short", Json::str(k.short)),
                ("schema", Json::str(k.schema)),
                ("schema_file", Json::str(k.schema_file)),
                ("keys", Json::obj([("original", Json::str(k.keys[0].form)), ("added", Json::str(k.keys[1].form))])),
                (
                    "ops",
                    Json::strs(
                        [
                            ("merge", true),
                            ("add", k.rules.add),
                            ("replace", k.rules.replace),
                            ("disabled", k.rules.disable),
                        ]
                        .into_iter()
                        .filter(|p| p.1)
                        .map(|p| p.0),
                    ),
                ),
                ("fields", fields_json(k.fields)),
            ])
        })
        .collect();
    let commands = COMMANDS
        .iter()
        .map(|c| {
            Json::obj([
                ("name", Json::str(c.name)),
                ("kind", Json::str(c.kind.word())),
                ("summary", Json::str(c.summary)),
                ("args", fields_json(c.args)),
            ])
        })
        .collect();
    let records = RECORD_KINDS
        .iter()
        .map(|k| {
            let records = k
                .records
                .iter()
                .map(|r| Json::obj([("tag", Json::str(r.tag)), ("fields", fields_json(r.fields))]))
                .collect();
            Json::obj([
                ("short", Json::str(k.short)),
                ("schema", Json::str(k.schema)),
                ("schema_file", Json::str(k.schema_file)),
                ("records", Json::Array(records)),
            ])
        })
        .collect();
    Json::obj([
        ("format", Json::Int(1)),
        ("kinds", Json::Array(kinds)),
        ("records", Json::Array(records)),
        ("commands", Json::Array(commands)),
    ])
}

/// Where the export is kept, beside this crate's manifest.
pub const EXPORT_FILE: &str = "schema/studio.json";

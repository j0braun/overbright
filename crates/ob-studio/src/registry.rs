//! The registries: every kind of content, its fields and every command, declared once. The declarations give the
//! schema export (`ob://schema/commands` and the kinds, [`export`]), and later the tables, inspector rows,
//! command-line verbs and docs; a test keeps each kind's fields in step with its file's schema in the asset format
//! (docs/modding/asset-format/schemas/), and the export in step with `crates/ob-studio/schema/studio.json`.

use ob_asset_format::json::Json;
use ob_asset_format::layering::{KindRules, ListKey};
use ob_map::tools::map::ops::Op;

/// A field's type, as Studio edits and shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Bool,
    /// An unsigned integer of this many bits.
    Uint(u8),
    /// A binary32 float.
    F32,
    /// Two binary32 floats (metres; a checkpoint's x and y).
    Vec2,
    /// Three binary32 floats (metres).
    Vec3,
    /// Nine binary32 floats, row by row (local axes).
    Mat3,
    Text,
    /// One of these words.
    Enum(&'static [&'static str]),
    /// A reference to an item of the kind with this short name (`solid:XO_CONE_01`).
    Ref(&'static str),
    /// A sub-table with its own fields.
    Table(&'static [Field]),
    /// A list of elements of the type.
    List(&'static Ty),
    /// `[extras]`: free content for tools.
    Extras,
}

impl Ty {
    fn name(&self) -> String {
        match self {
            Ty::Bool => "bool".into(),
            Ty::Uint(b) => format!("u{b}"),
            Ty::F32 => "f32".into(),
            Ty::Vec2 => "vec2".into(),
            Ty::Vec3 => "vec3".into(),
            Ty::Mat3 => "mat3".into(),
            Ty::Text => "text".into(),
            Ty::Enum(_) => "enum".into(),
            Ty::Ref(k) => format!("ref {k}"),
            Ty::Table(_) => "table".into(),
            Ty::List(t) => format!("list of {}", t.name()),
            Ty::Extras => "extras".into(),
        }
    }
}

/// When a field must be present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Req {
    /// Always.
    Yes,
    /// In an entry that adds the item.
    OnAdd,
    /// In an entry that merges into an original item.
    OnMerge,
    No,
}

/// One field of a kind's entry (or of a command's arguments).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    /// The file's key.
    pub key: &'static str,
    pub ty: Ty,
    pub req: Req,
    /// A pack may change it on an original item (a `merge`); otherwise only an entry that adds the item sets it.
    pub merge: bool,
    /// The inspector row's label; "" for a field the inspector does not show (keys, guards).
    pub label: &'static str,
}

const fn f(key: &'static str, ty: Ty, req: Req, merge: bool, label: &'static str) -> Field {
    Field { key, ty, req, merge, label }
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
    fn word(self) -> &'static str {
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

const GUARD: &[Field] =
    &[f("solid", Ty::Ref("solid"), Req::Yes, false, ""), f("position", Ty::Vec3, Req::Yes, false, "")];

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
    fields: &[
        f("op", Ty::Enum(&["merge", "add"]), Req::No, false, ""),
        f("key", Ty::Text, Req::OnMerge, false, ""),
        f("guard", Ty::Table(GUARD), Req::OnMerge, false, ""),
        f("id", Ty::Text, Req::OnAdd, false, ""),
        f("scenery", Ty::Uint(32), Req::OnAdd, false, "Scenery section"),
        f("solid", Ty::Ref("solid"), Req::OnAdd, false, "Object"),
        f("lod_b", Ty::Ref("solid"), Req::No, false, "Middle detail"),
        f("lod_c", Ty::Ref("solid"), Req::No, false, "Low detail"),
        f("radius_m", Ty::F32, Req::No, false, "Draw radius"),
        f("flags", Ty::Uint(16), Req::No, false, "Flags"),
        f("prop", Ty::Ref("prop-kind"), Req::No, false, "Knock-over prop"),
        f("position", Ty::Vec3, Req::OnAdd, true, "Position"),
        f("rotation", Ty::Mat3, Req::No, true, "Rotation"),
        f("disabled", Ty::Bool, Req::No, true, "Hidden"),
        f("extras", Ty::Extras, Req::No, false, ""),
    ],
    rules: KindRules { add: true, replace: false, disable: true, keyed_lists: &[] },
};

const SPOT: &[Field] =
    &[f("position", Ty::Vec3, Req::Yes, true, "Position"), f("heading_deg", Ty::F32, Req::Yes, true, "Heading")];

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
    fields: &[
        f("schema", Ty::Text, Req::Yes, false, ""),
        f("op", Ty::Enum(&["add"]), Req::No, false, ""),
        f("disabled", Ty::Bool, Req::No, true, "Off"),
        f("id", Ty::Text, Req::Yes, false, "Id"),
        f("name", Ty::Text, Req::No, true, "Name"),
        f("track", Ty::Uint(16), Req::Yes, true, "Base track"),
        f("backward", Ty::Bool, Req::No, true, "Backward"),
        f("mode", Ty::Enum(&["circuit", "sprint", "streetx", "drift"]), Req::Yes, true, "Mode"),
        f("laps", Ty::Uint(8), Req::No, true, "Laps"),
        f("checkpoints", Ty::List(&Ty::Vec2), Req::No, true, "Checkpoints"),
        f("grid", Ty::List(&Ty::Table(SPOT)), Req::No, true, "Grid"),
        f("start", Ty::Table(SPOT), Req::No, true, "Start line"),
        f("finish", Ty::Table(SPOT), Req::No, true, "Finish line"),
        f("extras", Ty::Extras, Req::No, false, ""),
        f("ext", Ty::Extras, Req::No, false, ""),
    ],
    // A race is its pack's own (`race:<pack>:<id>`): the pack adds it; a pack that depends on it may merge into it
    // or disable it, its grid merged slot by slot.
    rules: KindRules { add: true, replace: false, disable: true, keyed_lists: &[("grid", ListKey::Index)] },
};

/// Every kind Studio edits.
pub const KINDS: &[Kind] = &[PLACEMENT, RACE];

/// The kind with this short name.
pub fn kind(short: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|k| k.short == short)
}

// ---- the commands -------------------------------------------------------------------------------------------------

const KEY: Field = f("key", Ty::Text, Req::Yes, false, "Object");
const RACE_ID: Field = f("race", Ty::Text, Req::Yes, false, "Race");
const AT: Field = f("position", Ty::Vec3, Req::Yes, false, "Position");

/// Every command, by stable name. Each map edit ([`Op`]) has one ([`command_of`]).
pub const COMMANDS: &[Command] = &[
    Command {
        name: "props.move",
        kind: CommandKind::Edit,
        summary: "Move a prop",
        args: &[KEY, AT, f("rotation", Ty::Mat3, Req::Yes, false, "Rotation")],
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
            f("solid", Ty::Ref("solid"), Req::Yes, false, "Object"),
            AT,
            f("id", Ty::Text, Req::No, false, "Id"),
            f("ground", Ty::Bool, Req::No, false, "On the ground"),
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
        name: "race.create",
        kind: CommandKind::Edit,
        summary: "Make a race on a base track",
        args: &[f("track", Ty::Uint(16), Req::Yes, false, "Base track"), f("id", Ty::Text, Req::No, false, "Id")],
    },
    Command { name: "race.delete", kind: CommandKind::Edit, summary: "Delete a race", args: &[RACE_ID] },
    Command {
        name: "race.set",
        kind: CommandKind::Edit,
        summary: "Change a race's name, id, track, mode, laps or direction",
        args: &[RACE_ID],
    },
    Command {
        name: "race.point.add",
        kind: CommandKind::Edit,
        summary: "Add a checkpoint or grid slot, or place the start or finish line",
        args: &[RACE_ID, AT],
    },
    Command { name: "race.point.move", kind: CommandKind::Edit, summary: "Move a race point", args: &[RACE_ID, AT] },
    Command { name: "race.point.delete", kind: CommandKind::Edit, summary: "Delete a race point", args: &[RACE_ID] },
    Command { name: "race.put", kind: CommandKind::Edit, summary: "Put a whole race in place", args: &[RACE_ID] },
    Command {
        name: "pack.details",
        kind: CommandKind::Edit,
        summary: "Change the pack's name, author and description",
        args: &[],
    },
    Command { name: "pack.put", kind: CommandKind::Edit, summary: "Put the whole map in place", args: &[] },
    Command { name: "edit.group", kind: CommandKind::Edit, summary: "Several edits as one undo step", args: &[] },
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
        Req::Yes => "yes",
        Req::OnAdd => "add",
        Req::OnMerge => "merge",
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
                    ("type", Json::str(x.ty.name())),
                    ("required", Json::str(req_word(x.req))),
                    ("merge", Json::Bool(x.merge)),
                ]);
                if !x.label.is_empty() {
                    o.push("label", Json::str(x.label));
                }
                match x.ty {
                    Ty::Enum(words) => o.push("enum", Json::strs(words)),
                    Ty::Table(sub) | Ty::List(&Ty::Table(sub)) => o.push("fields", fields_json(sub)),
                    _ => {}
                }
                o
            })
            .collect(),
    )
}

/// The registries as one JSON document (`"format": 1`): the kinds with their keys, fields and rules, and the
/// commands with their kinds and arguments.
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
    Json::obj([("format", Json::Int(1)), ("kinds", Json::Array(kinds)), ("commands", Json::Array(commands))])
}

/// Where the export is kept, beside this crate's manifest.
pub const EXPORT_FILE: &str = "schema/studio.json";

//! Map edits as data: [`Op`], run by a `MapSession` (the library's `tools::map::session`).
//!
//! Every change the editor makes to a map mod is one `Op` (or a
//! [`Op::Group`] of them under one name): a value that can be stored,
//! sent and replayed, read and written as JSON (one op per line in an
//! `ops.jsonl`). Props are named by [`PropKey`] (what the mod file names
//! them by), never by their index in a loaded world; races by their id.
//!
//! # Stability
//!
//! The shapes of [`Op`], [`PropKey`], [`RacePoint`] and [`Change`], their
//! JSON forms and the undo step names [`Op::label`] gives are fixed: the
//! editor's panels, the console tool and the scripted tests build on
//! them. New ops and new [`Refused`] reasons may be added ([`Refused`] is
//! `#[non_exhaustive]`; [`Op`] is not, so the library's session and its
//! tests, in another crate, must name every op); existing ones keep their
//! fields, meaning and names.
//!
//! JSON: an op is an object tagged by `"op"` (`{"op":"delete","key":
//! {"added":"a3"}}`); vectors are arrays (`[x, y, z]`), rotations their
//! three rows (each a local axis in world coordinates, as in the mod
//! file). Floats round-trip exactly.

use glam::{Mat3, Vec3};
use serde::{Deserialize, Serialize};

use super::format::{CustomRace, MAX_CHECKPOINTS, MAX_GRID, MapMod, PropEdit, PropRef, RaceKind};

/// What a race edit works on (in JSON `checkpoint`, `grid`, `start`,
/// `finish`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RacePart {
    #[default]
    Checkpoint,
    Grid,
    Start,
    Finish,
}

impl RacePart {
    pub const ALL: [RacePart; 4] = [RacePart::Checkpoint, RacePart::Grid, RacePart::Start, RacePart::Finish];

    pub fn name(self) -> &'static str {
        match self {
            RacePart::Checkpoint => "Checkpoints",
            RacePart::Grid => "Start grid",
            RacePart::Start => "Start line",
            RacePart::Finish => "Finish line",
        }
    }
}

/// Why an edit was not made (a prop edit here, any [`Op`]
/// run by a `MapSession` (the library's `tools::map::session`)). New reasons may be added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refused {
    /// No such placement in the world.
    NoPlacement,
    /// The placement was added by another map mod.
    OtherMod,
    /// The placement is not changed by this mod (nothing to reset).
    NotEdited,
    /// The placement was added by this mod (it has no original to go
    /// back to: delete it instead).
    Added,
    /// The region has no solid of that name.
    NoSolid,
    /// The mod has no race of that id.
    NoRace,
    /// The race has no such point.
    NoPoint,
    /// The race holds no more points of that kind.
    Full,
    /// Another race or add of the mod has that id.
    IdTaken,
    /// A value out of range: a position off the map or not a number, an
    /// id or a text the file format does not allow, a lap count.
    BadValue,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Refused::NoPlacement => "No such placement",
            Refused::OtherMod => "This prop belongs to another map mod",
            Refused::NotEdited => "Not changed by this map mod",
            Refused::Added => "Added by this map mod: delete it instead",
            Refused::NoSolid => "The region has no such solid",
            Refused::NoRace => "The map mod has no such race",
            Refused::NoPoint => "The race has no such point",
            Refused::Full => "The race holds no more points of that kind",
            Refused::IdTaken => "That id is already in use",
            Refused::BadValue => "Value out of range",
        })
    }
}

impl std::error::Error for Refused {}

/// A prop as a map mod names it: an original placement by where the
/// original data has it, or one the mod added by its add's id. JSON:
/// `{"original": {"region": .., "section": .., "solid": .., "at": [..],
/// "nth": ..}}` (`nth` left out when 0) or `{"added": "a3"}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropKey {
    Original(PropRef),
    Added(String),
}

/// One point of a race: a checkpoint, a grid slot, the start or the
/// finish line (`index` within the part; 0 for the start and finish).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacePoint {
    pub part: RacePart,
    #[serde(default)]
    pub index: usize,
}

/// One edit of a map mod: one undo step when run (see the module's
/// stability note). Props are named by [`PropKey`], races by id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    // ---- props ----
    /// Put a prop at `position` with `rotation` (rows: local axes): a
    /// move of an original (showing it again when the mod deleted it),
    /// or the new place of an added one. "Move NAME".
    Place {
        key: PropKey,
        #[serde(with = "glam_json::vec3")]
        position: Vec3,
        #[serde(with = "glam_json::mat3")]
        rotation: Mat3,
    },
    /// Hide an original prop, or take an added one out of the mod.
    /// "Delete NAME".
    Delete { key: PropKey },
    /// Put an original prop back as the original has it. "Reset NAME".
    Reset { key: PropKey },
    /// A new prop of the region's solid `solid`; `id` names the add (a
    /// fresh one when left out); `ground`: stand it on the floor under
    /// `position`. "Add SOLID".
    Add {
        solid: String,
        #[serde(with = "glam_json::vec3")]
        position: Vec3,
        #[serde(with = "glam_json::mat3")]
        rotation: Mat3,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        ground: bool,
    },
    /// A new prop of the same solid and rotation as `key` at `position`.
    /// "Duplicate NAME".
    Duplicate {
        key: PropKey,
        #[serde(with = "glam_json::vec3")]
        position: Vec3,
    },
    /// Stand a prop on the floor under it. "Drop to ground NAME".
    DropToGround { key: PropKey },

    // ---- races ----
    /// A new race on base track `track`: id `id` (else `race-<n>`, the
    /// first free), named "Race <n>" (or its id). "New race on track N".
    NewRace {
        track: u16,
        kind: RaceKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    /// "Delete race NAME".
    DeleteRace { race: String },
    /// "Rename race".
    RenameRace { race: String, name: String },
    /// "Change race id".
    SetRaceId { race: String, id: String },
    /// "Base track N".
    SetTrack { race: String, track: u16 },
    /// The race type; the laps follow it (a loop has 2 or more, others
    /// 1). "Mode Sprint".
    SetKind { race: String, kind: RaceKind },
    /// "Laps N".
    SetLaps { race: String, laps: u32 },
    /// "Direction Forward" / "Direction Backward".
    SetDirection { race: String, backward: bool },
    /// A new point: a checkpoint at `index` (the end when left out;
    /// "Add checkpoint N" / "Insert checkpoint N"), a grid slot at
    /// `index` (the end when left out; a race on its default grid gets
    /// that grid first; "Add grid slot N"), or the start or finish line
    /// put at `position` facing `heading` ("Place start line" / "Place
    /// finish line"). A checkpoint keeps only x and y; a grid slot and a
    /// line keep `heading` (degrees around +z from +x).
    AddPoint {
        race: String,
        part: RacePart,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
        #[serde(with = "glam_json::vec3")]
        position: Vec3,
        #[serde(default)]
        heading: f32,
    },
    /// Move a point to `position`; `heading` turns a grid slot or line
    /// (kept when left out). "Move checkpoint N" / "Move grid slot N" /
    /// "Move start line" / "Move finish line".
    MovePoint {
        race: String,
        point: RacePoint,
        #[serde(with = "glam_json::vec3")]
        position: Vec3,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        heading: Option<f32>,
    },
    /// "Delete checkpoint N" / "Delete grid slot N" / "Delete start
    /// line" / "Delete finish line".
    DeletePoint { race: String, point: RacePoint },
    /// Checkpoint `index` one place later or earlier in the route. "Move
    /// checkpoint N down" / "... up".
    MoveCheckpoint { race: String, index: usize, later: bool },
    /// Race `race` replaced as a whole by `to` (which may have another
    /// id). "Edit race NAME".
    PutRace { race: String, to: CustomRace },

    // ---- the mod ----
    /// The mod's name, author and description. "Mod details".
    Details { name: String, author: String, description: String },
    /// The whole mod replaced by `map`. "Replace mod".
    PutMod { map: MapMod },
    /// Several ops as one undo step named `label`: all made, or none
    /// when one is refused.
    Group { label: String, ops: Vec<Op> },
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// What a run op did.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    /// The undo step's name.
    pub label: String,
    /// The mod changed: one undo step was recorded (false: the op made
    /// no difference and no step was recorded).
    pub changed: bool,
    /// The props the op added ([`Op::Add`], [`Op::Duplicate`]), in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<PropKey>,
    /// The race the op made or changed last (its id after the op).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub race: Option<String>,
    /// The race point the op made or moved last (after it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point: Option<RacePoint>,
}

impl Op {
    /// The undo step's name for this op on `map` (as it is before the op
    /// runs). These are the names the editor shows in Edit ▸ Undo.
    pub fn label(&self, map: &MapMod) -> String {
        let race_name =
            |id: &str| map.races.iter().find(|r| r.id == id).map_or_else(|| id.to_string(), |r| r.name.clone());
        match self {
            Op::Place { key, .. } => format!("Move {}", prop_name(map, key)),
            Op::Delete { key } => format!("Delete {}", prop_name(map, key)),
            Op::Reset { key } => format!("Reset {}", prop_name(map, key)),
            Op::Add { solid, .. } => format!("Add {solid}"),
            Op::Duplicate { key, .. } => format!("Duplicate {}", prop_name(map, key)),
            Op::DropToGround { key } => format!("Drop to ground {}", prop_name(map, key)),
            Op::NewRace { track, .. } => format!("New race on track {track}"),
            Op::DeleteRace { race } => format!("Delete race {}", race_name(race)),
            Op::RenameRace { .. } => "Rename race".into(),
            Op::SetRaceId { .. } => "Change race id".into(),
            Op::SetTrack { track, .. } => format!("Base track {track}"),
            Op::SetKind { kind, .. } => format!("Mode {}", kind_title(*kind)),
            Op::SetLaps { laps, .. } => format!("Laps {laps}"),
            Op::SetDirection { backward, .. } => {
                format!("Direction {}", if *backward { "Backward" } else { "Forward" })
            }
            Op::AddPoint { race, part, index, .. } => match part {
                RacePart::Checkpoint => {
                    let n = map.races.iter().find(|r| r.id == *race).map_or(0, |r| r.checkpoints.len());
                    match index {
                        Some(i) if *i < n => format!("Insert checkpoint {}", i + 1),
                        _ => format!("Add checkpoint {}", n + 1),
                    }
                }
                RacePart::Grid => {
                    let n =
                        map.races.iter().find(|r| r.id == *race).map_or(0, |r| super::race::effective_grid(r).len());
                    format!("Add grid slot {}", index.unwrap_or(n).min(n) + 1)
                }
                RacePart::Start => "Place start line".into(),
                RacePart::Finish => "Place finish line".into(),
            },
            Op::MovePoint { point, .. } => format!("Move {}", point_name(*point)),
            Op::DeletePoint { point, .. } => format!("Delete {}", point_name(*point)),
            Op::MoveCheckpoint { index, later, .. } => {
                format!("Move checkpoint {} {}", index + 1, if *later { "down" } else { "up" })
            }
            Op::PutRace { race, .. } => format!("Edit race {}", race_name(race)),
            Op::Details { .. } => "Mod details".into(),
            Op::PutMod { .. } => "Replace mod".into(),
            Op::Group { label, .. } => label.clone(),
        }
    }

    /// The op as one line of JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("an op always serialises")
    }

    /// An op from its JSON.
    pub fn from_json(s: &str) -> Result<Op, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// The ops of an `ops.jsonl` text: one op per line; empty lines and
    /// lines starting with `#` are skipped. The error names the line
    /// (1-based).
    pub fn read_lines(text: &str) -> Result<Vec<Op>, (usize, serde_json::Error)> {
        text.lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            .map(|(n, l)| Op::from_json(l).map_err(|e| (n + 1, e)))
            .collect()
    }
}

/// The name a prop goes by in the undo list: its solid's name.
pub fn prop_name(map: &MapMod, key: &PropKey) -> String {
    match key {
        PropKey::Original(r) => r.solid.clone(),
        PropKey::Added(id) => match map.add_index(id).map(|k| &map.props[k]) {
            Some(PropEdit::Add { solid, .. }) => solid.clone(),
            _ => id.clone(),
        },
    }
}

/// A race point as the undo list and the panels name it, lower case:
/// "checkpoint 3", "grid slot 1", "start line", "finish line".
pub fn point_name(p: RacePoint) -> String {
    match p.part {
        RacePart::Checkpoint => format!("checkpoint {}", p.index + 1),
        RacePart::Grid => format!("grid slot {}", p.index + 1),
        RacePart::Start => "start line".into(),
        RacePart::Finish => "finish line".into(),
    }
}

/// A race type as the panels show it: "Circuit", "Sprint", "StreetX",
/// "Drift".
pub fn kind_title(kind: RaceKind) -> &'static str {
    match kind {
        RaceKind::Circuit => "Circuit",
        RaceKind::Sprint => "Sprint",
        RaceKind::StreetX => "StreetX",
        RaceKind::Drift => "Drift",
    }
}

/// The most points of `part` a race holds.
pub fn max_points(part: RacePart) -> usize {
    match part {
        RacePart::Checkpoint => MAX_CHECKPOINTS,
        RacePart::Grid => MAX_GRID,
        RacePart::Start | RacePart::Finish => 1,
    }
}

/// A position the mod file can hold: finite and on the map.
pub fn valid_position(v: Vec3) -> bool {
    v.is_finite() && v.abs().max_element() <= super::format::MAX_COORD
}

/// Refused unless `ok`.
pub fn ensure(ok: bool, why: Refused) -> Result<(), Refused> {
    if ok { Ok(()) } else { Err(why) }
}

/// serde forms of glam's vectors and matrices for the map's JSON:
/// `[x, y]`, `[x, y, z]`, and a rotation as its three rows.
pub mod glam_json {
    use glam::{Mat3, Vec2, Vec3};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub mod vec3 {
        use super::*;
        pub fn serialize<S: Serializer>(v: &Vec3, s: S) -> Result<S::Ok, S::Error> {
            v.to_array().serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec3, D::Error> {
            <[f32; 3]>::deserialize(d).map(Vec3::from_array)
        }
    }

    pub mod mat3 {
        use super::*;
        pub fn serialize<S: Serializer>(m: &Mat3, s: S) -> Result<S::Ok, S::Error> {
            [m.row(0).to_array(), m.row(1).to_array(), m.row(2).to_array()].serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Mat3, D::Error> {
            <[[f32; 3]; 3]>::deserialize(d).map(|rows| Mat3::from_cols_array_2d(&rows).transpose())
        }
    }

    pub mod vec2s {
        use super::*;
        pub fn serialize<S: Serializer>(v: &[Vec2], s: S) -> Result<S::Ok, S::Error> {
            s.collect_seq(v.iter().map(|p| p.to_array()))
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec2>, D::Error> {
            <Vec<[f32; 2]>>::deserialize(d).map(|v| v.into_iter().map(Vec2::from_array).collect())
        }
    }
}

//! The Data grid's table (Studio's Data workspace, docs/modding/studio.md "Data"): one kind of record at a time as
//! rows and columns. The columns come from the kind's schema ([`crate::registry`]: the same fields the file check and
//! the inspector use), the rows from the Changes view's own rows ([`changes::Row`]), so a mod's value is an overlay on
//! the original's and each cell says whether it is yours, the original's or another mod's.
//!
//! * [`Grid`]: the rows of one kind, with sort, a search over every cell and a filter per column.
//! * [`op_for`]: the edit of one cell as the same [`Op`] the editor, the command line and agents run, so it is one
//!   undo step in the mod's history ([`edit`], [`edit_many`]).
//!
//! A column the format cannot write yet (a race's checkpoints and grid, a prop's object) is read-only.

use std::cmp::Ordering;

use glam::{Mat3, Vec3};
use ob_map::tools::map::format::{MapMod, PropEdit, RaceKind};
use ob_map::tools::map::ops::{Change, Op, PropKey};

use crate::changes::map::{PLACEMENT, RACE, added_key, placement_key};
use crate::changes::{From, Row, Value};
use crate::registry;
use crate::session::MapSession;

/// The kinds the grid shows (the Changes view's kinds that Studio can edit).
pub const KINDS: [&str; 2] = [PLACEMENT, RACE];

/// A column's type: how its cells read, sort and are typed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ty {
    Text,
    Int {
        min: i64,
        max: i64,
    },
    Bool,
    Enum(&'static [&'static str]),
    /// A point: x, y, z.
    Vec3,
    /// An angle in degrees.
    Degrees,
    /// A list shown by its length (never edited here).
    Count(&'static str),
}

/// One column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    /// The value's field in a Changes row (`type`, `heading`).
    pub field: &'static str,
    /// The schema key it stands for (`mode`, `rotation`): its label is the schema's.
    pub schema: &'static str,
    pub label: &'static str,
    pub ty: Ty,
    /// A cell of it can be edited here (the format writes it).
    pub editable: bool,
}

/// (row field, schema key, type, editable)
type Spec = (&'static str, &'static str, Ty, bool);

const PLACEMENT_COLUMNS: [Spec; 4] = [
    ("solid", "solid", Ty::Text, false),
    ("position", "position", Ty::Vec3, true),
    ("heading", "rotation", Ty::Degrees, true),
    ("disabled", "disabled", Ty::Bool, true),
];

fn race_columns() -> [Spec; 6] {
    [
        ("name", "name", Ty::Text, true),
        ("type", "mode", Ty::Enum(registry::enum_words(&mode_ty()).unwrap_or(&[])), true),
        ("track", "track", Ty::Int { min: 0, max: 65535 }, true),
        ("laps", "laps", Ty::Int { min: 1, max: 20 }, true),
        ("backward", "backward", Ty::Bool, true),
        ("checkpoints", "checkpoints", Ty::Count("checkpoint"), false),
    ]
}

fn mode_ty() -> ob_asset_format::fields::Ty {
    ob_asset_format::kinds::RACE
        .iter()
        .find(|f| f.key == "mode")
        .map_or(ob_asset_format::fields::Ty::Enum(&[]), |f| f.ty.clone())
}

/// The columns of `kind`, their labels from the kind's schema; empty for a kind the grid does not show.
pub fn columns(kind: &str) -> Vec<Column> {
    let specs: Vec<Spec> = match kind {
        PLACEMENT => PLACEMENT_COLUMNS.to_vec(),
        RACE => race_columns().to_vec(),
        _ => return Vec::new(),
    };
    let Some(k) = registry::kind(if kind == PLACEMENT { "placement" } else { "race" }) else { return Vec::new() };
    specs
        .into_iter()
        .map(|(field, schema, ty, editable)| Column {
            field,
            schema,
            label: k.fields.iter().find(|f| f.key == schema).map_or("", |f| f.label),
            ty,
            editable,
        })
        .collect()
}

/// A number as a cell writes it: up to three decimals, none left over.
fn num(x: f64) -> String {
    let s = format!("{x:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A value as its column reads it (a cell's text and what editing starts from).
pub fn text_of(ty: Ty, v: &toml::Value) -> String {
    use toml::Value as V;
    match (ty, v) {
        (Ty::Bool, V::Boolean(b)) => (if *b { "yes" } else { "no" }).into(),
        (Ty::Count(w), V::Array(a)) => format!("{} {w}{}", a.len(), if a.len() == 1 { "" } else { "s" }),
        (_, V::Float(f)) => num(*f),
        (_, V::Integer(i)) => i.to_string(),
        (_, V::String(s)) => s.clone(),
        (_, V::Boolean(b)) => b.to_string(),
        (_, V::Array(a)) => a
            .iter()
            .map(|x| match x {
                V::Float(f) => num(*f),
                V::Integer(i) => i.to_string(),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

/// One cell: the value the game uses, and where it comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// The game's value as text ("" when the row has none).
    pub text: String,
    pub from: From,
    /// This mod gives the value (an overlay on the original).
    pub yours: bool,
    /// The original game's value, when it is known and differs.
    pub original: Option<String>,
}

/// What a sort compares: numbers by value, text without case; a cell with no value last.
#[derive(Clone, Debug, PartialEq, PartialOrd)]
enum SortKey {
    Nums(Vec<f64>),
    Text(String),
    None,
}

fn sort_key(ty: Ty, v: Option<&toml::Value>) -> SortKey {
    use toml::Value as V;
    match v {
        None => SortKey::None,
        Some(V::Float(f)) => SortKey::Nums(vec![*f]),
        Some(V::Integer(i)) => SortKey::Nums(vec![*i as f64]),
        Some(V::Boolean(b)) => SortKey::Nums(vec![f64::from(u8::from(*b))]),
        Some(V::Array(a)) if matches!(ty, Ty::Count(_)) => SortKey::Nums(vec![a.len() as f64]),
        Some(V::Array(a)) => {
            SortKey::Nums(a.iter().filter_map(|x| x.as_float().or(x.as_integer().map(|i| i as f64))).collect())
        }
        Some(other) => SortKey::Text(text_of(ty, other).to_lowercase()),
    }
}

/// How cells compare when a column is sorted.
fn compare(a: &SortKey, b: &SortKey) -> Ordering {
    match (a, b) {
        (SortKey::None, SortKey::None) => Ordering::Equal,
        (SortKey::None, _) => Ordering::Greater,
        (_, SortKey::None) => Ordering::Less,
        _ => a.partial_cmp(b).unwrap_or(Ordering::Equal),
    }
}

/// One kind's rows as a table: the columns, the rows, and the order and the rows kept by the sort, the search and
/// the column filters.
#[derive(Clone, Debug)]
pub struct Grid {
    pub kind: &'static str,
    pub columns: Vec<Column>,
    rows: Vec<Row>,
    /// The sorted column and whether it runs up (smallest first).
    pub sort: Option<(usize, bool)>,
    /// Kept: rows where any cell has these words (case does not matter).
    pub search: String,
    /// Kept: rows whose cell in the column has the text.
    pub filters: Vec<(usize, String)>,
    /// The rows shown, as indexes into the rows.
    view: Vec<usize>,
}

impl Grid {
    /// The grid of `kind` over Changes rows (rows of other kinds are left out). None: a kind the grid does not show.
    pub fn from_rows(kind: &str, rows: &[Row]) -> Option<Grid> {
        let kind: &'static str = *KINDS.iter().find(|k| **k == kind)?;
        let mut g = Grid {
            kind,
            columns: columns(kind),
            rows: rows.iter().filter(|r| r.kind == kind).cloned().collect(),
            sort: None,
            search: String::new(),
            filters: Vec::new(),
            view: Vec::new(),
        };
        g.refresh();
        Some(g)
    }

    /// The grid of `kind` over one mod alone.
    pub fn build(map: &MapMod, pack: &str, kind: &str) -> Option<Grid> {
        let game = crate::changes::map::Game::new(&[(pack.to_string(), map.clone())]);
        Grid::from_rows(kind, &game.this_mod(pack))
    }

    /// How many rows the kind has, and how many the search and filters keep.
    pub fn counts(&self) -> (usize, usize) {
        (self.rows.len(), self.view.len())
    }

    /// The row at `i` of what is shown.
    pub fn row(&self, i: usize) -> Option<&Row> {
        self.view.get(i).map(|r| &self.rows[*r])
    }

    /// The key of the row at `i` of what is shown.
    pub fn key(&self, i: usize) -> Option<&str> {
        self.row(i).map(|r| r.key.as_str())
    }

    fn value<'a>(&'a self, r: &'a Row, col: usize) -> Option<&'a Value> {
        let f = self.columns.get(col)?.field;
        r.values.iter().find(|v| v.field == f)
    }

    /// The cell of shown row `i`, column `col`.
    pub fn cell(&self, i: usize, col: usize) -> Option<Cell> {
        let c = self.columns.get(col)?;
        let v = self.value(self.row(i)?, col);
        Some(match v {
            None => Cell { text: String::new(), from: From::Original, yours: false, original: None },
            Some(v) => {
                let shown = v.game.as_ref().or(v.yours.as_ref());
                let text = shown.map_or(String::new(), |x| text_of(c.ty, x));
                let original = v.original.as_ref().map(|o| text_of(c.ty, o)).filter(|o| *o != text);
                Cell { text, from: v.from.clone(), yours: v.yours.is_some(), original }
            }
        })
    }

    fn text_at(&self, row: usize, col: usize) -> String {
        let r = &self.rows[row];
        self.value(r, col)
            .and_then(|v| v.game.as_ref().or(v.yours.as_ref()))
            .map_or(String::new(), |x| text_of(self.columns[col].ty, x))
    }

    /// Sort by column `col`; the same column again turns it round, then (descending) off.
    pub fn sort_by(&mut self, col: usize) {
        self.sort = match self.sort {
            Some((c, true)) if c == col => Some((col, false)),
            Some((c, false)) if c == col => None,
            _ => Some((col, true)),
        };
        self.refresh();
    }

    pub fn set_search(&mut self, text: &str) {
        self.search = text.to_string();
        self.refresh();
    }

    /// Keep the rows whose cell in `col` has `text` (empty: no filter on it).
    pub fn set_filter(&mut self, col: usize, text: &str) {
        self.filters.retain(|(c, _)| *c != col);
        if !text.trim().is_empty() {
            self.filters.push((col, text.to_string()));
        }
        self.refresh();
    }

    /// Replace the rows (after an edit or an undo), keeping the sort, the search and the filters.
    pub fn reload(&mut self, rows: &[Row]) {
        self.rows = rows.iter().filter(|r| r.kind == self.kind).cloned().collect();
        self.refresh();
    }

    fn refresh(&mut self) {
        let words: Vec<String> = self.search.split_whitespace().map(str::to_lowercase).collect();
        let mut view: Vec<usize> = (0..self.rows.len())
            .filter(|&i| {
                let cells: Vec<String> = (0..self.columns.len()).map(|c| self.text_at(i, c).to_lowercase()).collect();
                words.iter().all(|w| cells.iter().any(|c| c.contains(w) || self.rows[i].key.to_lowercase().contains(w)))
                    && self
                        .filters
                        .iter()
                        .all(|(c, t)| cells.get(*c).is_some_and(|x| x.contains(&t.trim().to_lowercase())))
            })
            .collect();
        if let Some((col, up)) = self.sort {
            let ty = self.columns[col].ty;
            let keys: Vec<SortKey> = (0..self.rows.len())
                .map(|i| {
                    sort_key(ty, self.value(&self.rows[i], col).and_then(|v| v.game.as_ref().or(v.yours.as_ref())))
                })
                .collect();
            view.sort_by(|a, b| {
                let o = compare(&keys[*a], &keys[*b]);
                // A cell with no value stays last either way.
                if matches!((&keys[*a], &keys[*b]), (SortKey::None, _) | (_, SortKey::None)) || up {
                    o
                } else {
                    o.reverse()
                }
            });
        }
        self.view = view;
    }
}

// ---- edits ---------------------------------------------------------------------------------------------------------

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.trim().to_lowercase().as_str() {
        "yes" | "true" | "1" | "on" | "hidden" => Ok(true),
        "no" | "false" | "0" | "off" | "shown" => Ok(false),
        _ => Err(format!("\"{s}\" is not yes or no")),
    }
}

fn parse_int(s: &str, min: i64, max: i64) -> Result<i64, String> {
    let n: i64 = s.trim().parse().map_err(|_| format!("\"{s}\" is not a whole number"))?;
    if n < min || n > max { Err(format!("{n} is out of range ({min} to {max})")) } else { Ok(n) }
}

pub(crate) fn parse_vec3(s: &str) -> Result<Vec3, String> {
    let v: Vec<f32> = s
        .split([',', ' ', ';'])
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<f32>().map_err(|_| format!("\"{p}\" is not a number")))
        .collect::<Result<_, _>>()?;
    match v[..] {
        [x, y, z] => Ok(Vec3::new(x, y, z)),
        _ => Err("a position is three numbers: x, y, z".into()),
    }
}

/// The edit rotation of a placement: its rows are the local axes, turned about +z.
fn turn(r: Mat3, degrees: f32) -> Mat3 {
    let (s, c) = degrees.to_radians().sin_cos();
    let rot = |v: Vec3| Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z);
    Mat3::from_cols(rot(r.row(0)), rot(r.row(1)), rot(r.row(2))).transpose()
}

/// The prop the row `key` names: an original placement of the mod's edits, or one it added.
fn prop_key(map: &MapMod, pack: &str, key: &str) -> Option<(PropKey, Option<Mat3>)> {
    map.props.iter().find_map(|p| match p {
        PropEdit::Move { target, rotation, .. } if placement_key(target) == key => {
            Some((PropKey::Original(target.clone()), Some(*rotation)))
        }
        PropEdit::Delete { target } if placement_key(target) == key => Some((PropKey::Original(target.clone()), None)),
        PropEdit::Add { id, region, rotation, .. } if added_key(region, pack, id) == key => {
            Some((PropKey::Added(id.clone()), Some(*rotation)))
        }
        _ => None,
    })
}

/// The position of a placement as the mod gives it.
fn position_of(map: &MapMod, pack: &str, key: &str) -> Option<Vec3> {
    map.props.iter().find_map(|p| match p {
        PropEdit::Move { target, position, .. } if placement_key(target) == key => Some(*position),
        PropEdit::Add { id, region, position, .. } if added_key(region, pack, id) == key => Some(*position),
        _ => None,
    })
}

/// The op that writes `text` in the cell (`field` of row `key` of `kind`) of the mod `map` (named `pack`). The
/// words an error holds are the cell's.
pub fn op_for(map: &MapMod, pack: &str, kind: &str, key: &str, field: &str, text: &str) -> Result<Op, String> {
    op_for_in(map, pack, kind, key, field, text, None)
}

/// [`op_for`] when the same step also moves the prop to `moved_to` (a heading edit then turns it where it will be).
fn op_for_in(
    map: &MapMod,
    pack: &str,
    kind: &str,
    key: &str,
    field: &str,
    text: &str,
    moved_to: Option<Vec3>,
) -> Result<Op, String> {
    let col = columns(kind).into_iter().find(|c| c.field == field).ok_or_else(|| format!("No column {field}"))?;
    if !col.editable {
        return Err(format!("{} cannot be changed here yet", col.label));
    }
    match kind {
        RACE => {
            let race = key.strip_prefix(&format!("{pack}:")).unwrap_or(key).to_string();
            if !map.races.iter().any(|r| r.id == race) {
                return Err("This race is another mod's: change it there".into());
            }
            Ok(match (field, col.ty) {
                ("name", _) => Op::RenameRace { race, name: text.to_string() },
                ("type", _) => Op::SetKind {
                    race,
                    kind: RaceKind::parse(&text.trim().to_lowercase())
                        .ok_or_else(|| "Race type is circuit, sprint, streetx or drift".to_string())?,
                },
                ("track", Ty::Int { min, max }) => Op::SetTrack { race, track: parse_int(text, min, max)? as u16 },
                ("laps", Ty::Int { min, max }) => Op::SetLaps { race, laps: parse_int(text, min, max)? as u32 },
                ("backward", _) => Op::SetDirection { race, backward: parse_bool(text)? },
                _ => return Err(format!("{} cannot be changed here yet", col.label)),
            })
        }
        PLACEMENT => {
            let (pk, rotation) =
                prop_key(map, pack, key).ok_or_else(|| "This prop is not changed by this mod".to_string())?;
            match field {
                "disabled" => Ok(if parse_bool(text)? { Op::Delete { key: pk } } else { Op::Reset { key: pk } }),
                "position" => {
                    let rotation = rotation.ok_or("Its heading is in the game's files: show it first (Hidden: no)")?;
                    Ok(Op::Place { key: pk, position: parse_vec3(text)?, rotation })
                }
                "heading" => {
                    let rotation = rotation.ok_or("Its heading is in the game's files: show it first (Hidden: no)")?;
                    let position = moved_to
                        .or_else(|| position_of(map, pack, key))
                        .ok_or("Its position is in the game's files")?;
                    let now = crate::changes::map::heading(rotation) as f32;
                    let to: f32 = text.trim().parse().map_err(|_| format!("\"{text}\" is not a number of degrees"))?;
                    Ok(Op::Place { key: pk, position, rotation: turn(rotation, to - now) })
                }
                _ => Err(format!("{} cannot be changed here yet", col.label)),
            }
        }
        _ => Err(format!("The grid has no {kind} rows")),
    }
}

/// One cell edited as one undo step in `session` (its mod `pack`). The step's name is the op's ("Laps 4").
pub fn edit(
    session: &mut MapSession,
    pack: &str,
    kind: &str,
    key: &str,
    field: &str,
    text: &str,
) -> Result<Change, String> {
    let op = op_for(&session.map, pack, kind, key, field, text)?;
    session.run(op).map_err(|e| session.refused_why.take().unwrap_or_else(|| e.to_string()))
}

/// One column filled with values in several rows as one undo step named `label`: all or none. (Each cell is an
/// independent op on its row; two columns of one row are two edits.)
pub fn edit_many(
    session: &mut MapSession,
    pack: &str,
    kind: &str,
    field: &str,
    cells: &[(String, String)],
    label: &str,
) -> Result<Change, String> {
    let ops = cells
        .iter()
        .map(|(key, text)| op_for(&session.map, pack, kind, key, field, text))
        .collect::<Result<Vec<_>, _>>()?;
    session
        .run(Op::Group { label: label.to_string(), ops })
        .map_err(|e| session.refused_why.take().unwrap_or_else(|| e.to_string()))
}

mod exchange;
pub use exchange::{CellEdit, export_file, read_file, Refusal, Span, csv_read, csv_write, edit_cells, plan, refusals_sentence};

#[cfg(test)]
mod tests;

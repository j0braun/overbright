//! The map mod file (`map.obmap`): a versioned, line-based text format
//! (docs/modding/map-editor.md, "File format").
//!
//! ```text
//! overbright-map 1
//! name = Docks
//! [prop.move]
//! region = L4RA
//! section = 1407
//! solid = XO_CONE_01
//! at = 10 20 3
//! nth = 1
//! position = 11 20 3
//! rotation = 1 0 0 0 1 0 0 0 1
//! [race]
//! id = docks-sprint
//! track = 4001
//! mode = sprint
//! checkpoint = 100 200
//! checkpoint = 300 400
//! ```
//!
//! The first line names the format and its version; a file of a newer
//! version, an unknown section or key, a missing key, a number that does
//! not parse or lies out of range is refused with its line number, and
//! nothing of the file is used. Numbers are written in Rust's shortest
//! round-trip form, so writing and reading again gives the same values.
//!
//! Two keys are written only when they differ from their default, so a
//! file without them reads and writes byte for byte as before: `nth` on a
//! move or delete (which of several original placements at exactly the
//! same place it names, default 0) and `id` on an add (its name within
//! the mod, default `a<n>` for the mod's n-th add).

use glam::{Mat3, Vec2, Vec3};
use serde::{Deserialize, Serialize};

use super::ops::glam_json;

/// The first word of every map mod file.
pub const MAGIC: &str = "overbright-map";
/// The version this build writes and the newest it reads.
pub const VERSION: u32 = 1;
/// A larger file is refused before it is read.
pub const MAX_FILE_BYTES: u64 = 8 << 20;
/// At most this many prop edits and races in one mod.
pub const MAX_PROPS: usize = 50_000;
pub const MAX_RACES: usize = 256;
/// A race's checkpoints: at most this many (2 or more to be raced).
pub const MAX_CHECKPOINTS: usize = 64;
/// A race's start grid holds at most 8 slots (two rows of four, as the
/// game's markers).
pub const MAX_GRID: usize = 8;
pub const MAX_LAPS: u32 = 20;
/// Coordinates further from the origin than this are refused (the game's
/// regions lie within a few kilometres).
pub const MAX_COORD: f32 = 100_000.0;
/// A name, author, description or race name: at most this many characters.
pub const MAX_TEXT: usize = 200;

/// A whole map mod. Its JSON form (serde, [`super::ops`]) has the same
/// fields; vectors are arrays and rotations their three rows.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapMod {
    pub name: String,
    pub author: String,
    pub description: String,
    pub props: Vec<PropEdit>,
    pub races: Vec<CustomRace>,
}

/// An original placement, named by where the original data has it: its
/// region, its streaming section, its solid's name and its position, and
/// `nth` among the original placements of that section and solid at
/// exactly that position (in the original's order; 0 for the first or
/// only one).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropRef {
    pub region: String,
    pub section: u32,
    pub solid: String,
    #[serde(with = "glam_json::vec3")]
    pub at: Vec3,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub nth: u32,
}

/// One change to a region's scenery.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "snake_case", deny_unknown_fields)]
pub enum PropEdit {
    /// An original placement put somewhere else (`rotation`'s rows are
    /// its local x, y and z axes in world coordinates, the game's Z-up
    /// row-vector convention).
    Move {
        target: PropRef,
        #[serde(with = "glam_json::vec3")]
        position: Vec3,
        #[serde(with = "glam_json::mat3")]
        rotation: Mat3,
    },
    /// An original placement hidden.
    Delete { target: PropRef },
    /// A new placement of a solid the region has loaded; `id` names it
    /// within the mod (unique, kept through save and load).
    Add {
        id: String,
        region: String,
        solid: String,
        #[serde(with = "glam_json::vec3")]
        position: Vec3,
        #[serde(with = "glam_json::mat3")]
        rotation: Mat3,
    },
}

impl PropEdit {
    pub fn region(&self) -> &str {
        match self {
            PropEdit::Move { target, .. } | PropEdit::Delete { target } => &target.region,
            PropEdit::Add { region, .. } => region,
        }
    }

    /// The original placement it changes (None for an added one).
    pub fn target(&self) -> Option<&PropRef> {
        match self {
            PropEdit::Move { target, .. } | PropEdit::Delete { target } => Some(target),
            PropEdit::Add { .. } => None,
        }
    }

    /// An added placement's id (None for a move or delete).
    pub fn add_id(&self) -> Option<&str> {
        match self {
            PropEdit::Add { id, .. } => Some(id),
            _ => None,
        }
    }
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// The id the mod's `n`-th add (1-based) has when the file gives none.
pub fn default_add_id(n: usize) -> String {
    format!("a{n}")
}

/// At most this `nth` (no original has more placements at one place).
pub const MAX_NTH: u32 = 65_535;

/// The race types a custom race can be (in JSON by [`RaceKind::name`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RaceKind {
    Circuit,
    Sprint,
    StreetX,
    Drift,
}

impl RaceKind {
    pub const ALL: [RaceKind; 4] = [RaceKind::Circuit, RaceKind::Sprint, RaceKind::StreetX, RaceKind::Drift];

    pub fn name(self) -> &'static str {
        match self {
            RaceKind::Circuit => "circuit",
            RaceKind::Sprint => "sprint",
            RaceKind::StreetX => "streetx",
            RaceKind::Drift => "drift",
        }
    }

    pub fn parse(s: &str) -> Option<RaceKind> {
        RaceKind::ALL.into_iter().find(|k| k.name() == s)
    }

    /// Whether the race runs laps on a closed route.
    pub fn loops(self) -> bool {
        matches!(self, RaceKind::Circuit | RaceKind::StreetX)
    }
}

/// A marker: a position and a heading (degrees around +z from +x, the
/// game's convention).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spot {
    #[serde(with = "glam_json::vec3")]
    pub position: Vec3,
    pub heading: f32,
}

/// A race of the player's own on a base track's road network.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomRace {
    /// Unique within the mod: `a-z`, `0-9`, `-`, `_`.
    pub id: String,
    pub name: String,
    /// The base track: its region, road network (Paths<track>.bin), race
    /// barriers, AI lanes and traffic.
    pub track: u16,
    /// The base track's backward barriers and lanes.
    pub backward: bool,
    pub kind: RaceKind,
    pub laps: u32,
    /// The checkpoints in driving order (world x, y). A circuit closes the
    /// loop from the last back to the first.
    #[serde(with = "glam_json::vec2s")]
    pub checkpoints: Vec<Vec2>,
    /// The start grid, pole first.
    pub grid: Vec<Spot>,
    pub start: Option<Spot>,
    pub finish: Option<Spot>,
}

impl CustomRace {
    pub fn new(id: &str, track: u16, kind: RaceKind) -> CustomRace {
        CustomRace {
            id: id.into(),
            name: id.into(),
            track,
            backward: false,
            kind,
            laps: if kind.loops() { 2 } else { 1 },
            checkpoints: Vec::new(),
            grid: Vec::new(),
            start: None,
            finish: None,
        }
    }
}

/// Why a file was refused: the line (1-based, 0 for the file as a whole)
/// and the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            0 => write!(f, "{}", self.message),
            n => write!(f, "line {n}: {}", self.message),
        }
    }
}

impl std::error::Error for FormatError {}

fn err<T>(line: usize, message: impl Into<String>) -> Result<T, FormatError> {
    Err(FormatError { line, message: message.into() })
}

/// One `[section]` being read: its keys in file order.
struct Block {
    kind: String,
    line: usize,
    keys: Vec<(String, String, usize)>,
}

impl Block {
    fn one(&self, key: &str) -> Result<Option<(&str, usize)>, FormatError> {
        let mut found = self.keys.iter().filter(|(k, ..)| k == key);
        let first = found.next();
        if let Some((_, _, line)) = found.next() {
            return err(*line, format!("`{key}` given twice in [{}]", self.kind));
        }
        Ok(first.map(|(_, v, l)| (v.as_str(), *l)))
    }

    fn need(&self, key: &str) -> Result<(&str, usize), FormatError> {
        self.one(key)?.ok_or_else(|| FormatError { line: self.line, message: format!("[{}] needs `{key}`", self.kind) })
    }

    fn all(&self, key: &str) -> impl Iterator<Item = (&str, usize)> {
        self.keys.iter().filter(move |(k, ..)| k == key).map(|(_, v, l)| (v.as_str(), *l))
    }

    fn check_keys(&self, allowed: &[&str]) -> Result<(), FormatError> {
        match self.keys.iter().find(|(k, ..)| !allowed.contains(&k.as_str())) {
            Some((k, _, line)) => err(*line, format!("unknown key `{k}` in [{}]", self.kind)),
            None => Ok(()),
        }
    }
}

fn floats<const N: usize>(v: &str, line: usize) -> Result<[f32; N], FormatError> {
    let parts: Vec<&str> = v.split_whitespace().collect();
    if parts.len() != N {
        return err(line, format!("expected {N} numbers, found {}", parts.len()));
    }
    let mut out = [0.0; N];
    for (o, p) in out.iter_mut().zip(parts) {
        let x: f32 = p.parse().map_err(|_| FormatError { line, message: format!("`{p}` is not a number") })?;
        if !x.is_finite() || x.abs() > MAX_COORD {
            return err(line, format!("`{p}` is out of range"));
        }
        *o = x;
    }
    Ok(out)
}

fn vec3(v: &str, line: usize) -> Result<Vec3, FormatError> {
    floats::<3>(v, line).map(Vec3::from_array)
}

fn rotation(v: &str, line: usize) -> Result<Mat3, FormatError> {
    let r = floats::<9>(v, line)?;
    let m = Mat3::from_cols_array(&r).transpose();
    // Each row a unit axis (a scaled or degenerate rotation is refused;
    // mirrored ones, as the game's race-only barriers have, are kept).
    for row in [m.row(0), m.row(1), m.row(2)] {
        if (row.length() - 1.0).abs() > 1e-2 {
            return err(line, "rotation rows must be unit axes");
        }
    }
    if m.determinant().abs() < 0.9 {
        return err(line, "rotation is degenerate");
    }
    Ok(m)
}

fn spot(v: &str, line: usize) -> Result<Spot, FormatError> {
    let [x, y, z, h] = floats::<4>(v, line)?;
    Ok(Spot { position: Vec3::new(x, y, z), heading: h })
}

fn yes_no(v: &str, line: usize) -> Result<bool, FormatError> {
    match v {
        "yes" | "true" => Ok(true),
        "no" | "false" => Ok(false),
        _ => err(line, format!("`{v}` is not yes or no")),
    }
}

fn text(v: &str, line: usize) -> Result<String, FormatError> {
    if v.chars().count() > MAX_TEXT {
        return err(line, format!("text longer than {MAX_TEXT} characters"));
    }
    Ok(v.to_string())
}

/// A region name as the game's files have it (`L4RA`).
pub fn valid_region(s: &str) -> bool {
    (2..=8).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// A solid name: printable ASCII without spaces.
pub fn valid_solid(s: &str) -> bool {
    (1..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_graphic())
}

/// A race id or a mod's folder name: `a-z`, `0-9`, `-`, `_`, 1–40 long.
pub fn valid_id(s: &str) -> bool {
    (1..=40).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

fn region(v: &str, line: usize) -> Result<String, FormatError> {
    if !valid_region(v) {
        return err(line, format!("`{v}` is not a region name"));
    }
    Ok(v.to_string())
}

fn solid(v: &str, line: usize) -> Result<String, FormatError> {
    if !valid_solid(v) {
        return err(line, format!("`{v}` is not a solid name"));
    }
    Ok(v.to_string())
}

fn prop_ref(b: &Block) -> Result<PropRef, FormatError> {
    let (r, rl) = b.need("region")?;
    let (s, sl) = b.need("section")?;
    let (so, sol) = b.need("solid")?;
    let (at, atl) = b.need("at")?;
    let nth = match b.one("nth")? {
        Some((v, l)) => v
            .parse()
            .ok()
            .filter(|n| *n <= MAX_NTH)
            .ok_or_else(|| FormatError { line: l, message: format!("`{v}` is not a count (0 to {MAX_NTH})") })?,
        None => 0,
    };
    Ok(PropRef {
        region: region(r, rl)?,
        section: s.parse().map_err(|_| FormatError { line: sl, message: format!("`{s}` is not a section number") })?,
        solid: solid(so, sol)?,
        at: vec3(at, atl)?,
        nth,
    })
}

impl MapMod {
    /// The index in `props` of the add named `id`.
    pub fn add_index(&self, id: &str) -> Option<usize> {
        self.props.iter().position(|e| e.add_id() == Some(id))
    }

    /// An id no add of this mod has: the default of the next add when it
    /// is free (so it is not written), else one above the largest in use.
    pub fn fresh_add_id(&self) -> String {
        let ids: std::collections::HashSet<&str> = self.props.iter().filter_map(PropEdit::add_id).collect();
        let next = default_add_id(ids.len() + 1);
        if !ids.contains(next.as_str()) {
            return next;
        }
        let top = ids.iter().filter_map(|i| i.strip_prefix('a')?.parse::<usize>().ok()).max().unwrap_or(0);
        (top + 1..).map(default_add_id).find(|i| !ids.contains(i.as_str())).unwrap_or(next)
    }

    /// Read a map mod file's text; anything wrong refuses the whole file.
    pub fn parse(src: &str) -> Result<MapMod, FormatError> {
        let mut lines = src.lines().enumerate().map(|(i, l)| (i + 1, l.trim()));
        let head = lines.by_ref().find(|(_, l)| !l.is_empty() && !l.starts_with('#'));
        let Some((hl, head)) = head else { return err(0, "the file is empty") };
        let mut words = head.split_whitespace();
        if words.next() != Some(MAGIC) {
            return err(hl, format!("not a map mod (the first line must be `{MAGIC} {VERSION}`)"));
        }
        let version: u32 = match words.next().and_then(|v| v.parse().ok()) {
            Some(v) => v,
            None => return err(hl, "the format version is missing"),
        };
        if version == 0 || version > VERSION {
            return err(hl, format!("format version {version} is newer than this Overbright reads ({VERSION})"));
        }
        if words.next().is_some() {
            return err(hl, "unexpected text after the version");
        }
        let mut head_block = Block { kind: "header".into(), line: hl, keys: Vec::new() };
        let mut blocks: Vec<Block> = Vec::new();
        for (n, l) in lines {
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            if let Some(kind) = l.strip_prefix('[') {
                let Some(kind) = kind.strip_suffix(']') else { return err(n, "a section name must end with `]`") };
                if !["prop.move", "prop.delete", "prop.add", "race"].contains(&kind) {
                    return err(n, format!("unknown section [{kind}]"));
                }
                blocks.push(Block { kind: kind.into(), line: n, keys: Vec::new() });
                continue;
            }
            let Some((k, v)) = l.split_once('=') else { return err(n, "expected `key = value`") };
            let (k, v) = (k.trim(), v.trim());
            if k.is_empty() {
                return err(n, "a key is missing before `=`");
            }
            blocks.last_mut().unwrap_or(&mut head_block).keys.push((k.into(), v.into(), n));
        }
        head_block.check_keys(&["name", "author", "description"])?;
        let get =
            |k| -> Result<String, FormatError> { head_block.one(k)?.map_or(Ok(String::new()), |(v, l)| text(v, l)) };
        let mut m = MapMod {
            name: get("name")?,
            author: get("author")?,
            description: get("description")?,
            ..Default::default()
        };
        for b in &blocks {
            match b.kind.as_str() {
                "prop.move" => {
                    b.check_keys(&["region", "section", "solid", "at", "nth", "position", "rotation"])?;
                    let (p, pl) = b.need("position")?;
                    let (r, rl) = b.need("rotation")?;
                    m.props.push(PropEdit::Move {
                        target: prop_ref(b)?,
                        position: vec3(p, pl)?,
                        rotation: rotation(r, rl)?,
                    });
                }
                "prop.delete" => {
                    b.check_keys(&["region", "section", "solid", "at", "nth"])?;
                    m.props.push(PropEdit::Delete { target: prop_ref(b)? });
                }
                "prop.add" => {
                    b.check_keys(&["id", "region", "solid", "position", "rotation"])?;
                    // No `id`: the default, given once all adds are read.
                    let id = match b.one("id")? {
                        Some((v, _)) if valid_id(v) => v.to_string(),
                        Some((v, l)) => return err(l, format!("`{v}` is not an id (a-z, 0-9, - and _, at most 40)")),
                        None => String::new(),
                    };
                    let (r, rl) = b.need("region")?;
                    let (s, sl) = b.need("solid")?;
                    let (p, pl) = b.need("position")?;
                    let (ro, rol) = b.need("rotation")?;
                    m.props.push(PropEdit::Add {
                        id,
                        region: region(r, rl)?,
                        solid: solid(s, sl)?,
                        position: vec3(p, pl)?,
                        rotation: rotation(ro, rol)?,
                    });
                }
                _ => m.races.push(Self::parse_race(b)?),
            }
            if m.props.len() > MAX_PROPS {
                return err(b.line, format!("more than {MAX_PROPS} prop edits"));
            }
            if m.races.len() > MAX_RACES {
                return err(b.line, format!("more than {MAX_RACES} races"));
            }
        }
        // A placement changed by two edits is ambiguous; an add's id names
        // one add.
        let mut seen = std::collections::HashSet::new();
        let mut add_ids = std::collections::HashSet::new();
        let mut adds = 0;
        for (b, p) in blocks.iter().filter(|b| b.kind.starts_with("prop.")).zip(&mut m.props) {
            if let PropEdit::Add { id, .. } = p {
                adds += 1;
                if id.is_empty() {
                    *id = default_add_id(adds);
                }
                if !add_ids.insert(id.clone()) {
                    return err(b.line, format!("add id `{id}` used twice"));
                }
            }
            if let Some(t) = p.target()
                && !seen.insert((
                    t.region.clone(),
                    t.section,
                    t.solid.clone(),
                    t.at.to_array().map(f32::to_bits),
                    t.nth,
                ))
            {
                return err(b.line, "this placement is already changed by an earlier edit");
            }
        }
        let mut ids = std::collections::HashSet::new();
        for (b, r) in blocks.iter().filter(|b| b.kind == "race").zip(&m.races) {
            if !ids.insert(r.id.clone()) {
                return err(b.line, format!("race id `{}` used twice", r.id));
            }
        }
        Ok(m)
    }

    fn parse_race(b: &Block) -> Result<CustomRace, FormatError> {
        b.check_keys(&["id", "name", "track", "backward", "mode", "laps", "checkpoint", "grid", "start", "finish"])?;
        let (id, il) = b.need("id")?;
        if !valid_id(id) {
            return err(il, format!("`{id}` is not a race id (a-z, 0-9, - and _, at most 40)"));
        }
        let (t, tl) = b.need("track")?;
        let track: u16 =
            t.parse().map_err(|_| FormatError { line: tl, message: format!("`{t}` is not a track number") })?;
        let (k, kl) = b.need("mode")?;
        let kind = RaceKind::parse(k).ok_or_else(|| FormatError {
            line: kl,
            message: format!("`{k}` is not a race mode (circuit, sprint, streetx, drift)"),
        })?;
        let mut r = CustomRace::new(id, track, kind);
        if let Some((v, l)) = b.one("name")? {
            r.name = text(v, l)?;
        }
        if let Some((v, l)) = b.one("backward")? {
            r.backward = yes_no(v, l)?;
        }
        if let Some((v, l)) = b.one("laps")? {
            r.laps = v
                .parse()
                .ok()
                .filter(|n| (1..=MAX_LAPS).contains(n))
                .ok_or_else(|| FormatError { line: l, message: format!("laps must be 1 to {MAX_LAPS}") })?;
            if !kind.loops() && r.laps != 1 {
                return err(l, format!("a {} race runs one lap", kind.name()));
            }
        }
        for (v, l) in b.all("checkpoint") {
            r.checkpoints.push(Vec2::from_array(floats::<2>(v, l)?));
        }
        // Fewer than 2 is a race still being made: kept, not raceable
        // (`race::check`).
        if r.checkpoints.len() > MAX_CHECKPOINTS {
            return err(
                b.line,
                format!("a race has at most {MAX_CHECKPOINTS} checkpoints, it has {}", r.checkpoints.len()),
            );
        }
        for (v, l) in b.all("grid") {
            if r.grid.len() == MAX_GRID {
                return err(l, format!("at most {MAX_GRID} grid slots"));
            }
            r.grid.push(spot(v, l)?);
        }
        if let Some((v, l)) = b.one("start")? {
            r.start = Some(spot(v, l)?);
        }
        if let Some((v, l)) = b.one("finish")? {
            r.finish = Some(spot(v, l)?);
        }
        Ok(r)
    }

    /// The file's text ([`MapMod::parse`] reads it back to the same mod).
    pub fn write(&self) -> String {
        use std::fmt::Write;
        let mut s = format!("{MAGIC} {VERSION}\n");
        let line = |s: &mut String, k: &str, v: &str| {
            if !v.is_empty() {
                let _ = writeln!(s, "{k} = {}", one_line(v));
            }
        };
        line(&mut s, "name", &self.name);
        line(&mut s, "author", &self.author);
        line(&mut s, "description", &self.description);
        let v3 = |v: Vec3| format!("{} {} {}", v.x, v.y, v.z);
        let rot = |m: Mat3| m.transpose().to_cols_array().map(|x| x.to_string()).join(" ");
        let target = |s: &mut String, t: &PropRef| {
            let _ =
                writeln!(s, "region = {}\nsection = {}\nsolid = {}\nat = {}", t.region, t.section, t.solid, v3(t.at));
            if t.nth > 0 {
                let _ = writeln!(s, "nth = {}", t.nth);
            }
        };
        let mut adds = 0;
        for p in &self.props {
            match p {
                PropEdit::Move { target: t, position, rotation } => {
                    s.push_str("\n[prop.move]\n");
                    target(&mut s, t);
                    let _ = writeln!(s, "position = {}\nrotation = {}", v3(*position), rot(*rotation));
                }
                PropEdit::Delete { target: t } => {
                    s.push_str("\n[prop.delete]\n");
                    target(&mut s, t);
                }
                PropEdit::Add { id, region, solid, position, rotation } => {
                    adds += 1;
                    s.push_str("\n[prop.add]\n");
                    if *id != default_add_id(adds) {
                        let _ = writeln!(s, "id = {id}");
                    }
                    let _ = writeln!(
                        s,
                        "region = {region}\nsolid = {solid}\nposition = {}\nrotation = {}",
                        v3(*position),
                        rot(*rotation)
                    );
                }
            }
        }
        let spot = |p: &Spot| format!("{} {}", v3(p.position), p.heading);
        for r in &self.races {
            let _ = writeln!(s, "\n[race]\nid = {}", r.id);
            line(&mut s, "name", &r.name);
            let _ = writeln!(
                s,
                "track = {}\nbackward = {}\nmode = {}\nlaps = {}",
                r.track,
                if r.backward { "yes" } else { "no" },
                r.kind.name(),
                r.laps
            );
            for c in &r.checkpoints {
                let _ = writeln!(s, "checkpoint = {} {}", c.x, c.y);
            }
            for g in &r.grid {
                let _ = writeln!(s, "grid = {}", spot(g));
            }
            if let Some(p) = &r.start {
                let _ = writeln!(s, "start = {}", spot(p));
            }
            if let Some(p) = &r.finish {
                let _ = writeln!(s, "finish = {}", spot(p));
            }
        }
        s
    }
}

/// Text kept on one line (a line break would start a new key).
fn one_line(v: &str) -> String {
    v.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().trim().to_string()
}

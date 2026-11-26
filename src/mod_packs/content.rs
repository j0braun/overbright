//! A map mod's content as the kind files of a mod pack, and back (docs/modding/asset-format: world.md §3.9, roads-ai.md
//! §3.7, textures.md §3.5):
//!
//! - placements: `world/<REGION>/placements.toml` (`ob.placements/1.0`), one file per region;
//! - races: `races/<id>.toml` (`ob.race/1.0`), one per race;
//! - textures: image-only entries, `world/_any/textures/images/0x<HASH>.png` (what the world draws) and
//!   `cars/_any/textures/images/0x<HASH>.png` (cars);
//! - static collision edits have no kind file yet: they stay in a `map.obmap` that holds nothing else.
//!
//! A pack made before this (its map in `map.obmap`, textures in `textures/world|car/`) still reads: [`read`] takes the
//! map file first and adds what the kind files give. A placement the pack names by `<scenery>/<index>` needs the base
//! set's placement table to resolve and is refused here ([`read`] says so); the entries written from a map mod carry
//! the map mod's own reference (stream section, solid, position, `nth`) in `extras.legacy`, which is what the game
//! applies.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use glam::{Mat3, Vec2, Vec3};
use ob_map::tools::map::format::{CustomRace, MapMod, PropEdit, PropRef, RaceKind, Spot};

use crate::tools::map::store;

pub const PLACEMENTS_SCHEMA: &str = "ob.placements/1.0";
pub const RACE_SCHEMA: &str = "ob.race/1.0";
/// Where the world's texture images are in a pack (every region).
pub const WORLD_IMAGES: &str = "world/_any/textures/images";
/// Where the cars' texture images are in a pack (every car).
/// A converted texture pack's images (`<domain>/…`, `any/…`), named by `global/textures/overrides.toml`.
pub const GLOBAL_IMAGES: &str = "global/textures/images";
pub const CAR_IMAGES: &str = "cars/_any/textures/images";

fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// A float in the shortest form that reads back as the same binary32.
fn num(x: f32) -> String {
    format!("{x:?}")
}

fn vec3(v: Vec3) -> String {
    format!("[{}, {}, {}]", num(v.x), num(v.y), num(v.z))
}

fn rows(m: Mat3) -> String {
    let f = |i: usize| {
        let r = m.row(i);
        format!("{}, {}, {}", num(r.x), num(r.y), num(r.z))
    };
    format!("[{}, {}, {}]", f(0), f(1), f(2))
}

/// The kind files of `map` (path in the pack, bytes), in a fixed order.
pub fn write(map: &MapMod) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut regions: BTreeMap<&str, Vec<&PropEdit>> = BTreeMap::new();
    for p in &map.props {
        regions.entry(p.region()).or_default().push(p);
    }
    for (region, edits) in regions {
        let mut t = format!("schema = {}\n", q(PLACEMENTS_SCHEMA));
        for e in edits {
            t += "\n[[placement]]\n";
            match e {
                PropEdit::Move { target, position, rotation } => {
                    t += &entry_target(target);
                    t += &format!("position = {}\nrotation = {}\n", vec3(*position), rows(*rotation));
                }
                PropEdit::Delete { target } => {
                    t += &entry_target(target);
                    t += "disabled = true\n";
                }
                PropEdit::Add { id, solid, position, rotation, .. } => {
                    t += &format!(
                        "id = {}\nsolid = {}\nposition = {}\nrotation = {}\n",
                        q(id),
                        q(&format!("solid:{solid}")),
                        vec3(*position),
                        rows(*rotation)
                    );
                }
            }
        }
        out.push((format!("world/{region}/placements.toml"), t.into_bytes()));
    }
    for r in &map.races {
        out.push((format!("races/{}.toml", r.id), race_text(r).into_bytes()));
    }
    if !map.collision.is_empty() {
        let only = MapMod {
            name: map.name.clone(),
            author: map.author.clone(),
            description: map.description.clone(),
            collision: map.collision.clone(),
            ..Default::default()
        };
        out.push((store::FILE.to_string(), only.write().into_bytes()));
    }
    out
}

/// A moved or hidden original: the base's guard and the map mod's own reference.
fn entry_target(t: &PropRef) -> String {
    format!(
        "guard = {{ solid = {}, position = {} }}\nextras = {{ legacy = {{ section = {}, solid = {}, at = {}, nth = {} }} }}\n",
        q(&format!("solid:{}", t.solid)),
        vec3(t.at),
        t.section,
        q(&t.solid),
        vec3(t.at),
        t.nth
    )
}

fn spot(s: &Spot) -> String {
    format!("{{ position = {}, heading_deg = {} }}", vec3(s.position), num(s.heading))
}

fn race_text(r: &CustomRace) -> String {
    let mut t = format!(
        "schema = {}\nid = {}\nname = {}\ntrack = {}\nbackward = {}\nmode = {}\nlaps = {}\n",
        q(RACE_SCHEMA),
        q(&r.id),
        q(&r.name),
        r.track,
        r.backward,
        q(r.kind.name()),
        r.laps
    );
    let pts: Vec<String> = r.checkpoints.iter().map(|p| format!("[{}, {}]", num(p.x), num(p.y))).collect();
    t += &format!("checkpoints = [{}]\n", pts.join(", "));
    if let Some(s) = &r.start {
        t += &format!("start = {}\n", spot(s));
    }
    if let Some(s) = &r.finish {
        t += &format!("finish = {}\n", spot(s));
    }
    let grid: Vec<String> = r.grid.iter().map(|s| format!("  {},\n", spot(s))).collect();
    t += &format!("grid = [\n{}]\n", grid.concat());
    t
}

// ---- reading --------------------------------------------------------------------------------------------------------

fn f(v: &toml::Value) -> Option<f32> {
    match v {
        toml::Value::Float(x) => Some(*x as f32),
        toml::Value::Integer(i) => Some(*i as f32),
        _ => None,
    }
}

fn floats(v: &toml::Value, n: usize) -> Option<Vec<f32>> {
    let a = v.as_array()?;
    (a.len() == n).then_some(())?;
    a.iter().map(f).collect()
}

fn v3(v: &toml::Value) -> Option<Vec3> {
    let a = floats(v, 3)?;
    Some(Vec3::new(a[0], a[1], a[2]))
}

fn mat(v: &toml::Value) -> Option<Mat3> {
    let a = floats(v, 9)?;
    Some(Mat3::from_cols(Vec3::new(a[0], a[1], a[2]), Vec3::new(a[3], a[4], a[5]), Vec3::new(a[6], a[7], a[8])).transpose())
}

fn solid_name(v: &toml::Value) -> Option<String> {
    let s = v.as_str()?;
    Some(s.strip_prefix("solid:").unwrap_or(s).to_string())
}

fn spot_of(v: &toml::Value) -> Option<Spot> {
    Some(Spot { position: v3(v.get("position")?)?, heading: f(v.get("heading_deg")?)? })
}

fn table(text: &str, what: &str, schema: &str) -> Result<toml::Table, String> {
    let t: toml::Table = text.parse().map_err(|e: toml::de::Error| format!("{what}: {}", e.message()))?;
    match t.get("schema").and_then(toml::Value::as_str) {
        Some(s) if s.split('/').next() == schema.split('/').next() => Ok(t),
        _ => Err(format!("{what}: not an {schema} file")),
    }
}

/// The placements of the region `region` from the text of its `placements.toml`.
pub fn read_placements(text: &str, region: &str) -> Result<Vec<PropEdit>, String> {
    let what = format!("world/{region}/placements.toml");
    let t = table(text, &what, PLACEMENTS_SCHEMA)?;
    let mut out = Vec::new();
    for (i, e) in t.get("placement").and_then(toml::Value::as_array).into_iter().flatten().enumerate() {
        let bad = |why: &str| format!("{what}: entry {}: {why}", i + 1);
        if let Some(id) = e.get("id").and_then(toml::Value::as_str) {
            let solid = e.get("solid").and_then(solid_name).ok_or_else(|| bad("an addition needs a solid"))?;
            let position = e.get("position").and_then(v3).ok_or_else(|| bad("an addition needs a position"))?;
            let rotation = match e.get("rotation") {
                Some(r) => mat(r).ok_or_else(|| bad("the rotation is nine numbers"))?,
                None => Mat3::IDENTITY,
            };
            out.push(PropEdit::Add { id: id.to_string(), region: region.to_string(), solid, position, rotation });
            continue;
        }
        let legacy = e.get("extras").and_then(|x| x.get("legacy"));
        let Some(l) = legacy else {
            let key = e.get("key").and_then(toml::Value::as_str).unwrap_or("?");
            return Err(bad(&format!(
                "the placement {key} is named by its index in the base set, which this build cannot resolve yet"
            )));
        };
        let target = (|| {
            Some(PropRef {
                region: region.to_string(),
                section: u32::try_from(l.get("section")?.as_integer()?).ok()?,
                solid: l.get("solid")?.as_str()?.to_string(),
                at: v3(l.get("at")?)?,
                nth: l.get("nth").and_then(toml::Value::as_integer).and_then(|n| u32::try_from(n).ok()).unwrap_or(0),
            })
        })()
        .ok_or_else(|| bad("the original placement is not named well"))?;
        if e.get("disabled").and_then(toml::Value::as_bool) == Some(true) {
            out.push(PropEdit::Delete { target });
        } else {
            let position = e.get("position").and_then(v3).ok_or_else(|| bad("a move needs a position"))?;
            let rotation = match e.get("rotation") {
                Some(r) => mat(r).ok_or_else(|| bad("the rotation is nine numbers"))?,
                None => Mat3::IDENTITY,
            };
            out.push(PropEdit::Move { target, position, rotation });
        }
    }
    Ok(out)
}

/// The race of a `races/<id>.toml`.
pub fn read_race(text: &str, id: &str) -> Result<CustomRace, String> {
    let what = format!("races/{id}.toml");
    let t = table(text, &what, RACE_SCHEMA)?;
    let bad = |k: &str| format!("{what}: `{k}` is missing or not right");
    let get = |k: &str| t.get(k);
    let have = get("id").and_then(toml::Value::as_str).unwrap_or_default();
    if have != id {
        return Err(format!("{what}: `id` must equal the file name"));
    }
    let kind = get("mode").and_then(toml::Value::as_str).and_then(RaceKind::parse).ok_or_else(|| bad("mode"))?;
    let mut r = CustomRace::new(id, 0, kind);
    r.track = get("track").and_then(toml::Value::as_integer).and_then(|n| u16::try_from(n).ok()).ok_or_else(|| bad("track"))?;
    r.name = get("name").and_then(toml::Value::as_str).unwrap_or(id).to_string();
    r.backward = get("backward").and_then(toml::Value::as_bool).unwrap_or(false);
    if let Some(l) = get("laps") {
        r.laps = l.as_integer().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| bad("laps"))?;
    }
    r.checkpoints.clear();
    for p in get("checkpoints").and_then(toml::Value::as_array).into_iter().flatten() {
        let a = floats(p, 2).ok_or_else(|| bad("checkpoints"))?;
        r.checkpoints.push(Vec2::new(a[0], a[1]));
    }
    r.grid.clear();
    for s in get("grid").and_then(toml::Value::as_array).into_iter().flatten() {
        r.grid.push(spot_of(s).ok_or_else(|| bad("grid"))?);
    }
    r.start = match get("start") {
        Some(s) => Some(spot_of(s).ok_or_else(|| bad("start"))?),
        None => None,
    };
    r.finish = match get("finish") {
        Some(s) => Some(spot_of(s).ok_or_else(|| bad("finish"))?),
        None => None,
    };
    Ok(r)
}

fn read_text(p: &Path) -> Result<String, String> {
    let len = std::fs::metadata(p).map_err(|e| format!("{}: {e}", p.display()))?.len();
    if len > ob_map::tools::map::format::MAX_FILE_BYTES {
        return Err(format!("{}: larger than a map mod may be", p.display()));
    }
    std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))
}

/// The map mod a pack's files in `root` make: the old `map.obmap` first (an older pack, or the collision edits), then
/// the region files' placements and the races. None: nothing of the three is there.
pub fn read(root: &Path) -> Result<Option<MapMod>, String> {
    let mut m = None;
    let file = root.join(store::FILE);
    if file.is_file() {
        m = Some(store::read_file(&file).map_err(|e| format!("{e:#}"))?);
    }
    let sorted = |d: &Path| -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()).collect();
        v.sort();
        v
    };
    for d in sorted(&root.join("world")) {
        let Some(region) = d.file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
        let p = d.join("placements.toml");
        if region == "_any" || !p.is_file() {
            continue;
        }
        let edits = read_placements(&read_text(&p)?, &region)?;
        m.get_or_insert_with(MapMod::default).props.extend(edits);
    }
    for p in sorted(&root.join("races")) {
        let Some(id) = p.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".toml")) else { continue };
        if !p.is_file() {
            continue;
        }
        let r = read_race(&read_text(&p)?, id)?;
        m.get_or_insert_with(MapMod::default).races.push(r);
    }
    Ok(m)
}

/// The folders a pack's texture images for the world (`car` false) or the cars are read from, the strongest first: the
/// old `textures/world|car/`, the kind folders, and a converted texture pack's `global/textures/images/<world|car>`
/// then `any` (which both domains read, after their own).
pub fn texture_dirs(root: &Path, car: bool) -> Vec<PathBuf> {
    let d = if car { "car" } else { "world" };
    [
        root.join(store::TEXTURES).join(d),
        root.join(if car { CAR_IMAGES } else { WORLD_IMAGES }),
        root.join(GLOBAL_IMAGES).join(d),
        root.join(GLOBAL_IMAGES).join("any"),
    ]
    .into_iter()
    .filter(|p| p.is_dir())
    .collect()
}

/// The pack path of a map mod's texture `textures/<world|car>/<stem>.png`: image-only entries keyed by the hash.
pub fn texture_path(domain: &str, stem: &str) -> Option<String> {
    let h = u32::from_str_radix(stem, 16).ok().filter(|_| stem.len() <= 8)?;
    let dir = match domain {
        "world" => WORLD_IMAGES,
        "car" => CAR_IMAGES,
        _ => return None,
    };
    Some(format!("{dir}/0x{h:08X}.png"))
}

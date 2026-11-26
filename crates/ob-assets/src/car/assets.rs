//! Shared game-file reads and the process-wide single-flight caches.

use super::*;

/// A file every car load reads alike (GLOBALB.BUN, the shared parts'
/// models), read once a run: the game's data does not change while it runs.
pub fn shared_file(path: &Path) -> std::io::Result<std::sync::Arc<Vec<u8>>> {
    static FILES: SingleFlight<Option<std::sync::Arc<Vec<u8>>>> = SingleFlight::new();
    // A failed read is not kept: the caller reads again for the error.
    let read = || {
        #[cfg(test)]
        shared_file_reads::count(path);
        crate::game_fs::read_file(path).ok().map(std::sync::Arc::new)
    };
    match FILES.get(path, read) {
        Some(d) => Ok(d),
        None => {
            FILES.forget(path);
            Ok(std::sync::Arc::new(crate::game_fs::read_file(path)?))
        }
    }
}

/// How often [`shared_file`] read each file from disk (the read-once tests).
#[cfg(test)]
pub mod shared_file_reads {
    static READS: std::sync::Mutex<Option<std::collections::HashMap<std::path::PathBuf, usize>>> =
        std::sync::Mutex::new(None);
    pub fn count(path: &std::path::Path) {
        *READS.lock().unwrap().get_or_insert_with(Default::default).entry(path.to_path_buf()).or_default() += 1;
    }
    pub fn of(path: &std::path::Path) -> usize {
        READS.lock().unwrap().as_ref().and_then(|m| m.get(path).copied()).unwrap_or(0)
    }
}

/// A per-path cache whose value is computed once even when several
/// threads ask for it at the same time: the first caller computes it, the
/// others wait for that result instead of repeating the work.
pub struct SingleFlight<T> {
    map: std::sync::Mutex<Option<HashMap<std::path::PathBuf, std::sync::Arc<std::sync::OnceLock<T>>>>>,
}

impl<T: Clone> SingleFlight<T> {
    pub const fn new() -> SingleFlight<T> {
        SingleFlight { map: std::sync::Mutex::new(None) }
    }

    pub fn get(&self, path: &Path, make: impl FnOnce() -> T) -> T {
        let cell =
            self.map.lock().unwrap().get_or_insert_with(HashMap::new).entry(path.to_path_buf()).or_default().clone();
        cell.get_or_init(make).clone()
    }

    pub fn forget(&self, path: &Path) {
        if let Some(m) = self.map.lock().unwrap().as_mut() {
            m.remove(path);
        }
    }
}

/// The textures of every texture pack in `data`, in file order.
pub fn texture_packs(data: &[u8]) -> Result<Vec<Texture>> {
    let mut packs = Vec::new();
    bundle::walk(bundle::chunks(data, 0), &mut |c| {
        if c.id == id::TEXTURE_PACK {
            packs.push(c);
        }
    });
    let mut out = Vec::new();
    for pack in packs {
        out.extend(parse_texture_pack(pack)?);
    }
    Ok(out)
}

/// Every texture the car renderer can draw, by file (the texture-pack
/// generator, `upscale_pack`): each car's own CARS/<CAR>/TEXTURES.BIN, the
/// shared CARS/TEXTURES.BIN and CARS/WHEELS/TEXTURES.BIN, and GLOBALB's
/// window glass. Returns (file label, car type if any, textures).
pub fn car_texture_sources(game_root: &Path) -> Result<Vec<(String, Option<CarType>, Vec<Texture>)>> {
    let globalb_path = crate::game_fs::path(game_root, "GLOBAL/GLOBALB.BUN");
    let globalb = shared_file(&globalb_path).context("reading GLOBALB.BUN")?;
    let types = parse_car_types(&globalb)?;
    let mut out = Vec::new();
    for t in types {
        let file = crate::game_fs::path(game_root, &format!("CARS/{}/TEXTURES.BIN", t.name));
        if !file.is_file() {
            continue;
        }
        let data = crate::cache::SourceBytes::open(&file).with_context(|| format!("reading {}", file.display()))?;
        let textures = texture_packs(&data)?;
        out.push((format!("CARS/{}", t.name), Some(t), textures));
    }
    for rel in ["CARS/TEXTURES.BIN", "CARS/WHEELS/TEXTURES.BIN"] {
        out.push((rel.to_string(), None, shared_textures(&crate::game_fs::path(game_root, rel))?.to_vec()));
    }
    let windows: Vec<Texture> = shared_textures(&globalb_path)?
        .iter()
        .filter(|t| WINDOW_TEXTURES.iter().any(|w| bin_hash(w) == t.hash) || t.hash == bin_hash(REAR_DEFROSTER))
        .cloned()
        .collect();
    out.push(("GLOBAL/GLOBALB.BUN".to_string(), None, windows));
    Ok(out)
}

/// The rims' compositor inputs (0x61DEA0, see [`composite_wheel`]) over
/// every rim part of the part database: (sources, masks) by hash — each rim
/// base's `_WHEEL` / `_WHEEL_INNER_MASK` and each SPINNER_TEXTURE / `_MASK`.
/// The texture-pack generator (`upscale_pack`) classes the masks by these.
pub fn wheel_composite_hashes(
    game_root: &Path,
) -> Result<(std::collections::HashSet<u32>, std::collections::HashSet<u32>)> {
    let globalb = shared_file(&crate::game_fs::path(game_root, "GLOBAL/GLOBALB.BUN")).context("reading GLOBALB.BUN")?;
    let db = PartDb::load(&globalb)?;
    let (mut sources, mut masks) = (std::collections::HashSet::new(), std::collections::HashSet::new());
    for p in &db.parts {
        if let Some(b) = db.attribute(p, 0x10C9_8090).filter(|&h| h != 0) {
            sources.insert(bin_hash_from(b, "_WHEEL"));
            masks.insert(bin_hash_from(b, "_WHEEL_INNER_MASK"));
        }
        if let Some(sp) = db.attribute(p, bin_hash("SPINNER_TEXTURE")).filter(|&h| h != 0) {
            sources.insert(sp);
            masks.insert(bin_hash_from(sp, "_MASK"));
        }
    }
    Ok((sources, masks))
}

/// The files a load of the car `name` reads (`Car::load_tuned`), for
/// reading them ahead (`cache::read_ahead`, docs/developing/read-ahead.md): its
/// GEOMETRY.BIN (read whole: the collision hull), the cache files of its
/// solids and its textures (its TEXTURES.BIN without one), its part
/// animations, and the cache files of the texture packs every car shares
/// (CARS/TEXTURES.BIN, CARS/WHEELS/TEXTURES.BIN, GLOBALB's), each under
/// the path the load opens it by. The rims' and body kits' shared part
/// files are left out: a load reads only the pieces its parts use.
pub fn read_ahead_files(game_root: &Path, name: &str) -> Vec<std::path::PathBuf> {
    let car_dir = crate::game_fs::path(game_root, &format!("CARS/{name}"));
    let geometry = crate::game_fs::path(&car_dir, "GEOMETRY.BIN");
    let textures = crate::game_fs::path(&car_dir, "TEXTURES.BIN");
    let mut files = vec![geometry.clone()];
    files.extend(crate::cache::file_cached_entry(crate::BUILD_ID, &geometry, "solids"));
    files.push(crate::cache::file_cached_entry(crate::BUILD_ID, &textures, "textures").unwrap_or(textures));
    files.push(crate::game_fs::path(&car_dir, "PARTS_ANIMATIONS.BIN"));
    for shared in ["CARS/TEXTURES.BIN", "CARS/WHEELS/TEXTURES.BIN", "GLOBAL/GLOBALB.BUN"] {
        let p = crate::game_fs::path(game_root, shared);
        files.push(crate::cache::file_cached_entry(crate::BUILD_ID, &p, "textures").unwrap_or(p));
    }
    files
}

/// [`texture_packs`] of a file every car load reads alike (the shared car
/// and rim textures, GLOBALB's), decoded once a run.
pub fn shared_textures(path: &Path) -> Result<std::sync::Arc<Vec<Texture>>> {
    type Parsed = Result<std::sync::Arc<Vec<Texture>>, std::sync::Arc<anyhow::Error>>;
    static PACKS: SingleFlight<Parsed> = SingleFlight::new();
    let parsed = PACKS.get(path, || {
        let parse = || -> Result<std::sync::Arc<Vec<Texture>>> {
            let packs = crate::cache::file_cached(crate::BUILD_ID, path, "textures", || {
                let data =
                    crate::cache::SourceBytes::open(path).with_context(|| format!("reading {}", path.display()))?;
                texture_packs(&data)
            })?;
            Ok(std::sync::Arc::new(packs))
        };
        parse().map_err(std::sync::Arc::new)
    });
    parsed.map_err(|e| {
        PACKS.forget(path);
        anyhow::anyhow!("{e:#}")
    })
}

/// Each SOLID_LIST's solids that have a header, with their hashes.
pub fn part_file_lists(data: &[u8]) -> Vec<Vec<(u32, bundle::Chunk<'_>)>> {
    let mut lists = Vec::new();
    bundle::walk(bundle::chunks(data, 0), &mut |c| {
        if c.id == id::SOLID_LIST {
            lists.push(c);
        }
    });
    lists
        .into_iter()
        .map(|l| {
            l.children()
                .filter(|c| c.id == id::SOLID)
                .filter_map(|c| {
                    c.child(id::SOLID_HEADER)
                        .filter(|h| h.aligned(16).len() >= 0x14)
                        .map(|h| (read_u32(h.aligned(16), 0x10), c))
                })
                .collect()
        })
        .collect()
}

/// The solids of a shared part file `wanted` names that the car has not
/// (by the solids before each list), added to `solids` in file order. A
/// file that cannot be read adds none. The files hold 79 MB of models of
/// which a car takes a few: each file's solid hashes are kept for the run,
/// so a file is read again only when it has a wanted solid.
pub fn add_shared_part_solids(
    file: &Path,
    wanted: &std::collections::HashSet<u32>,
    solids: &mut Vec<Solid>,
) -> Result<()> {
    // Each file's solid hashes, read once a run (single-flight: parallel
    // loads wait for the one read). A file that cannot be read has none.
    static INDEX: SingleFlight<std::sync::Arc<Vec<Vec<u32>>>> = SingleFlight::new();
    let lists = INDEX.get(file, || {
        let Ok(d) = crate::cache::SourceBytes::open(file) else { return std::sync::Arc::new(Vec::new()) };
        let lists = crate::cache::file_cached(crate::BUILD_ID, file, "solid-index", || {
            Ok(part_file_lists(&d).iter().map(|l| l.iter().map(|e| e.0).collect()).collect())
        });
        std::sync::Arc::new(lists.unwrap_or_default())
    });
    if !lists.iter().flatten().any(|h| wanted.contains(h)) {
        return Ok(());
    }
    let data = crate::cache::SourceBytes::open(file).with_context(|| format!("reading {}", file.display()))?;
    for (li, list) in part_file_lists(&data).into_iter().enumerate() {
        let have: std::collections::HashSet<u32> = solids.iter().map(|s| s.hash).collect();
        let pick: Vec<bundle::Chunk<'_>> =
            list.iter().filter(|(h, _)| wanted.contains(h) && !have.contains(h)).map(|e| e.1).collect();
        debug_assert_eq!(list.len(), lists[li].len());
        solids.extend(crate::formats::solid::parse_in_order(pick)?);
    }
    Ok(())
}

/// The shared part files the game loads with the cars (0x57F040 over the
/// table 0x86658C, filled from 0x7FA978 by 0x77D850): EXHAUST, BRAKES,
/// MIRRORS_POST, MIRRORS_BODY, MIRRORS_SUV, MIRRORS_HUMMER, SPOILER,
/// SPOILER_HATCH, SPOILER_SUV, ROOF and AUDIO, then
/// `CARS\WHEELS\GEOMETRY_%s.BIN` for every part of type 0x43 in group
/// 0xA6E1B733 (its name the brand, under 0x1E3 characters); PLATES
/// (0x7FA9A8) is loaded by the front end's loader (0x57FBDE). Their
/// textures are all in CARS/TEXTURES.BIN.
pub fn shared_part_files(game_root: &Path, db: &PartDb) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = [
        "EXHAUST",
        "BRAKES",
        "MIRRORS_POST",
        "MIRRORS_BODY",
        "MIRRORS_SUV",
        "MIRRORS_HUMMER",
        "SPOILER",
        "SPOILER_HATCH",
        "SPOILER_SUV",
        "ROOF",
        "AUDIO",
        "PLATES",
    ]
    .iter()
    .map(|d| crate::game_fs::path(game_root, &format!("CARS/{d}/GEOMETRY.BIN")))
    .collect();
    if let Some(group) = db.groups.iter().position(|&g| g == 0xA6E1_B733) {
        for p in db.parts.iter().filter(|p| p.group as usize == group && p.slot == 0x43) {
            if let Some(brand) = db.string(p.name as u32).filter(|b| b.len() < 0x1E3) {
                files.push(crate::game_fs::path(game_root, &format!("CARS/WHEELS/GEOMETRY_{brand}.BIN")));
            }
        }
    }
    files
}

/// What [`crate::cache::file_cached`] would store for `what` of the game
/// file `path`, parsed now (`--verify-cache`); None for an unknown kind.
pub fn reparse_for_cache(what: &str, path: &Path) -> Option<Result<Vec<u8>>> {
    use crate::cache::{Persist, Writer};
    let parse = || -> Result<Writer> {
        let data = crate::cache::SourceBytes::open(path).with_context(|| format!("reading {}", path.display()))?;
        let mut w = Writer::default();
        match what {
            "textures" => texture_packs(&data)?.put(&mut w),
            "solids" => {
                let mut solids = Vec::new();
                for list in bundle::chunks(&data, 0).filter(|c| c.id == id::SOLID_LIST) {
                    solids.extend(parse_solid_list(list)?);
                }
                solids.put(&mut w)
            }
            _ => {
                let lists: Vec<Vec<u32>> =
                    part_file_lists(&data).iter().map(|l| l.iter().map(|e| e.0).collect()).collect();
                lists.put(&mut w)
            }
        }
        Ok(w)
    };
    matches!(what, "textures" | "solids" | "solid-index").then(|| parse().map(|w| w.buf))
}

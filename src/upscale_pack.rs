//! The generated texture pack as a library (docs/texture-pack-
//! generator.md): what `overbright-upscale` and the Graphics page's
//! "Generate Texture Pack" row run. Reads the player's own world and car
//! textures, writes `<HASH>.dds` files and a manifest per domain into a
//! folder (by default the player's data dir: `overbright/texture-pack/
//! {world,car}`), and holds the World Texture Pack row's choice for the
//! loader (`render::texture_pack`).
//!
//! Nothing here runs unless asked: with the row Off and no
//! `NFSU2_WORLD_TEXTURE_PACK` / `NFSU2_CAR_TEXTURE_PACK`, the loader never
//! sees a pack (native, byte-identical).
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use crate::formats::texture::Texture;
use crate::upscale::{self as up, Category, Verdict};

pub const ALL_TRACKS: &[&str] = &["L4RA", "L4RB", "L4RC", "L4RD", "L4RF", "L4RG", "L4RH", "L4RR"];
pub const MANIFEST: &str = "overbright-upscale.tsv";
const HEADER: &str = "Overbright upscaled pack";

pub use crate::render::enh::assets::pack_select::*;

// ------------------------------------------------------------- progress

/// A run's progress, shared with the menu.
#[derive(Default)]
pub struct Progress {
    pub phase: Mutex<String>,
    pub done: AtomicUsize,
    pub total: AtomicUsize,
    pub cancel: AtomicBool,
}

impl Progress {
    fn set(&self, phase: impl Into<String>, total: usize) {
        *self.phase.lock().unwrap_or_else(|e| e.into_inner()) = phase.into();
        self.done.store(0, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
    }

    fn check(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            bail!("cancelled");
        }
        Ok(())
    }

    pub fn line(&self) -> String {
        let phase = self.phase.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let (d, t) = (self.done.load(Ordering::Relaxed), self.total.load(Ordering::Relaxed));
        if t == 0 { phase } else { format!("{phase} {d}/{t} ({}%)", d * 100 / t) }
    }
}

// -------------------------------------------------------------- sources

/// A texture to consider, with its category and its rank (world: area
/// within 20 m of roads, m²; cars: 0).
pub struct Source {
    pub tex: Texture,
    pub category: Category,
    pub rank: f64,
}

/// Every world texture of `tracks`, by hash, ranked by area near roads.
pub fn collect_world(root: &Path, tracks: &[String], progress: &Progress) -> Result<Vec<Source>> {
    struct Acc {
        tex: Texture,
        reflect_area: f64,
        all_area: f64,
        near_road: f64,
    }
    let mut by_hash: HashMap<u32, Acc> = HashMap::new();
    progress.set("Reading the city", tracks.len());
    for name in tracks {
        progress.check()?;
        let t0 = Instant::now();
        let world = crate::world::World::load(root, name).with_context(|| format!("loading {name}"))?;
        // Near = within one 20 m cell of a road (worldreflect) triangle.
        let cell = |p: glam::Vec3| ((p.x / 20.0).floor() as i32, (p.y / 20.0).floor() as i32);
        let mut road = HashSet::new();
        for p in &world.placements {
            let s = &world.solids[p.solid];
            if s.mesh_flags & 0x40 != 0 {
                for v in &s.vertices {
                    road.insert(cell(p.transform.transform_point3(glam::Vec3::from(v.position))));
                }
            }
        }
        let near = |p: glam::Vec3| {
            let (x, y) = cell(p);
            (-1..=1).any(|dx| (-1..=1).any(|dy| road.contains(&(x + dx, y + dy))))
        };
        let mut area: HashMap<u32, (f64, f64, f64)> = HashMap::new();
        for p in &world.placements {
            let s = &world.solids[p.solid];
            let reflect = s.mesh_flags & 0x40 != 0;
            for m in &s.materials {
                let Some(&tex) = s.textures.get(m.texture_slot as usize) else { continue };
                let e = area.entry(tex).or_default();
                for t in s.indices[m.first_index as usize..(m.first_index + m.index_count) as usize].chunks_exact(3) {
                    let v = |i: u16| p.transform.transform_point3(glam::Vec3::from(s.vertices[i as usize].position));
                    let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
                    let a2 = 0.5 * (b - a).cross(c - a).length() as f64;
                    e.1 += a2;
                    if reflect {
                        e.0 += a2;
                    }
                    if near((a + b + c) / 3.0) {
                        e.2 += a2;
                    }
                }
            }
        }
        for (hash, t) in &world.textures {
            let (r, all, nr) = area.get(hash).copied().unwrap_or_default();
            let e = by_hash.entry(*hash).or_insert_with(|| Acc { tex: t.clone(), reflect_area: 0.0, all_area: 0.0, near_road: 0.0 });
            e.reflect_area += r;
            e.all_area += all;
            e.near_road += nr;
        }
        log::info!("texture pack: {name}: {} textures in {:.1} s", world.textures.len(), t0.elapsed().as_secs_f32());
        progress.done.fetch_add(1, Ordering::Relaxed);
    }
    let mut v: Vec<Source> = by_hash
        .into_values()
        .map(|a| {
            let reflect = a.reflect_area > 0.3 * a.all_area;
            Source { category: up::category(&a.tex, reflect), tex: a.tex, rank: a.near_road }
        })
        .collect();
    v.sort_by(|a, b| b.rank.total_cmp(&a.rank).then(a.tex.hash.cmp(&b.tex.hash)));
    Ok(v)
}

/// Every texture the car renderer draws (`car::car_texture_sources`), by
/// hash, with its car category ([`up::car_category`]; the rims'
/// compositor masks by the part database, `car::wheel_composite_hashes`).
/// The racers' composited textures (`<CAR>_SKIN1`, `_WHEEL1`, `_SPINNER1`)
/// are never in the pack, whatever a file holds under those names.
pub fn collect_cars(root: &Path) -> Result<Vec<Source>> {
    let files = crate::car::car_texture_sources(root)?;
    let globalb = crate::game_fs::read(root, "GLOBAL/GLOBALB.BUN").context("reading GLOBALB.BUN")?;
    let types = crate::car::parse_car_types(&globalb)?;
    let traffic: Vec<String> = types.iter().filter(|t| !t.composited_skin).map(|t| t.name.to_ascii_uppercase()).collect();
    let mut composited: HashSet<u32> = HashSet::new();
    for t in types.iter().filter(|t| t.composited_skin) {
        for suffix in ["_SKIN1", "_WHEEL1", "_SPINNER1"] {
            composited.insert(crate::formats::hash::bin_hash(&format!("{}{suffix}", t.name)));
        }
    }
    let (wheel_sources, wheel_masks) = crate::car::wheel_composite_hashes(root)?;
    let mut seen = HashSet::new();
    let mut v = Vec::new();
    for (_, _, textures) in files {
        for t in textures {
            if !seen.insert(t.hash) {
                continue;
            }
            let name = t.name.to_ascii_uppercase();
            let traffic_skin = name.contains("_SKIN") && traffic.iter().any(|n| name.starts_with(&format!("{n}_SKIN")));
            let mut category = up::car_category(&t, traffic_skin);
            if composited.contains(&t.hash) {
                category = Category::Skip;
            } else if wheel_masks.contains(&t.hash) {
                category = Category::CarMask;
            } else if wheel_sources.contains(&t.hash) && category != Category::Skip {
                category = Category::CarRim;
            }
            v.push(Source { tex: t, category, rank: 0.0 });
        }
    }
    v.sort_by(|a, b| a.category.cmp(&b.category).then(a.tex.name.cmp(&b.tex.name)).then(a.tex.hash.cmp(&b.tex.hash)));
    Ok(v)
}

// ------------------------------------------------------------- generate

pub fn source_key(t: &Texture) -> u64 {
    let mut b = t.data.to_vec();
    b.extend_from_slice(&t.palette);
    b.extend_from_slice(&[t.width.to_le_bytes(), t.height.to_le_bytes(), up::GENERATOR_VERSION.to_le_bytes()].concat());
    xxhash_rust::xxh3::xxh3_64(&b)
}

pub struct Row {
    pub hash: u32,
    pub name: String,
    pub size: String,
    pub category: Category,
    pub verdict: Verdict,
    pub out: String,
    pub format: String,
    pub de: (f32, f32, f32, u8),
    pub rank: f64,
    pub key: u64,
    pub bytes: usize,
    pub orig_bytes: usize,
}

/// What a run did in one folder.
pub struct Summary {
    pub rows: Vec<Row>,
    pub seconds: f32,
}

impl Summary {
    pub fn upscaled(&self) -> usize {
        self.rows.iter().filter(|r| r.verdict == Verdict::Upscaled).count()
    }

    pub fn pack_bytes(&self) -> usize {
        self.rows.iter().filter(|r| r.verdict == Verdict::Upscaled).map(|r| r.bytes).sum()
    }

    pub fn print(&self, out: &Path) {
        let mut by: std::collections::BTreeMap<(String, String), (usize, usize)> = Default::default();
        for r in &self.rows {
            let e = by.entry((format!("{:?}", r.category), format!("{:?}", r.verdict))).or_default();
            e.0 += 1;
            e.1 += r.bytes;
        }
        for ((c, v), (n, b)) in &by {
            println!("{c:9} {v:9} {n:5} {:8.1} MB", *b as f64 / 1048576.0);
        }
        let done: Vec<&Row> = self.rows.iter().filter(|r| r.verdict == Verdict::Upscaled).collect();
        let replaced: usize = done.iter().map(|r| r.orig_bytes).sum();
        let all: usize = self.rows.iter().map(|r| r.orig_bytes).sum();
        let worst = done.iter().map(|r| r.de.0).fold(0.0f32, f32::max);
        println!("textures {} upscaled {} | pack {:.1} MB (replaces {:.1} MB of {:.1} MB original) | worst mean dE {:.2} | {:.1} s | {}", self.rows.len(), done.len(), self.pack_bytes() as f64 / 1048576.0, replaced as f64 / 1048576.0, all as f64 / 1048576.0, worst, self.seconds, out.display());
    }
}

/// The verdict before upscaling, per category: the world's rules
/// ([`up::pre_verdict`]), except that the rims' masks are kept even when
/// flat (the compositor needs the mask whenever it has the rim).
fn pre_verdict(t: &Texture, cat: Category, rgba: &[u8]) -> Result<u32, Verdict> {
    if cat == Category::CarMask {
        if t.width.min(t.height) < up::MIN_SIDE {
            return Err(Verdict::Small);
        }
        return up::scale_for(t.width, t.height, &cat.params()).ok_or(Verdict::Large);
    }
    // Car decals are mostly one colour with the lettering in the alpha:
    // flat by colour is judged by the alpha too.
    if cat == Category::CarDecal && up::luma_std(rgba) < up::FLAT_LUMA_STD {
        let alpha: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[3], p[3], p[3], 255]).collect();
        if up::luma_std(&alpha) >= up::FLAT_LUMA_STD {
            if t.width.min(t.height) < up::MIN_SIDE {
                return Err(Verdict::Small);
            }
            return up::scale_for(t.width, t.height, &cat.params()).ok_or(Verdict::Large);
        }
    }
    up::pre_verdict(t, cat, rgba)
}

/// Upscale `sources` into `out` (`<HASH>.dds` + [`MANIFEST`]); files whose
/// source and generator are unchanged since the last run are kept unless
/// `force`. `threads`: the worker count. Cancelling (`progress.cancel`)
/// stops between textures; the manifest is then not rewritten.
pub fn generate(sources: &[Source], out: &Path, label: &str, force: bool, threads: usize, low_priority: bool, progress: &Progress) -> Result<Summary> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let t0 = Instant::now();
    let previous: HashMap<u32, String> = std::fs::read_to_string(out.join(MANIFEST))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("hash"))
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Some((u32::from_str_radix(f.first()?, 16).ok()?, l.to_string()))
        })
        .collect();
    progress.set(label, sources.len());
    use rayon::prelude::*;
    let mut b = rayon::ThreadPoolBuilder::new().num_threads(threads.max(1));
    if low_priority {
        b = b.start_handler(|_| lower_priority());
    }
    let pool = b.build()?;
    // The `.dds` files are synced in batches (one sync per batch, not per
    // texture); a cancelled or failed run still puts its finished ones in
    // place.
    let files = crate::cache::ReplaceBatch::default();
    let rows = pool.install(|| {
        sources
            .par_iter()
            .map(|s| -> Result<Row> {
                progress.check()?;
                let row = one(s, out, force, &previous, &files);
                progress.done.fetch_add(1, Ordering::Relaxed);
                row
            })
            .collect::<Result<Vec<_>>>()
    });
    let committed = files.commit();
    let rows = rows?;
    committed.with_context(|| format!("writing the textures in {}", out.display()))?;
    let mut m = format!("# {HEADER} v{} ({} textures, {label})\n", up::GENERATOR_VERSION, rows.len());
    m += "hash\tname\tsize\tcategory\tverdict\tout\tformat\tde_mean\tde_p99\tde_max\talpha_max\tnear_road_m2\tsource_key\n";
    for r in &rows {
        m += &format!("{:08X}\t{}\t{}\t{:?}\t{:?}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{}\t{:.0}\t{:016x}\n", r.hash, r.name, r.size, r.category, r.verdict, r.out, r.format, r.de.0, r.de.1, r.de.2, r.de.3, r.rank, r.key);
    }
    write_manifest(out, &m)?;
    REFRESH.store(true, Ordering::Relaxed);
    Ok(Summary { rows, seconds: t0.elapsed().as_secs_f32() })
}

#[cfg(unix)]
fn lower_priority() {
    // The calling thread only (Linux: a thread is a task for setpriority).
    // SAFETY: plain syscall on this thread.
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 10);
    }
}

/// Windows: below normal, so the game's own threads win the
/// cores while a pack is generated.
#[cfg(windows)]
fn lower_priority() {
    crate::platform::win::lower_thread_priority();
}

#[cfg(not(any(unix, windows)))]
fn lower_priority() {}

fn one(
    s: &Source,
    out: &Path,
    force: bool,
    previous: &HashMap<u32, String>,
    files: &crate::cache::ReplaceBatch,
) -> Result<Row> {
    let t = &s.tex;
    let cat = s.category;
    let key = source_key(t);
    let path = out.join(format!("{:08X}.dds", t.hash));
    let mut row = Row { hash: t.hash, name: t.name.clone(), size: format!("{}x{}", t.width, t.height), category: cat, verdict: Verdict::Category, out: String::new(), format: String::new(), de: (0.0, 0.0, 0.0, 0), rank: s.rank, key, bytes: 0, orig_bytes: t.data.len() };
    let rgba = t.decode_rgba();
    let scale = match pre_verdict(t, cat, &rgba) {
        Ok(s) => s,
        Err(v) => {
            row.verdict = v;
            let _ = std::fs::remove_file(&path);
            return Ok(row);
        }
    };
    if !force && path.is_file() {
        if let Some(prev) = previous.get(&t.hash) {
            let f: Vec<&str> = prev.split('\t').collect();
            if f.len() > 12 && f[12] == format!("{key:016x}") && f[4] == "Upscaled" && f[3] == format!("{cat:?}") {
                row.verdict = Verdict::Upscaled;
                row.out = f[5].into();
                row.format = f[6].into();
                row.de = (f[7].parse().unwrap_or(0.0), f[8].parse().unwrap_or(0.0), f[9].parse().unwrap_or(0.0), f[10].parse().unwrap_or(0));
                row.bytes = std::fs::metadata(&path).map_or(0, |m| m.len() as usize - 128);
                return Ok(row);
            }
        }
    }
    let use_ = up::AlphaUse::of(t);
    let u = up::upscale(&rgba, t.width, t.height, scale, &cat.params(), up::is_dxt(t), use_);
    row.de = up::low_frequency_delta_e(&rgba, t.width, t.height, &u, use_);
    row.out = format!("{}x{}", u.width, u.height);
    if row.de.0 > up::MAX_MEAN_DELTA_E || row.de.1 > up::MAX_P99_DELTA_E {
        row.verdict = Verdict::Colour;
        let _ = std::fs::remove_file(&path);
        return Ok(row);
    }
    let e = up::encode(&u);
    row.format = format!("{:?}", e.format);
    row.bytes = e.data.len();
    // Whole or not at all, through a temporary name unique per write (as
    // `write_png`, `cache::replace_file`): two runs writing one pack at
    // once never share one.
    files.add(&path, &up::dds_bytes(&e))?;
    row.verdict = Verdict::Upscaled;
    Ok(row)
}

// ------------------------------------------------ the loading screens

/// docs/loading-image.md: the generated UI folder,
/// `<data>/overbright/texture-pack/ui` (`NFSU2_UI_TEXTURE_PACK` names
/// another): the loading screens' textures the HD UI pack leaves out, as
/// `<HASH>.png`. Read by the UI loader only while HD UI Art is in force
/// (`render::texture_pack`), after the HD UI pack's own files.
pub fn ui_dir() -> PathBuf {
    match std::env::var_os("NFSU2_UI_TEXTURE_PACK").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => pack_dir().join("ui"),
    }
}

/// The loading screens' packages: PC_Loading (track loads and the boot's
/// front-end load, 0x57FF95 / 0x4C57C0), loading_boot (closed by the
/// flow's LOADING, 0x6039E7), MC_Background (PC_load.tga under the boot's
/// memory-card screens), HUD_World_Loading (streaming waits) and the
/// console-era Loading / Loading_Tips / Loading_Controller (in the data,
/// not opened on PC).
pub const LOADING_PACKAGES: &[&str] = &["PC_Loading", "loading_boot", "MC_Background", "HUD_World_Loading", "Loading", "Loading_Tips", "Loading_Controller"];

/// docs/loading-image.md §3: the title screen after the intro movie
/// (UG_LS_Splash, "Press Enter Key"; 0x4C5500 → 0x4A90F0). Not a loading
/// screen, but its picture is made the same way.
pub const TITLE_PACKAGES: &[&str] = &["UG_LS_Splash"];

/// Every texture the loading packages and the title screen name, once,
/// by hash.
pub fn collect_loading(assets: &crate::frontend::Assets) -> Vec<Texture> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for name in LOADING_PACKAGES.iter().chain(TITLE_PACKAGES) {
        let Some(p) = assets.package(name) else { continue };
        for r in p.resources.iter().filter(|r| matches!(r.kind, crate::frontend::feng::ResourceKind::Texture)) {
            if let Some(h) = assets.texture_for(&r.name)
                && seen.insert(h)
            {
                out.push(assets.textures[&h].clone());
            }
        }
    }
    out.sort_by_key(|t| t.hash);
    out
}

/// The file of `dir` the loader would take for `t` (`<HASH>.png`,
/// `<HASH>.dds`, `<NAME>.png`; `render::texture_pack::load`'s order).
pub fn pack_file(dir: &Path, t: &Texture) -> Option<PathBuf> {
    [format!("{:08X}.png", t.hash), format!("{:08X}.dds", t.hash), format!("{}.png", t.name)].into_iter().map(|f| dir.join(f)).find(|p| p.is_file())
}

/// The loading textures `hd` (the HD UI pack) does not cover,
/// upscaled into `out` as `<HASH>.png` (lossless; the UI loader makes the
/// mips) with the [`Category::UiPicture`] parameters, and a manifest.
/// Covered, flat and small ones are listed and left out. Unchanged
/// sources are kept unless `force`.
pub fn generate_ui(textures: &[Texture], hd: Option<&Path>, out: &Path, force: bool) -> Result<Summary> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let t0 = Instant::now();
    let previous: HashMap<u32, String> = std::fs::read_to_string(out.join(MANIFEST))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("hash"))
        .filter_map(|l| Some((u32::from_str_radix(l.split('\t').next()?, 16).ok()?, l.to_string())))
        .collect();
    let cat = Category::UiPicture;
    let mut rows = Vec::new();
    for t in textures {
        let key = source_key(t) ^ 0x5549_0000;
        let path = out.join(format!("{:08X}.png", t.hash));
        let mut row = Row { hash: t.hash, name: t.name.clone(), size: format!("{}x{}", t.width, t.height), category: cat, verdict: Verdict::Category, out: String::new(), format: "PNG".into(), de: (0.0, 0.0, 0.0, 0), rank: 0.0, key, bytes: 0, orig_bytes: t.data.len() };
        // The HD UI pack's own (hand-made) file wins: nothing to make.
        if hd.and_then(|d| pack_file(d, t)).is_some() {
            row.verdict = Verdict::Covered;
            row.format = "HD UI pack".into();
            let _ = std::fs::remove_file(&path);
            rows.push(row);
            continue;
        }
        let rgba = t.decode_rgba();
        let scale = match up::pre_verdict(t, cat, &rgba) {
            Ok(s) => s,
            Err(v) => {
                row.verdict = v;
                let _ = std::fs::remove_file(&path);
                rows.push(row);
                continue;
            }
        };
        if !force
            && path.is_file()
            && let Some(prev) = previous.get(&t.hash)
        {
            let f: Vec<&str> = prev.split('\t').collect();
            if f.len() > 12 && f[12] == format!("{key:016x}") && f[4] == "Upscaled" {
                row.verdict = Verdict::Upscaled;
                row.out = f[5].into();
                row.de = (f[7].parse().unwrap_or(0.0), f[8].parse().unwrap_or(0.0), f[9].parse().unwrap_or(0.0), f[10].parse().unwrap_or(0));
                row.bytes = std::fs::metadata(&path).map_or(0, |m| m.len() as usize);
                rows.push(row);
                continue;
            }
        }
        let use_ = up::AlphaUse::of(t);
        let u = up::upscale(&rgba, t.width, t.height, scale, &cat.params(), up::is_dxt(t), use_);
        row.de = up::low_frequency_delta_e(&rgba, t.width, t.height, &u, use_);
        row.out = format!("{}x{}", u.width, u.height);
        if row.de.0 > up::MAX_MEAN_DELTA_E || row.de.1 > up::MAX_P99_DELTA_E {
            row.verdict = Verdict::Colour;
            let _ = std::fs::remove_file(&path);
            rows.push(row);
            continue;
        }
        write_png(&path, &u.rgba, u.width, u.height)?;
        row.bytes = std::fs::metadata(&path).map_or(0, |m| m.len() as usize);
        row.verdict = Verdict::Upscaled;
        rows.push(row);
    }
    let mut m = format!("# {HEADER} v{} ({} textures, loading and title screens)\n", up::GENERATOR_VERSION, rows.len());
    m += "hash\tname\tsize\tcategory\tverdict\tout\tformat\tde_mean\tde_p99\tde_max\talpha_max\tnear_road_m2\tsource_key\n";
    for r in &rows {
        m += &format!("{:08X}\t{}\t{}\t{:?}\t{:?}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{}\t{:.0}\t{:016x}\n", r.hash, r.name, r.size, r.category, r.verdict, r.out, r.format, r.de.0, r.de.1, r.de.2, r.de.3, r.rank, r.key);
    }
    write_manifest(out, &m)?;
    Ok(Summary { rows, seconds: t0.elapsed().as_secs_f32() })
}

/// An RGBA image as a PNG at `path`, whole or not at all: encoded in
/// memory, written to a temporary name unique per write in the same folder
/// and renamed into place (`cache::replace_file`), so two runs writing one
/// pack at once never share a temporary file.
fn write_png(path: &Path, rgba: &[u8], width: u32, height: u32) -> std::io::Result<()> {
    let mut png = std::io::Cursor::new(Vec::new());
    image::write_buffer_with_format(&mut png, rgba, width, height, image::ColorType::Rgba8, image::ImageFormat::Png)
        .map_err(std::io::Error::other)?;
    crate::cache::replace_file(path, &png.into_inner())
}

/// `text` as [`MANIFEST`] in `out`, whole or not at all: written to a
/// temporary name unique per write and renamed into place
/// (`cache::replace_file`), so two runs writing one folder at once never
/// share a temporary file (each leaves its own whole manifest; the last
/// rename wins).
fn write_manifest(out: &Path, text: &str) -> std::io::Result<()> {
    crate::cache::replace_file(&out.join(MANIFEST), text.as_bytes())
}

// ------------------------------------------------------------ the job

/// The menu's background run: world then cars into [`pack_dir`].
struct Job {
    progress: Arc<Progress>,
    result: Arc<Mutex<Option<std::result::Result<String, String>>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

static JOB: Mutex<Option<Job>> = Mutex::new(None);

/// What the job is doing, for the detail line.
#[derive(Clone, Debug, PartialEq)]
pub enum JobStatus {
    Idle,
    Running(String),
    Cancelling,
    Done(String),
    Failed(String),
}

/// Run the job body so that a panic becomes a `Failed` result instead of a
/// thread that never stores one (the row would stay "Running" forever).
fn guarded(run: impl FnOnce() -> Result<String>, cancel: &AtomicBool) -> Result<String, String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(r) => r.map_err(|e| if cancel.load(Ordering::Relaxed) { "Cancelled.".to_string() } else { format!("Failed: {e:#}") }),
        Err(p) => {
            let why = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "panic".into());
            Err(format!("Failed: {why}"))
        }
    }
}

pub fn job_status() -> JobStatus {
    let job = JOB.lock().unwrap_or_else(|e| e.into_inner());
    let Some(j) = job.as_ref() else { return JobStatus::Idle };
    if let Some(r) = j.result.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return match r {
            Ok(s) => JobStatus::Done(s.clone()),
            Err(e) => JobStatus::Failed(e.clone()),
        };
    }
    if j.progress.cancel.load(Ordering::Relaxed) { JobStatus::Cancelling } else { JobStatus::Running(j.progress.line()) }
}

pub fn job_running() -> bool {
    matches!(job_status(), JobStatus::Running(_) | JobStatus::Cancelling)
}

/// Accept on "Generate Texture Pack": start a run, or cancel the one
/// running. The run takes half the CPU threads at a lower priority, so the
/// game keeps its frame rate.
pub fn toggle_job() {
    let mut job = JOB.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(j) = job.as_ref()
        && j.result.lock().unwrap_or_else(|e| e.into_inner()).is_none()
    {
        j.progress.cancel.store(true, Ordering::Relaxed);
        return;
    }
    if let Some(mut old) = job.take()
        && let Some(h) = old.handle.take()
    {
        let _ = h.join();
    }
    let progress = Arc::new(Progress::default());
    let result = Arc::new(Mutex::new(None));
    let (p, r) = (progress.clone(), result.clone());
    let root = crate::game_root();
    let (world, car) = (world_dir(), car_dir());
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get() / 2).clamp(1, 8);
    let handle = std::thread::Builder::new()
        .name("texture-pack".into())
        .spawn(move || {
            let run = || -> Result<String> {
                let tracks: Vec<String> = ALL_TRACKS.iter().map(|s| s.to_string()).collect();
                let w = collect_world(&root, &tracks, &p)?;
                let ws = generate(&w, &world, "World textures", false, threads, true, &p)?;
                p.set("Reading the cars", 0);
                let c = collect_cars(&root)?;
                let cs = generate(&c, &car, "Car textures", false, threads, true, &p)?;
                Ok(format!("Done: {} world and {} car textures, {:.0} MB.", ws.upscaled(), cs.upscaled(), (ws.pack_bytes() + cs.pack_bytes()) as f64 / 1048576.0))
            };
            let out = guarded(run, &p.cancel);
            match &out {
                Ok(s) => log::info!("texture pack: {s}"),
                Err(e) => log::warn!("texture pack: {e}"),
            }
            REFRESH.store(true, Ordering::Relaxed);
            *r.lock().unwrap_or_else(|e| e.into_inner()) = Some(out);
        })
        .expect("spawning the texture-pack thread");
    *job = Some(Job { progress, result, handle: Some(handle) });
}

/// Stop a running job and wait for it (the game's exit).
pub fn stop_job() {
    let mut job = JOB.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut j) = job.take() {
        j.progress.cancel.store(true, Ordering::Relaxed);
        if let Some(h) = j.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panicking_job_reports_failed() {
        let c = AtomicBool::new(false);
        assert_eq!(guarded(|| panic!("boom"), &c), Err("Failed: boom".to_string()));
        assert_eq!(guarded(|| Ok("ok".into()), &c), Ok("ok".to_string()));
        assert!(guarded(|| bail!("bad"), &c).unwrap_err().starts_with("Failed: bad"));
        c.store(true, Ordering::Relaxed);
        assert_eq!(guarded(|| bail!("x"), &c), Err("Cancelled.".to_string()));
    }

    /// Two writers of one pack picture at once: each write is whole (the
    /// file always decodes to one of the two images), no temporary file is
    /// left behind, and the bytes are the PNG `image::save_buffer` writes.
    #[test]
    fn png_writes_at_once_never_share_a_temporary_file() {
        let d = std::env::temp_dir().join(format!("ob-upscale-png-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let path = d.join("0000ABCD.png");
        let (w, h) = (64u32, 32u32);
        let image = |v: u8| vec![v; (w * h * 4) as usize];
        std::thread::scope(|s| {
            for v in [10u8, 200] {
                let (path, rgba) = (&path, image(v));
                s.spawn(move || {
                    for _ in 0..40 {
                        write_png(path, &rgba, w, h).unwrap();
                    }
                });
            }
        });
        let got = image::open(&path).unwrap().to_rgba8().into_raw();
        assert!(got == image(10) || got == image(200));
        let names: Vec<String> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["0000ABCD.png"], "only the picture is left");
        let reference = d.join("reference.png");
        image::save_buffer_with_format(&reference, &image(10), w, h, image::ColorType::Rgba8, image::ImageFormat::Png).unwrap();
        write_png(&path, &image(10), w, h).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&reference).unwrap());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn manifest_categories_follow_the_file() {
        let d = std::env::temp_dir().join(format!("ob-manifest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        assert!(manifest_categories(&d).is_empty());
        std::fs::write(d.join(MANIFEST), "hash\tx\ty\tBuilding\n0000abcd\ta\tb\tBuilding\n").unwrap();
        assert_eq!(manifest_categories(&d).get(&0xabcd), Some(&Category::Building));
        std::fs::write(d.join(MANIFEST), "hash\tx\ty\tBuilding\n0000abcd\ta\tb\tRoad\n0000abce\ta\tb\tRoad\n").unwrap();
        assert_eq!(manifest_categories(&d).len(), 2);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn cap_holds_bulk_categories_to_2x_only() {
        assert_eq!(capped_levels(Some(Category::Building), 1024, 256, Some(2)), 1);
        assert_eq!(capped_levels(Some(Category::Building), 512, 256, Some(2)), 0);
        assert_eq!(capped_levels(Some(Category::Road), 1024, 256, Some(2)), 0);
        assert_eq!(capped_levels(Some(Category::Sign), 1024, 256, Some(2)), 0);
        assert_eq!(capped_levels(Some(Category::Building), 1024, 256, None), 0);
        assert_eq!(capped_levels(None, 1024, 256, Some(2)), 0);
        assert_eq!(capped_levels(Some(Category::CarPart), 256, 64, Some(2)), 1);
    }

    #[test]
    fn choice_and_cap() {
        // The statics are process-wide: restore them.
        let before = choice();
        set_choice(0);
        assert_eq!(cap(), None);
        set_choice(2);
        assert_eq!(cap(), Some(2));
        set_choice(9);
        assert_eq!(choice(), 2);
        set_choice(before);
    }

    #[test]
    fn categories_round_trip_through_the_manifest_names() {
        for c in [Category::Road, Category::Building, Category::CarMask, Category::CarDecal, Category::UiPicture] {
            assert_eq!(category_named(&format!("{c:?}")), Some(c));
        }
    }

    fn ui_tex(name: &str, hash: u32, w: u32, h: u32, rgba: Vec<u8>) -> Texture {
        use crate::formats::texture::{Blend, PixelFormat};
        Texture { name: name.into(), hash, width: w, height: h, format: PixelFormat::Rgba8, mip_count: 1, data: rgba.into(), palette: vec![], alpha_test: false, blend: Blend::Alpha, rendering_order: 0, late_buckets: false, scroll: Default::default() }
    }

    /// The UI folder takes what the HD UI pack lacks (4×, PNG, the
    /// source's layout), leaves the pack's own and flat / small ones out,
    /// and reuses an unchanged file.
    #[test]
    fn loading_ui_pack_fills_only_the_gaps() {
        let base = std::env::temp_dir().join(format!("u6-ui-pack-{}", std::process::id()));
        let (hd, out) = (base.join("hd"), base.join("ui"));
        std::fs::create_dir_all(&hd).unwrap();
        // Smooth and periodic (the UI samples with WRAP, so the upscale wraps).
        let wave = |v: u32| (128.0 + 90.0 * (v as f32 * std::f32::consts::TAU / 32.0).sin()) as u8;
        let smooth: Vec<u8> = (0..32 * 32).flat_map(|i| [wave(i % 32), wave(i / 32), 128, 255]).collect();
        let picture = ui_tex("PICTURE", 0x1111_0001, 32, 32, smooth.clone());
        let covered = ui_tex("COVERED", 0x1111_0002, 32, 32, smooth);
        let flat = ui_tex("FLAT", 0x1111_0003, 32, 32, vec![200; 32 * 32 * 4]);
        let small = ui_tex("SMALL", 0x1111_0004, 32, 8, (0..32 * 8 * 4).map(|i| i as u8).collect());
        image::RgbaImage::new(4, 4).save(hd.join("11110002.png")).unwrap();
        let all = [picture, covered, flat, small];
        let s = generate_ui(&all, Some(&hd), &out, false).unwrap();
        let v: Vec<Verdict> = s.rows.iter().map(|r| r.verdict).collect();
        assert_eq!(v, [Verdict::Upscaled, Verdict::Covered, Verdict::Flat, Verdict::Small]);
        assert_eq!(image::image_dimensions(out.join("11110001.png")).unwrap(), (128, 128));
        assert!(!out.join("11110002.png").exists() && !out.join("11110003.png").exists());
        assert!(s.rows[0].de.0 <= up::MAX_MEAN_DELTA_E);
        // A rerun keeps the file (same source, same generator).
        let t = std::fs::metadata(out.join("11110001.png")).unwrap().modified().unwrap();
        let again = generate_ui(&all, Some(&hd), &out, false).unwrap();
        assert_eq!(again.rows[0].verdict, Verdict::Upscaled);
        assert_eq!(std::fs::metadata(out.join("11110001.png")).unwrap().modified().unwrap(), t);
        // No HD UI pack: the covered one is made too.
        let alone = generate_ui(&all, None, &base.join("alone"), false).unwrap();
        assert_eq!(alone.rows[1].verdict, Verdict::Upscaled);
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// The loading screens' textures from the player's files: PC_load.tga
    /// (C51CE6F1, 1024 × 1024, the picture PC_Loading draws) is among them.
    #[test]
    fn loading_textures_include_the_picture() {
        let Some(root) = game() else { return };
        let assets = crate::frontend::Assets::load(&root).unwrap();
        let v = collect_loading(&assets);
        let pc = v.iter().find(|t| t.hash == 0xC51C_E6F1).expect("PC_LOAD");
        assert_eq!((pc.name.as_str(), pc.width, pc.height), ("PC_LOAD", 1024, 1024));
        assert_eq!(up::pre_verdict(pc, Category::UiPicture, &pc.decode_rgba()), Ok(4));
    }

    /// The title screen's textures (UG_LS_Splash: PC_load.tga, the
    /// gradient, White16x16) are collected too, so its picture gets HD.
    #[test]
    fn title_screen_textures_are_collected() {
        let Some(root) = game() else { return };
        let assets = crate::frontend::Assets::load(&root).unwrap();
        let v: HashSet<u32> = collect_loading(&assets).iter().map(|t| t.hash).collect();
        let p = assets.package("UG_LS_Splash").expect("UG_LS_Splash");
        let named: Vec<u32> = p.resources.iter().filter(|r| matches!(r.kind, crate::frontend::feng::ResourceKind::Texture)).filter_map(|r| assets.texture_for(&r.name)).collect();
        assert!(named.contains(&0xC51C_E6F1), "the splash draws PC_LOAD");
        assert!(named.iter().all(|h| v.contains(h)), "{named:08X?}");
    }

    fn game() -> Option<PathBuf> {
        std::env::var_os("NFSU2_ROOT").map(PathBuf::from).filter(|r| r.join("CARS/TEXTURES.BIN").is_file())
    }

    /// The car sources (G7b2): the racers' composited skins and painted rims
    /// never go in the pack; every compositor mask present is a CarMask;
    /// lamps are left out by the review; decals are not judged flat by their
    /// colour alone.
    #[test]
    fn car_sources_keep_composites_out_and_class_masks() {
        let Some(root) = game() else { return };
        let v = collect_cars(&root).unwrap();
        let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let types = crate::car::parse_car_types(&globalb).unwrap();
        let (_, masks) = crate::car::wheel_composite_hashes(&root).unwrap();
        let by: HashMap<u32, &Source> = v.iter().map(|s| (s.tex.hash, s)).collect();
        for t in types.iter().filter(|t| t.composited_skin) {
            assert!(by.get(&crate::car::skin_hash(&t.name)).is_none_or(|s| s.category == Category::Skip), "{}", t.name);
        }
        let present: Vec<&u32> = masks.iter().filter(|h| by.contains_key(h)).collect();
        assert!(present.len() > 50, "{}", present.len());
        assert!(present.iter().all(|h| by[h].category == Category::CarMask));
        assert!(v.iter().filter(|s| s.category == Category::CarSkin).all(|s| types.iter().any(|t| !t.composited_skin && s.tex.name.starts_with(&t.name))));
        assert!(v.iter().any(|s| s.category == Category::CarLamp));
        assert!(v.iter().filter(|s| s.tex.name.contains("NEON") || s.tex.name.contains("SHADOW") || s.tex.name.contains("DOORLINE") || s.tex.name.starts_with("DUMMY_")).all(|s| s.category == Category::Skip));
        let decal = v.iter().find(|s| s.category == Category::CarDecal && s.tex.name.ends_with("_RECT")).unwrap();
        assert!(pre_verdict(&decal.tex, Category::CarDecal, &decal.tex.decode_rgba()).is_ok(), "{}", decal.tex.name);
    }

    /// A run writes `<HASH>.dds` + the manifest; a second run reuses the
    /// files; a cancelled run stops with an error and writes no manifest.
    #[test]
    fn generate_writes_reuses_and_cancels() {
        let Some(root) = game() else { return };
        let all = collect_cars(&root).unwrap();
        let mut pick: Vec<Source> = Vec::new();
        for c in [Category::CarRim, Category::CarMask, Category::CarDecal, Category::CarSkin, Category::CarLamp, Category::Skip] {
            if let Some(s) = all.iter().find(|s| s.category == c) {
                pick.push(Source { tex: s.tex.clone(), category: s.category, rank: 0.0 });
            }
        }
        let dir = std::env::temp_dir().join(format!("nfsu2-packgen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = Progress::default();
        let first = generate(&pick, &dir, "test", false, 2, false, &p).unwrap();
        assert_eq!(p.done.load(Ordering::Relaxed), pick.len());
        let done: Vec<&Row> = first.rows.iter().filter(|r| r.verdict == Verdict::Upscaled).collect();
        assert!(done.len() >= 3, "{}", done.len());
        for r in &done {
            let e = up::parse_dds(&std::fs::read(dir.join(format!("{:08X}.dds", r.hash))).unwrap()).unwrap();
            let src = &pick.iter().find(|s| s.tex.hash == r.hash).unwrap().tex;
            assert_eq!(e.width % src.width, 0);
            assert!(e.width > src.width && e.mips > 1);
        }
        assert!(first.rows.iter().any(|r| r.verdict == Verdict::NoGain), "lamps rejected");
        assert_eq!(manifest_categories(&dir).len(), pick.len());
        let second = generate(&pick, &dir, "test", false, 2, false, &p).unwrap();
        assert_eq!(second.upscaled(), first.upscaled());
        let p = Progress::default();
        p.cancel.store(true, Ordering::Relaxed);
        let other = dir.join("cancelled");
        assert!(generate(&pick, &other, "test", true, 2, false, &p).is_err());
        assert!(!other.join(MANIFEST).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

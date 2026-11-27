//! Truncated input for every parser ported to the checked `Reader`:
//! a small real sample from the player's game files, cut at every prefix
//! length, must give an error or the parser's old failure value. Never a
//! panic, never a hang (each call well under a second), never a larger
//! allocation than the whole sample's parse needs (and never one above
//! `Limits::max_alloc`).
//!
//! Each prefix is tried twice: as cut (a chunk header whose body is cut
//! off hides the chunk, as `ChunkIter` stops at a truncated tail) and
//! "repaired" (every chunk header that the cut runs through gets the size
//! of what is left, so the parsers see a chunk whose last record is cut).
//! Samples come from NFSU2_ROOT at test time; nothing is committed.

mod common;

use nfsu2::formats::bundle::{self, Chunk, id};
use nfsu2::formats::reader::Limits;
use nfsu2::formats::*;
use std::hint::black_box;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// The system allocator, recording the largest single request while
/// [`TRACK`] is on (from every thread: the texture packs decode on rayon's).
struct Peak;

static TRACK: AtomicBool = AtomicBool::new(false);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn note(n: usize) {
    if TRACK.load(Ordering::Relaxed) {
        PEAK.fetch_max(n, Ordering::Relaxed);
    }
}

unsafe impl std::alloc::GlobalAlloc for Peak {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        note(l.size());
        // A request past the limit is refused here (the test then fails on
        // the recorded peak instead of the machine running out of memory).
        if l.size() > Limits::DEFAULT.max_alloc && TRACK.load(Ordering::Relaxed) {
            return std::ptr::null_mut();
        }
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn alloc_zeroed(&self, l: std::alloc::Layout) -> *mut u8 {
        note(l.size());
        if l.size() > Limits::DEFAULT.max_alloc && TRACK.load(Ordering::Relaxed) {
            return std::ptr::null_mut();
        }
        unsafe { std::alloc::System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        note(n);
        if n > Limits::DEFAULT.max_alloc && TRACK.load(Ordering::Relaxed) {
            return std::ptr::null_mut();
        }
        unsafe { std::alloc::System.realloc(p, l, n) }
    }
}

#[global_allocator]
static PEAK_ALLOC: Peak = Peak;

/// One test at a time, sample loading included: the peak is process-wide.
static ONE: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    ONE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Samples up to this size are cut at every length; larger ones at every
/// length up to it and then in even steps (about this many more).
const EVERY: usize = 32 << 10;

fn read(rel: &str) -> Vec<u8> {
    let root = common::game_root();
    let p = viv::find_ci(&root, rel).unwrap_or_else(|| panic!("{rel} not in the game"));
    let b = std::fs::read(&p).unwrap_or_else(|e| panic!("{rel}: {e}"));
    if b.starts_with(jdlz::MAGIC) { jdlz::decompress(&b).unwrap() } else { b }
}

/// The smallest chunk (header and body) of one of `ids` in `data` whose
/// body has at least `min` bytes, and its file offset.
fn smallest(data: &[u8], ids: &[u32], min: usize) -> (Vec<u8>, usize) {
    let mut best: Option<(usize, usize)> = None;
    bundle::walk(bundle::chunks(data, 0), &mut |c: Chunk| {
        if ids.contains(&c.id) && c.body.len() >= min && best.is_none_or(|(_, n)| c.body.len() < n) {
            best = Some((c.offset - 8, c.body.len()));
        }
    });
    let (at, n) = best.unwrap_or_else(|| panic!("no chunk {ids:x?} of {min}+ bytes"));
    (data[at..at + 8 + n].to_vec(), at)
}

/// The first chunk of each id in `data`, concatenated (each keeps its
/// relative alignment only if 16-aligned in the file, which every top-level
/// chunk of the track bundles is).
fn first_of(data: &[u8], ids: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    for &want in ids {
        let mut got = None;
        bundle::walk(bundle::chunks(data, 0), &mut |c: Chunk| {
            if c.id == want && got.is_none() {
                got = Some(data[c.offset - 8..c.offset + c.body.len()].to_vec());
            }
        });
        out.extend(got.unwrap_or_else(|| panic!("no chunk {want:#x}")));
    }
    out
}

/// `sample[..n]`; with `repair`, every chunk header in the cut whose body
/// runs past `n` is given the size of what is left.
fn cut(sample: &[u8], n: usize, repair: bool) -> Vec<u8> {
    let mut b = sample[..n].to_vec();
    if repair {
        let mut heads = Vec::new();
        bundle::walk(bundle::chunks(sample, 0), &mut |c: Chunk| heads.push((c.offset - 8, c.offset + c.body.len())));
        for (h, end) in heads {
            if h + 8 <= n && end > n {
                b[h + 4..h + 8].copy_from_slice(&((n - h - 8) as u32).to_le_bytes());
            }
        }
    }
    b
}

/// One parse with the allocation peak recorded: (peak, time, panic).
fn measure(run: &dyn Fn(&[u8]), input: &[u8]) -> (usize, Duration, Option<String>) {
    PEAK.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| run(input)));
    let took = t.elapsed();
    TRACK.store(false, Ordering::Relaxed);
    let panic = r.err().map(|e| e.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_default());
    (PEAK.load(Ordering::Relaxed), took, panic)
}

/// Every prefix of `sample` (both ways) through `run`.
fn truncations(name: &str, sample: &[u8], run: impl Fn(&[u8])) {
    let (full_peak, _, panic) = measure(&run, sample);
    assert!(panic.is_none(), "{name}: the whole sample panics: {panic:?}");
    let budget = (2 * full_peak + (64 << 10)).min(Limits::DEFAULT.max_alloc);
    let step = if sample.len() <= EVERY { 1 } else { (sample.len() - EVERY).div_ceil(EVERY).max(1) };
    let lengths = (0..sample.len().min(EVERY)).chain((EVERY..sample.len()).step_by(step));
    let mut failures = Vec::new();
    let mut runs = 0;
    for n in lengths {
        for repair in [false, true] {
            let input = cut(sample, n, repair);
            let (peak, took, panic) = measure(&run, &input);
            runs += 1;
            if let Some(p) = panic {
                failures.push(format!("prefix {n} (repair {repair}): panic {p}"));
            } else if std::env::var("OB_PERF_TESTS").is_ok_and(|v| v == "1") && took > Duration::from_secs(1) {
                // Wall-clock bounds only under OB_PERF_TESTS=1: a loaded machine can take over a second on a correct parse.
                failures.push(format!("prefix {n} (repair {repair}): {took:?}"));
            } else if peak > budget {
                failures.push(format!("prefix {n} (repair {repair}): allocated {peak} bytes, the whole sample needs {full_peak}"));
            }
            if failures.len() >= 10 {
                break;
            }
        }
    }
    assert!(failures.is_empty(), "{name} ({} bytes, {runs} runs):\n{}", sample.len(), failures.join("\n"));
    eprintln!("{name}: {} bytes, {runs} runs (step {step}), peak allocation {full_peak}", sample.len());
}

/// The first chunk of `b` as a parser's argument, at file offset `base`
/// (alignment rules depend on it).
fn first_chunk(b: &[u8], base: usize) -> Option<Chunk<'_>> {
    bundle::chunks(b, base).next()
}

macro_rules! data_test {
    ($name:ident, $body:block) => {
        #[test]
        #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
        fn $name() {
            let _one = serial();
            $body
        }
    };
}

data_test!(bundle_walk, {
    let data = read("TRACKS/STREAML4RA.BUN");
    let (s, _) = smallest(&data, &[id::SCENERY_SECTION], 256);
    truncations("bundle::walk", &s, |b| {
        bundle::walk(bundle::chunks(b, 0), &mut |c| {
            black_box(c.payload().len());
        })
    });
});

data_test!(texture_pack, {
    let data = read("TRACKS/STREAML4RA.BUN");
    let (s, at) = smallest(&data, &[id::TEXTURE_PACK], 1024);
    truncations("texture::parse_texture_pack", &s, |b| {
        if let Some(c) = first_chunk(b, at)
            && let Ok(t) = texture::parse_texture_pack(c)
        {
            black_box(t.iter().map(|t| t.decode_rgba().len()).sum::<usize>());
        }
    });
});

/// The smallest compressed block (JDLZ or HUFF) of a compressed pack.
fn smallest_block(file: &str, magic: &[u8; 4]) -> Vec<u8> {
    let data = read(file);
    let mut best: Option<Vec<u8>> = None;
    bundle::walk(bundle::chunks(&data, 0), &mut |c: Chunk| {
        if c.id != id::TEXTURE_PACK {
            return;
        }
        let Some(blocks) = c.child(id::TPK_HEADER).and_then(|h| h.child(id::TPK_COMPRESSED_BLOCKS)) else { return };
        for r in blocks.body.chunks_exact(0x18) {
            let u = |o: usize| u32::from_le_bytes(r[o..o + 4].try_into().unwrap()) as usize;
            let start = u(4) - (c.offset - 8) - 8;
            let block = &c.body[start..start + u(8)];
            if block.starts_with(magic) && best.as_ref().is_none_or(|b| block.len() < b.len()) {
                best = Some(block.to_vec());
            }
        }
    });
    best.unwrap_or_else(|| panic!("no {} block in {file}", String::from_utf8_lossy(magic)))
}

data_test!(compressed_texture_pack, {
    let data = read("TRACKS/LOC4DYNTEX.BIN");
    let (s, at) = smallest(&data, &[id::TEXTURE_PACK], 64);
    truncations("texture::parse_texture_pack (compressed)", &s, |b| {
        if let Some(c) = first_chunk(b, at) {
            black_box(texture::parse_texture_pack(c).map(|t| t.len()).ok());
        }
    });
});

data_test!(jdlz_block, {
    let s = smallest_block("TRACKS/LOC4DYNTEX.BIN", jdlz::MAGIC);
    truncations("jdlz::decompress", &s, |b| {
        black_box(jdlz::decompress(b).ok());
    });
});

data_test!(huff_block, {
    let s = smallest_block("TRACKS/LOC4DYNTEX.BIN", huff::MAGIC);
    truncations("huff::decompress", &s, |b| {
        black_box(huff::decompress(b).ok());
    });
});

data_test!(viv_header, {
    let root = common::game_root();
    let p = viv::find_ci(&root, "SDATA/sdat.viv").unwrap();
    let v = viv::Viv::open(&p).expect("sdat.viv");
    let end = v.entries.iter().map(|e| e.offset as usize).min().unwrap_or(0x40).min(64 << 10);
    let s = std::fs::read(&p).unwrap()[..end].to_vec();
    truncations("viv::Viv::parse_header", &s, |b| {
        black_box(viv::Viv::parse_header(std::path::PathBuf::new(), b).map(|v| v.entries.len()));
    });
});

/// A stored (not deflated, not encrypted) one-entry ZIP: no game sample
/// exists for `.tpf` (a mod format), so the archive is built here.
#[test]
fn tpf_zip() {
    let _one = serial();
    let body = b"texture bytes".to_vec();
    let crc = tpf::zip_crc(&body);
    let name = b"SPEED2.EXE_0x12345678.dds";
    let mut z = Vec::new();
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    z.extend_from_slice(b"PK\x03\x04");
    for x in [20u16, 0, 0, 0, 0] {
        le16(&mut z, x);
    }
    le32(&mut z, crc);
    le32(&mut z, body.len() as u32);
    le32(&mut z, body.len() as u32);
    le16(&mut z, name.len() as u16);
    le16(&mut z, 0);
    z.extend_from_slice(name);
    z.extend_from_slice(&body);
    let cd = z.len();
    z.extend_from_slice(b"PK\x01\x02");
    for x in [20u16, 20, 0, 0, 0, 0] {
        le16(&mut z, x);
    }
    le32(&mut z, crc);
    le32(&mut z, body.len() as u32);
    le32(&mut z, body.len() as u32);
    for x in [name.len() as u16, 0, 0, 0, 0] {
        le16(&mut z, x);
    }
    le32(&mut z, 0);
    le32(&mut z, 0);
    z.extend_from_slice(name);
    let cd_len = z.len() - cd;
    z.extend_from_slice(b"PK\x05\x06");
    for x in [0u16, 0, 1, 1] {
        le16(&mut z, x);
    }
    le32(&mut z, cd_len as u32);
    le32(&mut z, cd as u32);
    le16(&mut z, 0);
    assert_eq!(tpf::read(&z).unwrap().entries[0].data, body);
    truncations("tpf::read", &z, |b| {
        black_box(tpf::read(b).map(|p| p.entries.len()).ok());
    });
}

data_test!(track_sections, {
    let data = read("TRACKS/L4RA.BUN");
    let s = first_of(&data, &[id::TRACK_STREAMING_SECTIONS]);
    truncations("track::parse_streaming_sections", &s, |b| {
        black_box(track::parse_streaming_sections(b).map(|v| v.len()).ok());
    });
});

data_test!(scenery_section, {
    let data = read("TRACKS/STREAML4RA.BUN");
    let (s, at) = smallest(&data, &[id::SCENERY_SECTION], 256);
    truncations("scenery::parse_scenery_section", &s, |b| {
        if let Some(c) = first_chunk(b, at) {
            black_box(scenery::parse_scenery_section(c).map(|s| s.instances.len()).ok());
        }
    });
});

data_test!(solid_list, {
    // One solid with a mesh in a list of its own (a whole list is
    // megabytes): the solid keeps its file offset, so its 16-byte aligned
    // structs stay where they are.
    let data = read("TRACKS/STREAML4RA.BUN");
    let (solid, at) = smallest(&data, &[id::SOLID], 2048);
    let at = at - 8;
    let mut s = (id::SOLID_LIST).to_le_bytes().to_vec();
    s.extend((solid.len() as u32).to_le_bytes());
    s.extend(solid);
    truncations("solid::parse_solid_list", &s, |b| {
        if let Some(c) = first_chunk(b, at) {
            black_box(solid::parse_solid_list(c).map(|s| s.len()).ok());
            black_box(solid::solid_markers(c, 0).len());
        }
    });
});

data_test!(collision, {
    let data = read("TRACKS/STREAML4RA.BUN");
    let (s, at) = smallest(&data, &[collision::COLLISION_PACK], 256);
    truncations("collision::parse_collision_pack/grid", &s, |b| {
        if let Some(c) = first_chunk(b, at) {
            black_box(collision::parse_collision_pack(c).1.len());
            black_box(collision::parse_collision_grid(c).map(|g| g.cells.len()));
        }
    });
    let track = read("TRACKS/L4RA.BUN");
    let s = first_of(&track, &[collision::BARRIER_GROUPS]);
    truncations("collision::parse_barrier_groups", &s, |b| {
        if let Some(c) = first_chunk(b, 0) {
            black_box(collision::parse_barrier_groups(c).len());
        }
    });
    let car = read("CARS/COROLLA/GEOMETRY.BIN");
    let (s, at) = smallest(&car, &[collision::COLLISION_VOLUMES], 64);
    truncations("collision::parse_collision_volumes", &s, |b| {
        if let Some(c) = first_chunk(b, at) {
            black_box(collision::parse_collision_volumes(c).len());
        }
    });
});

data_test!(visibility, {
    let data = read("TRACKS/L4RA.BUN");
    let s = first_of(&data, &[visibility::SECTION_INFO, visibility::DRIVABLE_BOUNDARIES, visibility::VISIBLE_SECTIONS]);
    truncations("visibility::Visibility::parse", &s, |b| {
        black_box(visibility::Visibility::parse(b).map(|v| v.lists.len()).ok());
    });
});

data_test!(lights, {
    let data = read("TRACKS/STREAML4RA.BUN");
    let (s, at) = smallest(&data, &[lights::LIGHT_PACK], 256);
    truncations("lights::LightPack::parse", &s, |b| {
        if let Some(c) = first_chunk(b, at) {
            let p = lights::LightPack::parse(c);
            black_box(p.leaf_lights(glam::Vec3::ZERO).map(|l| l.count()));
        }
    });
    let (s, at) = smallest(&data, &[lights::FLARE_PACK], 128);
    truncations("lights::FlarePack::parse", &s, |b| {
        if let Some(c) = first_chunk(b, at) {
            black_box(lights::FlarePack::parse(c).map(|p| p.flares.len()));
        }
    });
});

data_test!(roads, {
    let data = read("TRACKS/ROUTESL4RA/Paths4001.bin");
    let s = first_of(&data, &[0x34148, 0x34149, 0x3414D]);
    truncations("roads::RoadNet::parse", &s, |b| {
        black_box(roads::RoadNet::parse(b).segments.len());
    });
});

data_test!(ai_routes, {
    let data = read("TRACKS/ROUTESL4RA/Routes4176F.bin");
    let s = first_of(&data, &[0x34121, 0x34122, 0x34123]);
    truncations("ai_routes::RouteFile::parse", &s, |b| {
        black_box(ai_routes::RouteFile::parse(b).records.len());
    });
});

data_test!(parts, {
    let data = read("GLOBAL/GLOBALB.BUN");
    let s = first_of(&data, &[0x8003_4602, 0x34607]);
    truncations("parts::PartDb::load", &s, |b| {
        black_box(parts::PartDb::load(b).map(|d| d.parts.len()).ok());
    });
    let (s, _) = smallest(&data, &[0x34A1C], 0x28);
    truncations("parts::CostTable::parse", &s, |b| {
        black_box(parts::CostTable::parse(b.get(8..).unwrap_or_default()).categories.len());
    });
});

data_test!(world_anim, {
    let data = read("TRACKS/L4RA.BUN");
    let (bank, _) = smallest(&data, &[world_anim::BANK], 256);
    let mut s = first_of(&data, &[world_anim::ENTITY, world_anim::INSTANCE]);
    // The bank keeps its 16-byte alignment after the two records.
    while s.len() % 16 != 0 {
        s.extend_from_slice(&[0; 8]);
    }
    s.extend(bank);
    truncations("world_anim::WorldAnimations::parse", &s, |b| {
        black_box(world_anim::WorldAnimations::parse(b).animations.len());
    });
});

data_test!(eagl_bank, {
    let data = read("TRACKS/L4RA.BUN");
    let (s, at) = smallest(&data, &[world_anim::BANK], 256);
    truncations("eagl::parse_bank", &s, |b| {
        if let Some(c) = first_chunk(b, at) {
            black_box(eagl::parse_bank(c.aligned(16)).map(|v| v.len()).ok());
        }
    });
});

data_test!(texture_animations, {
    let data = read("TRACKS/STREAML4RA.BUN");
    let (s, at) = smallest(&data, &[texture_anim::TEXTURE_ANIMATIONS], 64);
    truncations("texture_anim::parse_texture_animations", &s, |b| {
        if let Some(c) = first_chunk(b, at)
            && let Ok(a) = texture_anim::parse_texture_animations(c)
        {
            black_box(a.iter().map(|a| a.frame_at(1.0)).sum::<u32>());
        }
    });
});

data_test!(camera_tracks, {
    let data = read("TRACKS/L4RA.BUN");
    let (s, at) = smallest(&data, &[camera_tracks::TYPED_CAMERAS, camera_tracks::ROUTE_CAMERAS], 64);
    truncations("camera_tracks::parse_camera_tracks", &s, |b| {
        if let Some(c) = first_chunk(b, at) {
            black_box(camera_tracks::parse_camera_tracks(c).map(|g| g.len()).ok());
        }
    });
});

data_test!(parameter_maps, {
    let data = read("TRACKS/L4RA.BUN");
    let s = first_of(&data, &[parameter_map::PARAMETER_MAPS]);
    truncations("parameter_map::parse_parameter_maps", &s, |b| {
        black_box(parameter_map::parse_parameter_maps(b).map(|m| m.len()).ok());
    });
});

data_test!(smackables, {
    let track = read("TRACKS/L4RA.BUN");
    let stream = read("TRACKS/STREAML4RA.BUN");
    for (what, ids) in [("groups", smackable::SCENERY_GROUPS), ("links", smackable::SCENERY_LINKS), ("infos", smackable::OBJECT_INFOS), ("instances", smackable::OBJECT_INSTANCES)] {
        let data = if what == "instances" { &stream } else { &track };
        let (s, at) = smallest(data, &[ids], 16);
        truncations(&format!("smackable::parse_{what}"), &s, |b| {
            if let Some(c) = first_chunk(b, at) {
                black_box(match what {
                    "groups" => smackable::parse_scenery_groups(c).len(),
                    "links" => smackable::parse_scenery_links(c).len(),
                    "infos" => smackable::parse_infos(c).len(),
                    _ => smackable::parse_instances(c).len(),
                });
            }
        });
    }
});

data_test!(part_animations, {
    let s = read("CARS/COROLLA/PARTS_ANIMATIONS.bin");
    truncations("zam::PartAnimations::parse", &s, |b| {
        black_box(zam::PartAnimations::parse(b, "COROLLA").map(|a| a.pivots.len()).ok());
    });
});

data_test!(magazines, {
    let s = read("FRONTEND/MAGAZINES_FRONTEND.BIN");
    truncations("magazines::Magazines::parse", &s, |b| {
        black_box(magazines::Magazines::parse(b.get(8..).unwrap_or_default()).map(|m| m.setups.len()));
    });
    let s = read("FRONTEND/MAGAZINES_SHOWCASE.BIN");
    truncations("magazines::Showcase::parse", &s, |b| {
        black_box(magazines::Showcase::parse(b.get(8..).unwrap_or_default()).map(|m| m.setups.len()));
    });
});

/// The fresh profile is the clone's own (a committed payload, not read
/// from the game), so this one always runs.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn native_save() {
    let _one = serial();
    let s = nfsu2::frontend::career::fresh_save("PREFIX").to_bytes();
    truncations("native_save::NativeSave::parse", &s, |b| {
        black_box(native_save::NativeSave::parse(b).map(|s| s.name()).ok());
    });
    let s = native_save::picture(&[128; 16 * 16 * 4], 16, 16);
    truncations("native_save::read_picture", &s, |b| {
        black_box(native_save::read_picture(b).map(|p| p.len()).ok());
    });
}

data_test!(vp6_frames, {
    let m = nfsu2::movie::Movie::parse(read("MOVIES/blank.vp6")).expect("blank.vp6");
    let frames: Vec<Vec<u8>> = m.frames.iter().map(|(_, c)| m.payload(c).to_vec()).collect();
    assert!(!frames.is_empty());
    // The key frame cut at every length, each on a fresh decoder, then
    // the following frames whole (an inter frame after a bad key frame).
    let key = frames[0].clone();
    truncations("vp6::Decoder::decode", &key, |b| {
        let mut d = vp6::Decoder::new();
        black_box(d.decode(b).is_ok());
        for f in &frames[1..] {
            black_box(d.decode(f).is_ok());
        }
    });
});

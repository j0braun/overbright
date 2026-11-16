//! Fuzzing of the file readers (docs/io-robustness.md, "Fuzzing"):
//! every reader that takes bytes from a game file, a save or options.txt
//! gets malformed input — random bytes (proptest), and real chunks with
//! truncated / oversized / lying chunk sizes, boundary counts and offsets,
//! bit flips and cut-off tails (a seeded mutation engine). A reader must
//! return (an error, an empty result, its defaults), never panic, hang or
//! allocate without bound.
//!
//! The real-data seeds come from NFSU2_ROOT (the tests are ignored without it); the
//! synthetic ones always run. `NFSU2_FUZZ_ITERS=n` (default 1000) sets the
//! mutations per target, `NFSU2_FUZZ_SEED` the engine's seed.

mod common;

use nfsu2::formats::bundle::{self, Chunk, id};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The test binary's allocator: the system's, with a guard — a request
/// past 1 GiB (no reader needs one; a size field taken on trust does)
/// names the target being run and the size, with a backtrace, before the
/// abort the failed allocation brings.
struct Guard;

static RUNNING: std::sync::atomic::AtomicPtr<u8> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

unsafe impl std::alloc::GlobalAlloc for Guard {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        if l.size() > 1 << 30 {
            let name = RUNNING.load(std::sync::atomic::Ordering::Relaxed);
            let name = if name.is_null() { "?" } else { unsafe { std::ffi::CStr::from_ptr(name.cast()).to_str().unwrap_or("?") } };
            // Not eprintln!: libtest captures that, and the abort loses it.
            let _ = std::io::Write::write_all(&mut std::io::stderr(), format!("fuzz: allocation of {} bytes in target {name}\n{}\n", l.size(), std::backtrace::Backtrace::force_capture()).as_bytes());
            return std::ptr::null_mut();
        }
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn alloc_zeroed(&self, l: std::alloc::Layout) -> *mut u8 {
        if l.size() > 1 << 30 {
            return unsafe { self.alloc(l) };
        }
        unsafe { std::alloc::System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        if n > 1 << 30 {
            return unsafe { self.alloc(std::alloc::Layout::from_size_align_unchecked(n, l.align())) };
        }
        unsafe { std::alloc::System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GUARD: Guard = Guard;

/// A reader under test: its name, the chunk ids it is given (empty: whole
/// files / byte streams), and the call.
struct Target {
    name: &'static str,
    ids: &'static [u32],
    run: fn(&[u8]),
}

/// The bytes as a chunk (header and body), as the readers' callers find it.
fn as_chunk(b: &[u8]) -> Option<Chunk<'_>> {
    bundle::chunks(b, 0).next()
}

fn targets() -> Vec<Target> {
    use nfsu2::formats::*;
    vec![
        Target { name: "bundle::walk", ids: &[], run: |b| bundle::walk(bundle::chunks(b, 0), &mut |c| { std::hint::black_box(c.payload().len()); }) },
        Target { name: "jdlz::decompress", ids: &[], run: |b| { let _ = jdlz::decompress(b); } },
        Target { name: "huff::decompress", ids: &[], run: |b| { let _ = huff::decompress(b); } },
        Target { name: "texture::parse_texture_pack", ids: &[id::TEXTURE_PACK], run: |b| if let Some(c) = as_chunk(b) { let _ = texture::parse_texture_pack(c).map(|t| t.iter().map(|t| t.decode_rgba().len()).sum::<usize>()); } },
        Target { name: "solid::parse_solid_list", ids: &[id::SOLID_LIST], run: |b| if let Some(c) = as_chunk(b) { let _ = solid::parse_solid_list(c); let _ = solid::solid_markers(c, 0); } },
        Target { name: "scenery::parse_scenery_section", ids: &[id::SCENERY_SECTION], run: |b| if let Some(c) = as_chunk(b) { let _ = scenery::parse_scenery_section(c); } },
        Target { name: "lights::LightPack/FlarePack", ids: &[lights::LIGHT_PACK, lights::FLARE_PACK], run: |b| if let Some(c) = as_chunk(b) { if c.id == lights::FLARE_PACK { let _ = lights::FlarePack::parse(c); } else { let _ = lights::LightPack::parse(c); } } },
        Target { name: "texture_anim::parse_texture_animations", ids: &[texture_anim::TEXTURE_ANIMATIONS], run: |b| if let Some(c) = as_chunk(b) { let _ = texture_anim::parse_texture_animations(c); } },
        Target { name: "camera_tracks::parse_camera_tracks", ids: &[camera_tracks::ROUTE_CAMERAS, camera_tracks::TYPED_CAMERAS], run: |b| if let Some(c) = as_chunk(b) { let _ = camera_tracks::parse_camera_tracks(c); } },
        Target { name: "character::SkinnedModel::parse_list", ids: &[id::SOLID_LIST], run: |b| if let Some(c) = as_chunk(b) { let _ = nfsu2::character::SkinnedModel::parse_list(c); } },
        Target { name: "track (sections, visibility, parameter maps, world animations)", ids: &[], run: |b| {
            let _ = track::parse_streaming_sections(b);
            let _ = visibility::Visibility::parse(b);
            let _ = parameter_map::parse_parameter_maps(b);
            let _ = world_anim::WorldAnimations::parse(b);
        } },
        Target { name: "routes, roads, magazines", ids: &[], run: |b| {
            let _ = ai_routes::RouteFile::parse(b);
            let _ = roads::RoadNet::parse(b);
            let _ = magazines::Magazines::parse(b);
            let _ = magazines::Showcase::parse(b);
        } },
        Target { name: "globalb (car types, presets, lights, tricks, bounds, parts, particles)", ids: &[], run: |b| {
            let _ = nfsu2::car::parse_car_types(b);
            let _ = nfsu2::car::parse_presets(b);
            let _ = nfsu2::car::parse_light_materials(b);
            let _ = nfsu2::drift::parse_tricks(b);
            let _ = nfsu2::opponents::Bounds::parse(b);
            let _ = parts::PartDb::load(b);
            let _ = nfsu2::particles::parse_descriptions(b);
        } },
        Target { name: "career tables (costs, tuning)", ids: &[], run: |b| {
            let _ = parts::CostTable::parse(b);
            let _ = nfsu2::tuning::parse(b);
        } },
        Target { name: "native_save (parse, fields, picture)", ids: &[], run: |b| {
            if let Ok(s) = native_save::NativeSave::parse(b) {
                let _ = (s.name(), s.visits(), s.to_bytes());
                if let Some(ctx) = nfsu2::frontend::career::NativeContext::get() {
                    let _ = nfsu2::frontend::career::Profile::from_native(&s, ctx);
                }
            }
            let _ = native_save::read_picture(b);
        } },
        Target { name: "native_save payload (fixed container)", ids: &[], run: |b| {
            // The payload behind a valid header and checksum: the field
            // readers on whatever the payload holds.
            if let Ok(s) = native_save::NativeSave::parse(&native_save::container(b)) {
                let _ = (s.name(), s.visits(), (0..8).map(|k| s.stage(k)).sum::<usize>());
                if let Some(ctx) = nfsu2::frontend::career::NativeContext::get() {
                    let _ = nfsu2::frontend::career::Profile::from_native(&s, ctx);
                }
            }
        } },
        Target { name: "options.txt (Settings::from_text)", ids: &[], run: |b| {
            let s = nfsu2::frontend::options::Settings::from_text(&String::from_utf8_lossy(b));
            let _ = s.options_text();
        } },
        Target { name: "frontend (font, FEng package, responses, language)", ids: &[], run: |b| {
            let _ = nfsu2::frontend::font::Font::parse(b);
            let _ = nfsu2::frontend::feng::parse(b);
            let _ = nfsu2::frontend::feng::parse_responses(b);
            let _ = nfsu2::frontend::profiles::parse_loc(b);
        } },
        Target { name: "audio (ABK, GIN, SCHL)", ids: &[], run: |b| {
            let _ = nfsu2::audio::abk::Bank::parse(b.to_vec());
            let _ = nfsu2::audio::gin::Gin::parse(b);
            let _ = nfsu2::audio::schl::decode(b);
        } },
        Target { name: "movie (VP6 container)", ids: &[], run: |b| { let _ = nfsu2::movie::Movie::parse(b.to_vec()); } },
        Target { name: "animation (EAGL bank, skeleton, scene characters, part animations)", ids: &[], run: |b| {
            let _ = eagl::parse_bank(b);
            let _ = nfsu2::character::Skeleton::parse(b);
            let _ = nfsu2::character::SceneCharacters::parse(b);
            let _ = zam::PartAnimations::parse(b, "COROLLA");
        } },
        Target { name: "VIV header, DDS (texture packs)", ids: &[], run: |b| {
            let _ = viv::Viv::parse_header(PathBuf::from("x.viv"), b);
            let _ = nfsu2::upscale::parse_dds(b);
        } },
    ]
}

/// xorshift64*: the mutation engine's numbers (deterministic per seed).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

/// The chunk headers (offset of the header) in a chunk tree.
fn headers(b: &[u8]) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    bundle::walk(bundle::chunks(b, 0), &mut |c: Chunk| v.push((c.offset - 8, c.body.len())));
    v
}

/// One mutation of `seed`.
fn mutate(seed: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut b = seed.to_vec();
    let boundary = |rng: &mut Rng, len: usize| -> u32 {
        let v = [0u32, 1, 2, 7, 8, 0x10, 0xFF, 0xFFFF, 0x7FFF_FFF8, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFF0, 0xFFFF_FFFF, len as u32, len as u32 + 1, (len as u32).wrapping_sub(1), (len as u32).wrapping_mul(2)];
        v[rng.below(v.len())]
    };
    for _ in 0..1 + rng.below(3) {
        let len = b.len();
        match rng.below(7) {
            // A chunk's size: lying (past its parent, huge, zero, odd).
            0 | 1 => {
                let h = headers(&b);
                if let Some(&(at, body)) = h.get(rng.below(h.len())) {
                    let v = if rng.below(2) == 0 { boundary(rng, body) } else { (body as u32).wrapping_add(rng.below(64) as u32).wrapping_sub(32) };
                    b[at + 4..at + 8].copy_from_slice(&v.to_le_bytes());
                }
            }
            // A u32 in a body (counts, offsets) at a 4-aligned place.
            2 | 3 if len >= 4 => {
                let at = rng.below(len / 4) * 4;
                let v = boundary(rng, len);
                b[at..at + 4].copy_from_slice(&v.to_le_bytes());
            }
            // Cut off.
            4 if len > 0 => b.truncate(rng.below(len)),
            // Bytes flipped.
            5 if len > 0 => {
                for _ in 0..1 + rng.below(8) {
                    let at = rng.below(len);
                    b[at] ^= 1 << rng.below(8);
                }
            }
            // Grown with zeros or junk.
            _ => {
                let n = rng.below(64);
                for _ in 0..n {
                    b.push(if rng.below(2) == 0 { 0 } else { rng.next() as u8 });
                }
            }
        }
    }
    b
}

thread_local! {
    static QUIET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static WHERE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// The panic hook once for the binary: silent while a reader runs under
/// [`run_one`] (its panic is collected), the default otherwise.
fn quiet_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !QUIET.with(|q| q.get()) {
                default(info);
            } else {
                let mut at = info.location().map_or_else(String::new, |l| format!("{}:{}", l.file(), l.line()));
                // A panic in a shared helper (bundle's readers): the first
                // caller in the crate too.
                if at.contains("bundle.rs") || at.contains("library/") {
                    let bt = std::backtrace::Backtrace::force_capture().to_string();
                    if let Some(f) = bt.lines().find(|l| (l.contains("at ./src/") || l.contains("at src/")) && !l.contains("bundle.rs")) {
                        at += &format!(" via {}", f.trim());
                    }
                }
                WHERE.with(|w| *w.borrow_mut() = at);
            }
        }));
    });
}

/// Run one input; `Err` on a panic or a slow call.
fn run_one(t: &Target, input: &[u8]) -> Result<(), String> {
    quiet_hook();
    let start = Instant::now();
    QUIET.with(|q| q.set(true));
    let name = std::ffi::CString::new(t.name).unwrap();
    RUNNING.store(name.as_ptr().cast_mut().cast(), std::sync::atomic::Ordering::Relaxed);
    let r = catch_unwind(AssertUnwindSafe(|| (t.run)(input)));
    QUIET.with(|q| q.set(false));
    RUNNING.store(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);
    drop(name);
    let took = start.elapsed();
    match r {
        Err(e) => Err(format!(
            "panic at {}: {}",
            WHERE.with(|w| w.borrow().clone()),
            e.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_else(|| "(panic)".into())
        )),
        Ok(()) if took > Duration::from_secs(5) => Err(format!("slow: {took:?} for {} bytes", input.len())),
        Ok(()) => Ok(()),
    }
}

/// Real seeds by target: chunks of the game's files with the target's ids,
/// or for whole-file targets the files (small ones) and the top-level
/// chunks of the big ones.
fn real_seeds() -> Vec<(String, Vec<u8>)> {
    let root = common::game_root();
    assert!(root.join("GLOBAL/GLOBALB.BUN").is_file(), "{} has no GLOBAL/GLOBALB.BUN", root.display());
    let mut v = Vec::new();
    // Whole files, or (big ones) their first 2 MB.
    for f in [
        "GLOBAL/InGameCommon.bun",
        "TRACKS/L4RA.BUN",
        "GLOBAL/GLOBALB.BUN",
        "CARS/COROLLA/GEOMETRY.BIN",
        "CARS/COROLLA/PARTS_ANIMATIONS.bin",
        "FRONTEND/FrontB.lzc",
        "FRONTEND/MAGAZINES_SHOWCASE.BIN",
        "GLOBAL/GlobalB.lzc",
        "LANGUAGES/English.bin",
        "TRACKS/ROUTESL4RA/Paths4001.bin",
        "SOUND/NOS/Nitrous_00_MB.abk",
        "SOUND/ENGINE/GIN_Mazda_RX7_DCL.gin",
        "MOVIES/IndustEast.vp6",
    ] {
        let p = nfsu2::formats::viv::find_ci(&root, f).unwrap_or_else(|| panic!("seed file {f} is missing from the game"));
        let file = std::fs::File::open(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        {
            use std::io::Read;
            let mut b = Vec::new();
            let limit = if f.ends_with(".vp6") { 2 << 20 } else { 16 << 20 };
            file.take(limit).read_to_end(&mut b).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            v.push((f.to_string(), b));
        }
    }
    // The first chunks of the streamed world (its sections' scenery,
    // lights, texture packs and solids).
    {
        let p = nfsu2::formats::viv::find_ci(&root, "TRACKS/STREAML4RA.BUN").expect("TRACKS/STREAML4RA.BUN is missing from the game");
        let f = std::fs::File::open(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        use std::io::Read;
        let mut b = vec![0u8; 24 << 20];
        let n = f.take(24 << 20).read(&mut b).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        b.truncate(n);
        v.push(("TRACKS/STREAML4RA.BUN (first 24 MB)".into(), b));
    }
    v
}

/// Seeds for one target out of the real files.
fn seeds_for(t: &Target, files: &[(String, Vec<u8>)]) -> Vec<Vec<u8>> {
    const MAX: usize = 2 << 20;
    let mut out = Vec::new();
    for (name, b) in files {
        if t.ids.is_empty() {
            if b.len() <= MAX {
                out.push(b.clone());
            }
            // The top-level chunks (whole-file readers search them by id).
            for c in bundle::chunks(b, 0).take(400) {
                if c.body.len() + 8 <= MAX {
                    out.push(b[c.offset - 8..c.offset + c.body.len()].to_vec());
                }
            }
            // The JDLZ files decompressed, as the readers get them.
            if name.ends_with(".lzc")
                && let Ok(d) = nfsu2::formats::jdlz::decompress(b)
            {
                for c in bundle::chunks(&d, 0).take(400) {
                    if c.body.len() + 8 <= MAX {
                        out.push(d[c.offset - 8..c.offset + c.body.len()].to_vec());
                    }
                }
            }
        } else {
            let mut n = 0;
            bundle::walk(bundle::chunks(b, 0), &mut |c: Chunk| {
                if t.ids.contains(&c.id) && c.body.len() + 8 <= MAX && n < 6 {
                    out.push(b[c.offset - 8..c.offset + c.body.len()].to_vec());
                    n += 1;
                }
            });
        }
    }
    out
}

/// Synthetic seeds every target gets: a small chunk tree, the save
/// fixture, an options.txt.
fn synthetic_seeds(t: &Target) -> Vec<Vec<u8>> {
    let chunk = |id: u32, body: &[u8]| -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend((body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    };
    let mut seeds = vec![Vec::new(), vec![0u8; 64]];
    let id0 = t.ids.first().copied().unwrap_or(0x8003_4100);
    let inner = [chunk(id0 & 0x7FFF_FFFF | 1, &[1u8; 24]), chunk(id0 & 0x7FFF_FFFF | 2, &(0..64u8).collect::<Vec<_>>())].concat();
    seeds.push(chunk(id0, &inner));
    // The original's save of a finished career, with the private set (OB_PRIVATE).
    if cfg!(ob_private) {
        seeds.push(common::private("tests/fixtures/native_saves/career_complete.sav").to_vec());
    }
    let mut s = nfsu2::frontend::options::Settings::default();
    s.enh.parse("enh.hud_scale", "120");
    seeds.push(s.options_text().into_bytes());
    seeds
}

fn iters() -> usize {
    std::env::var("NFSU2_FUZZ_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(1000)
}

fn seed() -> u64 {
    std::env::var("NFSU2_FUZZ_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(0x5EED_0F_F022)
}

/// The mutation engine over every target and its seeds; all failures are
/// listed at the end (target, seed index, iteration, message).
fn fuzz(files: &[(String, Vec<u8>)], real: bool) {
    let mut failures = Vec::new();
    let mut runs = 0usize;
    let only = std::env::var("NFSU2_FUZZ_ONLY").ok();
    for t in targets() {
        if only.as_deref().is_some_and(|o| !t.name.contains(o)) {
            continue;
        }
        let seeds = if real { seeds_for(&t, files) } else { synthetic_seeds(&t) };
        if seeds.is_empty() {
            continue;
        }
        for (k, s) in seeds.iter().enumerate() {
            if let Err(e) = run_one(&t, s) {
                failures.push(format!("{}: seed {k} as is: {e}", t.name));
            }
        }
        let mut rng = Rng(seed() ^ (t.name.len() as u64) << 32 | 1);
        for i in 0..iters() {
            let k = rng.below(seeds.len());
            let input = mutate(&seeds[k], &mut rng);
            runs += 1;
            if let Err(e) = run_one(&t, &input) {
                if failures.len() < 200 {
                    failures.push(format!("{}: seed {k} iteration {i} ({} bytes): {e}", t.name, input.len()));
                }
                if std::env::var_os("NFSU2_FUZZ_DUMP").is_some() {
                    let _ = std::fs::write(format!("{}/fuzz-{}-{i}.bin", std::env::var("NFSU2_FUZZ_DUMP").unwrap(), t.name.split(&[' ', ':'][..]).next().unwrap()), &input);
                }
            }
        }
    }
    eprintln!("fuzz ({}): {runs} mutated inputs, {} failures", if real { "real seeds" } else { "synthetic seeds" }, failures.len());
    assert!(failures.is_empty(), "{} reader failures:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn readers_survive_mutated_synthetic_input() {
    fuzz(&[], false);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn readers_survive_mutated_game_files() {
    let files = real_seeds();
    // The context Profile::from_native needs (career data from GLOBALB).
    let _ = nfsu2::frontend::career::NativeContext::get();
    fuzz(&files, true);
}

mod props {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 64, failure_persistence: None, ..ProptestConfig::default() })]

        /// Random bytes into every reader.
        #[test]
        fn random_bytes_never_panic(b in proptest::collection::vec(any::<u8>(), 0..2048)) {
            for t in targets() {
                prop_assert!(run_one(&t, &b).is_ok(), "{}: {:?}", t.name, run_one(&t, &b));
            }
        }

        /// A chunk header with any id and size, then random bytes.
        #[test]
        fn any_chunk_header_never_panics(cid in any::<u32>(), size in any::<u32>(), body in proptest::collection::vec(any::<u8>(), 0..512)) {
            let mut b = cid.to_le_bytes().to_vec();
            b.extend(size.to_le_bytes());
            b.extend(body);
            for t in targets() {
                prop_assert!(run_one(&t, &b).is_ok(), "{}: {:?}", t.name, run_one(&t, &b));
            }
        }

        /// options.txt lines of known keys with any value.
        #[test]
        fn options_lines_with_any_value(key in proptest::sample::select(vec!["resolution", "fullscreen", "aspect", "vsync_off", "detail", "custom.3", "custom.99", "key.0.0", "key.17.1", "key.99.9", "enh.hud_scale", "enh.fov", "enh.post_aa", "enh.raised.3", "enh.mark_spot", "enh.power_saving"]), value in ".{0,40}") {
            let s = nfsu2::frontend::options::Settings::from_text(&format!("{key} {value}\n"));
            let _ = s.options_text();
        }
    }
}

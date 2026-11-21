use super::*;

#[test]
fn file_names_map_to_the_loader_keys() {
    assert_eq!(Key::of_file("ABCDEF12.png"), Some(Key::Hash(0xABCD_EF12)));
    assert_eq!(Key::of_file("0000ABCD.dds"), Some(Key::Hash(0xABCD)));
    assert_eq!(Key::of_file("ROAD_MAIN.png"), Some(Key::Name("ROAD_MAIN".into())));
    // The loader asks for upper-case hashes and PNG names only.
    assert_eq!(Key::of_file("abcdef12.png"), None);
    assert_eq!(Key::of_file("ROAD_MAIN.dds"), None);
    assert_eq!(Key::of_file("manifest.json"), None);
    assert_eq!(Key::of_file(".png"), None);
    assert!(Key::Hash(5).matches(5, "x") && Key::Name("x".into()).matches(9, "x") && !Key::Hash(5).matches(6, "x"));
}

fn sig(len: u64) -> Sig {
    (len, None, 0)
}

fn list(files: &[(&str, u64)]) -> HashMap<String, Sig> {
    files.iter().map(|(n, l)| (n.to_string(), sig(*l))).collect()
}

/// A folder's reads for the scanner: `check` says whether a file decodes;
/// the content hashes are `hashes`' (each read another one by default).
struct Fake<F: FnMut(&str) -> Result<(), String>> {
    check: F,
    hashes: HashMap<String, u64>,
    reads: u64,
}

fn fake<F: FnMut(&str) -> Result<(), String>>(check: F) -> Fake<F> {
    Fake { check, hashes: HashMap::new(), reads: 0 }
}

impl<F: FnMut(&str) -> Result<(), String>> Reader for Fake<F> {
    fn check(&mut self, name: &str) -> Result<u64, String> {
        (self.check)(name)?;
        Ok(self.hash(name).unwrap())
    }
    fn hash(&mut self, name: &str) -> Option<u64> {
        static READS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1000);
        self.reads += 1;
        Some(self.hashes.get(name).copied().unwrap_or_else(|| READS.fetch_add(1, Ordering::Relaxed)))
    }
}

fn at() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000)
}

/// A whole change shows at the scan that first sees it; one still being
/// written (not decoding yet) once it stays the same for a scan and
/// decodes; removals show once the file stays gone; the first scan is a
/// baseline.
#[test]
fn scanner_shows_whole_changes_at_once_and_waits_for_the_rest() {
    let mut s = Scanner::default();
    let mut ok = fake(|_| Ok(()));
    assert!(s.scan(list(&[("A.png", 1), ("B.png", 1)]), at(), &mut ok).is_empty(), "baseline");
    assert!(s.scan(list(&[("A.png", 1), ("B.png", 1)]), at(), &mut ok).is_empty(), "nothing changed");
    // Saved whole (through a rename): at once, and once.
    assert_eq!(s.scan(list(&[("A.png", 2), ("B.png", 1)]), at(), &mut ok), [Event::Changed("A.png".into())]);
    assert!(s.scan(list(&[("A.png", 2), ("B.png", 1)]), at(), &mut ok).is_empty(), "reported once");
    // Being written in place: half a file does not decode; it shows once it
    // holds and decodes.
    let whole = std::cell::Cell::new(false);
    let mut half = fake(|_| if whole.get() { Ok(()) } else { Err("short".into()) });
    assert!(s.scan(list(&[("A.png", 3), ("B.png", 1)]), at(), &mut half).is_empty());
    assert!(s.scan(list(&[("A.png", 4), ("B.png", 1)]), at(), &mut half).is_empty(), "still changing");
    whole.set(true);
    assert_eq!(s.scan(list(&[("A.png", 4), ("B.png", 1)]), at(), &mut half), [Event::Changed("A.png".into())]);
    // Added and removed in the same scans: the addition at once, the
    // removal once it stays gone.
    assert_eq!(s.scan(list(&[("A.png", 4), ("C.png", 4)]), at(), &mut ok), [Event::Changed("C.png".into())]);
    assert_eq!(s.scan(list(&[("A.png", 4), ("C.png", 4)]), at(), &mut ok), [Event::Removed("B.png".into())]);
    // Removed and written back within a scan (a save that deletes first): no
    // original shown between.
    assert!(s.scan(list(&[("A.png", 4)]), at(), &mut ok).is_empty());
    assert_eq!(s.scan(list(&[("A.png", 4), ("C.png", 5)]), at(), &mut ok), [Event::Changed("C.png".into())]);
    // Written again with the same content: nothing to reload.
    let mut same = fake(|_| Ok(()));
    same.hashes.insert("C.png".into(), 77);
    assert_eq!(s.scan(list(&[("A.png", 4), ("C.png", 6)]), at(), &mut same), [Event::Changed("C.png".into())]);
    assert!(s.scan(list(&[("A.png", 4), ("C.png", 7)]), at(), &mut same).is_empty(), "the same bytes");
}

/// A save the signature does not show (the same size, and a modification
/// time within the clock's granularity of the last): a recent file's content
/// is compared, so it still shows; an old file's is not read again.
#[test]
fn scanner_sees_a_rewrite_its_signature_misses() {
    let now = at();
    let recent: Sig = (10, Some(now - Duration::from_millis(500)), 0);
    let mut s = Scanner::default();
    let mut r = fake(|_| Ok(()));
    r.hashes.insert("A.png".into(), 1);
    let one = || HashMap::from([("A.png".to_string(), recent)]);
    assert!(s.scan(one(), now, &mut r).is_empty(), "baseline");
    // Recent at the baseline (written after the loader read it, before the
    // watcher started): shown once.
    assert_eq!(s.scan(one(), now, &mut r), [Event::Changed("A.png".into())]);
    assert!(s.scan(one(), now, &mut r).is_empty(), "once");
    r.hashes.insert("A.png".into(), 2);
    assert_eq!(s.scan(one(), now, &mut r), [Event::Changed("A.png".into())], "new bytes, same signature");
    assert!(s.scan(one(), now, &mut r).is_empty(), "once");
    // Older than RACY: no more reads.
    let later = now + RACY + Duration::from_secs(1);
    s.scan(one(), later, &mut r);
    let reads = r.reads;
    r.hashes.insert("A.png".into(), 3);
    for _ in 0..5 {
        assert!(s.scan(one(), later, &mut r).is_empty());
    }
    assert_eq!(r.reads, reads, "an old file is not read at each scan");
}

/// A file that does not decode is retried, then left out with one event
/// (the texture in use stays); a later good version is taken.
#[test]
fn scanner_retries_then_rejects_bad_files() {
    let tick = |k: u32| at() + TICK * k;
    let mut s = Scanner::default();
    s.scan(list(&[("A.png", 1)]), tick(0), &mut fake(|_| Ok(())));
    let mut bad = fake(|_| Err("truncated".to_string()));
    s.scan(list(&[("A.png", 2)]), tick(1), &mut bad);
    let mut events = Vec::new();
    for k in 0..2 * (RETRIES + 2) {
        events.extend(s.scan(list(&[("A.png", 2)]), tick(2 + k), &mut bad));
    }
    assert_eq!(events, [Event::Rejected("A.png".into(), "truncated".into())], "one warning, no reload");
    // Many quick scans (right after a change) do not reject it sooner than
    // that many ticks.
    let mut s = Scanner::default();
    s.scan(list(&[("A.png", 1)]), at(), &mut fake(|_| Ok(())));
    for k in 0..4 * RETRIES {
        let t = at() + FAST_TICK * k;
        assert!(s.scan(list(&[("A.png", 2)]), t, &mut bad).is_empty(), "scan {k}");
    }
    // Becomes whole a moment later (half-written): taken within the retries.
    let mut s = Scanner::default();
    s.scan(list(&[("A.png", 1)]), tick(0), &mut fake(|_| Ok(())));
    s.scan(list(&[("A.png", 2)]), tick(1), &mut bad);
    let mut tries = 0;
    let mut events = Vec::new();
    let mut slow = fake(|_| {
        tries += 1;
        if tries < 3 { Err("short".into()) } else { Ok(()) }
    });
    for k in 0..6 {
        events.extend(s.scan(list(&[("A.png", 2)]), tick(2 + k), &mut slow));
    }
    assert_eq!(events, [Event::Changed("A.png".into())]);
    // The rejected file changing again is looked at again.
    let mut s = Scanner::default();
    s.scan(list(&[("A.png", 1)]), tick(0), &mut fake(|_| Ok(())));
    for k in 0..2 * (RETRIES + 2) {
        s.scan(list(&[("A.png", 2)]), tick(1 + k), &mut bad);
    }
    let t = tick(4 * RETRIES);
    assert_eq!(s.scan(list(&[("A.png", 5)]), t, &mut fake(|_| Ok(()))), [Event::Changed("A.png".into())]);
}

#[test]
fn listing_takes_pack_files_only() {
    let dir = std::env::temp_dir().join(format!("nfsu2-pack-watch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["ABCDEF12.png", "ROAD.png", "notes.txt", "hd-ui-pack.list"] {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    std::fs::create_dir_all(dir.join("12345678.png")).unwrap();
    let mut names: Vec<String> = listing(&dir).unwrap().into_keys().collect();
    names.sort();
    assert_eq!(names, ["ABCDEF12.png", "ROAD.png", "hd-ui-pack.list"]);
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(listing(&dir).is_none());
}

/// End to end on the loader (in a child process: the pack variables are
/// read once, and the log is process-wide): a world pack folder watched by
/// [`Watcher::tick`]; a changed PNG is published for the world only, the
/// loader's cache forgets it, a broken one keeps the texture in use, a
/// removed one falls back to the original, and the folder going reloads
/// the domain as a whole.
#[test]
fn watcher_publishes_changes_and_the_loader_follows() {
    const CHILD: &str = "NFSU2_PACK_WATCH_TEST_CHILD";
    let dir = match crate::render::debug_env::test_child(CHILD) {
        Some(_) => PathBuf::from(crate::render::debug_env::nfsu2_world_texture_pack_raw().unwrap()),
        None => std::env::temp_dir().join(format!("nfsu2-pack-watch-e2e-{}", std::process::id())),
    };
    if crate::render::debug_env::test_child(CHILD).is_none() {
        std::fs::create_dir_all(&dir).unwrap();
        image::RgbaImage::from_pixel(8, 2, image::Rgba([20, 40, 60, 255])).save(dir.join("ABCDEF12.png")).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "render::enh::assets::pack_reload::tests::watcher_publishes_changes_and_the_loader_follows",
            ])
            .env(CHILD, "1")
            .env("NFSU2_WORLD_TEXTURE_PACK", &dir)
            .env_remove("NFSU2_CAR_TEXTURE_PACK")
            .env_remove("NFSU2_TEXTURE_PACK")
            .status()
            .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(status.success());
        return;
    }
    crate::hd_ui_pack::initialize(false);
    use crate::formats::texture::{Blend, PixelFormat, Scroll, Texture};
    let t = Texture {
        name: "road".into(),
        hash: 0xabcdef12,
        width: 2,
        height: 2,
        format: PixelFormat::Rgba8,
        mip_count: 1,
        data: vec![255; 16].into(),
        palette: vec![],
        alpha_test: false,
        blend: Blend::None,
        rendering_order: 0,
        late_buckets: false,
        scroll: Scroll::default(),
    };
    let top = |t: &Texture| {
        let (key, bytes) = crate::render::texture_layer_with_pack(Domain::World, Some(t), false, 4096);
        (key.1, key.2, bytes[..4].to_vec())
    };
    assert_eq!(top(&t), (8, 2, vec![20, 40, 60, 255]));
    let mut w = Watcher::default();
    w.tick();
    let start = generation();
    // Rewritten (another size and colour), whole: published at the next tick.
    image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255])).save(dir.join("ABCDEF12.png")).unwrap();
    assert_eq!(since(start, Domain::World).1, Pending::None, "not seen yet");
    assert_eq!(w.tick(), FAST_TICK, "looked at again soon after a change");
    let (g, p) = since(start, Domain::World);
    assert_eq!(p, Pending::Some([Key::Hash(0xABCD_EF12)].into_iter().collect()));
    assert_eq!(since(start, Domain::Car).1, Pending::None, "the world's folder only");
    assert_eq!(since(start, Domain::Ui).1, Pending::None);
    assert_eq!(since(g, Domain::World).1, Pending::None);
    assert_eq!(top(&t), (4, 4, vec![1, 2, 3, 255]), "the loader reads the new file");
    // Broken (half a PNG, never completed): kept out, the texture stays.
    let bytes = std::fs::read(dir.join("ABCDEF12.png")).unwrap();
    std::fs::write(dir.join("ABCDEF12.png"), &bytes[..bytes.len() / 2]).unwrap();
    for _ in 0..RETRIES + 4 {
        w.tick();
    }
    assert_eq!(since(g, Domain::World).1, Pending::None, "nothing to reload");
    assert_eq!(top(&t), (4, 4, vec![1, 2, 3, 255]), "the texture in use stays");
    // Removed: the original again.
    std::fs::remove_file(dir.join("ABCDEF12.png")).unwrap();
    w.tick();
    w.tick();
    let (g, p) = since(g, Domain::World);
    assert!(p.hits(0xabcdef12, "road"));
    assert_eq!(top(&t), (2, 2, vec![255; 4]));
    // A name file is a key of its own.
    image::RgbaImage::from_pixel(2, 4, image::Rgba([9, 9, 9, 255])).save(dir.join("road.png")).unwrap();
    w.tick();
    w.tick();
    let (g, p) = since(g, Domain::World);
    assert_eq!(p, Pending::Some([Key::Name("road".into())].into_iter().collect()));
    assert_eq!(top(&t), (2, 4, vec![9, 9, 9, 255]));
    // The folder goes: the domain reloads as a whole.
    std::fs::remove_dir_all(&dir).unwrap();
    w.tick();
    let (g, p) = since(g, Domain::World);
    assert_eq!(p, Pending::All);
    // Ctrl+R: everything, every domain.
    request_full();
    w.tick();
    assert_eq!(since(g, Domain::Ui).1, Pending::All);
    assert_eq!(since(g, Domain::Car).1, Pending::All);
    assert!(MAP_DIRTY.swap(false, Ordering::Relaxed));
    // A renderer far behind the log reloads everything.
    for _ in 0..LOG_CAP + 1 {
        publish(CAR, Some(Key::Hash(1)));
    }
    assert_eq!(since(g, Domain::Car).1, Pending::All);
}

/// The setting off: no watcher, nothing changes (the native path).
#[test]
fn frame_does_nothing_with_the_setting_off() {
    let _g = crate::qol::test_globals();
    crate::qol::Enhancements::default().apply();
    let g = generation();
    assert!(!frame(true));
    assert!(!RUNNING.load(Ordering::Relaxed));
    assert_eq!(generation(), g);
}

/// GPU (opt-in, `NFSU2_PACK_TEST_GPU=1`, as the texture-pack upload
/// checks): a world renderer and a car renderer follow published changes;
/// the world rewrites a same-shape layer in place and lays its arrays out
/// again for a new size, without validation errors.
#[test]
fn renderers_follow_changes_on_the_gpu() {
    const CHILD: &str = "NFSU2_PACK_WATCH_GPU_CHILD";
    if !crate::render::debug_env::nfsu2_pack_test_gpu() {
        return;
    }
    let sky = crate::formats::hash::bin_hash(crate::sky::SKY_TABLE[0]);
    let file = format!("{sky:08X}.png");
    let dir = match crate::render::debug_env::test_child(CHILD) {
        Some(_) => PathBuf::from(crate::render::debug_env::nfsu2_world_texture_pack_raw().unwrap()),
        None => {
            let dir = std::env::temp_dir().join(format!("nfsu2-pack-watch-gpu-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            image::RgbaImage::from_pixel(8, 8, image::Rgba([20, 40, 60, 255])).save(dir.join(&file)).unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "render::enh::assets::pack_reload::tests::renderers_follow_changes_on_the_gpu"])
                .env(CHILD, "1")
                .env("NFSU2_WORLD_TEXTURE_PACK", &dir)
                .env("NFSU2_CAR_TEXTURE_PACK", &dir)
                .env_remove("NFSU2_TEXTURE_PACK")
                .status()
                .unwrap();
            let _ = std::fs::remove_dir_all(&dir);
            assert!(status.success());
            return;
        }
    };
    crate::hd_ui_pack::initialize(false);
    use crate::formats::texture::{Blend, PixelFormat, Scroll, Texture};
    let t = Texture {
        name: crate::sky::SKY_TABLE[0].into(),
        hash: sky,
        width: 4,
        height: 4,
        format: PixelFormat::Rgba8,
        mip_count: 1,
        data: vec![255; 64].into(),
        palette: vec![],
        alpha_test: false,
        blend: Blend::None,
        rendering_order: 0,
        late_buckets: false,
        scroll: Scroll::default(),
    };
    let mut world = crate::world::World::default();
    world.textures.insert(sky, t.clone());
    let gpu = pollster::block_on(crate::render::Gpu::new(
        None,
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
    ))
    .unwrap();
    gpu.device.on_uncaptured_error(std::sync::Arc::new(|e| panic!("wgpu: {e}")));
    let mut r = crate::render::WorldRenderer::new(&gpu, &world, wgpu::TextureFormat::Rgba8Unorm, &crate::render::RenderOptions::in_force());
    let shape = |r: &crate::render::WorldRenderer| {
        r.core.texture_arrays.as_ref().map(|a| {
            let x = &a.arrays[a.slots[1].0 as usize];
            (x.width(), x.height())
        })
    };
    assert!(!r.sync_texture_pack(&gpu, &world), "nothing published");
    let arrays = r.core.texture_arrays.is_some();
    if arrays {
        assert_eq!(shape(&r), Some((8, 8)));
    }
    // Same size: in place.
    image::RgbaImage::from_pixel(8, 8, image::Rgba([1, 2, 3, 255])).save(dir.join(&file)).unwrap();
    publish(WORLD | CAR, Key::of_file(&file));
    assert!(r.sync_texture_pack(&gpu, &world));
    assert!(!r.sync_texture_pack(&gpu, &world), "done once");
    // A new size: the arrays laid out again.
    image::RgbaImage::from_pixel(16, 4, image::Rgba([1, 2, 3, 255])).save(dir.join(&file)).unwrap();
    publish(WORLD, Key::of_file(&file));
    assert!(r.sync_texture_pack(&gpu, &world));
    if arrays {
        assert_eq!(shape(&r), Some((16, 4)));
    }
    // Removed: the original's 4x4.
    std::fs::remove_file(dir.join(&file)).unwrap();
    publish(WORLD, Key::of_file(&file));
    assert!(r.sync_texture_pack(&gpu, &world));
    if arrays {
        assert_eq!(shape(&r), Some((4, 4)));
    }
    // The texture editor's highlight: drawn instead of the original and
    // every pack, then taken away again; the slot names its texture.
    assert_eq!(r.slot_hash(1), Some(sky));
    highlight(false, sky, Some((12, 4, vec![200; 12 * 4 * 4])));
    assert!(r.sync_texture_pack(&gpu, &world));
    if arrays {
        assert_eq!(shape(&r), Some((12, 4)));
    }
    highlight(false, sky, None);
    assert!(r.sync_texture_pack(&gpu, &world));
    if arrays {
        assert_eq!(shape(&r), Some((4, 4)));
    }
    // A change for a texture the world does not have: nothing to do.
    publish(WORLD, Some(Key::Hash(sky ^ 1)));
    assert!(!r.sync_texture_pack(&gpu, &world));
    gpu.queue.submit([]);
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
}

/// The cost of one poll of a real pack folder (opt-in:
/// `NFSU2_PACK_WATCH_TIME=<folder>`), printed.
#[test]
fn listing_cost() {
    let Some(dir) = crate::render::debug_env::nfsu2_pack_watch_time() else { return };
    let dir = PathBuf::from(dir);
    let mut s = Scanner::default();
    let t = std::time::Instant::now();
    let mut n = 0;
    for _ in 0..20 {
        let l = listing(&dir).unwrap();
        n = l.len();
        s.scan(l, SystemTime::now(), &mut fake(|_| Ok(())));
    }
    println!("pack poll: {n} files, {:.2} ms a tick", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
}

/// Every completed save is shown, however the saves and the disk are timed
/// (in a child process: the live folder, the watcher and the log are
/// process-wide): rounds of quick saves (`NFSU2_PACK_STRESS_ROUNDS`, 12 by
/// default) into a live folder whose texture folder does not exist yet,
/// written in place or through a rename, a few ms to a few ticks apart, with
/// the watcher's and the loader's steps slowed at random (a busy disk, a
/// reload in flight). A game thread reloads the texture only when the log
/// says it changed, as the renderers do; after each round's last save it
/// must show that save's pixels.
#[test]
fn every_save_shows_under_a_slow_disk() {
    const CHILD: &str = "NFSU2_PACK_STRESS_CHILD";
    if crate::render::debug_env::test_child(CHILD).is_none() {
        let scratch = std::env::temp_dir().join(format!("nfsu2-pack-stress-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "render::enh::assets::pack_reload::tests::every_save_shows_under_a_slow_disk", "--nocapture"])
            .env(CHILD, &scratch)
            .env("XDG_DATA_HOME", scratch.join("data"))
            .env("XDG_CACHE_HOME", scratch.join("cache"))
            .env("XDG_CONFIG_HOME", scratch.join("config"))
            .env("NFSU2_DATA_DIR", scratch.join("data"))
            .env("NFSU2_CACHE_DIR", scratch.join("cache"))
            .env_remove("NFSU2_WORLD_TEXTURE_PACK")
            .env_remove("NFSU2_CAR_TEXTURE_PACK")
            .env_remove("NFSU2_TEXTURE_PACK")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
        for l in text.lines().filter(|l| l.starts_with("pack stress:")) {
            println!("{l}");
        }
        let _ = std::fs::remove_dir_all(&scratch);
        assert!(out.status.success(), "{text}");
        return;
    }
    use crate::formats::texture::{Blend, PixelFormat, Scroll, Texture};
    use std::sync::Arc;
    use std::time::Instant;
    let scratch = PathBuf::from(crate::render::debug_env::test_child(CHILD).unwrap());
    let rounds: u32 = std::env::var("NFSU2_PACK_STRESS_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(12);
    const HASH: u32 = 0x5EED_0002;
    let tex = Texture {
        name: "STRESS".into(),
        hash: HASH,
        width: 8,
        height: 8,
        format: PixelFormat::Rgba8,
        mip_count: 1,
        data: [9u8, 9, 9, 255].repeat(64).into(),
        palette: vec![],
        alpha_test: false,
        blend: Blend::None,
        rendering_order: 0,
        late_buckets: false,
        scroll: Scroll::default(),
    };
    // What the game draws now, kept by the game thread.
    let drawn = Arc::new(Mutex::new([0u8; 4]));
    let stop = Arc::new(AtomicBool::new(false));
    let game = {
        let (drawn, stop, tex) = (drawn.clone(), stop.clone(), tex.clone());
        std::thread::spawn(move || {
            let mut seen = generation();
            *drawn.lock().unwrap() = loaded_top(false, &tex).2;
            while !stop.load(Ordering::Relaxed) {
                frame(false);
                let (now, hit) = changed_since(seen, false, HASH);
                seen = now;
                if hit {
                    *drawn.lock().unwrap() = loaded_top(false, &tex).2;
                }
                std::thread::sleep(Duration::from_millis(16));
            }
        })
    };
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move |n: u64| {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng % n
    };
    let mut worst = Duration::ZERO;
    let mut worst_new = Duration::ZERO;
    for round in 0..rounds {
        SLOW_MS.store([0, 40, 150, 400][round as usize % 4], Ordering::Relaxed);
        let live_dir = scratch.join(format!("mod{round}"));
        std::fs::create_dir_all(&live_dir).unwrap();
        set_live(Some(live_dir.clone()));
        // The watcher takes the new folder in at its next tick (a test run
        // starts before the first save).
        std::thread::sleep(TICK + FAST_TICK);
        let world = live_dir.join("world");
        let file = world.join(format!("{HASH:08X}.png"));
        let saves = 2 + next(4);
        let mut last = [0u8; 4];
        let mut done = Instant::now();
        for s in 0..saves {
            // Same size every time (one colour, 8x8): only the time tells the saves apart.
            let c = (round * 37 + s as u32 * 71 + 13) as u8;
            last = [c, 255 - c, c / 2 | 1, 255];
            let png = {
                let img = image::RgbaImage::from_pixel(8, 8, image::Rgba(last));
                let mut b = Vec::new();
                img.write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png).unwrap();
                b
            };
            std::fs::create_dir_all(&world).unwrap();
            let before = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
            let mode = next(3);
            if mode == 0 {
                ob_core::platform::fs::write_atomic(&file, &png, false).unwrap();
            } else if mode == 1 && before.is_some() {
                // In place, its time put back to the last save's (a clock
                // too coarse to tell two quick saves apart).
                std::fs::write(&file, &png).unwrap();
                std::fs::File::options().write(true).open(&file).unwrap().set_modified(before.unwrap()).unwrap();
            } else {
                // In place, in two halves (a paint program's save).
                use std::io::Write;
                let mut f = std::fs::File::create(&file).unwrap();
                f.write_all(&png[..png.len() / 2]).unwrap();
                std::thread::sleep(Duration::from_millis(next(30)));
                f.write_all(&png[png.len() / 2..]).unwrap();
            }
            done = Instant::now();
            // Every other round, the first save (into the folder that did
            // not exist) is waited for: how fast a new folder shows.
            if s == 0 && round % 2 == 0 {
                let took = wait_for(&drawn, last, done, round, &file);
                if SLOW_MS.load(Ordering::Relaxed) == 0 {
                    worst_new = worst_new.max(took);
                }
            }
            if s + 1 < saves {
                std::thread::sleep(Duration::from_millis(next(700)));
            }
        }
        let took = wait_for(&drawn, last, done, round, &file);
        if SLOW_MS.load(Ordering::Relaxed) == 0 {
            worst = worst.max(took);
        }
    }
    stop.store(true, Ordering::Relaxed);
    game.join().unwrap();
    set_live(None);
    println!(
        "pack stress: {rounds} rounds, all shown; unslowed worst {:.0} ms ({:.0} ms into a new folder)",
        worst.as_secs_f64() * 1000.0,
        worst_new.as_secs_f64() * 1000.0
    );
}

/// Waits until the game draws `last` (written at `done`); the time it took.
fn wait_for(drawn: &Mutex<[u8; 4]>, last: [u8; 4], done: std::time::Instant, round: u32, file: &Path) -> Duration {
    let limit = Duration::from_secs(20);
    loop {
        if *drawn.lock().unwrap() == last {
            return done.elapsed();
        }
        assert!(
            done.elapsed() < limit,
            "pack stress: round {round} (slow {} ms): the last save never showed: drawn {:?}, wanted {last:?}, the \
             file {:?}",
            SLOW_MS.load(Ordering::Relaxed),
            *drawn.lock().unwrap(),
            std::fs::read(file).ok().map(|b| b.len())
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

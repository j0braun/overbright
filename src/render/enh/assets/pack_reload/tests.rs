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
    (len, None)
}

fn list(files: &[(&str, u64)]) -> HashMap<String, Sig> {
    files.iter().map(|(n, l)| (n.to_string(), sig(*l))).collect()
}

/// The debounce: a change shows once the file stays the same for a tick;
/// one still changing waits; removals show; the first scan is a baseline.
#[test]
fn scanner_debounces_changes_and_removals() {
    let mut s = Scanner::default();
    let ok = |_: &str| Ok(());
    assert!(s.scan(list(&[("A.png", 1), ("B.png", 1)]), ok).is_empty(), "baseline");
    assert!(s.scan(list(&[("A.png", 1), ("B.png", 1)]), ok).is_empty(), "nothing changed");
    // A grows over three scans (being written), then holds.
    assert!(s.scan(list(&[("A.png", 2), ("B.png", 1)]), ok).is_empty());
    assert!(s.scan(list(&[("A.png", 3), ("B.png", 1)]), ok).is_empty(), "still changing");
    assert_eq!(s.scan(list(&[("A.png", 3), ("B.png", 1)]), ok), [Event::Changed("A.png".into())]);
    assert!(s.scan(list(&[("A.png", 3), ("B.png", 1)]), ok).is_empty(), "reported once");
    // Added and removed in the same scans.
    assert!(s.scan(list(&[("A.png", 3), ("C.png", 4)]), ok).is_empty());
    assert_eq!(
        s.scan(list(&[("A.png", 3), ("C.png", 4)]), ok),
        [Event::Changed("C.png".into()), Event::Removed("B.png".into())]
    );
    // Changed and changed back within a tick: nothing to do.
    assert!(s.scan(list(&[("A.png", 9), ("C.png", 4)]), ok).is_empty());
    assert!(s.scan(list(&[("A.png", 3), ("C.png", 4)]), ok).is_empty());
    assert!(s.scan(list(&[("A.png", 3), ("C.png", 4)]), ok).is_empty());
}

/// A file that does not decode is retried, then left out with one event
/// (the texture in use stays); a later good version is taken.
#[test]
fn scanner_retries_then_rejects_bad_files() {
    let mut s = Scanner::default();
    s.scan(list(&[("A.png", 1)]), |_| Ok(()));
    let bad = |_: &str| Err("truncated".to_string());
    s.scan(list(&[("A.png", 2)]), bad);
    let mut events = Vec::new();
    for _ in 0..2 * (RETRIES + 2) {
        events.extend(s.scan(list(&[("A.png", 2)]), bad));
    }
    assert_eq!(events, [Event::Rejected("A.png".into(), "truncated".into())], "one warning, no reload");
    // Becomes whole a moment later (half-written): taken within the retries.
    let mut s = Scanner::default();
    s.scan(list(&[("A.png", 1)]), |_| Ok(()));
    s.scan(list(&[("A.png", 2)]), bad);
    let mut tries = 0;
    let mut events = Vec::new();
    for _ in 0..6 {
        events.extend(s.scan(list(&[("A.png", 2)]), |_| {
            tries += 1;
            if tries < 3 { Err("short".into()) } else { Ok(()) }
        }));
    }
    assert_eq!(events, [Event::Changed("A.png".into())]);
    // The rejected file changing again is looked at again.
    let mut s = Scanner::default();
    s.scan(list(&[("A.png", 1)]), |_| Ok(()));
    for _ in 0..2 * (RETRIES + 2) {
        s.scan(list(&[("A.png", 2)]), bad);
    }
    s.scan(list(&[("A.png", 5)]), |_| Ok(()));
    assert_eq!(s.scan(list(&[("A.png", 5)]), |_| Ok(())), [Event::Changed("A.png".into())]);
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
    // Rewritten (another size and colour): published after it settles.
    image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255])).save(dir.join("ABCDEF12.png")).unwrap();
    w.tick();
    assert_eq!(since(start, Domain::World).1, Pending::None, "not settled yet");
    w.tick();
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
        s.scan(l, |_| Ok(()));
    }
    println!("pack poll: {n} files, {:.2} ms a tick", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
}

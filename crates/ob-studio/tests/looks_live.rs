//! Studio Looks live in the test game (docs/modding/studio-looks.md), end to end and headless: a stand-in paint
//! program (a shell script that writes the work file, as a paint program's save does) saves; Studio takes the save
//! in as one step of the mod's history and writes the mod's pack data; the game's side of a test run started with
//! `--looks-live` (its pack watcher, ticked by the game's frames, and its texture loader) shows the new pixels.
//! The time from the save to the frame that would draw it is measured and must stay under a second.
//!
//! It runs in a child process of this test: the live folder and the pack watcher are process-wide, as in the
//! game. Its data and cache folders are the test's own, so no pack of the player's is read.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ob_formats::formats::texture::{Blend, PixelFormat, Scroll, Texture};
use ob_studio::history::{History, disk};
use ob_studio::looks::work::{Imported, Workbench};
use ob_studio::looks::{self, Domain, Looks, Original, TextureId, Textures};
use ob_studio::studio_api::looks as live;

const CHILD: &str = "OB_LOOKS_LIVE_CHILD";
const HASH: u32 = 0x5EED_0001;
/// A save must show within this.
const LIMIT: Duration = Duration::from_secs(1);
/// The game's frame time in this test (60 fps).
const FRAME: Duration = Duration::from_micros(16_667);

fn texture() -> Texture {
    Texture {
        name: "LOOKS_TEST".into(),
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
    }
}

struct Game(Texture);

impl Textures for Game {
    fn original(&self, id: TextureId) -> Option<Original> {
        (id == TextureId::new(Domain::World, HASH)).then(|| Original {
            name: self.0.name.clone(),
            width: self.0.width,
            height: self.0.height,
            rgba: self.0.decode_rgba(),
            used_by: Vec::new(),
        })
    }
}

/// The stand-in paint program: a script that writes the image over the work file in place (not atomically, as
/// many programs save).
#[cfg(unix)]
fn paint(work: &Path, png: &[u8], scratch: &Path) {
    let src = scratch.join("brush.png");
    std::fs::write(&src, png).unwrap();
    let ok = std::process::Command::new("sh")
        .args(["-c", "cat \"$1\" > \"$2\"", "paint", &src.to_string_lossy(), &work.to_string_lossy()])
        .status()
        .unwrap()
        .success();
    assert!(ok, "the stand-in paint program");
}

#[cfg(unix)]
#[test]
fn a_save_shows_in_the_test_game_within_a_second() {
    if std::env::var_os(CHILD).is_none() {
        let scratch = std::env::temp_dir().join(format!("ob-looks-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "a_save_shows_in_the_test_game_within_a_second", "--nocapture"])
            .env(CHILD, &scratch)
            .env("XDG_DATA_HOME", scratch.join("data"))
            .env("XDG_CACHE_HOME", scratch.join("cache"))
            .env("XDG_CONFIG_HOME", scratch.join("config"))
            .env_remove("NFSU2_WORLD_TEXTURE_PACK")
            .env_remove("NFSU2_CAR_TEXTURE_PACK")
            .env_remove("NFSU2_TEXTURE_PACK")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
        for l in text.lines().filter(|l| l.starts_with("looks live:")) {
            println!("{l}");
        }
        let _ = std::fs::remove_dir_all(&scratch);
        assert!(out.status.success(), "{text}");
        return;
    }
    child(&PathBuf::from(std::env::var_os(CHILD).unwrap()));
}

#[cfg(unix)]
fn child(scratch: &Path) {
    let dir = scratch.join("docks");
    let edir = dir.join(".editor");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(ob_map::tools::map::store::FILE), "").unwrap();
    let tex = texture();
    let id = TextureId::new(Domain::World, HASH);
    assert_eq!(live::loaded_top(false, &tex), (8, 8, [9, 9, 9, 255]), "the game's own texture");

    // Not a Studio test run (no live folder, the setting off): the watcher does not run.
    let g = live::generation();
    assert!(!live::frame(false));
    assert_eq!(live::live(), None);

    // Studio opens the texture in the paint program.
    let mut h = History::open(&dir, &edir, ob_studio::MOD_DOCS, "Overbright Studio", Arc::new(disk::Real)).unwrap();
    let mut opener = Workbench::open(&dir, &edir);
    let f = opener.start(&Looks::default(), id, &Game(tex.clone()), false).unwrap();
    let work = opener.path(&f);
    drop(opener);

    // The test game starts with `--looks-live <mod>/textures`.
    live::set_live(Some(dir.join(looks::PACK_DIR)));
    assert_eq!(live::generation(), g, "nothing published before a change");

    // The game's frames on their own thread: each one ticks the watcher and asks, as the renderers do, what
    // changed; the first frame that would draw the expected pixel is noted.
    let want: Arc<Mutex<Option<([u8; 4], Instant)>>> = Arc::new(Mutex::new(None));
    let seen: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let stop = Arc::new(AtomicBool::new(false));
    let game = {
        let (want, seen, stop, tex) = (want.clone(), seen.clone(), stop.clone(), tex.clone());
        std::thread::spawn(move || {
            let mut seen_gen = live::generation();
            while !stop.load(Ordering::Relaxed) {
                live::frame(false);
                let (now, hit) = live::changed_since(seen_gen, false, HASH);
                seen_gen = now;
                if hit
                    && let Some((px, _)) = *want.lock().unwrap()
                    && live::loaded_top(false, &tex).2 == px
                {
                    seen.lock().unwrap().get_or_insert(Instant::now());
                }
                std::thread::sleep(FRAME);
            }
        })
    };

    // Studio watches the work file (`looks watch`'s loop) on its own thread.
    let steps = Arc::new(Mutex::new(Vec::new()));
    let studio = {
        let (stop, steps) = (stop.clone(), steps.clone());
        let (dir, edir) = (dir.clone(), edir.clone());
        std::thread::spawn(move || {
            let mut w = Workbench::open(&dir, &edir);
            w.watch(&mut h, "Looks", Duration::from_millis(50), None, &|| stop.load(Ordering::Relaxed), &mut |_, r| {
                steps.lock().unwrap().push(r)
            })
            .unwrap();
            h
        })
    };
    std::thread::sleep(Duration::from_millis(300));

    let mut times = Vec::new();
    for (n, c) in [200u8, 40, 120, 250, 77].into_iter().enumerate() {
        let px = [c, 255 - c, c / 2, 255];
        let png = looks::encode_png(8, 8, &px.repeat(64)).unwrap();
        *seen.lock().unwrap() = None;
        *want.lock().unwrap() = Some((px, Instant::now()));
        paint(&work, &png, scratch);
        let saved = Instant::now();
        let shown = loop {
            if let Some(t) = *seen.lock().unwrap() {
                break t;
            }
            assert!(saved.elapsed() < Duration::from_secs(5), "save {n} never showed");
            std::thread::sleep(Duration::from_millis(2));
        };
        let ms = shown.saturating_duration_since(saved).as_secs_f64() * 1000.0;
        println!("looks live: save {n} shown after {ms:.0} ms");
        times.push(ms);
        // The next save after this one has shown (a modder paints, saves, looks).
        std::thread::sleep(Duration::from_millis(100));
    }
    stop.store(true, Ordering::Relaxed);
    game.join().unwrap();
    let h = studio.join().unwrap();

    let steps = steps.lock().unwrap();
    assert_eq!(steps.len(), 5, "one import per save: {steps:?}");
    assert!(steps.iter().all(|r| matches!(r, Ok(Imported::Step(_)))), "{steps:?}");
    let painted: Vec<&str> = h.steps().iter().map(|s| s.label.as_str()).collect();
    assert_eq!(painted, vec!["Paint LOOKS_TEST"; 5], "each save one step");
    let mut sorted = times.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let worst = sorted[sorted.len() - 1];
    println!(
        "looks live: save to shown median {:.0} ms, worst {worst:.0} ms (limit {} ms)",
        sorted[2],
        LIMIT.as_millis()
    );
    assert!(worst < LIMIT.as_secs_f64() * 1000.0, "a save took {worst:.0} ms to show");
}

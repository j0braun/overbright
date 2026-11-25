//! Studio Looks live in the test game (docs/modding/studio-looks.md), end to end and headless: a stand-in paint
//! program (a shell script that writes the work file, as a paint program's save does) saves; Studio takes the save
//! in as one step of the mod's history and writes the mod's pack data; the game's side of a test run started with
//! `--looks-live` (its pack watcher, ticked by the game's frames, and its texture loader) shows the new pixels.
//! The time from the save to the frame that would draw it is measured in the pipeline's own terms (polls, CPU
//! time, watcher, frames; not the disk's or a busy machine's waits) and must stay under a second.
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
        // Over the limit while the disk held the saves up for seconds (a machine busy with other work): measured
        // once more, the reason logged. Slow twice, or slow on a quiet disk, fails.
        let mut text = run_child();
        if let Err(t) = &text
            && let Some(why) = t.lines().find(|l| l.contains("a save took") && l.contains(BUSY))
        {
            println!("looks live: measuring once more: {why}");
            text = run_child();
        }
        if let Err(t) = text {
            panic!("{t}");
        }
        return;
    }
    child(&PathBuf::from(std::env::var_os(CHILD).unwrap()));
}

/// The child's run of the test: its output, Err when it failed.
#[cfg(unix)]
fn run_child() -> Result<String, String> {
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
    if out.status.success() { Ok(text) } else { Err(text) }
}

/// What a failed limit says when the disk held the saves up (see [`BUSY_DISK`]).
#[cfg(unix)]
const BUSY: &str = "the disk held the saves up";
/// A save's wait for the disk beyond which the machine counts as busy.
#[cfg(unix)]
const BUSY_DISK: Duration = Duration::from_secs(1);

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
    assert!(!live::frame(false, false)); // Reload Packs off (its default)
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
    // changed; the frame that first hears of a change and the first that would draw the expected pixel are noted.
    let want: Arc<Mutex<Option<[u8; 4]>>> = Arc::new(Mutex::new(None));
    let heard: Arc<Mutex<Option<(Instant, u64)>>> = Arc::new(Mutex::new(None));
    let seen: Arc<Mutex<Option<(Instant, u64)>>> = Arc::new(Mutex::new(None));
    let stop = Arc::new(AtomicBool::new(false));
    let game = {
        let (want, heard, seen, stop, tex) = (want.clone(), heard.clone(), seen.clone(), stop.clone(), tex.clone());
        std::thread::spawn(move || {
            let mut seen_gen = live::generation();
            let mut frames = 0u64;
            while !stop.load(Ordering::Relaxed) {
                frames += 1;
                live::frame(false, false);
                let (now, hit) = live::changed_since(seen_gen, false, HASH);
                seen_gen = now;
                // `want` held for the whole check: a new save's reset of `heard` and `seen` is never seen halfway.
                let want = want.lock().unwrap();
                if hit && want.is_some() {
                    heard.lock().unwrap().get_or_insert((Instant::now(), frames));
                }
                if let Some(px) = *want
                    && heard.lock().unwrap().is_some()
                    && live::loaded_top(false, &tex).2 == px
                {
                    seen.lock().unwrap().get_or_insert((Instant::now(), frames));
                }
                drop(want);
                std::thread::sleep(FRAME);
            }
        })
    };

    // Studio watches the work file (`looks watch`'s loop) on its own thread; each pass of the loop (it asks
    // `stop` once a pass) and each import are noted with the thread's CPU time.
    const EVERY: Duration = Duration::from_millis(50);
    let steps = Arc::new(Mutex::new(Vec::new()));
    let passes: Arc<Mutex<Vec<(Instant, Duration)>>> = Arc::new(Mutex::new(Vec::new()));
    let studio = {
        let (stop, steps, passes) = (stop.clone(), steps.clone(), passes.clone());
        let (dir, edir) = (dir.clone(), edir.clone());
        std::thread::spawn(move || {
            let mut w = Workbench::open(&dir, &edir);
            let pass = || {
                passes.lock().unwrap().push((Instant::now(), thread_cpu()));
                stop.load(Ordering::Relaxed)
            };
            w.watch(&mut h, "Looks", EVERY, None, &pass, &mut |_, r| {
                steps.lock().unwrap().push((r, Instant::now(), thread_cpu()))
            })
            .unwrap();
            h
        })
    };
    std::thread::sleep(Duration::from_millis(300));

    // The time a save takes to show, in the pipeline's own terms, so that a busy machine (a build beside the
    // test, a disk busy with other writes) does not count as the product's: Studio's polls waited (each `EVERY`)
    // and its CPU time for the import; then the game's pack watcher and frames from the pack file appearing to
    // the frame that draws the pixel (the watcher's ticks: sleeps and a folder listing), less the time the
    // watcher thread stood ready without a CPU. The import's wait for the disk (its syncs) and the run queue
    // are the machine's and are only logged, as is the wall-clock total.
    let pack = dir.join(looks::PACK_DIR).join(id.pack_file());
    let stat = |p: &Path| std::fs::metadata(p).ok().map(|m| (m.len(), m.modified().ok()));
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    // Only a hang fails by the wall clock: a disk busy with other writes can hold a write or a sync for tens of
    // seconds.
    const HANG: Duration = Duration::from_secs(300);
    let mut times = Vec::new();
    let mut disk = Duration::ZERO;
    for (n, c) in [200u8, 40, 120, 250, 77].into_iter().enumerate() {
        let px = [c, 255 - c, c / 2, 255];
        let png = looks::encode_png(8, 8, &px.repeat(64)).unwrap();
        let before = stat(&pack);
        let before_passes = passes.lock().unwrap().len();
        {
            let mut want = want.lock().unwrap();
            *heard.lock().unwrap() = None;
            *seen.lock().unwrap() = None;
            *want = Some(px);
        }
        paint(&work, &png, scratch);
        let saved = Instant::now();
        let mut visible = None;
        let mut w0 = None;
        let shown = loop {
            if visible.is_none() && stat(&pack) != before {
                visible = Some(Instant::now());
                w0 = watcher_sched();
            }
            if let Some(s) = *seen.lock().unwrap() {
                break s;
            }
            assert!(
                saved.elapsed() < HANG,
                "save {n} never showed (the pack file {}, imports {:?})",
                if visible.is_some() { "written" } else { "not written" },
                steps.lock().unwrap().iter().map(|s| &s.0).collect::<Vec<_>>()
            );
            std::thread::sleep(Duration::from_millis(2));
        };
        let w1 = watcher_sched();
        let visible = visible.unwrap_or(shown.0);
        let heard = heard.lock().unwrap().unwrap();
        let step = loop {
            let made = steps
                .lock()
                .unwrap()
                .iter()
                .filter(|s| matches!(s.0, Ok(Imported::Step(_))))
                .nth(n)
                .map(|s| (s.1, s.2));
            if let Some(s) = made {
                break s;
            }
            assert!(saved.elapsed() < HANG, "save {n} never taken in");
            std::thread::sleep(Duration::from_millis(2));
        };
        let passes = passes.lock().unwrap();
        // The passes that ended between the save and its import, and the one the save landed in.
        let waited = passes[before_passes..].iter().filter(|p| p.0 > saved && p.0 <= step.0).count() + 1;
        let cpu_from = passes[..before_passes.max(1)].last().map_or(Duration::ZERO, |p| p.1);
        let cpu = step.1.saturating_sub(cpu_from);
        // The watcher's ticks and the frames up to the drawn one, less the time the watcher thread stood ready
        // without a CPU (an over-committed machine's, not the pipeline's).
        let queued = match (w0, w1) {
            (Some(a), Some(b)) => Duration::from_nanos(b.saturating_sub(a)),
            _ => Duration::ZERO,
        };
        let watcher = shown.0.saturating_duration_since(visible).saturating_sub(queued);
        let frames = shown.1 - heard.1;
        let total = EVERY * waited as u32 + cpu + watcher;
        println!(
            "looks live: save {n} shown after {:.0} ms of the pipeline: {waited} poll(s), import {:.1} ms CPU \
             ({:.0} ms with its disk syncs), watcher and frames {:.0} ms ({frames} frame(s) after the change \
             was heard, {:.0} ms waiting for a CPU); {:.0} ms by the wall clock",
            ms(total),
            ms(cpu),
            ms(visible.saturating_duration_since(saved)),
            ms(watcher),
            ms(queued),
            ms(shown.0.saturating_duration_since(saved)),
        );
        times.push(ms(total));
        disk = disk.max(visible.saturating_duration_since(saved).saturating_sub(EVERY * waited as u32 + cpu));
        // The next save after this one has shown (a modder paints, saves, looks).
        drop(passes);
        std::thread::sleep(Duration::from_millis(100));
    }
    stop.store(true, Ordering::Relaxed);
    game.join().unwrap();
    let h = studio.join().unwrap();

    let steps = steps.lock().unwrap();
    // A save the disk held up halfway (the stand-in writes in place, as many programs do) can be read before it
    // is whole: Studio says it is not a PNG yet and takes the whole one in at its next change.
    let (made, half): (Vec<_>, Vec<_>) = steps.iter().partition(|r| matches!(r.0, Ok(Imported::Step(_))));
    assert_eq!(made.len(), 5, "one import per save: {steps:?}");
    assert!(half.iter().all(|r| r.0.as_ref().is_err_and(|e| e.starts_with("not a PNG"))), "{steps:?}");
    if !half.is_empty() {
        println!("looks live: {} save(s) read halfway written before they were whole", half.len());
    }
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
    let busy = if disk > BUSY_DISK { format!(" ({BUSY} for up to {:.0} ms)", ms(disk)) } else { String::new() };
    assert!(worst < ms(LIMIT), "a save took {worst:.0} ms to show{busy}");
}

/// The calling thread's CPU time.
#[cfg(unix)]
fn thread_cpu() -> Duration {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: a valid timespec to write to.
    let r = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    assert_eq!(r, 0, "the thread's CPU clock");
    Duration::new(t.tv_sec as u64, t.tv_nsec as u32)
}

/// How long the game's pack watcher thread has stood ready without a CPU, where the system tells it (Linux's
/// /proc schedstat: its second field, in ns).
#[cfg(unix)]
fn watcher_sched() -> Option<u64> {
    for t in std::fs::read_dir("/proc/self/task").ok()?.flatten() {
        if std::fs::read_to_string(t.path().join("comm")).is_ok_and(|c| c.trim() == "pack watcher") {
            return std::fs::read_to_string(t.path().join("schedstat")).ok()?.split_whitespace().nth(1)?.parse().ok();
        }
    }
    None
}

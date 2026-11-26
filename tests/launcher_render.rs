//! The launcher (docs/launcher.md): every launcher screen drawn headless with the
//! fixed fake data at the seven sizes (the Steam Deck's 1280 × 800 and the
//! laptop's 1366 × 768 among them), and every named state (no game found, a
//! refused folder, …) at 1366 × 768 with 150 % scaling, compared with the
//! committed renders in tests/launcher/ (our own images; nothing from the game), and
//! the game check's and the Saves page's states at the seven sizes too. A changed look
//! is re-blessed with `OB_LAUNCHER_BLESS=1 cargo test --release --test
//! launcher_render` and the PNGs looked at before they are committed.

use nfsu2::launcher::{headless, screens::Screen};

const SIZES: [(u32, u32); 7] =
    [(640, 480), (1280, 720), (1280, 800), (1366, 768), (1920, 1080), (2560, 1080), (3840, 2160)];
/// The game check's states, also rendered at every size of [`SIZES`]
/// (`state-<name>-<w>x<h>.png`).
const CHECK_STATES: [&str; 7] = [
    "check-ok",
    "check-movies-missing",
    "check-details",
    "check-changed",
    "check-unknown-edition",
    "check-no-english",
    "err-missing",
];
/// The crash report window's states, at every size of [`SIZES`] at 150 %
/// (`state-<name>-<w>x<h>@150.png`).
const CRASH_STATES: [&str; 5] =
    ["crash-panic", "crash-signal", "crash-windows", "crash-studio-panic", "crash-studio-killed"];
/// The states' size and scale: a common laptop screen at 150 %.
const STATE_SIZE: (u32, u32, f32) = (1366, 768, 1.5);

fn dir() -> std::path::PathBuf {
    std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/launcher")).to_path_buf()
}

/// Pixels whose largest channel difference is over 2, and the largest.
fn diff(a: &image::RgbaImage, b: &image::RgbaImage) -> (usize, u8) {
    assert_eq!(a.dimensions(), b.dimensions());
    let mut n = 0;
    let mut max = 0u8;
    for (p, q) in a.pixels().zip(b.pixels()) {
        let d = p.0.iter().zip(q.0.iter()).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
        max = max.max(d);
        if d > 2 {
            n += 1;
        }
    }
    (n, max)
}

/// Within 0.05 % of the pixels off by more than 2 (another CPU's rounding).
fn same(a: &image::RgbaImage, b: &image::RgbaImage) -> bool {
    let (n, _) = diff(a, b);
    n * 2000 <= (a.width() * a.height()) as usize
}

#[test]
fn screens_match_their_renders() {
    let bless = std::env::var_os("OB_LAUNCHER_BLESS").is_some();
    let mut bad = Vec::new();
    let (sw, sh, sd) = STATE_SIZE;
    let screens = Screen::ALL
        .iter()
        .flat_map(|s| SIZES.map(|(w, h)| (format!("{}-{w}x{h}", s.name()), headless::render(*s, w, h, 1.0))));
    let states = headless::STATES.iter().map(|(n, _)| {
        let img = headless::render_state(n, sw, sh, sd).expect("a state");
        (format!("state-{n}-{sw}x{sh}@{}", (sd * 100.0) as u32), img)
    });
    let checks = CHECK_STATES.iter().flat_map(|n| {
        SIZES.map(|(w, h)| (format!("state-{n}-{w}x{h}"), headless::render_state(n, w, h, 1.0).expect("a state")))
    });
    // The Saves, Dev and Game data pages' states also at every size (lists,
    // rows and actions that stack on narrow windows).
    let saves = headless::STATES.iter().filter(|(n, _)| ["saves-", "dev-", "game-data-"].iter().any(|p| n.starts_with(p))).flat_map(|(n, _)| {
        SIZES.map(|(w, h)| (format!("state-{n}-{w}x{h}"), headless::render_state(n, w, h, 1.0).expect("a state")))
    });
    let crashes = CRASH_STATES.iter().flat_map(|n| {
        SIZES.map(|(w, h)| (format!("state-{n}-{w}x{h}@150"), headless::render_state(n, w, h, 1.5).expect("a state")))
    });
    for (name, img) in screens.chain(states).chain(checks).chain(saves).chain(crashes) {
        let path = dir().join(format!("{name}.png"));
        if bless {
            std::fs::create_dir_all(dir()).unwrap();
            img.save(&path).unwrap();
            continue;
        }
        let base = image::open(&path)
            .unwrap_or_else(|e| panic!("{}: {e} (bless with OB_LAUNCHER_BLESS=1)", path.display()))
            .to_rgba8();
        if !same(&img, &base) {
            let (n, max) = diff(&img, &base);
            let out = std::env::temp_dir().join(format!("launcher-{name}.png"));
            img.save(&out).unwrap();
            bad.push(format!("{}: {n} pixels differ (max {max}); new render in {}", path.display(), out.display()));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The comparison fails on a changed screen (another screen, a moved
/// focus) and passes on the same one.
#[test]
fn the_comparison_catches_a_change() {
    let a = headless::render(Screen::Main, 640, 480, 1.0);
    assert!(same(&a, &headless::render(Screen::Main, 640, 480, 1.0)));
    assert!(!same(&a, &headless::render(Screen::Widgets, 640, 480, 1.0)));
    let mut b = a.clone();
    for y in 100..140 {
        for x in 100..140 {
            b.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
        }
    }
    assert!(!same(&a, &b), "a 40 x 40 px change");
}

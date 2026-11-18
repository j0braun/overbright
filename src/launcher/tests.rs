//! The launcher's layout, focus and settings tests (fake data, no game
//! files, no window).

use super::font::Fonts;
use super::model::{Choices, Env, FakeEnv, Graphics, SaveError, save_choices};
use super::screens::{App, Outcome, Screen};
use super::ui::{Event, Input, Key, Kind, Mark, Metrics, Rect, Ui};

/// The sizes every screen is laid out and rendered at: the smallest
/// window, portrait, 720p, 1080p, 21:9 and 4K.
pub const SIZES: [(u32, u32); 6] = [(640, 480), (800, 1280), (1280, 720), (1920, 1080), (2560, 1080), (3840, 2160)];

fn app<'e>(env: &'e FakeEnv, fonts: &Fonts, s: Screen, w: u32, h: u32, dpi: f32) -> App<'e> {
    App::new(env, env.load(), s, (w, h), dpi, fonts)
}

/// Every text inside its widget (and between a cycle's arrows, inside a
/// list's frame), every widget inside the window, no two widgets and no
/// two texts overlapping, every character drawn by the font, no text under
/// 14 px (at 100 %).
fn check_layout(ui: &Ui, fonts: &Fonts, what: &str) {
    let view = ui.view;
    assert!(ui.content_h <= view.h + 0.5, "{what}: the page does not fit ({} > {})", ui.content_h, view.h);
    for (i, w) in ui.widgets.iter().enumerate() {
        assert!(w.rect.inside(&view), "{what}: widget {i} {:?} {:?} outside the window", w.id, w.rect);
        let frame = match &w.kind {
            Kind::List { pad, .. } => {
                Rect::new(w.rect.x + pad, w.rect.y + pad, w.rect.w - 2.0 * pad, w.rect.h - 2.0 * pad)
            }
            _ => w.rect,
        };
        for t in &w.texts {
            assert!(
                t.rect.inside(&frame),
                "{what}: text {:?} ({:?}) outside its widget {:?} {:?}",
                t.text,
                t.rect,
                w.id,
                w.rect
            );
            assert!(fonts.covers(t.face, &t.text), "{what}: a character of {:?} has no glyph", t.text);
            assert!(t.size >= 12.0, "{what}: {:?} at {} px", t.text, t.size);
            // The measured width is the box's.
            assert!((fonts.width(t.face, t.size, &t.text) - t.rect.w).abs() < 0.5, "{what}: {:?} box width", t.text);
            if let Kind::Cycle { left, right } = &w.kind {
                assert!(!t.rect.overlaps(left) && !t.rect.overlaps(right), "{what}: {:?} under a cycle arrow", t.text);
            }
            if let Kind::Status { icon, .. } | Kind::Checkbox { check: icon, .. } = &w.kind {
                assert!(!t.rect.overlaps(icon), "{what}: {:?} over its mark", t.text);
            }
        }
        for (j, o) in ui.widgets.iter().enumerate().skip(i + 1) {
            // A card holds the widgets inside it.
            if matches!(w.kind, Kind::Card) && o.rect.inside(&w.rect)
                || matches!(o.kind, Kind::Card) && w.rect.inside(&o.rect)
            {
                continue;
            }
            assert!(
                !w.rect.overlaps(&o.rect),
                "{what}: widgets {i} {:?} {:?} and {j} {:?} {:?} overlap",
                w.id,
                w.rect,
                o.id,
                o.rect
            );
        }
    }
    let texts: Vec<_> = ui.widgets.iter().flat_map(|w| w.texts.iter()).collect();
    for (i, a) in texts.iter().enumerate() {
        for b in &texts[i + 1..] {
            assert!(
                !a.rect.overlaps(&b.rect),
                "{what}: texts {:?} {:?} and {:?} {:?} overlap",
                a.text,
                a.rect,
                b.text,
                b.rect
            );
        }
    }
    let body = ui.metrics.unwrap().body();
    assert!(body >= 14.0, "{what}: body text {body} px");
}

#[test]
fn every_screen_fits_every_size() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    for s in Screen::ALL {
        for (w, h) in SIZES {
            let a = app(&env, &fonts, s, w, h, 1.0);
            let mut ui = a.ui.clone();
            if s.document() {
                // A document scrolls; its Back button is in view at the top.
                assert!(a.ui.get("back").unwrap().rect.bottom() <= h as f32, "{} {w}x{h}", s.name());
                ui.view.h = ui.content_h.max(ui.view.h);
            }
            check_layout(&ui, &fonts, &format!("{} {w}x{h}", s.name()));
        }
    }
}

/// The browser with Windows' dialog button (the Windows layout).
#[test]
fn browser_with_the_windows_dialog_button_fits() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    for (w, h) in SIZES {
        let mut m = env.load();
        m.native_dialog = true;
        let a = App::new(&env, m, Screen::Browse, (w, h), 1.0, &fonts);
        check_layout(&a.ui, &fonts, &format!("browse (Windows) {w}x{h}"));
        assert!(a.ui.get("system").is_some());
    }
}

/// Play (and the main screen's whole footer) in the window without
/// scrolling at every size, also while the probe runs and before the
/// game's first run (the note shown), and with no controller.
#[test]
fn play_is_reachable_without_scrolling() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    for (w, h) in SIZES {
        for variant in 0..3 {
            let mut m = env.load();
            match variant {
                1 => {
                    m.settings_file = false;
                    m.probe = super::model::ProbeState::Pending;
                    m.controller = None;
                }
                2 => {
                    m.settings_file = false;
                    m.probe = super::model::ProbeState::Failed("the game's hardware check ended with code 1".into());
                }
                _ => {}
            }
            let a = App::new(&env, m, Screen::Main, (w, h), 1.0, &fonts);
            let what = format!("main {w}x{h} variant {variant}");
            check_layout(&a.ui, &fonts, &what);
            for id in ["about", "import", "quit", "play"] {
                let r = a.ui.get(id).unwrap().rect;
                assert!(r.bottom() <= h as f32 && a.st.scroll == 0.0, "{what}: {id} at {r:?}");
            }
        }
    }
}

/// The same with long values: long paths, many languages, an error
/// message, a usable = false check (shortened or wrapped, never clipped).
#[test]
fn long_values_are_shortened_or_wrapped() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let long = "/data/someone-with-a-long-name/.local/share/Steam/steamapps/compatdata/123456789/pfx/drive_c/Program Files (x86)/EA GAMES/Need for Speed Underground 2";
    for s in Screen::ALL {
        for (w, h) in SIZES {
            let mut m = env.load();
            m.root = Some(long.into());
            m.found[0].path = long.into();
            m.path_input = long.repeat(2);
            m.choices.texture_pack = long.into();
            m.choices.graphics = Graphics::Custom;
            m.languages.extend(["Chinese", "Danish", "Dutch", "Japanese", "Korean", "Thailand"].map(String::from));
            m.message =
                Some((super::ui::Mark::Error, format!("The settings could not be saved: {long}: permission denied")));
            if let Some(c) = &mut m.check {
                c.root = long.into();
                c.usable = false;
                c.lines.push((
                    super::ui::Mark::Error,
                    "Missing: TRACKS/STREAML4RA.BUN — reinstall or copy the full game folder".into(),
                ));
            }
            let a = App::new(&env, m, s, (w, h), 1.0, &fonts);
            let what = format!("long {} {w}x{h}", s.name());
            // The page may scroll at the smallest sizes; the rest holds.
            let mut ui = a.ui.clone();
            ui.view.h = ui.content_h.max(ui.view.h);
            check_layout(&ui, &fonts, &what);
            if matches!(s, Screen::FirstRun | Screen::Check) {
                assert!(
                    a.ui.widgets.iter().flat_map(|w| &w.texts).any(|t| t.text.contains('…')),
                    "{what}: the path not shortened"
                );
            }
        }
    }
}

#[test]
fn scale_follows_size_and_display_scale() {
    let m = |w, h, d| Metrics::for_size(w, h, d).u;
    assert_eq!(m(960, 720, 1.0), 1.0);
    assert_eq!(m(1920, 1080, 1.0), 1.5);
    assert_eq!(m(3840, 2160, 1.0), 3.0);
    assert_eq!(m(2560, 1080, 1.0), 1.5, "21:9: by the height");
    assert!((m(800, 1280, 1.0) - 800.0 / 960.0).abs() < 1e-6, "portrait: by the width");
    assert_eq!(m(640, 480, 1.0), super::ui::MIN_UNIT, "the floor");
    assert_eq!(m(1280, 720, 2.0), 2.0 * super::ui::MIN_UNIT, "the floor follows the display scale");
    // At 200 % a 1280 × 720 window keeps 2x text; at 4K the text is 3x
    // the 960 × 720 size.
    let fonts = Fonts::new();
    let env = FakeEnv;
    let a = app(&env, &fonts, Screen::Main, 2560, 1440, 2.0);
    assert!(a.ui.metrics.unwrap().body() >= 28.0);
    check_layout(&a.ui, &fonts, "main 2560x1440 @2x");
    let b = app(&env, &fonts, Screen::Main, 3840, 2160, 1.0);
    assert_eq!(b.ui.metrics.unwrap().body(), 57.0);
}

fn key(a: &mut App, fonts: &Fonts, k: Key) -> Outcome {
    a.input(&Input::Key(k), fonts)
}

fn focus(a: &App) -> &'static str {
    a.ui.focused(&a.st).unwrap()
}

#[test]
fn focus_order_and_arrow_moves() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    for (w, h) in SIZES {
        let mut a = app(&env, &fonts, Screen::Main, w, h, 1.0);
        assert_eq!(
            a.ui.focus_order(),
            [
                "change",
                "play",
                "display",
                "resolution",
                "graphics",
                "skip_movies",
                "skip_title",
                "mods",
                "skip",
                "about",
                "import",
                "prepare",
                "quit"
            ],
            "{w}x{h}"
        );
        assert_eq!(focus(&a), "play", "the default focus");
        key(&mut a, &fonts, Key::Tab);
        assert_eq!(focus(&a), "display", "Tab from Play: the settings");
        key(&mut a, &fonts, Key::BackTab);
        key(&mut a, &fonts, Key::BackTab);
        assert_eq!(focus(&a), "change");
        key(&mut a, &fonts, Key::BackTab);
        assert_eq!(focus(&a), "quit", "Shift+Tab wraps");
        key(&mut a, &fonts, Key::Left);
        assert_eq!(focus(&a), "prepare", "{w}x{h}: Left in the footer");
        a.st.focus = Some("play");
        key(&mut a, &fonts, Key::Left);
        assert_eq!(focus(&a), "change", "{w}x{h}: Left from Play: the folder's button");
        key(&mut a, &fonts, Key::Down);
        assert!(["display", "mods"].contains(&focus(&a)), "{w}x{h}: Down into the panels: {}", focus(&a));
        a.st.focus = Some("display");
        for want in ["resolution", "graphics"] {
            key(&mut a, &fonts, Key::Down);
            assert_eq!(focus(&a), want, "{w}x{h}: Down");
        }
        // Every control is reached with the arrows alone: from Mods up and
        // on to Play.
        a.st.focus = Some("mods");
        let mut seen = vec![focus(&a)];
        for _ in 0..6 {
            key(&mut a, &fonts, Key::Up);
            seen.push(focus(&a));
        }
        assert!(seen.contains(&"play") || seen.contains(&"change"), "{w}x{h}: Up from Mods reaches the hero: {seen:?}");
        a.st.focus = Some("play");
        key(&mut a, &fonts, Key::Up);
        assert_eq!(focus(&a), "play", "no wrap at the top");
    }
}

/// The Resolution row walks the list of the display path chosen, as the
/// game's (`display::modes_for` on the monitor): exclusive full screen the
/// monitor's own modes, windowed the fixed sizes up to the desktop,
/// Wayland's full screen the desktop size alone; switching Display moves
/// the size to one the new path offers.
#[test]
fn resolution_list_per_display_path() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 1280, 720, 1.0);
    assert!(a.model.choices.fullscreen);
    assert_eq!(a.model.modes(), vec![(1280, 720), (1600, 900), (1920, 1080)]);
    a.st.focus = Some("display");
    key(&mut a, &fonts, Key::Right);
    let windowed = a.model.modes();
    assert!(windowed.contains(&(640, 480)) && windowed.contains(&(1280, 960)) && windowed.contains(&(1920, 1080)));
    assert!(!windowed.iter().any(|&(w, h)| w > 1920 || h > 1080), "{windowed:?}");
    // Windowed 1280 × 960 (no such full-screen mode) → full screen 1280 × 720.
    a.model.choices.resolution = (1280, 960);
    key(&mut a, &fonts, Key::Right);
    assert_eq!((a.model.choices.fullscreen, a.model.choices.resolution), (true, (1280, 720)));
    // Wayland: full screen is the desktop size only.
    a.model.monitor.as_mut().unwrap().exclusive = false;
    assert_eq!(a.model.modes(), vec![(1920, 1080)]);
    // The probe's facts read back into the same monitor.
    let p = FakeEnv::probe();
    let back = super::probe::Probe::parse(&p.to_text()).unwrap();
    let info = back.monitor_info().unwrap();
    assert_eq!((info.desktop, info.modes.clone(), info.exclusive), ((1920, 1080), p.monitor_modes.clone(), true));
}

#[test]
fn controls_change_the_model() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 1280, 720, 1.0);
    a.st.focus = Some("display");
    key(&mut a, &fonts, Key::Right);
    assert!(!a.model.choices.fullscreen);
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.choices.fullscreen);
    a.st.focus = Some("resolution");
    let before = a.model.choices.resolution;
    key(&mut a, &fonts, Key::Right);
    let after = a.model.choices.resolution;
    // Full screen at the desktop size, the list's last: Right wraps.
    assert_eq!((before, after), ((1920, 1080), (1280, 720)));
    key(&mut a, &fonts, Key::Left);
    assert_eq!(a.model.choices.resolution, before);
    a.st.focus = Some("graphics");
    key(&mut a, &fonts, Key::Right);
    assert_eq!(a.model.choices.graphics, Graphics::Enhanced);
    key(&mut a, &fonts, Key::Right);
    assert_eq!(a.model.choices.graphics, Graphics::Original);
    a.st.focus = Some("skip");
    key(&mut a, &fonts, Key::Space);
    assert!(a.model.choices.skip);
    // The mouse: a click on Play (down and up on it) plays.
    let r = a.ui.get("play").unwrap().rect;
    a.input(&Input::MouseDown(r.cx(), r.cy()), &fonts);
    assert_eq!(a.input(&Input::MouseUp(r.cx(), r.cy()), &fonts), Outcome::Play);
    // A press that leaves the button does nothing.
    a.input(&Input::MouseDown(r.cx(), r.cy()), &fonts);
    assert_eq!(a.input(&Input::MouseUp(1.0, 1.0), &fonts), Outcome::Stay);
    // A click on a cycle's left arrow steps back.
    let c = a.ui.get("display").unwrap().rect;
    let fs = a.model.choices.fullscreen;
    a.input(&Input::MouseDown(c.x + 4.0, c.cy()), &fonts);
    assert_ne!(a.model.choices.fullscreen, fs);
}

#[test]
fn first_run_flow() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::FirstRun, 960, 720, 1.0);
    assert_eq!(focus(&a), "found");
    key(&mut a, &fonts, Key::Down);
    assert_eq!(a.model.found_sel, 1, "Down moves in the list");
    key(&mut a, &fonts, Key::Down);
    key(&mut a, &fonts, Key::Down);
    assert_eq!(focus(&a), "path", "past the list's end: the next control");
    key(&mut a, &fonts, Key::Up);
    assert_eq!(focus(&a), "found");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Check);
    assert_eq!(a.model.check.as_ref().unwrap().root, a.model.found[2].path);
    assert_eq!(focus(&a), "use");
    key(&mut a, &fonts, Key::Escape);
    assert_eq!(a.screen, Screen::FirstRun);
    a.st.focus = Some("check");
    key(&mut a, &fonts, Key::Enter);
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Main);
    assert_eq!(a.model.languages, ["English", "French", "German"]);
    // Escape on the first-run screen quits.
    let mut b = app(&env, &fonts, Screen::FirstRun, 960, 720, 1.0);
    assert_eq!(key(&mut b, &fonts, Key::Escape), Outcome::Quit);
}

#[test]
fn events_from_the_list_and_wheel() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Widgets, 640, 480, 1.0);
    let l = a.ui.get("list").unwrap().clone();
    let Kind::List { row_h, pad, first, .. } = l.kind else { panic!() };
    // A click on the first visible row selects it; a second click chooses.
    let y = l.rect.y + pad + 0.5 * row_h;
    a.input(&Input::MouseDown(l.rect.cx(), y), &fonts);
    assert_eq!(a.model.demo.sel, first);
    let ev = super::ui::handle(&a.ui, &mut a.st, &Input::MouseDown(l.rect.cx(), y));
    assert_eq!(ev, [Event::Activate("list")]);
    // Selecting past the visible rows scrolls the list.
    a.st.focus = Some("list");
    key(&mut a, &fonts, Key::Down);
    key(&mut a, &fonts, Key::Down);
    let Kind::List { first, selected, .. } = a.ui.get("list").unwrap().kind else { panic!() };
    assert_eq!((selected, first), (2, 1));
    // The wheel scrolls nothing on a page that fits.
    a.input(&Input::Wheel(-3.0), &fonts);
    assert_eq!(a.st.scroll, 0.0);
}

/// L1's trap: without options.txt the launcher writes nothing unless it
/// knows the card (the game's first start chooses the display values for
/// the hardware only while there is none); with a card it writes what the
/// game's first start would plus the choices; with a file it changes only
/// its own values.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn saving_respects_the_first_run() {
    use crate::frontend::options::Settings;
    let dir = std::env::temp_dir().join(format!("ob-launcher-save-{}", crate::unique_tag()));
    crate::frontend::profiles::sandbox::enter(&dir);
    let path = super::dirs::options_file();
    assert!(path.starts_with(&dir) && !path.exists());
    let c = Choices {
        fullscreen: false,
        resolution: (1600, 900),
        graphics: Graphics::Original,
        skip: true,
        skip_movies: true,
        skip_title: false,
        texture_pack: String::new(),
        hud_mod: false,
    };
    assert_eq!(save_choices(&c, None), Err(SaveError::FirstRunPending));
    assert!(!path.exists(), "nothing written before the game's first run");
    let card = (0x8086, 0x0166);
    // What `game --probe` prints for this card (its first_run block).
    let first = super::probe::first_run(card);
    save_choices(&c, Some(&first)).unwrap();
    let mut want = Settings::default();
    want.first_run_display(card);
    super::model::apply_choices(&mut want, &c);
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, want.options_text());
    assert!(
        text.contains("launcher.skip 1\n") && text.contains("resolution 1600x900\n") && text.contains("fullscreen 0\n")
    );
    // An existing file: its other values stay (a key, an enhancement).
    let mut s = Settings::from_text(&text);
    s.enh.fov_offset = 10;
    s.keys[0][0] = "Q".into();
    s.write_file().unwrap();
    let c2 = Choices { fullscreen: true, graphics: Graphics::Enhanced, skip: false, ..c.clone() };
    save_choices(&c2, None).unwrap();
    let back = Settings::load_text();
    assert_eq!((back.enh.fov_offset, back.keys[0][0].as_str()), (10, "Q"));
    assert!(back.fullscreen && !back.enh.launcher_skip);
    assert_eq!(Graphics::of(&back.enh), Graphics::Enhanced);
    assert_eq!(super::model::load_choices().0.graphics, Graphics::Enhanced);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The start boxes Skip Intro Movies and Skip Title Screen are the game's
/// own rows: toggled on the main screen, saved to options.txt as the
/// game writes them, read back by the game's parser and by the launcher;
/// each independent of the other; an older file's `enh.skip_title 2` shows
/// both ticked.
#[test]
fn skip_boxes_round_trip_through_options_txt() {
    use crate::frontend::options::Settings;
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 1280, 720, 1.0);
    assert!(!a.model.choices.skip_movies && !a.model.choices.skip_title, "off by default");
    a.st.focus = Some("skip_movies");
    key(&mut a, &fonts, Key::Space);
    assert!(a.model.choices.skip_movies && !a.model.choices.skip_title, "the movies alone");
    let mut s = Settings::from_text("resolution 1280x960\n");
    super::model::apply_choices(&mut s, &a.model.choices);
    let text = s.options_text();
    assert!(text.contains("enh.skip_movies 1\n") && !text.contains("enh.skip_title"), "{text}");
    let back = Settings::from_text(&text);
    assert!(back.enh.skips_movies() && !back.enh.skips_title());
    assert_eq!(super::model::choices_of(&back), a.model.choices);
    a.st.focus = Some("skip_title");
    key(&mut a, &fonts, Key::Space);
    a.st.focus = Some("skip_movies");
    key(&mut a, &fonts, Key::Space);
    assert!(!a.model.choices.skip_movies && a.model.choices.skip_title, "the title alone");
    super::model::apply_choices(&mut s, &a.model.choices);
    let text = s.options_text();
    assert!(text.contains("enh.skip_title 1\n") && !text.contains("enh.skip_movies"), "{text}");
    let back = Settings::from_text(&text);
    assert!(!back.enh.skips_movies() && back.enh.skips_title());
    // An older file: both ticked, written back as two values.
    let old = Settings::from_text("enh.skip_title 2\n");
    let c = super::model::choices_of(&old);
    assert!(c.skip_movies && c.skip_title);
    let text = old.options_text();
    assert!(text.contains("enh.skip_title 1\n") && text.contains("enh.skip_movies 1\n"), "{text}");
    // Unticked both: native, no line.
    let mut s = old.clone();
    super::model::apply_choices(&mut s, &Choices { skip_movies: false, skip_title: false, ..c });
    let text = s.options_text();
    assert!(!text.contains("enh.skip_"), "{text}");
}

#[test]
fn edits_and_text_shortening() {
    let fonts = Fonts::new();
    let mut s = String::from("abc");
    let mut cur = usize::MAX;
    super::ui::apply_edit(&mut s, &mut cur, &super::ui::Edit::Insert("é".into()));
    assert_eq!((s.as_str(), cur), ("abcé", 4));
    cur = 1;
    super::ui::apply_edit(&mut s, &mut cur, &super::ui::Edit::Backspace);
    assert_eq!((s.as_str(), cur), ("bcé", 0));
    let f = super::font::Face::Regular;
    let long = "/a/very/long/path/to/the/game/Need for Speed Underground 2";
    for cut in [super::font::Cut::End, super::font::Cut::Start, super::font::Cut::Middle] {
        let t = fonts.fit(f, 16.0, long, 150.0, cut);
        assert!(fonts.width(f, 16.0, &t) <= 150.0 && t.contains('…'), "{cut:?} {t}");
    }
    assert!(fonts.fit(f, 16.0, long, 150.0, super::font::Cut::Middle).ends_with("d 2"));
    let lines = fonts.wrap(f, 16.0, "one two three four five six seven", 80.0);
    assert!(lines.len() > 2 && lines.iter().all(|l| fonts.width(f, 16.0, l) <= 80.0));
    assert!(fonts.covers(f, "…×—é") && fonts.covers(super::font::Face::Bold, "Overbright"));
}

/// The probe's first-run values are the ones the launcher's first write
/// starts from: a card from a captured `game --probe` written before the
/// game's first run gives exactly `Settings::default()` +
/// `first_run_display(card)` + the choices, the same detail and Customize
/// Display values `game --probe` reports for that card.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn first_write_uses_the_probes_card() {
    use crate::frontend::options::Settings;
    let p = super::probe::Probe::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/launcher/probe-sample.txt"
    )))
    .unwrap();
    let card = p.card().unwrap();
    let dir = std::env::temp_dir().join(format!("ob-launcher-probe-{}", crate::unique_tag()));
    crate::frontend::profiles::sandbox::enter(&dir);
    let path = super::dirs::options_file();
    assert!(path.starts_with(&dir) && !path.exists());
    let c = Choices {
        fullscreen: true,
        resolution: (1920, 1080),
        graphics: Graphics::Original,
        skip: false,
        skip_movies: false,
        skip_title: false,
        texture_pack: String::new(),
        hud_mod: false,
    };
    // The probe on this machine for the sample's card.
    let fr = super::probe::first_run(card);
    save_choices(&c, Some(&fr)).unwrap();
    let back = Settings::load_text();
    assert_eq!((back.detail, back.custom.to_vec()), (fr.detail, fr.custom.clone()));
    let mut want = Settings::default();
    want.first_run_display(card);
    super::model::apply_choices(&mut want, &c);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), want.options_text());
    // The probe's own first-run block reads back as written.
    let pf = p.first_run.as_ref().unwrap();
    assert_eq!((pf.resolution, pf.fullscreen, pf.custom.len()), ((1280, 960), false, 22));
    assert_eq!((fr.resolution, fr.fullscreen), (pf.resolution, pf.fullscreen));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Without options.txt the display rows wait for the probe's first-run
/// values.
#[test]
fn display_rows_follow_the_probe() {
    let env = FakeEnv;
    let mut m = env.load();
    m.settings_file = false;
    m.probe = super::model::ProbeState::Pending;
    assert!(!super::screens::display_rows_on(&m));
    m.probe = super::model::ProbeState::Failed("x".into());
    assert!(!super::screens::display_rows_on(&m));
    m.probe = super::model::ProbeState::Ready(FakeEnv::probe());
    assert!(super::screens::display_rows_on(&m));
    // The card alone (no usable game folder yet): not before the probe
    // ran with the folder and gave the first-run values.
    let mut no_folder = FakeEnv::probe();
    no_folder.first_run = None;
    m.probe = super::model::ProbeState::Ready(no_folder);
    assert!(!super::screens::display_rows_on(&m));
    let fonts = Fonts::new();
    m.probe = super::model::ProbeState::Pending;
    let a = App::new(&env, m, Screen::Main, (1280, 720), 1.0, &fonts);
    assert!(!a.ui.get("display").unwrap().enabled);
}

#[test]
fn about_licence_and_notices() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 1280, 720, 1.0);
    a.st.focus = Some("about");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::About);
    a.st.focus = Some("licence");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Licence);
    assert!(a.ui.content_h > 5.0 * 720.0, "the whole GPL text ({})", a.ui.content_h);
    key(&mut a, &fonts, Key::PageDown);
    let s1 = a.st.scroll;
    assert!(s1 > 0.0);
    key(&mut a, &fonts, Key::End);
    assert_eq!(a.st.scroll, a.ui.content_h - 720.0);
    key(&mut a, &fonts, Key::Home);
    assert_eq!(a.st.scroll, 0.0);
    key(&mut a, &fonts, Key::Escape);
    assert_eq!(a.screen, Screen::About);
    a.st.focus = Some("notices");
    a.model.notices = super::screens::NOTICES;
    key(&mut a, &fonts, Key::Enter);
    let all: String = a.ui.widgets.iter().flat_map(|w| &w.texts).map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
    assert!(all.contains("Go fonts"), "the notices list the font");
    key(&mut a, &fonts, Key::Escape);
    key(&mut a, &fonts, Key::Escape);
    assert_eq!(a.screen, Screen::Main);
    // Paragraphs: wrapped lines joined, list items and headings kept.
    let p = super::screens::paragraphs("# Title\n\nline one\nline two\n- item a\n- item b\n\n© 2026 “x”", &fonts);
    assert_eq!(p, ["# Title", "line one line two", "- item a", "- item b", "© 2026 “x”"]);
}

#[test]
fn failure_screen_buttons() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Failure, 1280, 720, 1.0);
    assert_eq!(focus(&a), "retry");
    let all: Vec<&str> = a.ui.widgets.iter().flat_map(|w| &w.texts).map(|t| t.text.as_str()).collect();
    assert!(all.iter().any(|t| t.contains("note: run with")), "the tail's last line shown");
    assert!(all.iter().any(|t| t.contains("during its start")));
    assert_eq!(key(&mut a, &fonts, Key::Enter), Outcome::Play, "Try again");
    a.st.focus = Some("open_logs");
    assert_eq!(key(&mut a, &fonts, Key::Enter), Outcome::Stay);
    a.st.focus = Some("settings");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Main);
}

#[test]
fn folder_browser_flow() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::FirstRun, 960, 720, 1.0);
    a.st.focus = Some("browse");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Browse);
    // Started in the parent of the first found folder.
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player/Games"));
    assert_eq!(focus(&a), "dirs");
    // Row 0 is "..": up.
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player"));
    // Down to "Games" (.., Documents, Games), Enter opens it.
    key(&mut a, &fonts, Key::Down);
    key(&mut a, &fonts, Key::Down);
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player/Games"));
    // The game's folder (row 4 with ".."), opened, used: the check.
    for _ in 0..4 {
        key(&mut a, &fonts, Key::Down);
    }
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.browse.dir, super::model::FakeEnv::root());
    a.st.focus = Some("use");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Check);
    assert_eq!(a.model.check.as_ref().unwrap().root, super::model::FakeEnv::root());
    // A typed path; Cancel goes back where the browser came from.
    let mut b = app(&env, &fonts, Screen::Mods, 960, 720, 1.0);
    b.st.focus = Some("pack_browse");
    key(&mut b, &fonts, Key::Enter);
    assert_eq!((b.screen, b.model.browse.purpose), (Screen::Browse, super::model::BrowseFor::TexturePack));
    b.st.focus = Some("browse_path");
    b.input(&Input::Text("/data/player/Games".into()), &fonts);
    key(&mut b, &fonts, Key::Enter);
    assert_eq!(b.model.browse.dir, std::path::Path::new("/data/player/Games"));
    b.st.focus = Some("use");
    key(&mut b, &fonts, Key::Enter);
    assert_eq!((b.screen, b.model.choices.texture_pack.as_str()), (Screen::Mods, "/data/player/Games"));
    assert_eq!(b.model.pack_status, super::model::PackStatus::Incomplete);
    assert!(b.model.hud.as_ref().is_some_and(|h| h.present == 708), "the HUD folder checked");
    b.st.focus = Some("pack_browse");
    key(&mut b, &fonts, Key::Enter);
    key(&mut b, &fonts, Key::Escape);
    assert_eq!(b.screen, Screen::Mods);
}

#[test]
fn import_flow() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 1280, 720, 1.0);
    a.st.focus = Some("import");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Import);
    assert_eq!(focus(&a), "copy");
    assert_eq!(a.ui.get("copy").unwrap().texts[0].text, "Copy 3 saves");
    key(&mut a, &fonts, Key::Enter);
    let done = a.model.import.done.clone().unwrap();
    assert_eq!(done, ["NIGHTOWL/NIGHTOWL", "NIGHTOWL DVD 1/NIGHTOWL DVD 1", "NIGHTOWL Magazine 3/NIGHTOWL Magazine 3"]);
    assert!(!a.ui.get("copy").unwrap().enabled, "nothing left to copy");
    a.st.focus = Some("pick");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!((a.screen, a.model.browse.purpose), (Screen::Browse, super::model::BrowseFor::Saves));
    a.st.focus = Some("use");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Import);
    assert!(a.model.import.done.is_none());
}

#[test]
fn found_folders_say_where_they_come_from() {
    use super::model::source_words;
    let p = std::path::Path::new("/data/p/.wine/drive_c/Program Files/EA GAMES/Need for Speed Underground 2");
    assert_eq!(source_words("found", p), if cfg!(windows) { "found on this computer" } else { "inside a Wine prefix" });
    assert_eq!(source_words("found", std::path::Path::new("/games/nfsu2")), "found on this computer");
    assert_eq!(source_words("kept", p), "used last time");
    assert_eq!(source_words("NFSU2_ROOT", p), "set by NFSU2_ROOT");
}

/// The about screen names where the source is (`OB_SOURCE_URL`), or says
/// plainly that this build has no source link (the placeholder and an
/// empty value count as none; the release check fails on it).
#[test]
fn about_names_the_source_or_says_unofficial() {
    use super::model::source_of;
    assert_eq!(source_of(""), None);
    assert_eq!(source_of("SOURCE_URL"), None);
    assert_eq!(source_of("https://"), None);
    assert_eq!(source_of(" https://example.org/overbright \n").as_deref(), Some("https://example.org/overbright"));
    let fonts = Fonts::new();
    let env = FakeEnv;
    let texts = |a: &App| -> String {
        a.ui.widgets.iter().flat_map(|w| w.texts.iter().map(|t| t.text.clone())).collect::<Vec<_>>().join(" ")
    };
    let a = app(&env, &fonts, Screen::About, 1280, 720, 1.0);
    assert!(texts(&a).contains("Source code: https://example.org/overbright"), "{}", texts(&a));
    let mut m = env.load();
    m.source = None;
    let a = App::new(&env, m, Screen::About, (1280, 720), 1.0, &fonts);
    let t = texts(&a);
    assert!(t.contains("unofficial build without a source link") && !t.contains("Source code:"), "{t}");
}

/// Portable mode on the about screen: the folder where things are kept,
/// Make portable (outside portable mode), what to copy after it, and
/// "Portable" in the header of every screen once on.
#[test]
fn about_shows_and_turns_on_portable_mode() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let texts = |a: &App| -> String {
        a.ui.widgets.iter().flat_map(|w| w.texts.iter().map(|t| t.text.clone())).collect::<Vec<_>>().join(" ")
    };
    let mut a = app(&env, &fonts, Screen::About, 1280, 720, 1.0);
    let t = texts(&a);
    assert!(t.contains("/data/player/.local/share/overbright") && !t.contains("· Portable"), "{t}");
    a.st.focus = Some("portable");
    key(&mut a, &fonts, Key::Enter);
    let (mark, text) = a.model.message.clone().expect("a message after Make portable");
    assert_eq!(mark, Mark::Ok, "{text}");
    assert!(
        text.contains("/data/player/.local/share/overbright")
            && text.contains("/data/player/.local/share/NFS Underground 2"),
        "what to copy: {text}"
    );
    let (_, m) = super::headless::state_model("portable").unwrap();
    for s in [Screen::About, Screen::Main] {
        let a = App::new(&env, m.clone(), s, (1280, 720), 1.0, &fonts);
        let t = texts(&a);
        assert!(t.contains("· Portable"), "{s:?}: {t}");
        if s == Screen::About {
            assert!(t.contains("/media/player/SSD/Overbright/data") && a.ui.get("portable").is_none(), "{t}");
        }
    }
}

/// The display sizes and scalings players have (laptop 768p, 720p, 1080p,
/// 4K at 100 / 150 / 200 %): every screen and every named state laid out
/// clean. A page fits without scrolling whenever the window is at least
/// the launcher's smallest (480 logical pixels high); smaller (768p at
/// 200 %) it may scroll, with nothing clipped or overlapping.
#[test]
fn every_state_fits_every_size_and_scale() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let sizes = [(1280, 720), (1366, 768), (1920, 1080), (3840, 2160)];
    // A screen with the fake data, or a named state.
    let names: Vec<(&str, Option<Screen>)> = Screen::ALL
        .iter()
        .map(|s| (s.name(), Some(*s)))
        .chain(super::headless::STATES.iter().map(|(n, _)| (*n, None)))
        .collect();
    for (name, screen) in &names {
        for (w, h) in sizes {
            for dpi in [1.0, 1.5, 2.0] {
                let a = match screen {
                    Some(s) => app(&env, &fonts, *s, w, h, dpi),
                    None => super::headless::state_app(&env, &fonts, name, w, h, dpi).unwrap(),
                };
                let what = format!("{name} {w}x{h} at {}%", dpi * 100.0);
                let mut ui = a.ui.clone();
                let may_scroll = a.screen.document() || (h as f32 / dpi) < 480.0;
                if may_scroll {
                    ui.view.h = ui.content_h.max(ui.view.h);
                }
                check_layout(&ui, &fonts, &what);
                assert!(a.ui.metrics.unwrap().body() >= 14.0 * dpi.min(1.5), "{what}: body text too small");
                if a.screen == Screen::Main && !may_scroll {
                    let r = a.ui.get("play").unwrap().rect;
                    assert!(r.bottom() <= h as f32 && a.st.scroll == 0.0, "{what}: Play at {r:?}");
                }
            }
        }
    }
}

/// A screen's texts, joined with spaces (wrapped lines rejoined).
fn all_text(a: &App) -> String {
    a.ui.widgets.iter().flat_map(|w| w.texts.iter().map(|t| t.text.clone())).collect::<Vec<_>>().join(" ")
}

/// A refused folder says why in plain words and what to do, and its main
/// button opens the folder browser next to it ("Use this folder" stays
/// off).
#[test]
fn a_refused_folder_offers_another() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    for (state, words) in [
        ("wrong-version", "not the version 1.2 executable whose game data Overbright reads"),
        ("protected-exe", "copy-protected retail executable"),
        ("not-a-game-folder", "the game's files are not here"),
        ("files-missing", "some of the game's files are missing"),
    ] {
        let mut a = super::headless::state_app(&env, &fonts, state, 1280, 720, 1.0).unwrap();
        let t = all_text(&a);
        assert!(t.contains(words), "{state}: {t}");
        assert!(!a.ui.get("use").unwrap().enabled, "{state}: Use this folder is on");
        assert_eq!(focus(&a), "another", "{state}: the default focus");
        let parent = a.model.check.as_ref().unwrap().root.parent().unwrap().to_path_buf();
        key(&mut a, &fonts, Key::Enter);
        assert_eq!(a.screen, Screen::Browse, "{state}");
        assert_eq!(a.model.browse.dir, parent, "{state}: the browser opens beside the refused folder");
        key(&mut a, &fonts, Key::Escape);
        assert_eq!(a.screen, Screen::Check, "{state}: Cancel goes back to the check");
    }
    let a = super::headless::state_app(&env, &fonts, "check-ok", 1280, 720, 1.0).unwrap();
    assert!(a.ui.get("another").is_none() && a.ui.get("use").unwrap().enabled);
    assert_eq!(focus(&a), "use");
}

/// The copy-protected retail SPEED2.EXE: the check screen names it, says
/// Overbright cannot read it yet, links the issue where the work is
/// followed, and suggests no other executable; drawn whole at every size.
#[test]
fn the_protected_executable_is_named_with_its_issue() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    for (w, h) in SIZES {
        let a = super::headless::state_app(&env, &fonts, "protected-exe", w, h, 1.0).unwrap();
        let what = format!("protected-exe {w}x{h}");
        let mut ui = a.ui.clone();
        ui.view.h = ui.content_h.max(ui.view.h);
        check_layout(&ui, &fonts, &what);
        let t = all_text(&a);
        for words in [
            "This SPEED2.EXE is the copy-protected retail executable (SafeDisc)",
            "cannot read its game data yet",
            "being worked on",
            "github.com/j0braun/overbright/issues/16",
            "does not remove copy protection",
        ] {
            assert!(t.contains(words), "{what}: {words:?} not in {t}");
        }
        let lower = t.to_lowercase();
        for banned in ["no-cd", "nocd", "no cd", "crack", "patch"] {
            assert!(!lower.contains(banned), "{what}: {banned:?} in {t}");
        }
        assert!(!a.ui.get("use").unwrap().enabled, "{what}: Use this folder is on");
    }
}

/// No Vulkan driver: the main screen says so in plain words with the
/// command that installs one (the probe's line, `platform::vulkan`), not
/// wgpu's message and not the "set them in the game's Options" hint (the
/// game cannot start until a driver is there).
#[test]
fn no_vulkan_driver_says_what_to_install() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    for (w, h) in [(960, 720), (1280, 720)] {
        let a = super::headless::state_app(&env, &fonts, "no-vulkan", w, h, 1.0).unwrap();
        let t = all_text(&a);
        for words in ["no Vulkan driver for this graphics card was found", "sudo apt install libvulkan1", "NVIDIA"] {
            assert!(t.contains(words), "{w}x{h}: {words:?} not in {t}");
        }
        assert!(!t.contains("in the game's Options") && !t.contains("noop"), "{w}x{h}: {t}");
    }
}

/// The main screen: the game folder in plain text before its Change
/// button (both ends of a long path kept), About and Import saves at the
/// left, Quit and Play at the right; Play is the widest of them.
#[test]
fn main_screen_hierarchy() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    for (w, h) in [(960, 720), (1280, 720), (1920, 1080)] {
        let a = super::headless::state_app(&env, &fonts, "long-values", w, h, 1.0).unwrap();
        let what = format!("{w}x{h}");
        let change = a.ui.get("change").unwrap().rect;
        let path =
            a.ui.widgets
                .iter()
                .flat_map(|w| &w.texts)
                .find(|t| t.text.starts_with("/data/player/") && t.text.ends_with("Deutsche Version"))
                .unwrap_or_else(|| panic!("{what}: the game path is not shown with both ends"));
        assert!(path.rect.right() <= change.x && (path.rect.cy() - change.cy()).abs() < change.h / 2.0, "{what}");
        assert_eq!(path.color, super::draw::theme::TEXT, "{what}: the path is plain text, not dim");
        let r = |id| a.ui.get(id).unwrap().rect;
        assert!(r("about").right() < r("quit").x && r("import").right() < r("quit").x, "{what}");
        // Play: the hero's, the largest control, beside the game's state.
        let play = r("play");
        let buttons = a.ui.widgets.iter().filter(|w| matches!(w.kind, Kind::Button { .. }) && w.id != Some("play"));
        assert!(buttons.clone().count() >= 4, "{what}");
        assert!(
            buttons.into_iter().all(|w| w.rect.w * w.rect.h < play.w * play.h),
            "{what}: Play is the largest button"
        );
        assert!(play.y < r("display").y && play.x > change.x, "{what}: Play in the hero, right of the folder");
        assert!(all_text(&a).contains("ready to play"), "{what}: the game's state beside Play");
    }
}

/// The window's first size fits the monitor (a task bar and a title bar
/// beside it) and is 960 × 720 where that fits.
#[test]
fn first_window_fits_the_monitor() {
    use super::window::initial_size;
    assert_eq!(initial_size(1920, 1080, 1.0), (960.0, 720.0));
    assert_eq!(initial_size(3840, 2160, 2.0), (960.0, 720.0));
    assert_eq!(initial_size(1366, 768, 1.0), (960.0, 652.0), "768p laptop: under the task bar");
    let (w, h) = initial_size(1366, 768, 2.0);
    assert!(w <= 683.0 && h <= 384.0, "768p at 200 %: {w} x {h} (never over the screen)");
    assert_eq!(initial_size(0, 0, 1.0), (960.0, 720.0), "no monitor size");
}

/// The theme's contrast (WCAG 2 ratios): body and dim text, the empty
/// field's hint and the status colours readable on the page and on a
/// control; controls' outlines at least 3:1 against the page.
#[test]
fn theme_contrast() {
    use super::draw::{Rgb, theme};
    fn lum(c: Rgb) -> f64 {
        let f = |v: u8| {
            let x = f64::from(v) / 255.0;
            if x <= 0.039_28 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.0) + 0.7152 * f(c.1) + 0.0722 * f(c.2)
    }
    let ratio = |a: Rgb, b: Rgb| {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    };
    for (name, c) in [
        ("text", theme::TEXT),
        ("dim", theme::DIM),
        ("placeholder", theme::PLACEHOLDER),
        ("ok", theme::OK),
        ("warn", theme::WARN),
        ("info", theme::INFO),
        ("error", theme::ERROR),
        ("accent", theme::ACCENT),
    ] {
        for (bg_name, bg) in [("page", theme::BG), ("control", theme::SURFACE)] {
            assert!(ratio(c, bg) >= 4.5, "{name} on {bg_name}: {:.2}", ratio(c, bg));
        }
    }
    assert!(ratio(theme::ON_ACCENT, theme::ACCENT) >= 4.5, "the main button's text");
    assert!(ratio(theme::BORDER, theme::BG) >= 3.0, "a control's outline: {:.2}", ratio(theme::BORDER, theme::BG));
    assert!(ratio(theme::DISABLED, theme::SURFACE) >= 3.0, "disabled text");
}

/// The Mods page: the list's order and on / off kept through the
/// environment, a removal only after its confirmation, packages added
/// from the browser (a file, or every package in a folder) with a result
/// line each, the HUD mod's folder and switch.
#[test]
fn mods_page_flow() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 1280, 720, 1.0);
    a.st.focus = Some("mods");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!((a.screen, focus(&a)), (Screen::Mods, "pkgs"));
    let ids = |a: &App| a.model.mods.list.iter().map(|(e, _)| (e.id.clone(), e.on)).collect::<Vec<_>>();
    assert_eq!(ids(&a), [("roads-hd".to_string(), true), ("garage-redux".to_string(), false)]);
    // Enter on the list switches the chosen package.
    key(&mut a, &fonts, Key::Enter);
    assert!(!a.model.mods.list[0].0.on);
    a.st.focus = Some("down");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(ids(&a)[1].0, "roads-hd", "moved down, still chosen");
    assert_eq!(a.model.mods.sel, 1);
    assert!(!a.ui.get("down").unwrap().enabled, "the last cannot move down");
    // Remove asks first; Escape keeps it.
    a.st.focus = Some("remove");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.mods.confirm.as_deref(), Some("roads-hd"));
    assert_eq!(focus(&a), "keep");
    key(&mut a, &fonts, Key::Escape);
    assert!(a.model.mods.confirm.is_none() && a.screen == Screen::Mods);
    a.st.focus = Some("remove");
    key(&mut a, &fonts, Key::Enter);
    a.st.focus = Some("remove_yes");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.mods.results[0].1.contains("removed (your package file is kept)"));
    assert!(!ids(&a).iter().any(|(id, _)| id == "roads-hd"));
    // Add: the browser lists packages with the folders; one imports.
    a.st.focus = Some("add");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!((a.screen, a.model.browse.purpose), (Screen::Browse, super::model::BrowseFor::Packages));
    a.st.focus = Some("browse_path");
    a.input(&Input::Text("/data/player/Downloads".into()), &fonts);
    key(&mut a, &fonts, Key::Enter);
    let row = a.model.browse.entries.iter().position(|e| e.name == "roads-hd.tpf").unwrap() + 1;
    a.st.focus = Some("dirs");
    a.model.browse.sel = row;
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Mods);
    assert!(a.model.mods.results[0].1.starts_with("roads-hd.tpf: 412 of 430 textures"), "{:?}", a.model.mods.results);
    // "Import all here": every package of the folder, one line each.
    a.st.focus = Some("add");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player/Downloads"), "the browser opens where it was");
    a.st.focus = Some("use");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.mods.results.len(), 2);
    // The HUD mod: the switch is the game's HD UI + Maps.
    a.st.focus = Some("hud");
    key(&mut a, &fonts, Key::Space);
    assert!(a.model.choices.hud_mod);
    let mut s = crate::frontend::options::Settings::default();
    super::model::apply_choices(&mut s, &a.model.choices);
    assert!(s.enh.hd_ui);
    key(&mut a, &fonts, Key::Escape);
    assert_eq!(a.screen, Screen::Main);
    assert_eq!(super::screens::mods_summary(&a.model), "1 texture package on");
}

/// A 200-character game folder: drawn with its middle left out (both ends
/// kept), while the screen trace (what the start tests and assistive
/// output read) carries the whole path. The launcher has no tooltips.
#[test]
fn a_long_path_is_drawn_short_and_traced_whole() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut path = String::from("/data/player/Games");
    while path.len() < 170 {
        path += "/a-rather-long-folder";
    }
    path += "/Need for Speed Underground 2";
    path.truncate(200);
    assert_eq!(path.len(), 200);
    for (w, h) in [(640, 480), (1280, 720), (1920, 1080)] {
        let mut m = env.load();
        m.root = Some(std::path::PathBuf::from(&path));
        let a = App::new(&env, m, Screen::Main, (w, h), 1.0, &fonts);
        let shown = a
            .ui
            .widgets
            .iter()
            .flat_map(|w| w.texts.iter())
            .find(|t| t.full.as_deref() == Some(path.as_str()))
            .unwrap_or_else(|| panic!("{w}x{h}: the path is not on the main screen with its whole value"));
        assert!(shown.text.len() < path.len() && shown.text.contains('…'), "{w}x{h}: {}", shown.text);
        let (head, tail) = shown.text.split_once('…').unwrap();
        assert!(path.starts_with(head) && path.ends_with(tail) && !head.is_empty() && !tail.is_empty());
        let traced: Vec<String> = a.ui.widgets.iter().flat_map(super::window::trace_texts).collect();
        assert!(traced.iter().any(|t| t.contains(&path) && t.starts_with(&shown.text)), "{w}x{h}: {traced:?}");
    }
}

/// The HUD mod from its downloads: the browser lists the archives with the
/// folders; a 7-Zip file says to extract it; the map's .zip is one part
/// (the line asks for the other); the HUD's extracted folder is the second,
/// the conversion runs, its line shows, the converted folder is checked
/// and the switch is focused.
#[test]
fn hud_mod_from_its_downloads() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = super::headless::state_app(&env, &fonts, "mods-empty", 1280, 720, 1.0).unwrap();
    let open_browser = |a: &mut App| {
        a.st.focus = Some("hud_add");
        key(a, &fonts, Key::Enter);
        assert_eq!((a.screen, a.model.browse.purpose), (Screen::Browse, super::model::BrowseFor::HudMod));
    };
    open_browser(&mut a);
    a.st.focus = Some("browse_path");
    a.input(&Input::Text("/data/player/mods".into()), &fonts);
    key(&mut a, &fonts, Key::Enter);
    let row = |a: &App, name: &str| a.model.browse.entries.iter().position(|e| e.name == name).unwrap() + 1;
    a.model.browse.sel = row(&a, "hud-part.7z");
    a.st.focus = Some("dirs");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Mods);
    assert_eq!(a.model.mods.hud_results[0].0, Mark::Error);
    assert!(a.model.mods.hud_results[0].1.contains("extract it first"), "{:?}", a.model.mods.hud_results);
    open_browser(&mut a);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player/mods"), "the browser opens where it was");
    a.model.browse.sel = row(&a, "map-part.zip");
    a.st.focus = Some("dirs");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.mods.hud_results.len(), 1);
    assert!(a.model.mods.hud_results[0].1.contains("Now add the HUD part"), "{:?}", a.model.mods.hud_results);
    assert_eq!(focus(&a), "hud_add");
    open_browser(&mut a);
    a.st.focus = Some("browse_path");
    a.input(&Input::Text("/data/player/mods/hud-part".into()), &fonts);
    key(&mut a, &fonts, Key::Enter);
    a.st.focus = Some("use");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Mods);
    assert_eq!(a.model.mods.hud_results[0].0, Mark::Ok);
    assert!(a.model.mods.hud_results[0].1.starts_with("HUD mod set up"));
    assert!(a.model.choices.texture_pack.ends_with("hud-mod"));
    assert!(a.model.hud.as_ref().is_some_and(|h| h.complete));
    assert!(a.model.mods.hud_parts == Default::default(), "the parts are cleared for the next time");
    assert_eq!(focus(&a), "hud");
    key(&mut a, &fonts, Key::Space);
    assert!(a.model.choices.hud_mod);
    let t = all_text(&a);
    assert!(t.contains("Complete: 710 of 710 files"), "{t}");
}

/// A fresh install: no packages, the sentence on what can be added, and
/// "Add package…" focused.
#[test]
fn mods_page_when_empty() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let a = super::headless::state_app(&env, &fonts, "mods-empty", 1280, 720, 1.0).unwrap();
    assert_eq!(focus(&a), "add");
    let t = all_text(&a);
    assert!(t.contains("Overbright ships no texture art") && t.contains("None added"), "{t}");
    assert!(!a.ui.get("toggle").unwrap().enabled && !a.ui.get("remove").unwrap().enabled);
    assert_eq!(super::screens::mods_summary(&a.model), "None: the game's own textures");
}

/// Text drawn on the page itself (over the backdrop, not on a panel or a
/// control) keeps 4.5:1 against the brightest the backdrop can get (a
/// white picture at full strength and the glow's peak), on every screen
/// and state; text on a panel is the theme's (`theme_contrast`).
#[test]
fn backdrop_keeps_text_readable() {
    use super::draw::Rgb;
    fn lum(c: Rgb) -> f64 {
        let f = |v: u8| {
            let x = f64::from(v) / 255.0;
            if x <= 0.039_28 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.0) + 0.7152 * f(c.1) + 0.0722 * f(c.2)
    }
    let ratio = |a: Rgb, b: Rgb| {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    };
    let worst = super::backdrop::brightest();
    let fonts = Fonts::new();
    let env = FakeEnv;
    let names: Vec<&str> =
        Screen::ALL.iter().map(|s| s.name()).chain(super::headless::STATES.iter().map(|(n, _)| *n)).collect();
    for name in names {
        for (w, h) in [(1280, 720), (3840, 2160)] {
            let a = super::headless::named_app(&env, &fonts, name, w, h, 1.0).unwrap();
            let holders: Vec<Rect> =
                a.ui.widgets
                    .iter()
                    .filter(|w| {
                        !matches!(
                            w.kind,
                            Kind::Label
                                | Kind::Status { .. }
                                | Kind::Checkbox { .. }
                                | Kind::Rule
                                | Kind::Accent
                                | Kind::Logo
                        )
                    })
                    .map(|w| w.rect)
                    .collect();
            for wd in &a.ui.widgets {
                for t in &wd.texts {
                    if holders.iter().any(|r| t.rect.inside(r)) {
                        continue;
                    }
                    let r = ratio(t.color, worst);
                    assert!(r >= 4.5, "{name} {w}x{h}: {:?} at {r:.2}:1 over the backdrop ({worst:?})", t.text);
                }
            }
        }
    }
    // The picture fades in over Overbright's own page.
    let pic = super::backdrop::Picture { w: 2, h: 2, rgba: vec![200; 16] };
    let plain = super::backdrop::compose(64, 48, None, 0.0);
    assert_eq!(plain, super::backdrop::compose(64, 48, Some(&pic), 0.0), "faded out: the fallback");
    assert_ne!(plain, super::backdrop::compose(64, 48, Some(&pic), 1.0));
}

/// Pad and keyboard alone: on every screen and state every control is
/// reached with the d-pad / arrow keys from the default focus (LB / RB and
/// Tab reach them in order too).
#[test]
fn every_control_is_reached_with_the_arrows() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let names: Vec<&str> =
        Screen::ALL.iter().map(|s| s.name()).chain(super::headless::STATES.iter().map(|(n, _)| *n)).collect();
    for name in names {
        for (w, h) in [(1280, 720), (640, 480)] {
            let a = super::headless::named_app(&env, &fonts, name, w, h, 1.0).unwrap();
            let all = a.ui.focus_order();
            let Some(start) = a.ui.focused(&a.st) else { continue };
            let mut seen = vec![start];
            let mut i = 0;
            while i < seen.len() {
                for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
                    if let Some(n) = a.ui.neighbour(seen[i], dx, dy)
                        && !seen.contains(&n)
                    {
                        seen.push(n);
                    }
                }
                i += 1;
            }
            let missing: Vec<_> = all.iter().filter(|id| !seen.contains(id)).collect();
            assert!(missing.is_empty(), "{name} {w}x{h}: not reached with the arrows: {missing:?}");
        }
    }
}

// ---- The mouse alone ---------------------------------------------------------------------------

/// A widget's box on the window (the page's scroll taken off).
fn on_window(a: &App, id: &str) -> Rect {
    let w = a.ui.widgets.iter().find(|w| w.id == Some(id)).unwrap_or_else(|| panic!("no {id} on {:?}", a.screen));
    w.rect.offset(-a.st.scroll)
}

/// A click (move, press, release) at (x, y); the release's outcome.
fn click_at(a: &mut App, fonts: &Fonts, x: f32, y: f32) -> Outcome {
    a.input(&Input::MouseMove(x, y), fonts);
    a.input(&Input::MouseDown(x, y), fonts);
    a.input(&Input::MouseUp(x, y), fonts)
}

/// A click in the middle of control `id`.
fn click(a: &mut App, fonts: &Fonts, id: &str) -> Outcome {
    let r = on_window(a, id);
    click_at(a, fonts, r.cx(), r.cy())
}

/// The link (a place or a part of the path) showing `text`.
fn link(a: &App, text: &str) -> &'static str {
    a.ui.widgets
        .iter()
        .find(|w| matches!(w.kind, Kind::Link) && w.texts[0].text == text)
        .and_then(|w| w.id)
        .unwrap_or_else(|| panic!("no link {text:?}"))
}

/// Where list `id` shows the row named `name` (None: not in view).
fn row_at(a: &App, id: &'static str, name: &str) -> Option<(f32, f32)> {
    let w = a.ui.get(id)?;
    let Kind::List { row_h, pad, .. } = w.kind else { return None };
    let names: Vec<&str> =
        w.texts.iter().filter(|t| t.color != super::draw::theme::DIM).map(|t| t.text.as_str()).collect();
    let k = names.iter().position(|n| *n == name)?;
    let r = w.rect.offset(-a.st.scroll);
    Some((r.x + r.w / 3.0, r.y + pad + (k as f32 + 0.5) * row_h))
}

/// Wheels list `id` down (the pointer over it) until `name` shows, then
/// clicks it (twice: a double click).
fn row_click(a: &mut App, fonts: &Fonts, id: &'static str, name: &str, double: bool) -> Outcome {
    for _ in 0..50 {
        if row_at(a, id, name).is_some() {
            break;
        }
        let r = on_window(a, id);
        a.input(&Input::MouseMove(r.cx(), r.cy()), fonts);
        a.input(&Input::Wheel(-1.0), fonts);
    }
    let (x, y) = row_at(a, id, name).unwrap_or_else(|| panic!("{name} never in view"));
    let o = click_at(a, fonts, x, y);
    if double { click_at(a, fonts, x, y) } else { o }
}

fn list_state(a: &App, id: &'static str) -> (usize, usize, usize) {
    match a.ui.get(id).unwrap().kind {
        Kind::List { first, selected, len, .. } => (first, selected, len),
        _ => panic!(),
    }
}

/// The browser with `n` folders in it (more than it shows).
fn crowded_browser<'e>(env: &'e FakeEnv, fonts: &Fonts, n: usize, w: u32, h: u32, dpi: f32) -> App<'e> {
    let mut a = app(env, fonts, Screen::FirstRun, w, h, dpi);
    click(&mut a, fonts, "browse");
    assert_eq!(a.screen, Screen::Browse);
    a.model.browse.entries = (1..=n)
        .map(|i| super::model::DirEntry {
            name: format!("Folder {i:02}"),
            path: std::path::PathBuf::from(format!("/data/player/Games/Folder {i:02}")),
            note: String::new(),
        })
        .collect();
    a.rebuild(fonts);
    a
}

/// From the first start to Play with nothing but clicks: Browse…, the
/// game folder found by a double click, Use this folder, Use this folder,
/// a cycle's arrow, the start checkbox, Play.
#[test]
fn mouse_only_from_the_first_start_to_play() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::FirstRun, 960, 720, 1.0);
    click(&mut a, &fonts, "browse");
    assert_eq!((a.screen, a.model.browse.dir.as_path()), (Screen::Browse, std::path::Path::new("/data/player/Games")));
    // One click on the row chosen when the browser opened ("..") only
    // chooses it; a second goes up.
    let (x, y) = row_at(&a, "dirs", "..").unwrap();
    click_at(&mut a, &fonts, x, y);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player/Games"), "one click on .. went up");
    click_at(&mut a, &fonts, x, y);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player"));
    row_click(&mut a, &fonts, "dirs", "Games", true);
    // One click on another row chooses it, without opening.
    row_click(&mut a, &fonts, "dirs", "Need for Speed Underground 2", false);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player/Games"));
    row_click(&mut a, &fonts, "dirs", "Need for Speed Underground 2", false);
    assert_eq!(a.model.browse.dir, FakeEnv::root(), "a second click on the chosen row opens it");
    click(&mut a, &fonts, "use");
    assert_eq!(a.screen, Screen::Check);
    click(&mut a, &fonts, "use");
    assert_eq!(a.screen, Screen::Main);
    let fs = a.model.choices.fullscreen;
    let d = on_window(&a, "display");
    click_at(&mut a, &fonts, d.right() - 4.0, d.cy());
    assert_ne!(a.model.choices.fullscreen, fs, "the cycle's right arrow");
    // The checkbox: on release inside; a press dragged off does nothing.
    let skip = a.model.choices.skip;
    let s = on_window(&a, "skip");
    a.input(&Input::MouseDown(s.x + 4.0, s.cy()), &fonts);
    assert_eq!(a.model.choices.skip, skip, "not on the press");
    a.input(&Input::MouseUp(1.0, 1.0), &fonts);
    assert_eq!(a.model.choices.skip, skip, "released outside");
    click_at(&mut a, &fonts, s.x + 4.0, s.cy());
    assert_ne!(a.model.choices.skip, skip);
    assert_eq!(click(&mut a, &fonts, "play"), Outcome::Play);
}

/// Mods with the mouse: a package removed (the confirmation by click),
/// then one added from the Downloads place by a double click.
#[test]
fn mouse_only_mods_add_and_remove() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 1280, 720, 1.0);
    click(&mut a, &fonts, "mods");
    assert_eq!(a.screen, Screen::Mods);
    let ids = |a: &App| a.model.mods.list.iter().map(|(e, _)| e.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&a)[0], "roads-hd");
    click(&mut a, &fonts, "remove");
    assert_eq!(a.model.mods.confirm.as_deref(), Some("roads-hd"));
    click(&mut a, &fonts, "keep");
    assert!(a.model.mods.confirm.is_none());
    click(&mut a, &fonts, "remove");
    click(&mut a, &fonts, "remove_yes");
    assert!(!ids(&a).contains(&"roads-hd".to_string()));
    click(&mut a, &fonts, "add");
    assert_eq!((a.screen, a.model.browse.purpose), (Screen::Browse, super::model::BrowseFor::Packages));
    let dl = link(&a, "Downloads");
    click(&mut a, &fonts, dl);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player/Downloads"));
    row_click(&mut a, &fonts, "dirs", "roads-hd.tpf", true);
    assert_eq!(a.screen, Screen::Mods);
    assert!(a.model.mods.results[0].1.starts_with("roads-hd.tpf: 412 of 430 textures"), "{:?}", a.model.mods.results);
    click(&mut a, &fonts, "back");
    assert_eq!(a.screen, Screen::Main);
}

/// The wheel scrolls the list under the pointer: 3 rows a notch, a
/// touchpad's pixels adding up to whole rows (nothing lost to rounding);
/// the selection stays where it was until a key moves it.
#[test]
fn the_wheel_scrolls_the_list_under_the_pointer() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    for dpi in [1.0, 1.25, 1.5, 2.0] {
        let (w, h) = ((960.0 * dpi) as u32, (720.0 * dpi) as u32);
        let mut a = crowded_browser(&env, &fonts, 40, w, h, dpi);
        let l = on_window(&a, "dirs");
        let Kind::List { row_h, .. } = a.ui.get("dirs").unwrap().kind else { panic!() };
        assert_eq!(list_state(&a, "dirs"), (0, 0, 41));
        a.input(&Input::MouseMove(l.cx(), l.cy()), &fonts);
        a.input(&Input::Wheel(-1.0), &fonts);
        assert_eq!(list_state(&a, "dirs").0, 3, "{dpi}: a notch is 3 rows");
        // Pixel deltas (window pixels, as winit gives them): a third of a
        // row at a time.
        let step = row_h / 3.0 + 0.01;
        a.input(&Input::WheelPx(-step), &fonts);
        a.input(&Input::WheelPx(-step), &fonts);
        assert_eq!(list_state(&a, "dirs").0, 3, "{dpi}: two thirds of a row: not yet");
        a.input(&Input::WheelPx(-step), &fonts);
        assert_eq!(list_state(&a, "dirs").0, 4, "{dpi}: a whole row");
        a.input(&Input::WheelPx(row_h * 2.0), &fonts);
        assert_eq!(list_state(&a, "dirs").0, 2, "{dpi}: up again");
        for _ in 0..30 {
            a.input(&Input::Wheel(-1.0), &fonts);
        }
        let shown = super::ui::list_rows(a.ui.get("dirs").unwrap());
        assert_eq!(list_state(&a, "dirs"), (41 - shown, 0, 41), "{dpi}: at the end, the selection kept");
        assert_eq!(a.st.scroll, 0.0, "the page did not move");
        // A key brings the selection back into view.
        key(&mut a, &fonts, Key::Down);
        assert_eq!(list_state(&a, "dirs"), (1, 1, 41));
        // Outside the list the wheel scrolls the page (it fits: nothing).
        a.input(&Input::MouseMove(2.0, 2.0), &fonts);
        a.input(&Input::Wheel(-3.0), &fonts);
        assert_eq!((list_state(&a, "dirs").0, a.st.scroll), (1, 0.0));
    }
}

/// The list's scroll bar: the thumb dragged, the track clicked; the row
/// under the pointer shown; the chosen row kept.
#[test]
fn the_list_scroll_bar_and_hover() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = crowded_browser(&env, &fonts, 40, 1280, 720, 1.0);
    let u = a.ui.metrics.unwrap().u;
    let w = a.ui.get("dirs").unwrap().clone();
    let (track, thumb) = super::ui::list_bar(&w, u).expect("a scroll bar");
    assert!(track.inside(&w.rect));
    // Drag the thumb to the bottom.
    a.input(&Input::MouseMove(thumb.cx(), thumb.cy()), &fonts);
    a.input(&Input::MouseDown(thumb.cx(), thumb.cy()), &fonts);
    a.input(&Input::MouseMove(thumb.cx(), track.bottom() + 50.0), &fonts);
    a.input(&Input::MouseUp(thumb.cx(), track.bottom() + 50.0), &fonts);
    let shown = super::ui::list_rows(a.ui.get("dirs").unwrap());
    assert_eq!(list_state(&a, "dirs"), (41 - shown, 0, 41));
    // A click on the track above the thumb: a page up.
    let (track, thumb) = super::ui::list_bar(a.ui.get("dirs").unwrap(), u).unwrap();
    click_at(&mut a, &fonts, track.cx(), track.y + 2.0);
    assert_eq!(list_state(&a, "dirs").0, 41 - 2 * shown);
    assert!(thumb.y > track.y, "the thumb was not at the top");
    // Hover: the row under the pointer.
    let (x, y) = row_at(&a, "dirs", "Folder 20").unwrap();
    a.input(&Input::MouseMove(x, y), &fonts);
    assert_eq!(a.st.hover_row, Some(("dirs", 20)));
    assert_eq!(a.ui.pointer_at(x, y, &a.st), super::ui::Pointer::Hand);
    // Chosen, then the pointer leaves: still chosen.
    click_at(&mut a, &fonts, x, y);
    a.input(&Input::MouseMove(1.0, 1.0), &fonts);
    assert_eq!((list_state(&a, "dirs").1, a.st.hover_row), (20, None));
    // Open does what a double click does.
    click(&mut a, &fonts, "open");
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player/Games/Folder 20"));
}

/// The path's parts, the places, the Up button and the mouse's back button.
#[test]
fn path_parts_places_and_back_button() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::FirstRun, 1280, 720, 1.0);
    click(&mut a, &fonts, "browse");
    let data = link(&a, "data");
    click(&mut a, &fonts, data);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data"));
    let home = link(&a, "Home");
    click(&mut a, &fonts, home);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player"));
    let game = link(&a, "Game folder");
    click(&mut a, &fonts, game);
    assert_eq!(a.model.browse.dir, FakeEnv::root());
    a.input(&Input::MouseBack, &fonts);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player/Games"));
    let (_, sel, _) = list_state(&a, "dirs");
    assert_eq!(a.model.browse.entries[sel - 1].path, FakeEnv::root(), "the folder come from stays chosen");
    click(&mut a, &fonts, "up");
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player"));
    // A deep folder: its last parts, "…" to the one before them.
    a.model.browse.input = "/a/b/c/d/e/f/g/h/i/j/k".into();
    a.st.focus = Some("browse_path");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/a/b/c/d/e/f/g/h/i/j/k"));
    let more = link(&a, "…");
    assert!(a.ui.widgets.iter().all(|w| w.texts.first().is_none_or(|t| t.text != "a")), "/a is left out");
    click(&mut a, &fonts, more);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/a/b"));
    // Elsewhere the back button is Back.
    let mut b = app(&env, &fonts, Screen::About, 1280, 720, 1.0);
    b.input(&Input::MouseBack, &fonts);
    assert_eq!(b.screen, Screen::Main);
}

#[test]
fn path_parts_on_each_system() {
    use super::model::path_parts;
    let p = |v: &[(&str, &str)]| v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>();
    assert_eq!(path_parts("/data/player", false), p(&[("/", "/"), ("data", "/data"), ("player", "/data/player")]));
    assert_eq!(path_parts("/", false), p(&[("/", "/")]));
    assert_eq!(path_parts("", true), p(&[("This PC", "")]));
    assert_eq!(path_parts(r"C:\", true), p(&[("This PC", ""), (r"C:\", r"C:\")]));
    assert_eq!(
        path_parts(r"D:\Games\Need for Speed Underground 2", true),
        p(&[
            ("This PC", ""),
            (r"D:\", r"D:\"),
            ("Games", r"D:\Games"),
            ("Need for Speed Underground 2", r"D:\Games\Need for Speed Underground 2")
        ])
    );
    assert_eq!(path_parts("C:/Games/x", true)[3], ("x".to_string(), r"C:\Games\x".to_string()));
    assert_eq!(
        path_parts(r"\\nas\games\Racing", true),
        p(&[("This PC", ""), (r"\\nas\games\", r"\\nas\games\"), ("Racing", r"\\nas\games\Racing")])
    );
    assert_eq!(
        path_parts(r"\\?\C:\very\long", true),
        p(&[("This PC", ""), (r"C:\", r"\\?\C:\"), ("very", r"\\?\C:\very"), ("long", r"\\?\C:\very\long")])
    );
    assert_eq!(
        path_parts(r"\\?\UNC\nas\games\x", true),
        p(&[("This PC", ""), (r"\\nas\games\", r"\\?\UNC\nas\games\"), ("x", r"\\?\UNC\nas\games\x")])
    );
    // Which form a path is split in: a Windows path on Windows only.
    use super::model::windows_form;
    assert!(windows_form("", true) && windows_form(r"C:\Games", true) && windows_form(r"\\nas\games", true));
    assert!(!windows_form("/data/player", true) && !windows_form(r"C:\Games", false) && !windows_form("", false));
    // A long path: every part kept whole (the drawing shortens the text).
    let long = format!(r"C:\{}", ["Ünterordner mit langem Namen"; 12].join(r"\"));
    let parts = path_parts(&long, true);
    assert_eq!(parts.len(), 14);
    assert_eq!(parts.last().unwrap().1, long);
}

/// A text field: the caret where it is clicked; a double click selects
/// the whole text, which typing replaces; the keys as before.
#[test]
fn text_field_by_mouse() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::FirstRun, 1280, 720, 1.0);
    click(&mut a, &fonts, "browse");
    click(&mut a, &fonts, "browse_path");
    a.input(&Input::Text("/data/abc".into()), &fonts);
    let Kind::TextField { stops, .. } = a.ui.get("browse_path").unwrap().kind.clone() else { panic!() };
    assert_eq!(stops.len(), 10);
    let r = on_window(&a, "browse_path");
    let x = stops[3] + 1.0;
    a.st.last_down = None;
    click_at(&mut a, &fonts, x, r.cy());
    assert_eq!(a.st.cursor, 3);
    assert_eq!(a.ui.pointer_at(x, r.cy(), &a.st), super::ui::Pointer::Text);
    a.input(&Input::Text("X".into()), &fonts);
    assert_eq!(a.model.browse.input, "/daXta/abc");
    // A double click selects all; typing replaces it.
    click_at(&mut a, &fonts, x, r.cy());
    click_at(&mut a, &fonts, x, r.cy());
    assert!(a.st.field_all);
    a.input(&Input::Text("/data".into()), &fonts);
    assert_eq!(a.model.browse.input, "/data");
    assert!(!a.st.field_all);
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data"));
}

/// Dropped files: a game folder on the first-run screen is checked; a
/// package and a HUD archive on the Mods page are added.
#[test]
fn dropped_files_do_what_choosing_them_does() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::FirstRun, 1280, 720, 1.0);
    a.input(&Input::Drop(vec![FakeEnv::root()]), &fonts);
    assert_eq!(a.screen, Screen::Check);
    assert_eq!(a.model.check.as_ref().unwrap().root, FakeEnv::root());
    let mut m = app(&env, &fonts, Screen::Mods, 1280, 720, 1.0);
    let dl = std::path::Path::new("/data/player/Downloads");
    m.input(&Input::Drop(vec![dl.join("roads-hd.tpf")]), &fonts);
    assert_eq!(m.screen, Screen::Mods);
    assert!(m.model.mods.results[0].1.starts_with("roads-hd.tpf"), "{:?}", m.model.mods.results);
    m.input(&Input::Drop(vec![dl.join("hud-part.zip")]), &fonts);
    assert!(!m.model.mods.hud_results.is_empty() || m.model.mods.hud_busy.is_some());
    // The pointer is a hand over buttons, an arrow over plain text.
    let p = on_window(&m, "back");
    assert_eq!(m.ui.pointer_at(p.cx(), p.cy(), &m.st), super::ui::Pointer::Hand);
    assert_eq!(m.ui.pointer_at(1.0, 1.0, &m.st), super::ui::Pointer::Default);
}

/// winit's wheel deltas as the launcher's input: notches as lines, a
/// touchpad's smooth scrolling as window pixels (winit gives those in
/// physical pixels, the layout's own: X11, Wayland at any scale, Windows
/// per-monitor DPI), so nothing is scaled again.
#[test]
fn wheel_deltas_from_winit() {
    use winit::dpi::PhysicalPosition;
    use winit::event::MouseScrollDelta;
    assert_eq!(super::window::wheel_input(MouseScrollDelta::LineDelta(0.0, -1.0)), Input::Wheel(-1.0));
    assert_eq!(
        super::window::wheel_input(MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 15.0))),
        Input::WheelPx(15.0)
    );
}

/// A long page (the licence): the page's scroll bar dragged and clicked.
#[test]
fn page_scroll_bar_by_mouse() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Licence, 1280, 720, 1.0);
    let (track, thumb) = a.ui.page_bar(&a.st).expect("the licence is longer than the window");
    a.input(&Input::MouseMove(thumb.cx(), thumb.cy()), &fonts);
    a.input(&Input::MouseDown(track.cx(), thumb.cy()), &fonts);
    a.input(&Input::MouseMove(track.cx(), track.bottom() + 10.0), &fonts);
    a.input(&Input::MouseUp(track.cx(), track.bottom() + 10.0), &fonts);
    let max = a.ui.content_h - a.ui.view.h;
    assert!((a.st.scroll - max).abs() < 1.0, "dragged to the end: {} of {max}", a.st.scroll);
    click_at(&mut a, &fonts, track.cx(), 2.0);
    assert!(a.st.scroll < max - 100.0, "a click above the thumb pages up");
    // The wheel scrolls it too.
    let before = a.st.scroll;
    a.input(&Input::MouseMove(640.0, 360.0), &fonts);
    a.input(&Input::Wheel(1.0), &fonts);
    assert!(a.st.scroll < before);
}

/// The prepare step (`game --prepare`): offered by itself once when the
/// hardware check says nothing is prepared for this build, adapter and
/// driver; its screen shows the bar and the step, Skip stops the run and
/// goes to the main screen, where it is not offered again in the session;
/// "Prepare again" there starts it anew; a finished or failed run has
/// Continue. Every state's layout fits at every size.
#[test]
fn the_prepare_step_is_offered_once_skippable_and_can_be_asked_for() {
    use super::model::ProbeState;
    use super::prepare::State;
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 960, 720, 1.0);
    // The fake machine is prepared: nothing offered.
    assert!(!a.offer_prepare());
    assert_eq!(a.screen, Screen::Main);
    let ProbeState::Ready(p) = &mut a.model.probe else { panic!("the fake probe") };
    p.prepared = Some(false);
    // Offered by itself: run behind the main screen, Play still first.
    assert!(a.offer_prepare());
    assert_eq!(a.screen, Screen::Main);
    assert!(a.model.prepare.wanted && a.model.prepare.offered && a.model.prepare.background);
    // The window started the run; its lines arrive: the line beside Play says so.
    a.model.prepare.wanted = false;
    a.model.prepare.state = State::default().after(super::prepare::parse_line("step=2/5 shader programs").unwrap());
    a.rebuild(&fonts);
    assert_eq!(focus(&a), "play", "Play keeps the focus");
    assert!(
        a.ui.widgets
            .iter()
            .any(|w| w.texts.iter().any(|t| t.full.as_deref().unwrap_or(&t.text).contains("in the background: shader programs (2 of 5)"))),
        "the main screen says what it is doing"
    );
    for (w, h) in SIZES {
        let mut b = app(&env, &fonts, Screen::Main, w, h, 1.0);
        b.model.prepare = a.model.prepare.clone();
        b.rebuild(&fonts);
        check_layout(&b.ui, &fonts, &format!("main, prepare in the background {w}x{h}"));
    }
    // Done: the line reads as before.
    a.model.prepare.state = State::Done;
    a.rebuild(&fonts);
    assert!(!a.ui.widgets.iter().any(|w| w.texts.iter().any(|t| t.full.as_deref().unwrap_or(&t.text).contains("in the background"))));
    assert!(!a.offer_prepare(), "not offered again in this session");
    // "Prepare again" on the main screen.
    while focus(&a) != "prepare" {
        key(&mut a, &fonts, Key::Tab);
    }
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Prepare);
    assert!(a.model.prepare.wanted);
    // Done: Continue; Escape leaves too.
    a.model.prepare.wanted = false;
    a.model.prepare.state = State::Done;
    a.rebuild(&fonts);
    assert_eq!(a.ui.focus_order(), ["continue"]);
    key(&mut a, &fonts, Key::Escape);
    assert_eq!(a.screen, Screen::Main);
    // Every state fits at every size.
    for (w, h) in SIZES {
        for state in [
            State::Idle,
            State::Running { n: 3, of: 5, what: "the city".into() },
            State::Done,
            State::Failed("it ended with code 1".into()),
        ] {
            let mut a = app(&env, &fonts, Screen::Prepare, w, h, 1.0);
            a.model.prepare.state = state.clone();
            a.rebuild(&fonts);
            check_layout(&a.ui, &fonts, &format!("prepare {state:?} {w}x{h}"));
        }
    }
}

/// The prepare step offered again when the Graphics row saved a level other
/// than the one the hardware check found prepared (the stamp carries the
/// settings' program key: `game --probe` says not prepared once the level
/// differs): offered once, skippable, not again in the session; a level set
/// back offers nothing.
#[test]
fn the_prepare_step_is_offered_again_after_the_graphics_level_changed() {
    use super::model::{Graphics, ProbeState};
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 960, 720, 1.0);
    let ProbeState::Ready(p) = &a.model.probe else { panic!("the fake probe") };
    assert_eq!(p.prepared, Some(true));
    let level = a.model.saved.graphics;
    assert!(!a.offer_prepare(), "prepared for the level in force");
    let other = if level == Graphics::Original { Graphics::Enhanced } else { Graphics::Original };
    a.model.saved.graphics = other;
    a.model.choices.graphics = other;
    a.model.saved.graphics = level;
    assert!(!a.offer_prepare(), "the level set back: nothing");
    a.model.saved.graphics = other;
    assert!(a.offer_prepare(), "another level saved: offered");
    assert_eq!(a.screen, Screen::Main, "run behind the main screen");
    assert!(a.model.prepare.background);
    // Another change while it runs: not offered again in this session.
    a.model.prepare.wanted = false;
    a.model.prepare.state = super::prepare::State::Running { n: 2, of: 5, what: "shader programs".into() };
    a.rebuild(&fonts);
    a.model.saved.graphics = level;
    assert!(!a.offer_prepare(), "once a session");
}

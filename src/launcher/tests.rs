//! The launcher's layout, focus and settings tests (fake data, no game
//! files, no window).

use super::font::Fonts;
use super::model::{Choices, Env, FakeEnv, Graphics, SaveError, save_choices};
use super::screens::{App, Outcome, Screen};
use super::ui::{Event, Input, Key, Kind, Mark, Metrics, Rect, Ui};

/// The sizes every screen is laid out and rendered at: the smallest
/// window, 720p, the Steam Deck (1280 × 800), the common laptop
/// (1366 × 768), 1080p, 21:9 and 4K.
pub const SIZES: [(u32, u32); 7] =
    [(640, 480), (1280, 720), (1280, 800), (1366, 768), (1920, 1080), (2560, 1080), (3840, 2160)];

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
        // A dialog lies over the page (its scrim first): the page behind
        // it is not laid out against it.
        let scrim = ui.widgets.iter().position(|w| matches!(w.kind, Kind::Scrim)).unwrap_or(usize::MAX);
        for (j, o) in ui.widgets.iter().enumerate().skip(i + 1) {
            if i < scrim && j >= scrim {
                continue;
            }
            // A card holds the widgets inside it.
            let holder = |k: &Kind| matches!(k, Kind::Card | Kind::Message { .. } | Kind::Dialog | Kind::Scrim);
            if holder(&w.kind) && o.rect.inside(&w.rect) || holder(&o.kind) && w.rect.inside(&o.rect) {
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
    let shown = ui.widgets.iter().position(|w| matches!(w.kind, Kind::Scrim)).unwrap_or(0);
    let texts: Vec<_> = ui.widgets[shown..].iter().flat_map(|w| w.texts.iter()).collect();
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
            if matches!(s, Screen::Mods | Screen::Storage | Screen::Dev) {
                // A long list of mods scrolls (the Dev page's tools too, on
                // the smallest windows).
                ui.view.h = ui.content_h.max(ui.view.h);
            }
            if s.document() {
                // A document scrolls; its Back button is in view at the top.
                assert!(a.ui.get("back").unwrap().rect.bottom() <= h as f32, "{} {w}x{h}", s.name());
                ui.view.h = ui.content_h.max(ui.view.h);
            }
            check_layout(&ui, &fonts, &format!("{} {w}x{h}", s.name()));
        }
    }
}

/// Settings › Advanced with every language the engine cannot show
/// installed: the "Not supported yet" line and the rows fit every size.
#[test]
fn unsupported_languages_line_fits_every_size() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    for (w, h) in SIZES {
        let a = super::headless::state_app(&env, &fonts, "language-unsupported", w, h, 1.0).unwrap();
        let t = all_text(&a);
        assert!(t.contains("Not supported yet") && t.contains("Korean") && t.contains("Thailand"), "{w}x{h}: {t}");
        check_layout(&a.ui, &fonts, &format!("language-unsupported {w}x{h}"));
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
            for id in ["play", "row_graphics", "row_screen", "row_mods"] {
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
        assert_eq!(a.ui.focus_order(), ["details", "play", "row_graphics", "row_screen", "row_mods"], "{w}x{h}");
        assert_eq!(focus(&a), "play", "the default focus");
        key(&mut a, &fonts, Key::Tab);
        assert_eq!(focus(&a), "row_graphics", "Tab from Play: the first row");
        key(&mut a, &fonts, Key::BackTab);
        key(&mut a, &fonts, Key::BackTab);
        assert_eq!(focus(&a), "details", "Shift+Tab from Play: the game check above it");
        a.st.focus = Some("play");
        key(&mut a, &fonts, Key::Up);
        assert_eq!(focus(&a), "details", "Up from Play: the game check");
        key(&mut a, &fonts, Key::Up);
        assert_eq!(focus(&a), "details", "no wrap at the top");
        a.st.focus = Some("play");
        for want in ["row_graphics", "row_screen", "row_mods"] {
            key(&mut a, &fonts, Key::Down);
            assert_eq!(focus(&a), want, "{w}x{h}: Down");
        }
        // Settings: the rows top to bottom, then Back to defaults.
        let mut s = app(&env, &fonts, Screen::Settings, w, h, 1.0);
        assert_eq!(
            s.ui.focus_order(),
            ["display", "resolution", "graphics", "skip_movies", "skip_title", "skip", "advanced", "reset"],
            "{w}x{h}"
        );
        assert_eq!(focus(&s), "display");
        for want in ["resolution", "graphics", "skip_movies", "skip_title", "skip", "advanced", "reset"] {
            key(&mut s, &fonts, Key::Down);
            assert_eq!(focus(&s), want, "{w}x{h}: Down on Settings");
        }
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
    let mut a = app(&env, &fonts, Screen::Settings, 1280, 720, 1.0);
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

/// A desktop below the panel's native modes (scaled handheld): the
/// launcher's full-screen list offers them, as the game does.
#[test]
fn resolution_list_offers_modes_above_the_desktop() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Settings, 1280, 720, 1.0);
    let m = a.model.monitor.as_mut().unwrap();
    m.desktop = (1280, 800);
    m.modes = vec![(1280, 800), (1920, 1200)];
    a.model.choices.fullscreen = true;
    assert_eq!(a.model.modes(), vec![(1280, 800), (1920, 1200)]);
    a.model.choices.fullscreen = false;
    assert!(!a.model.modes().iter().any(|&(w, h)| w > 1280 || h > 800));
}

#[test]
fn controls_change_the_model() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Settings, 1280, 720, 1.0);
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
    // A click on a value row's left arrow steps back.
    let c = a.ui.get("display").unwrap().rect;
    let fs = a.model.choices.fullscreen;
    a.input(&Input::MouseDown(c.x + 4.0, c.cy()), &fonts);
    assert_ne!(a.model.choices.fullscreen, fs);
    // The mouse: a click on Play (down and up on it) plays.
    a.go(Screen::Main);
    a.rebuild(&fonts);
    let r = a.ui.get("play").unwrap().rect;
    a.input(&Input::MouseDown(r.cx(), r.cy()), &fonts);
    assert_eq!(a.input(&Input::MouseUp(r.cx(), r.cy()), &fonts), Outcome::Play);
    // A press that leaves the button does nothing.
    a.input(&Input::MouseDown(r.cx(), r.cy()), &fonts);
    assert_eq!(a.input(&Input::MouseUp(1.0, 1.0), &fonts), Outcome::Stay);
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
    assert!(["check", "browse"].contains(&focus(&a)), "past the list's end: the buttons under it");
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
    // The next step: the own copy, Make copy chosen; Not now goes on to the main screen.
    assert_eq!(a.screen, Screen::GameData);
    assert_eq!(focus(&a), "import");
    a.st.focus = Some("later");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Main);
    assert_eq!(a.model.languages, ["English", "French", "German"]);
    // Escape on the first-run screen asks; Quit quits.
    let mut b = app(&env, &fonts, Screen::FirstRun, 960, 720, 1.0);
    assert_eq!(key(&mut b, &fonts, Key::Escape), Outcome::Stay);
    assert_eq!(focus(&b), "stay");
    b.st.focus = Some("quit");
    assert_eq!(key(&mut b, &fonts, Key::Enter), Outcome::Quit);
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
        language: String::new(),
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

/// The Game language row (Settings › Advanced) cycles the installed languages the engine can
/// show; English is "nothing kept"; the ones it cannot show are named under
/// the row with the reason beside it; the game gets `--language` only for a
/// chosen one.
#[test]
fn game_language_row_lists_only_supported_languages() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Advanced, 1280, 720, 1.0);
    assert_eq!(a.model.language_name(), "English");
    assert!(a.model.choices.language.is_empty(), "nothing chosen");
    assert!(a.model.unsupported_languages().is_empty());
    a.st.focus = Some("language");
    key(&mut a, &fonts, Key::Right);
    assert_eq!((a.model.choices.language.as_str(), a.model.language_name().as_str()), ("French", "French"));
    key(&mut a, &fonts, Key::Left);
    assert!(a.model.choices.language.is_empty(), "English again keeps nothing");
    key(&mut a, &fonts, Key::Left);
    assert_eq!(a.model.choices.language, "Swedish", "the list wraps");
    // Korean and Thailand are installed but not supported: named, with why.
    a.model.languages.extend(["Korean".to_string(), "Thailand".to_string()]);
    assert_eq!(a.model.supported_languages(), ["English", "French", "German", "Italian", "Spanish", "Swedish"]);
    let no = a.model.unsupported_languages();
    assert_eq!(no.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), ["Korean", "Thailand"]);
    assert!(no.iter().all(|(_, why)| !why.is_empty()));
    a.rebuild(&fonts);
    let t = all_text(&a);
    assert!(t.contains("Game language") && t.contains("Not supported yet: Korean ("), "{t}");
    assert!(t.contains("Thailand ("), "{t}");
    // A copy without English plays its own language; no copy yet, English.
    a.model.choices.language.clear();
    a.model.languages = vec!["German".into(), "Korean".into()];
    assert_eq!(a.model.language_name(), "German");
    a.model.languages.clear();
    assert_eq!((a.model.language_name().as_str(), a.model.supported_languages().len()), ("English", 1));
    assert!(super::window::language_args("").is_empty());
    assert_eq!(super::window::language_args("German"), ["--language", "German"]);
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
    let mut a = app(&env, &fonts, Screen::Settings, 1280, 720, 1.0);
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
    a.go(Screen::Settings);
    a.rebuild(&fonts);
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
        language: String::new(),
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
    let a = App::new(&env, m, Screen::Settings, (1280, 720), 1.0, &fonts);
    assert!(!a.ui.get("display").unwrap().enabled);
}

#[test]
fn about_licence_and_notices() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Advanced, 1280, 720, 1.0);
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
    assert!(all.contains("Sofia Sans"), "the notices list the font");
    key(&mut a, &fonts, Key::Escape);
    key(&mut a, &fonts, Key::Escape);
    assert_eq!(a.screen, Screen::Advanced);
    key(&mut a, &fonts, Key::Escape);
    assert_eq!(a.screen, Screen::Settings);
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
    // The kind's title and one sentence; the game's own lines only
    // behind Details.
    assert!(all.iter().any(|t| t.contains("stopped unexpectedly")), "{all:?}");
    assert!(!all.iter().any(|t| t.contains("note: run with")), "the tail hidden");
    a.st.focus = Some("details");
    key(&mut a, &fonts, Key::Enter);
    let all: Vec<&str> = a.ui.widgets.iter().flat_map(|w| &w.texts).map(|t| t.text.as_str()).collect();
    assert!(all.iter().any(|t| t.contains("note: run with")), "the tail's last line shown");
    a.st.focus = Some("retry");
    assert_eq!(key(&mut a, &fonts, Key::Enter), Outcome::Play, "Try again");
    a.st.focus = Some("open_logs");
    assert_eq!(key(&mut a, &fonts, Key::Enter), Outcome::Stay);
    a.st.focus = Some("support");
    assert_eq!(key(&mut a, &fonts, Key::Enter), Outcome::Stay, "the support file");
    a.st.focus = Some("settings");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Settings);
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

/// Settings › Advanced › Import saves opens the Saves page; a found
/// source shows its plan, Import brings the profiles in; Choose folder…
/// picks the original's folder in the browser and shows its plan.
#[test]
fn import_flow() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Advanced, 1280, 720, 1.0);
    a.st.focus = Some("import");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Saves);
    assert_eq!(focus(&a), "find");
    assert!(a.model.saves.found.is_some(), "looked for the original's saves");
    a.st.focus = Some("src0");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(focus(&a), "import_go");
    assert_eq!(a.ui.get("import_go").unwrap().texts[0].text, "Import 2 saves");
    let text = all_text(&a);
    assert!(text.contains("TUNER: a profile of this name is already here: imported as TUNER2"), "{text}");
    assert!(text.contains("NIGHTOWL: already here"), "{text}");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.saves.plan.is_none());
    assert!(a.model.saves.rows.iter().any(|r| r.s.name == "RACER"));
    assert_eq!(a.model.saves.row().unwrap().s.name, "RACER", "the imported profile chosen");
    assert!(all_text(&a).contains("Imported 2 profiles."));
    a.model.pad_input = true;
    a.st.focus = Some("pick");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!((a.screen, a.model.browse.purpose), (Screen::Browse, super::model::BrowseFor::Saves));
    a.st.focus = Some("use");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Saves);
    assert!(a.model.saves.plan.is_some(), "the chosen folder's plan shown");
}

/// The Saves page: Delete asks (Cancel focused), deletes and offers
/// Undo; Rename takes a name of letters and digits; Duplicate asks; the
/// keys X, Y, Delete and F2 do what their legends say; a restore over a
/// profile of that name asks as a copy first.
#[test]
fn saves_page_flow() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 1280, 720, 1.0);
    a.go_page(3);
    a.rebuild(&fonts);
    assert_eq!(a.screen, Screen::Saves);
    assert_eq!(focus(&a), "saves_list");
    assert_eq!(a.model.saves.row().unwrap().s.name, "NIGHTOWL");
    // Delete key on the list: the question, Cancel focused.
    key(&mut a, &fonts, Key::Delete);
    assert_eq!(a.model.ask, Some(super::model::Dialog::SaveDelete("NIGHTOWL".into())));
    assert_eq!(focus(&a), "save_cancel");
    assert!(all_text(&a).contains("Its 3 saved covers go with it."));
    key(&mut a, &fonts, Key::Escape);
    let t = all_text(&a);
    assert!(t.contains("Saved covers (3)") && t.contains("DVD 1") && t.contains("Magazine 2"), "{t}");
    key(&mut a, &fonts, Key::Delete);
    key(&mut a, &fonts, Key::Escape);
    assert_eq!(a.model.ask, None);
    assert!(a.model.saves.rows.iter().any(|r| r.s.name == "NIGHTOWL"), "nothing deleted");
    key(&mut a, &fonts, Key::Delete);
    a.st.focus = Some("delete_yes");
    key(&mut a, &fonts, Key::Enter);
    assert!(!a.model.saves.rows.iter().any(|r| r.s.name == "NIGHTOWL"));
    assert!(all_text(&a).contains("Deleted NIGHTOWL"));
    assert!(a.ui.get("undo").is_some());
    key(&mut a, &fonts, Key::Undo);
    assert!(a.model.saves.undo.is_none());
    assert!(all_text(&a).contains("Undone: Deleted NIGHTOWL."));
    // Undo goes away by itself.
    a.model.saves.undo = Some(super::saves::store::Undo { remove: None, restore: None, said: "x".into() });
    a.model.saves.undo_until = Some(std::time::Instant::now());
    assert!(a.poll_saves() && a.model.saves.undo.is_none());
    // Rename: a bad name refused, a good one done.
    a.model.saves.sel = 0;
    a.st.focus = Some("saves_list");
    key(&mut a, &fonts, Key::Rename);
    assert_eq!(focus(&a), "rename_field");
    a.input(&Input::Text("BAD NAME".into()), &fonts);
    key(&mut a, &fonts, Key::Enter);
    assert!(all_text(&a).contains("A name is 1 to 7 letters or digits."));
    a.model.saves.renaming = Some("OWL".into());
    a.st.focus = Some("rename_ok");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.saves.row().unwrap().s.name, "OWL");
    assert!(all_text(&a).contains("Renamed NIGHTOWL to OWL"), "{:?} {}", a.model.saves.undo, all_text(&a));
    // Y: back up.
    key(&mut a, &fonts, Key::ActionY);
    assert!(all_text(&a).contains("backed up."), "{}", all_text(&a));
    // Duplicate asks first (under More…, with the less used actions).
    a.model.saves.more = false;
    a.rebuild(&fonts);
    assert!(a.ui.get("duplicate").is_none() && a.ui.get("delete").is_none());
    a.st.focus = Some("more");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.ui.get("rename").is_some() && a.ui.get("export").is_some() && a.ui.get("delete").is_some());
    a.st.focus = Some("duplicate");
    key(&mut a, &fonts, Key::Enter);
    assert!(matches!(a.model.ask, Some(super::model::Dialog::SaveDuplicate(_))));
    a.st.focus = Some("duplicate_yes");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.saves.row().unwrap().s.name.ends_with('2'));
    // A backup of a profile that is there: as a copy (focused) or Replace.
    a.model.saves.sel = 0;
    let name = a.model.saves.row().unwrap().s.name.clone();
    a.model.saves.backups[0].name = name.clone();
    a.st.focus = Some("backups");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.saves.show_backups);
    a.st.focus = Some("restore");
    key(&mut a, &fonts, Key::Enter);
    assert!(matches!(a.model.ask, Some(super::model::Dialog::SaveRestore(_))));
    assert_eq!(focus(&a), "restore_copy");
    key(&mut a, &fonts, Key::Enter);
    assert!(all_text(&a).contains(&format!("Restored as {name}2.")));
    // Back leaves the actions, then the page.
    key(&mut a, &fonts, Key::Escape);
    key(&mut a, &fonts, Key::Escape);
    assert_eq!(a.screen, Screen::Main);
}

/// The relative times and dates of "last played".
#[test]
fn saves_times() {
    use super::saves_page::{ago, date};
    super::localtime::fixed();
    let t0 = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_795_003_200);
    let before = |s: u64| Some(t0 - std::time::Duration::from_secs(s));
    assert_eq!(ago(t0, before(10)), "just now");
    assert_eq!(ago(t0, before(600)), "10 min ago");
    assert_eq!(ago(t0, before(7200)), "2 h ago");
    assert_eq!(ago(t0, before(86_400)), "1 day ago");
    assert_eq!(ago(t0, before(3 * 86_400)), "3 days ago");
    assert_eq!(date(t0), "18 Nov 2026");
    assert_eq!(ago(t0, before(30 * 86_400)), "19 Oct 2026");
    assert_eq!(date(std::time::UNIX_EPOCH), "1 Jan 1970");
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
                // The main screen with the shaders' block may scroll (Play stays in view).
                let shaders = a.screen == Screen::Main && super::screens::shaders(&a.model) != super::screens::Shaders::Quiet;
                let may_scroll = a.screen.document() || matches!(a.screen, Screen::Mods | Screen::Storage | Screen::Dev) || shaders || (h as f32 / dpi) < 480.0;
                if may_scroll {
                    ui.view.h = ui.content_h.max(ui.view.h);
                }
                check_layout(&ui, &fonts, &what);
                assert!(a.ui.metrics.unwrap().body() >= 14.0 * dpi.min(1.5), "{what}: body text too small");
                // (Play is missing where its fix takes its place, and has no
                // id under a question.)
                if a.screen == Screen::Main
                    && (!may_scroll || shaders)
                    && let Some(r) = a.ui.get("play").map(|w| w.rect)
                {
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
    // Missing files: the fix is to check again after copying them.
    let mut a = super::headless::state_app(&env, &fonts, "files-missing", 1280, 720, 1.0).unwrap();
    assert!(all_text(&a).contains("Some game files are missing"), "{}", all_text(&a));
    assert!(a.ui.get("use").is_none(), "no Use this folder for a refused folder");
    assert_eq!(focus(&a), "recheck");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Check, "Check again stays on the check");
    for (state, words) in [("not-a-game-folder", "No game here"), ("own-folder", "This is Overbright's folder")] {
        let mut a = super::headless::state_app(&env, &fonts, state, 1280, 720, 1.0).unwrap();
        let t = all_text(&a);
        assert!(t.contains(words), "{state}: {t}");
        assert!(a.ui.get("use").is_none(), "{state}: Use this folder is shown");
        assert_eq!(focus(&a), "another", "{state}: the default focus");
        let parent = a.model.check.as_ref().unwrap().root.parent().unwrap().to_path_buf();
        key(&mut a, &fonts, Key::Enter);
        assert_eq!(a.screen, Screen::Browse, "{state}");
        assert_eq!(a.model.browse.dir, parent, "{state}: the browser opens beside the refused folder");
        key(&mut a, &fonts, Key::Escape);
        assert_eq!(a.screen, Screen::Check, "{state}: Cancel goes back to the check");
    }
    let a = super::headless::state_app(&env, &fonts, "not-a-game-folder", 1280, 720, 1.0).unwrap();
    assert!(all_text(&a).contains("GLOBAL, FRONTEND and TRACKS"), "names the folder by its contents");
    let a = super::headless::state_app(&env, &fonts, "check-ok", 1280, 720, 1.0).unwrap();
    assert!(a.ui.get("another").is_none() && a.ui.get("use").unwrap().enabled);
    assert_eq!(focus(&a), "use");
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
        for words in ["Vulkan driver for this graphics card was found", "sudo apt install libvulkan1", "NVIDIA"] {
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
        let path =
            a.ui.widgets
                .iter()
                .flat_map(|w| &w.texts)
                .find(|t| {
                    let f = t.full.as_deref().unwrap_or(&t.text);
                    f.starts_with("/data/player/") && f.ends_with("Deutsche Version")
                })
                .unwrap_or_else(|| panic!("{what}: the game path is not shown"));
        let r = |id| a.ui.get(id).unwrap().rect;
        let play = r("play");
        assert!(path.rect.bottom() <= play.y, "{what}: the folder above Play");
        // Play: the largest control, above the summary rows.
        assert!(
            a.ui.widgets
                .iter()
                .filter(|w| w.id.is_some() && w.id != Some("play") && !matches!(w.kind, Kind::SumRow { .. }))
                .all(|w| w.rect.w * w.rect.h < play.w * play.h),
            "{what}: Play is the largest button"
        );
        assert!(play.bottom() <= r("row_graphics").y, "{what}: Play above the rows");
        assert!(all_text(&a).contains("Ready"), "{what}: the game's state above Play");
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
    a.st.focus = Some("row_mods");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!((a.screen, focus(&a)), (Screen::Mods, "pk0"));
    let lines = |a: &App| super::screens::pack_lines(&a.model).into_iter().map(|l| (l.key, l.on)).collect::<Vec<_>>();
    assert_eq!(
        lines(&a),
        [("own:night-roads".to_string(), false), ("roads-hd".to_string(), true), ("garage-redux".to_string(), false)],
        "own packs first, then the imported ones"
    );
    // A row's switch: Enter or Space.
    a.st.focus = Some("pk1");
    key(&mut a, &fonts, Key::Enter);
    assert!(!a.model.mods.list[0].0.on);
    key(&mut a, &fonts, Key::Space);
    assert!(a.model.mods.list[0].0.on);
    // Two packs on that change the same textures: said, the order only for
    // packs of one list.
    a.st.focus = Some("pk0");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.mods.conflict.as_ref().is_some_and(|c| !c.same_list));
    assert!(all_text(&a).contains("Two packs change the same textures") && a.ui.get("order").is_none());
    // Remove (X) asks first; Escape keeps it.
    a.st.focus = Some("pk1");
    key(&mut a, &fonts, Key::ActionX);
    assert_eq!(a.model.mods.confirm.as_deref(), Some("roads-hd"));
    assert_eq!(focus(&a), "keep");
    key(&mut a, &fonts, Key::Escape);
    assert!(a.model.mods.confirm.is_none() && a.screen == Screen::Mods);
    a.st.focus = Some("pk1");
    key(&mut a, &fonts, Key::ActionX);
    a.st.focus = Some("remove_yes");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.mods.results[0].1.contains("removed (your package file is kept)"), "{:?}", a.model.mods.results);
    assert!(!a.model.mods.list.iter().any(|(e, _)| e.id == "roads-hd"));
    // Add a mod: the browser lists packages with the folders; one imports.
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
    // Use mods: off sets every mod aside and keeps the list; on gives it back.
    let before = lines(&a);
    a.st.focus = Some("use_mods");
    key(&mut a, &fonts, Key::Space);
    assert!(a.model.mods.paused && lines(&a).iter().all(|(_, on)| !on));
    assert!(!a.ui.get("pk0").unwrap().enabled, "the list is kept, greyed");
    assert_eq!(super::screens::mods_summary(&a.model), "Off: the game's own textures");
    key(&mut a, &fonts, Key::Space);
    assert!(!a.model.mods.paused);
    assert_eq!(lines(&a), before, "given back as it was");
    // Map mods: a switch each.
    a.st.focus = Some("mp1");
    key(&mut a, &fonts, Key::Space);
    assert!(a.model.mods.maps[1].on);
    key(&mut a, &fonts, Key::Escape);
    assert_eq!(a.screen, Screen::Main);
}

/// The order: Move up / Move down after "Change the order", within a list.
#[test]
fn mods_order_only_on_a_collision() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let a = app(&env, &fonts, Screen::Mods, 1280, 720, 1.0);
    assert!(a.ui.get("up").is_none() && a.ui.get("order").is_none(), "no order without a collision");
    let mut a = super::headless::state_app(&env, &fonts, "mods-order", 1280, 720, 1.0).unwrap();
    assert!(a.ui.get("up").unwrap().enabled && !a.ui.get("down").unwrap().enabled);
    a.st.focus = Some("up");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.mods.list[0].0.id, "garage-redux", "moved up");
    assert_eq!(a.model.mods.sel, 1);
}

/// The launcher's own preferences read back as written; Use mods' set-aside
/// list too.
#[test]
fn launcher_prefs_and_paused_round_trip() {
    use super::model::{Paused, Prefs};
    let p = Prefs { text: 150, reduced_motion: true, sounds: true };
    assert_eq!(Prefs::parse(&p.text()), p);
    assert_eq!(Prefs::parse("text_size 999\n"), Prefs::default(), "an unknown size: 100 %");
    assert!(!Prefs::default().sounds, "sounds are off by default");
    let q = Paused {
        own: vec![("night-roads".into(), true)],
        packages: vec![("roads-hd".into(), false)],
        maps: vec!["drift-park".into()],
        packs: vec!["docks-remix".into()],
        hud: true,
    };
    assert_eq!(Paused::parse(&q.text()), q);
}

/// Text size above 100 %: the page keeps the larger text and scrolls.
#[test]
fn text_size_scales_the_page() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let a = app(&env, &fonts, Screen::Settings, 1280, 720, 1.0);
    let mut m = env.load();
    m.prefs.text = 150;
    let b = App::new(&env, m, Screen::Settings, (1280, 720), 1.0, &fonts);
    let u = |a: &App| a.ui.metrics.unwrap().u;
    assert!((u(&b) / u(&a) - 1.5).abs() < 0.01, "{} vs {}", u(&b), u(&a));
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
        let shown =
            a.ui.widgets
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
    assert!(t.contains("HUD mod · complete"), "{t}");
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
    assert!(t.contains("Overbright copies each mod") && t.contains("None added"), "{t}");
    assert!(a.ui.get("pk0").is_none() && !a.ui.get("map_export").unwrap().enabled);
    assert!(a.ui.get("map_editor").is_none(), "the map editor's own button is on the Dev page");
    assert!(!a.ui.get("map_open").unwrap().enabled, "no mod to open");
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
/// Make copy, Continue, a cycle's arrow, the start checkbox, Play.
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
    // The own copy offered next: one click makes it (in the background), Continue goes on.
    assert_eq!(a.screen, Screen::GameData);
    click(&mut a, &fonts, "import");
    a.poll_jobs(&fonts);
    click(&mut a, &fonts, "continue");
    assert_eq!(a.screen, Screen::Main);
    click(&mut a, &fonts, "row_screen");
    assert_eq!(a.screen, Screen::Settings);
    let fs = a.model.choices.fullscreen;
    let d = on_window(&a, "display");
    let Kind::ValueRow { right, .. } = a.ui.get("display").unwrap().kind else { panic!("a value row") };
    click_at(&mut a, &fonts, right.cx(), d.cy());
    assert_ne!(a.model.choices.fullscreen, fs, "the row's right arrow");
    let skip = a.model.choices.skip;
    let Kind::ValueRow { right, .. } = a.ui.get("skip").unwrap().kind else { panic!("a value row") };
    let s = on_window(&a, "skip");
    click_at(&mut a, &fonts, right.cx(), s.cy());
    assert_ne!(a.model.choices.skip, skip);
    click(&mut a, &fonts, "page:game");
    assert_eq!(click(&mut a, &fonts, "play"), Outcome::Play);
}

/// Mods with the mouse: a package removed (the confirmation by click),
/// then one added from the Downloads place by a double click.
#[test]
fn mouse_only_mods_add_and_remove() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 1280, 720, 1.0);
    click(&mut a, &fonts, "row_mods");
    assert_eq!(a.screen, Screen::Mods);
    let ids = |a: &App| a.model.mods.list.iter().map(|(e, _)| e.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&a)[0], "roads-hd");
    // A click on a row chooses it (and turns it); the Remove legend asks.
    click(&mut a, &fonts, "pk1");
    assert_eq!(a.model.mods.sel, 1);
    a.input(&Input::Key(Key::ActionX), &fonts);
    assert_eq!(a.model.mods.confirm.as_deref(), Some("roads-hd"));
    click(&mut a, &fonts, "keep");
    assert!(a.model.mods.confirm.is_none());
    a.st.focus = Some("pk1");
    a.input(&Input::Key(Key::ActionX), &fonts);
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
    click(&mut a, &fonts, "page:game");
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
    // Hover: the row under the pointer (one in view: row k is "Folder k").
    let k = list_state(&a, "dirs").0 + 1;
    let name = format!("Folder {k:02}");
    let (x, y) = row_at(&a, "dirs", &name).unwrap();
    a.input(&Input::MouseMove(x, y), &fonts);
    assert_eq!(a.st.hover_row, Some(("dirs", k)));
    assert_eq!(a.ui.pointer_at(x, y, &a.st), super::ui::Pointer::Hand);
    // Chosen, then the pointer leaves: still chosen.
    click_at(&mut a, &fonts, x, y);
    a.input(&Input::MouseMove(1.0, 1.0), &fonts);
    assert_eq!((list_state(&a, "dirs").1, a.st.hover_row), (k, None));
    // Open does what a double click does.
    click(&mut a, &fonts, "open");
    assert_eq!(a.model.browse.dir, std::path::Path::new(&format!("/data/player/Games/{name}")));
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
    assert_eq!(b.screen, Screen::Advanced);
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
    let p = on_window(&m, "add");
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
        a.ui.widgets.iter().any(|w| w.texts.iter().any(|t| t
            .full
            .as_deref()
            .unwrap_or(&t.text)
            .contains("Getting the shaders ready"))),
        "the main screen says what it is doing"
    );
    for (w, h) in SIZES {
        let mut b = app(&env, &fonts, Screen::Main, w, h, 1.0);
        b.model.prepare = a.model.prepare.clone();
        b.rebuild(&fonts);
        let mut ui = b.ui.clone();
        ui.view.h = ui.content_h.max(ui.view.h);
        check_layout(&ui, &fonts, &format!("main, prepare in the background {w}x{h}"));
    }
    // Done: the line reads as before.
    a.model.prepare.state = State::Done;
    a.rebuild(&fonts);
    assert!(
        !a.ui.widgets.iter().any(|w| w.texts.iter().any(|t| t
            .full
            .as_deref()
            .unwrap_or(&t.text)
            .contains("in the background")))
    );
    assert!(!a.offer_prepare(), "not offered again in this session");
    // "Get shaders ready again" under Settings › Advanced.
    a.go(Screen::Advanced);
    a.rebuild(&fonts);
    a.st.focus = Some("prepare");
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

/// The game's `error.kind=` line: written and read back, kept out of the
/// lines shown, and the kind inferred without one.
#[test]
fn the_games_error_kind_line() {
    use super::run::{Ended, ErrorKind};
    for k in ErrorKind::ALL {
        assert_eq!(ErrorKind::parse(&k.line()), Some(k));
    }
    assert_eq!(ErrorKind::parse("error.kind=nonsense"), None);
    assert_eq!(ErrorKind::parse("could not open a window"), None);
    assert_eq!(
        ErrorKind::of_window_error("no GPU adapter (Vulkan or DX12) that can show the window"),
        ErrorKind::NoGpu
    );
    assert_eq!(ErrorKind::of_window_error("the display refused the size"), ErrorKind::NoWindow);
    let ended = |named, panicked, no_window| Ended {
        code: Some(2),
        signal: None,
        pid: 1,
        started_at: std::time::SystemTime::UNIX_EPOCH,
        secs: 1.0,
        panicked,
        no_window,
        named,
        tail: Vec::new(),
        log: None,
    };
    assert_eq!(ended(Some(ErrorKind::GameFiles), false, false).kind(), ErrorKind::GameFiles);
    assert_eq!(ended(None, true, false).kind(), ErrorKind::Crash);
    assert_eq!(ended(None, false, true).kind(), ErrorKind::NoWindow);
    assert_eq!(ended(None, false, false).kind(), ErrorKind::Other);
    // Every kind has a title, a sentence and one fixing button, with no
    // developer text in them.
    for k in ErrorKind::ALL {
        for during in [false, true] {
            let w = super::screens::failure_words(k, during);
            let all = format!("{} {} {}", w.title, w.sentence, w.fix.1);
            for bad in ["0x", "SPEED2", "--", "bytes", "hash", "North American"] {
                assert!(!all.contains(bad), "{k:?}: {all}");
            }
            // The advice fits the kind: driver advice only without a
            // graphics driver.
            assert_eq!(all.contains("driver"), k == ErrorKind::NoGpu, "{k:?}: {all}");
        }
    }
}

/// The failure screen's fixing buttons: Play windowed turns full screen
/// off and starts; the game-files kind checks the folder again.
#[test]
fn failure_fixes() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = super::headless::state_app(&env, &fonts, "no-game-window", 1280, 720, 1.0).unwrap();
    assert!(all_text(&a).contains("The game window didn't open"));
    assert_eq!(focus(&a), "windowed");
    assert!(a.model.choices.fullscreen);
    assert_eq!(key(&mut a, &fonts, Key::Enter), Outcome::Play);
    assert!(!a.model.choices.fullscreen, "Play windowed");
    let mut a = app(&env, &fonts, Screen::Failure, 1280, 720, 1.0);
    a.model.failure.as_mut().unwrap().kind = super::run::ErrorKind::GameFiles;
    a.rebuild(&fonts);
    assert_eq!(focus(&a), "folder");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Check);
}

/// The system folder dialog's answers: a folder is taken (a game folder
/// checked), a cancel changes nothing, no dialog opens Overbright's own
/// browser; a pad goes straight to the own browser.
#[test]
fn the_system_folder_dialog() {
    use super::model::{BrowseFor, Picked};
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::FirstRun, 960, 720, 1.0);
    a.dialog_done(BrowseFor::Game, None, Picked::Cancelled);
    assert_eq!(a.screen, Screen::FirstRun);
    a.dialog_done(BrowseFor::Game, None, Picked::Folder(FakeEnv::root()));
    assert_eq!(a.screen, Screen::Check);
    assert_eq!(a.model.check.as_ref().unwrap().root, FakeEnv::root());
    let mut a = app(&env, &fonts, Screen::FirstRun, 960, 720, 1.0);
    a.dialog_done(BrowseFor::Game, Some("/data/player/Games".into()), Picked::NoDialog);
    assert_eq!(a.screen, Screen::Browse);
    assert_eq!(a.model.browse.dir, std::path::Path::new("/data/player/Games"));
    let mut a = app(&env, &fonts, Screen::Saves, 960, 720, 1.0);
    a.model.pad_input = true;
    a.choose_folder(BrowseFor::Saves, None);
    assert_eq!((a.screen, a.model.browse.purpose), (Screen::Browse, BrowseFor::Saves));
    assert!(a.model.dialog.is_none());
}

/// A chosen folder next to the game folder is taken for it; Overbright's
/// own folder is refused with its own words; Windows' verbatim paths are
/// shown plain.
#[test]
fn folders_near_the_game_and_overbrights_own() {
    use super::model::{Problem, RealEnv, game_dir_near, is_overbright_dir, plain_windows_path};
    let dir = std::env::temp_dir().join(format!("ob-launcher-near-{}", crate::unique_tag()));
    let game = dir.join("EA GAMES").join("NFSU2");
    std::fs::create_dir_all(game.join("GLOBAL")).unwrap();
    let looks = |p: &std::path::Path| p.join("GLOBAL").is_dir();
    assert_eq!(game_dir_near(&game, &looks), game);
    assert_eq!(game_dir_near(&game.join("GLOBAL"), &looks), game, "one level up");
    assert_eq!(game_dir_near(&dir.join("EA GAMES"), &looks), game, "one level down");
    assert_eq!(game_dir_near(&dir, &looks), dir, "two levels: not guessed");
    let own = dir.join("overbright-0.3.0");
    std::fs::create_dir_all(&own).unwrap();
    std::fs::write(own.join("overbright"), b"").unwrap();
    assert!(is_overbright_dir(&own, None) && !is_overbright_dir(&game, None));
    assert!(is_overbright_dir(&game, Some(&game)), "the folder this program runs from");
    let c = RealEnv.check(&own);
    assert_eq!(c.problem, Some(Problem::OwnFolder), "{c:?}");
    assert!(!c.usable);
    let c = RealEnv.check(&dir);
    assert_eq!(c.problem, Some(Problem::NoGame), "{c:?}");
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(
        plain_windows_path(r"\\?\C:\Program Files (x86)\EA GAMES").as_deref(),
        Some(r"C:\Program Files (x86)\EA GAMES")
    );
    assert_eq!(plain_windows_path(r"\\?\UNC\srv\share\x").as_deref(), Some(r"\\srv\share\x"));
    assert_eq!(plain_windows_path(r"C:\x"), None);
}

/// The check leaves out the notes nothing can be done about (mod files,
/// what SPEED2.EXE is), and says why a folder is refused.
#[test]
fn check_result_lines_and_problem() {
    use super::model::{Problem, Verdict, check_result};
    use crate::qol::setup;
    let root = std::path::Path::new("/nonexistent/overbright-test");
    let ok = setup::Check { missing: Vec::new(), different: Vec::new(), ..Default::default() };
    let r = check_result(root, &ok, None);
    assert!(r.usable && r.problem.is_none() && r.verdict() == Verdict::Ready, "{r:?}");
    let some = setup::Check { missing: vec!["TRACKS/L4RA.BUN".into()], different: Vec::new(), ..Default::default() };
    let r = check_result(root, &some, None);
    assert_eq!(r.problem, Some(Problem::NoGame), "not a game folder at all: {r:?}");
    let all =
        setup::Check { missing: setup::KEY_FILES.iter().map(|f| f.0.to_string()).collect(), different: Vec::new(), ..Default::default() };
    assert_eq!(check_result(root, &all, None).verdict(), Verdict::Refused(Problem::NoGame));
    let differ = setup::Check { missing: Vec::new(), different: vec!["GLOBAL/GlobalB.lzc".into()], ..Default::default() };
    assert_eq!(check_result(root, &differ, None).verdict(), Verdict::Notes);
}

/// "Use this folder" with the game folder's row highlighted takes that
/// row (not the folder shown above, which holds it); with another row
/// highlighted, the folder shown above.
#[test]
fn use_this_folder_takes_the_highlighted_game_folder() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Browse, 1280, 720, 1.0);
    // "..", then Emulators, GOG, Heroic, Need for Speed Underground 2 (game folder).
    a.model.browse.sel = 4;
    a.event(&Event::Activate("use"));
    assert_eq!(a.screen, Screen::Check);
    assert_eq!(
        a.model.check.as_ref().unwrap().root,
        std::path::Path::new("/data/player/Games/Need for Speed Underground 2")
    );
    let mut a = app(&env, &fonts, Screen::Browse, 1280, 720, 1.0);
    a.model.browse.sel = 2;
    a.event(&Event::Activate("use"));
    assert_eq!(a.model.check.as_ref().unwrap().root, std::path::Path::new("/data/player/Games"));
}

/// Inside gamescope (Steam Deck Game Mode) there is no system file
/// chooser: folders are chosen in Overbright's own browser at once.
#[test]
fn gamescope_uses_the_own_browser() {
    use super::model::{BrowseFor, in_gamescope};
    let env_of = |pairs: &'static [(&'static str, &'static str)]| {
        move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| std::ffi::OsString::from(v))
    };
    assert!(in_gamescope(&env_of(&[("SteamDeck", "1")])));
    assert!(in_gamescope(&env_of(&[("SteamGamepadUI", "1")])));
    assert!(in_gamescope(&env_of(&[("XDG_CURRENT_DESKTOP", "gamescope")])));
    assert!(in_gamescope(&env_of(&[("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")])));
    assert!(!in_gamescope(&env_of(&[("XDG_CURRENT_DESKTOP", "KDE"), ("SteamDeck", "0")])));
    assert!(!in_gamescope(&env_of(&[])));
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Saves, 960, 720, 1.0);
    a.model.gamescope = true;
    a.choose_folder(BrowseFor::Saves, None);
    assert_eq!(a.screen, Screen::Browse);
    assert!(a.model.dialog.is_none(), "no system dialog asked for");
}

/// Play is the main, focused action only when the game can start: while
/// this computer is checked it is an ordinary button; without a graphics
/// driver Check again (beside the folder) takes the focus and runs the
/// check again.
#[test]
fn play_is_the_main_action_only_when_the_game_can_start() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let live = |a: &App| a.ui.get("play").is_some_and(|w| w.enabled);
    let a = app(&env, &fonts, Screen::Main, 1280, 720, 1.0);
    assert!(live(&a));
    assert_eq!(focus(&a), "play");
    let a = super::headless::state_app(&env, &fonts, "checking", 1280, 720, 1.0).unwrap();
    assert!(!live(&a), "Play while checking");
    let mut a = super::headless::state_app(&env, &fonts, "no-vulkan", 1280, 720, 1.0).unwrap();
    assert!(a.ui.get("play").is_none(), "the fix takes Play's place");
    assert_eq!(focus(&a), "probe_again");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.probe_again && a.model.probe == super::model::ProbeState::Pending);
}

/// Back on the main screen (Escape, a pad's B) does not quit at once: it
/// moves to Quit, and a second Back there quits.
#[test]
fn back_on_the_main_screen_goes_to_quit_first() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 1280, 720, 1.0);
    assert_eq!(key(&mut a, &fonts, Key::Escape), Outcome::Stay);
    assert_eq!(a.model.ask, Some(super::model::Dialog::Quit));
    assert_eq!(focus(&a), "stay", "Stay is focused");
    assert_eq!(a.ui.focus_order(), ["stay", "quit"], "the page behind takes no focus");
    assert_eq!(key(&mut a, &fonts, Key::Escape), Outcome::Stay, "Back closes the question");
    assert_eq!((a.model.ask.clone(), focus(&a)), (None, "play"));
    key(&mut a, &fonts, Key::Escape);
    a.st.focus = Some("quit");
    assert_eq!(key(&mut a, &fonts, Key::Enter), Outcome::Quit);
}

// ---- Contrast and glyph size on rendered pixels --------------------------------------------------

/// WCAG 2 relative luminance of an sRGB colour.
fn wcag_lum(c: [u8; 3]) -> f64 {
    let f = |v: u8| {
        let x = f64::from(v) / 255.0;
        if x <= 0.040_45 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
}

fn wcag_ratio(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (x, y) = (wcag_lum(a), wcag_lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// APCA (0.0.98G-4g constants) lightness contrast |Lc| of text on a background.
fn apca_lc(text: [u8; 3], bg: [u8; 3]) -> f64 {
    let y = |c: [u8; 3]| {
        let f = |v: u8| (f64::from(v) / 255.0).powf(2.4);
        let y = 0.212_672_9 * f(c[0]) + 0.715_152_2 * f(c[1]) + 0.072_175 * f(c[2]);
        if y < 0.022 { y + (0.022 - y).powf(1.414) } else { y }
    };
    let (yt, yb) = (y(text), y(bg));
    if (yb - yt).abs() < 0.0005 {
        return 0.0;
    }
    let s = if yb > yt { (yb.powf(0.56) - yt.powf(0.57)) * 1.14 } else { (yb.powf(0.65) - yt.powf(0.62)) * 1.14 };
    if s.abs() < 0.1 { 0.0 } else { (s.abs() - 0.027) * 100.0 }
}

/// The background under a text box on the rendered image: the median (by
/// luminance) of the pixels along the box's edge one pixel outside it.
fn background_of(img: &image::RgbaImage, r: Rect) -> [u8; 3] {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mut px: Vec<[u8; 3]> = Vec::new();
    let (x0, y0, x1, y1) =
        (r.x.floor() as i32 - 1, r.y.floor() as i32 - 1, r.right().ceil() as i32, r.bottom().ceil() as i32);
    for x in x0..=x1 {
        for y in [y0, y1] {
            if (0..w).contains(&x) && (0..h).contains(&y) {
                let p = img.get_pixel(x as u32, y as u32).0;
                px.push([p[0], p[1], p[2]]);
            }
        }
    }
    px.sort_by(|a, b| wcag_lum(*a).total_cmp(&wcag_lum(*b)));
    px[px.len() / 2]
}

/// The screens and states the contrast and size checks look at: every
/// screen, and the states that change what is drawn on the main screen
/// and the setup screens (dialogs aside: the page behind them is dimmed on
/// purpose).
fn checked_names() -> Vec<String> {
    Screen::ALL
        .iter()
        .map(|s| s.name().to_string())
        .chain(
            [
                "first-none",
                "err-own",
                "err-missing",
                "main-checking",
                "main-novulkan",
                "main-gone",
                "main-prepare",
                "main-shaders",
                "main-shaders-ready",
                "settings-suggested",
            ]
            .map(String::from),
        )
        .collect()
}

/// Every text of every screen, measured on the rendered pixels against
/// what is really under it: WCAG 2 at least 4.5:1, and APCA Lc at least
/// 60 (reading text in the look's ink and secondary colours reaches 75
/// and 70 on the slab; buttons' labels and the legends over the backdrop
/// at least 60). Disabled controls and the licence texts aside.
#[test]
fn rendered_text_contrast() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let t = super::draw::theme();
    let mut bad = Vec::new();
    for name in checked_names() {
        for (w, h) in [(1280, 800), (1366, 768), (1920, 1080)] {
            let a = super::headless::named_app(&env, &fonts, &name, w, h, 1.0).unwrap();
            if a.screen.document() || a.screen == Screen::Widgets {
                continue;
            }
            let img = super::headless::draw(&a, &fonts);
            for wd in a.ui.widgets.iter().filter(|wd| wd.enabled) {
                for tb in &wd.texts {
                    if tb.rect.bottom() > h as f32 || tb.text.trim().is_empty() {
                        continue;
                    }
                    let bg = background_of(&img, tb.rect);
                    let c = [tb.color.0, tb.color.1, tb.color.2];
                    let (r, lc) = (wcag_ratio(c, bg), apca_lc(c, bg));
                    let reading = tb.color == t.ink && matches!(wd.kind, Kind::Label | Kind::Status { .. });
                    let need_lc = if reading { 75.0 } else { 60.0 };
                    if r < 4.5 || lc < need_lc {
                        bad.push(format!("{name} {w}x{h}: {:?} {r:.1}:1 Lc {lc:.0} on {bg:?}", tb.text));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} texts under the contrast floor:\n{}", bad.len(), bad.join("\n"));
}

/// The smallest text: at 1280 × 800 a capital at least 9 px high and a line
/// (ascender to descender) at least 12 px; at 1920 × 1080 every text at
/// least 18 px of em (the Deck review's floor and aim, XAG 101).
#[test]
fn smallest_glyphs() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut bad = Vec::new();
    for name in checked_names() {
        for (w, h, cap_min, em_min) in [(1280, 800, 9.0, 0.0), (1920, 1080, 0.0, 18.0)] {
            let a = super::headless::named_app(&env, &fonts, &name, w, h, 1.0).unwrap();
            for tb in a.ui.widgets.iter().flat_map(|wd| &wd.texts) {
                if tb.text.trim().is_empty() {
                    continue;
                }
                // The text calls take the line box; the em is that over
                // the face's line box at 1 px of em.
                let em = tb.size / fonts.box_size(super::font::text(tb.face, 1.0, 0.0));
                let cap = fonts.cap_height(tb.face, em);
                let line = fonts.line_height(tb.face, tb.size);
                if cap < cap_min || line < 12.0 || em + 0.01 < em_min {
                    bad.push(format!("{name} {w}x{h}: {:?} em {em:.1} cap {cap:.1} line {line:.0}", tb.text));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} texts too small:\n{}", bad.len(), bad.join("\n"));
}

/// The Mods page's real calls on a sandboxed data folder: Overbright packs
/// listed and turned off (order.txt), a collision found by texture, map
/// mods listed and turned on (enabled.txt), "Use mods" kept in its file.
#[test]
fn mods_on_overbright_packs_and_map_mods() {
    use super::model::{Env, Paused, RealEnv};
    let dir = std::env::temp_dir().join(format!("ob-launcher-mods-{}", crate::unique_tag()));
    crate::frontend::profiles::sandbox::enter(&dir);
    let packs = crate::obpack::packs_dir();
    assert!(packs.starts_with(&dir), "sandboxed: {}", packs.display());
    let mut png = Vec::new();
    image::RgbaImage::new(4, 4).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    for (id, name) in [("night-roads", "Night Roads"), ("wet-roads", "Wet Roads")] {
        let mut mf = crate::obpack::Manifest::bare(id);
        mf.name = name.into();
        let p = crate::obpack::create(&packs, id, &mf).unwrap();
        crate::obpack::put_png(&p, crate::obpack::Domain::World, 0x1234_5678, "ROAD", &png).unwrap();
    }
    let env = RealEnv;
    let own = env.own_packs();
    assert_eq!(own.iter().map(|o| (o.id.as_str(), o.on, o.textures)).collect::<Vec<_>>(), [
        ("night-roads", true, 1),
        ("wet-roads", true, 1)
    ]);
    let c = env.conflict(&own, &[]).expect("both change ROAD");
    assert_eq!((c.first.as_str(), c.second.as_str(), c.textures, c.same_list), ("Night Roads", "Wet Roads", 1, true));
    env.save_own(&[("wet-roads".into(), true), ("night-roads".into(), false)]).unwrap();
    let own = env.own_packs();
    assert_eq!((own[0].id.as_str(), own[1].on), ("wet-roads", false), "the order and off kept");
    assert!(env.conflict(&own, &[]).is_none());
    // Map mods.
    let maps = crate::tools::map::store::maps_dir();
    let m = crate::tools::map::MapMod { name: "Drift park".into(), ..Default::default() };
    crate::tools::map::store::save(&maps, "drift-park", &m).unwrap();
    assert_eq!(env.maps(), [super::model::MapRow { folder: "drift-park".into(), name: "Drift park".into(), on: false, races: 0, props: 0, textures: 0 }]);
    env.set_map("drift-park", true).unwrap();
    assert!(env.maps()[0].on);
    // Its painted textures: counted on the row, exported, imported back
    // (asked first: a pack file without a record counts as derived).
    let tex = maps.join("drift-park").join(crate::tools::map::store::TEXTURES).join("world").join("1A2B3C4D.png");
    std::fs::create_dir_all(tex.parent().unwrap()).unwrap();
    std::fs::write(&tex, &png).unwrap();
    std::fs::create_dir_all(maps.join("drift-park").join("work")).unwrap();
    std::fs::write(maps.join("drift-park").join("work").join("paint.png"), &png).unwrap();
    assert_eq!(env.maps()[0].textures, 1);
    assert_eq!(env.map_derived("drift-park"), 1);
    let out = dir.join("exports");
    assert!(env.export_map("drift-park", &out, false).is_err(), "not without the yes");
    let zip = env.export_map("drift-park", &out, true).unwrap();
    let back = env.import_map(&zip).unwrap();
    assert_ne!(back, "drift-park");
    assert_eq!(std::fs::read(maps.join(&back).join("textures/world/1A2B3C4D.png")).unwrap(), png);
    assert!(!maps.join(&back).join("work").exists(), "work files stay home");
    assert_eq!(env.maps().iter().find(|r| r.folder == back).unwrap().textures, 1);
    crate::tools::map::share::files(&maps.join(&back)).unwrap();
    std::fs::remove_dir_all(maps.join(&back)).unwrap();
    // Use mods: off kept in its file until turned on again.
    assert!(env.paused().is_none());
    let p = Paused { maps: vec!["drift-park".into()], ..Default::default() };
    env.set_paused(Some(&p)).unwrap();
    assert_eq!(env.paused(), Some(p));
    env.set_paused(None).unwrap();
    assert!(env.paused().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Mods page's mod packs on a sandboxed data folder: a map mod exported as a pack file, imported (off, new),
/// switched on, moved, and "Use mods: Off" remembering it.
#[test]
fn mods_on_mod_packs() {
    use super::model::{Env, RealEnv};
    let dir = std::env::temp_dir().join(format!("ob-launcher-packs-{}", crate::unique_tag()));
    crate::frontend::profiles::sandbox::enter(&dir);
    let env = RealEnv;
    assert!(env.packs().is_empty(), "nothing installed: nothing listed");
    let maps = crate::tools::map::store::maps_dir();
    for f in ["docks", "cones"] {
        let m = crate::tools::map::MapMod { name: f.to_uppercase(), ..Default::default() };
        crate::tools::map::store::save(&maps, f, &m).unwrap();
    }
    let out = dir.join("exports");
    for f in ["docks", "cones"] {
        let file = env.export_pack(f, &out, false).unwrap();
        assert_eq!(file, out.join(format!("{f}-1.0.0.obpk")));
        assert_eq!(env.import_pack(&file).unwrap(), f);
    }
    assert!(env.import_pack(&out.join("docks-1.0.0.obpk")).unwrap_err().contains("already installed"));
    let rows = env.packs();
    assert_eq!(rows.iter().map(|r| (r.id.as_str(), r.on, r.new)).collect::<Vec<_>>(), [("cones", false, true), ("docks", false, true)]);
    env.set_mod_pack("docks", true).unwrap();
    env.set_mod_pack("cones", true).unwrap();
    assert_eq!(env.packs().iter().map(|r| (r.id.as_str(), r.on)).collect::<Vec<_>>(), [("docks", true), ("cones", true)]);
    env.move_pack("cones", -1).unwrap();
    assert_eq!(env.packs()[0].id, "cones");
    assert!(env.move_pack("cones", -1).is_err(), "not past the top");
    // order.txt: first line strongest, as the rows show it; the file is what moved.
    let mods = crate::mod_packs::mods_dir();
    let listed = |dir: &std::path::Path| -> Vec<String> {
        std::fs::read_to_string(dir.join("order.txt"))
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .map(|l| l.trim_start_matches('-').to_string())
            .collect()
    };
    assert_eq!(listed(&mods), ["cones", "docks"]);
    env.move_pack("cones", 1).unwrap();
    assert_eq!(env.packs().iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["docks", "cones"]);
    assert_eq!(listed(&mods), ["docks", "cones"]);
    assert!(env.move_pack("cones", 1).is_err(), "not past the bottom");
    assert!(env.move_pack("nothing", 1).is_err(), "not installed: not moved");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The value rows with the "suggested" tag in force, at every size of the
/// matrix, at 100 / 150 / 200 % display scale and 100 / 125 / 150 % text
/// size: every text and the tag inside the row, nothing under an arrow,
/// the tag clear of the value.
#[test]
fn value_rows_and_the_suggested_tag_never_clip() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    for (w, h) in SIZES {
        for dpi in [1.0, 1.5, 2.0] {
            for text in super::model::Prefs::TEXT_SIZES {
                let (screen, mut m) = super::headless::state_model("settings-suggested").unwrap();
                m.prefs.text = text;
                let a = App::new(&env, m, screen, (w, h), dpi, &fonts);
                let what = format!("settings-suggested {w}x{h} at {}% text {text}%", dpi * 100.0);
                let mut ui = a.ui.clone();
                ui.view.h = ui.content_h.max(ui.view.h);
                check_layout(&ui, &fonts, &what);
                let row = a.ui.get("graphics").unwrap();
                let Kind::ValueRow { left, right, tag } = &row.kind else { panic!("{what}: graphics is not a value row") };
                let tag = tag.unwrap_or_else(|| panic!("{what}: no tag on the suggested graphics"));
                assert!(tag.inside(&row.rect), "{what}: the tag {tag:?} outside its row {:?}", row.rect);
                assert!(!tag.overlaps(left) && !tag.overlaps(right), "{what}: the tag under an arrow");
                for wd in a.ui.widgets.iter().filter(|wd| matches!(wd.kind, Kind::ValueRow { .. })) {
                    let Kind::ValueRow { left, right, tag } = &wd.kind else { unreachable!() };
                    for t in &wd.texts {
                        assert!(t.rect.inside(&wd.rect), "{what}: {:?} outside its row", t.text);
                        assert!(!t.rect.overlaps(left) && !t.rect.overlaps(right), "{what}: {:?} under an arrow", t.text);
                        if t.text != "suggested"
                            && let Some(tg) = tag
                        {
                            assert!(!t.rect.overlaps(tg), "{what}: {:?} under the tag", t.text);
                        }
                    }
                }
            }
        }
    }
}

/// The shaders' block on the main screen: not prepared (Prepare now),
/// running (bar, percent, step, Stop), just ready; Play stays focused and
/// still plays; Stop and Prepare now do what they say.
#[test]
fn the_shaders_block() {
    use super::screens::{Shaders, shaders};
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = super::headless::state_app(&env, &fonts, "main-shaders", 1280, 720, 1.0).unwrap();
    assert_eq!(shaders(&a.model), Shaders::NotPrepared);
    assert!(all_text(&a).contains("Shaders aren't prepared yet") && a.ui.get("prepare_now").is_some());
    assert_eq!(focus(&a), "play", "Play keeps the focus");
    a.st.focus = Some("prepare_now");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.prepare.wanted && a.model.prepare.background);
    assert_eq!(focus(&a), "play");
    let mut a = super::headless::state_app(&env, &fonts, "main-prepare", 1280, 720, 1.0).unwrap();
    let t = all_text(&a);
    assert!(t.contains("Getting the shaders ready") && t.contains("40 % · step 3 of 5: shader programs"), "{t}");
    assert!(t.contains("left"), "the time left: {t}");
    assert!(a.ui.widgets.iter().any(|w| matches!(w.kind, Kind::Bar { .. })));
    assert_eq!(focus(&a), "play");
    assert_eq!(key(&mut a, &fonts, Key::Enter), Outcome::Play, "Play still plays");
    let mut a = super::headless::state_app(&env, &fonts, "main-prepare", 1280, 720, 1.0).unwrap();
    a.st.focus = Some("prepare_stop");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.prepare.stop && !a.model.prepare.background);
    let a = super::headless::state_app(&env, &fonts, "main-shaders-ready", 1280, 720, 1.0).unwrap();
    assert_eq!(shaders(&a.model), Shaders::JustReady);
    assert!(all_text(&a).contains("Shaders ready") && a.shaders_ready_shown());
    for (w, h) in SIZES {
        for name in ["main-shaders", "main-prepare", "main-shaders-ready"] {
            let a = super::headless::state_app(&env, &fonts, name, w, h, 1.0).unwrap();
            let mut ui = a.ui.clone();
            ui.view.h = ui.content_h.max(ui.view.h);
            check_layout(&ui, &fonts, &format!("{name} {w}x{h}"));
            let play = a.ui.get("play").unwrap().rect;
            assert!(play.bottom() <= h as f32 && a.st.scroll == 0.0, "{name} {w}x{h}: Play in view");
        }
    }
}

/// Storage: every folder with Change…, Open and Reset; a missing drive
/// says so and keeps Reset; a folder put elsewhere asks before copying;
/// portable mode asks before turning on.
#[test]
fn storage_page() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Advanced, 1280, 720, 1.0);
    a.st.focus = Some("storage");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Storage);
    for (c, o, r) in super::screens::LOC_IDS {
        assert!(a.ui.get(c).is_some() && a.ui.get(o).is_some() && a.ui.get(r).is_some(), "{c}");
    }
    for id in ["change", "game_open", "pack_browse", "hud_open", "pack_forget", "portable"] {
        assert!(a.ui.get(id).is_some(), "{id}");
    }
    let m = super::headless::state_app(&env, &fonts, "storage-missing", 1280, 720, 1.0).unwrap();
    assert!(all_text(&m).contains("its drive may be unplugged"));
    assert!(m.ui.get("loc_cache_reset").unwrap().enabled && !m.ui.get("loc_cache_open").unwrap().enabled);
    // Change… (no system dialog here: the own browser), Use this folder: the question.
    a.st.focus = Some("loc_screenshots_change");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!((a.screen, a.model.browse.purpose), (Screen::Browse, super::model::BrowseFor::Location));
    a.st.focus = Some("use");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::Storage);
    assert!(matches!(a.model.ask, Some(super::model::Dialog::MoveLocation(crate::platform::dirs::Location::Screenshots, _))));
    assert_eq!(focus(&a), "loc_copy");
    a.st.focus = Some("loc_empty");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.ask.is_none() && a.model.message.as_ref().is_some_and(|(k, _)| *k == Mark::Ok));
    // Portable mode asks first.
    a.st.focus = Some("portable");
    key(&mut a, &fonts, Key::Space);
    assert_eq!(a.model.ask, Some(super::model::Dialog::PortableOn));
    for (w, h) in SIZES {
        for name in ["storage", "storage-portable", "storage-custom", "storage-missing"] {
            let a = super::headless::named_app(&env, &fonts, name, w, h, 1.0).unwrap();
            let mut ui = a.ui.clone();
            ui.view.h = ui.content_h.max(ui.view.h);
            check_layout(&ui, &fonts, &format!("{name} {w}x{h}"));
        }
    }
}

/// The locations file: read and written; a folder put elsewhere is where
/// every user of `platform::dirs` looks; Reset puts it back; a copy is
/// checked and deletes nothing.
#[test]
fn locations_file_round_trip_and_copy() {
    use crate::platform::dirs::{self, Location};
    let dir = std::env::temp_dir().join(format!("ob-locations-{}", crate::unique_tag()));
    crate::frontend::profiles::sandbox::enter(&dir);
    let shots = dir.join("pictures");
    let maps = dir.join("maps-elsewhere");
    assert_eq!(dirs::screenshots(), dirs::app().join("Screenshots"));
    dirs::set_location(Location::Screenshots, Some(&shots)).unwrap();
    dirs::set_location(Location::Maps, Some(&maps)).unwrap();
    assert!(dirs::locations_file().starts_with(&dir));
    assert_eq!(dirs::screenshots(), shots);
    assert_eq!(crate::tools::map::store::maps_dir(), maps);
    let text = std::fs::read_to_string(dirs::locations_file()).unwrap();
    assert_eq!(dirs::parse_locations(&text), [(Location::Screenshots, shots.clone()), (Location::Maps, maps.clone())]);
    dirs::set_location(Location::Screenshots, None).unwrap();
    assert_eq!(dirs::screenshots(), dirs::app().join("Screenshots"));
    dirs::set_location(Location::Maps, None).unwrap();
    assert!(!dirs::locations_file().exists(), "no file once nothing is put elsewhere");
    // A copy: every file there with its size, the old ones kept.
    let from = dir.join("from");
    std::fs::create_dir_all(from.join("sub")).unwrap();
    std::fs::write(from.join("a.txt"), b"abc").unwrap();
    std::fs::write(from.join("sub/b.bin"), vec![7u8; 4096]).unwrap();
    let to = dir.join("to");
    let plan = super::transfer::location_plan(Location::Screenshots, &from, &to, None);
    let done = super::transfer::apply(&plan, super::transfer::Choice::Keep).unwrap();
    assert_eq!((done.copied, done.bytes), (2, 4099));
    assert_eq!(std::fs::read(to.join("sub/b.bin")).unwrap().len(), 4096);
    assert!(from.join("a.txt").is_file(), "nothing deleted");
    assert!(dirs::location_missing(std::path::Path::new("/nonexistent-drive-ob/folder")));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Studio's state on the Dev page, headless: not beside the launcher (the
/// button greyed, the Studio row says so), found and not open (the last
/// mod named), started here with a mod open (its lock and holder in the
/// State row's help), open elsewhere (another program's lock); the Mods
/// page's map mod rows say which mod is being edited in Studio. Every row
/// is reached by the keys (Down) and has its help.
#[test]
fn dev_page_studio_state() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let state = |n: &str| super::headless::state_app(&env, &fonts, n, 1280, 720, 1.0).unwrap();
    let missing = state("dev-studio-missing");
    let t = all_text(&missing);
    assert!(!missing.ui.get("studio").unwrap().enabled, "greyed without Studio's program");
    assert!(t.contains("Not beside the launcher") && t.contains("None yet"), "{t}");
    assert!(t.contains("(overbright-studio) is not beside the launcher"), "the State row's help says why: {t}");
    let idle = state("dev-studio-idle");
    let t = all_text(&idle);
    assert!(idle.ui.get("studio").unwrap().enabled);
    assert!(t.contains(&format!("Studio {}", idle.model.version)) && t.contains("Ready, not open"), "{t}");
    assert!(t.contains("Drift park") && t.contains("No mod is open for writing now"), "{t}");
    let run = state("dev-studio-running");
    let t = all_text(&run);
    assert!(!run.ui.get("studio").unwrap().enabled, "open: greyed until it closes");
    assert!(t.contains("Open on Drift park"), "{t}");
    assert!(t.contains("Drift park is open for writing in Overbright Studio (process 4242) since"), "{t}");
    let away = state("dev-studio-elsewhere");
    let t = all_text(&away);
    assert!(away.ui.get("studio").unwrap().enabled, "a Studio not started here leaves the button usable");
    assert!(t.contains("Open elsewhere on Open docks"), "{t}");
    assert!(t.contains("Drift park is open for writing in Studio's command-line tool (process 4242)"), "{t}");
    // The keys reach every Studio row, in order, each with its help.
    let mut a = state("dev-studio-idle");
    a.st.focus = Some("studio");
    a.rebuild(&fonts);
    for (id, want) in [("studio_state", "holds a mod's lock"), ("studio_last", "one list")] {
        key(&mut a, &fonts, Key::Down);
        assert_eq!(focus(&a), id);
        assert!(all_text(&a).contains(want), "{id}: {}", all_text(&a));
    }
    key(&mut a, &fonts, Key::Down);
    assert_eq!(focus(&a), "studio_cli");
    // The Mods page: the mod Studio has open is marked; others are not.
    let (_, mut m) = super::headless::state_model("dev-studio-running").unwrap();
    m.studio = super::model::EditorRun::Closed;
    let mut mods = App::new(&env, m, Screen::Mods, (1280, 720), 1.0, &fonts);
    mods.rebuild(&fonts);
    let t = all_text(&mods);
    assert!(t.contains("2 races · 9 prop edits · being edited in Studio"), "{t}");
    assert_eq!(t.matches("being edited").count(), 1, "{t}");
    for (w, h) in SIZES {
        for dpi in [1.0, 1.5] {
            for n in ["dev-studio-missing", "dev-studio-running", "dev-studio-elsewhere"] {
                let a = super::headless::state_app(&env, &fonts, n, w, h, dpi).unwrap();
                let mut ui = a.ui.clone();
                ui.view.h = ui.content_h.max(ui.view.h);
                check_layout(&ui, &fonts, &format!("{n} {w}x{h} at {dpi}"));
            }
        }
    }
}

/// The Dev page: fourth in the page bar (LB / RB go round all four); the
/// map editor's button (preview); the game with the editor tools once; the
/// support file and the logs.
#[test]
fn dev_page() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Mods, 1280, 720, 1.0);
    key(&mut a, &fonts, Key::PageNext);
    assert_eq!(a.screen, Screen::Saves);
    key(&mut a, &fonts, Key::PageNext);
    assert_eq!(a.screen, Screen::Dev);
    key(&mut a, &fonts, Key::PageNext);
    assert_eq!(a.screen, Screen::Main, "round to the first page");
    key(&mut a, &fonts, Key::PagePrev);
    assert_eq!(a.screen, Screen::Dev);
    assert!(a.ui.get("map_editor").unwrap().enabled, "the editor is beside the launcher");
    assert!(all_text(&a).contains("Open map editor (preview)"), "{}", all_text(&a));
    let mut gone = app(&env, &fonts, Screen::Dev, 1280, 720, 1.0);
    gone.model.editor_here = false;
    gone.rebuild(&fonts);
    assert!(!gone.ui.get("map_editor").unwrap().enabled, "greyed without the editor's program");
    assert!(all_text(&gone).contains("is not beside the launcher"), "the reason is shown");
    assert!(!all_text(&a).contains("is not beside the launcher"));
    // Studio: its button, its greyed state without its program, its open
    // state, and the command-line tool's row with its help.
    assert!(a.ui.get("studio").unwrap().enabled, "Studio is beside the launcher");
    assert!(all_text(&a).to_lowercase().contains("standalone map editor"), "{}", all_text(&a));
    let mut st = app(&env, &fonts, Screen::Dev, 1280, 720, 1.0);
    st.model.studio_here = false;
    st.rebuild(&fonts);
    assert!(!st.ui.get("studio").unwrap().enabled, "greyed without Studio's program");
    assert!(all_text(&st).contains("Not beside the launcher"), "{}", all_text(&st));
    st.model.studio_here = true;
    st.rebuild(&fonts);
    st.st.focus = Some("studio");
    key(&mut st, &fonts, Key::Enter);
    assert!(st.studio_job.is_some() && !st.ui.get("studio").unwrap().enabled, "open: greyed until it closes");
    assert!(all_text(&st).contains("Open, no mod open yet"), "{}", all_text(&st));
    for (id, want) in [("studio", "World"), ("studio_cli", "overbright-studio-cli"), ("map_editor", "without Studio")] {
        st.st.focus = Some(id);
        st.rebuild(&fonts);
        assert!(all_text(&st).contains(want), "{id}: {}", all_text(&st));
    }
    let t = all_text(&a);
    assert!(t.contains("F7: the texture editor") && t.contains("F8: the free camera") && t.contains("F3"), "{t}");
    assert_eq!(focus(&a), "studio", "the page leads to Studio");
    a.st.focus = Some("editor_play");
    assert_eq!(key(&mut a, &fonts, Key::Enter), Outcome::Play);
    assert_eq!(a.model.play_args, ["--editor"]);
    a.st.focus = Some("support");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.support.is_some());
    let adv = app(&env, &fonts, Screen::Advanced, 1280, 720, 1.0);
    assert!(adv.ui.get("support").is_none(), "Advanced stays lean");
    for (w, h) in SIZES {
        for dpi in [1.0, 1.5] {
            let a = app(&env, &fonts, Screen::Dev, w, h, dpi);
            let mut ui = a.ui.clone();
            ui.view.h = ui.content_h.max(ui.view.h);
            check_layout(&ui, &fonts, &format!("dev {w}x{h} at {dpi}"));
        }
    }
}

/// Portable mode switched in Storage: the footer and the page say at once
/// what the next start brings, with Restart now focused; after a restart
/// (a new launcher, the marker as left) the footer is plain again.
#[test]
fn portable_switch_shows_its_state_at_once() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let footer = |a: &App| {
        a.ui.widgets
            .iter()
            .flat_map(|w| &w.texts)
            .find(|t| t.text.starts_with(&a.model.version))
            .map(|t| t.text.clone())
            .unwrap_or_default()
    };
    // On.
    let mut a = app(&env, &fonts, Screen::Storage, 1280, 720, 1.0);
    assert_eq!(footer(&a), a.model.version, "the version alone, no name");
    a.st.focus = Some("portable");
    key(&mut a, &fonts, Key::Space);
    a.st.focus = Some("port_on");
    key(&mut a, &fonts, Key::Enter);
    assert!(footer(&a).ends_with("· Portable after restart"), "{}", footer(&a));
    assert_eq!(focus(&a), "restart");
    assert!(all_text(&a).contains("Portable mode: on after a restart"));
    assert_eq!(key(&mut a, &fonts, Key::Enter), Outcome::Restart);
    // After the restart: in force.
    let mut m = env.load();
    m.portable.on = true;
    m.portable.marker = true;
    m.folders.portable = true;
    let mut b = App::new(&env, m, Screen::Storage, (1280, 720), 1.0, &fonts);
    assert!(footer(&b).ends_with("· Portable") && b.ui.get("restart").is_none(), "{}", footer(&b));
    // Off.
    b.st.focus = Some("portable");
    key(&mut b, &fonts, Key::Space);
    assert_eq!(b.model.ask, Some(super::model::Dialog::PortableOff));
    b.st.focus = Some("port_off");
    key(&mut b, &fonts, Key::Enter);
    assert!(footer(&b).ends_with("· Portable · off after restart"), "{}", footer(&b));
    assert_eq!(focus(&b), "restart");
}

/// The map editor: "Open in map editor" (Mods page) starts it on the
/// chosen mod, the Dev page's button on its own Open / New; the page waits while it
/// is open (one at a time), reads the map mods again when it closes, and
/// says why when it could not start or ended badly.
#[test]
fn mods_page_starts_the_map_editor() {
    use super::model::{EditorRun, FakeEditor};
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Mods, 1280, 720, 1.0);
    let t = all_text(&a);
    assert!(t.contains("2 races · 9 prop edits") && t.contains("14 prop edits · 5 textures"), "{t}");
    a.model.mods.cur = Some("mp1");
    a.rebuild(&fonts);
    assert!(a.ui.get("map_open").unwrap().enabled);
    a.model.mods.map_sel = Some("open-docks".into());
    a.event(&Event::Activate("map_open"));
    a.rebuild(&fonts);
    assert_eq!(a.model.mods.editor, EditorRun::Open(Some("open-docks".into())));
    for id in ["map_open", "pack_import", "map_export"] {
        assert!(!a.ui.get(id).unwrap().enabled, "{id} waits while the editor is open");
    }
    assert!(all_text(&a).contains("The map editor is open on Open docks"), "{}", all_text(&a));
    // A second start is not made (the Dev page's button is greyed too).
    a.screen = Screen::Dev;
    a.rebuild(&fonts);
    assert!(!a.ui.get("map_editor").unwrap().enabled);
    a.event(&Event::Activate("map_editor"));
    a.screen = Screen::Mods;
    a.rebuild(&fonts);
    assert_eq!(a.model.mods.editor, EditorRun::Open(Some("open-docks".into())));
    // It closes: the page is usable again.
    let end = std::rc::Rc::new(std::cell::Cell::new(None));
    a.editor_job = Some(Box::new(FakeEditor(end.clone())));
    assert!(!a.poll_jobs(&fonts), "still open");
    end.set(Some(Ok(())));
    assert!(a.poll_jobs(&fonts));
    assert!(a.editor_job.is_none() && a.model.mods.editor == EditorRun::Closed);
    assert!(a.ui.get("map_open").unwrap().enabled);
    // The editor on its own choice (Dev page), ending badly.
    a.screen = Screen::Dev;
    a.rebuild(&fonts);
    a.event(&Event::Activate("map_editor"));
    assert_eq!(a.model.mods.editor, EditorRun::Open(None));
    let end = std::rc::Rc::new(std::cell::Cell::new(Some(Err("the map editor ended with exit status: 101".into()))));
    a.editor_job = Some(Box::new(FakeEditor(end)));
    assert!(a.poll_jobs(&fonts));
    assert!(all_text(&a).contains("The map editor: the map editor ended with exit status: 101"), "{}", all_text(&a));
    a.rebuild(&fonts);
    assert!(a.ui.get("map_editor").unwrap().enabled, "it can be started again");
}

/// The real start: `overbright-editor --mod <folder>` beside the launcher
/// (here a stand-in named by OB_EDITOR_EXE), with the game folder shown as
/// NFSU2_ROOT; a missing editor is an error, not a hang.
#[cfg(unix)]
#[test]
fn the_map_editor_starts_with_the_mod_and_the_game_folder() {
    use super::model::{Env, RealEnv};
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("ob-launcher-editor-{}", crate::unique_tag()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join("overbright-editor");
    std::fs::write(&exe, "#!/bin/sh\necho \"$* root=$NFSU2_ROOT\" > \"$(dirname \"$0\")/args\"\n").unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    // SAFETY: only the editor's start reads this variable.
    unsafe { std::env::set_var("OB_EDITOR_EXE", &exe) };
    assert_eq!(super::dirs::editor_exe(), exe);
    let mut job = RealEnv.start_editor(Some("drift-park"), Some(std::path::Path::new("/games/nfsu2"))).unwrap();
    let t0 = std::time::Instant::now();
    let end = loop {
        if let Some(e) = job.poll() {
            break e;
        }
        assert!(t0.elapsed().as_secs() < 20);
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(end, Ok(()));
    assert_eq!(std::fs::read_to_string(dir.join("args")).unwrap(), "--mod drift-park root=/games/nfsu2\n");
    unsafe { std::env::set_var("OB_EDITOR_EXE", dir.join("missing")) };
    assert!(RealEnv.start_editor(None, None).err().unwrap().contains("could not be started"));
    unsafe { std::env::remove_var("OB_EDITOR_EXE") };
    let _ = std::fs::remove_dir_all(&dir);
}

/// Portable mode switched off with "copy my files back" into a home
/// folder that already has its own settings: the question lists them as
/// different and kept (no replace offered for settings), and after the
/// copy they are byte for byte what they were; the portable copies sit
/// beside them with their kept paths made absolute, and the saves go to
/// the data home's profiles folder.
#[cfg(unix)]
#[test]
fn portable_off_keeps_the_home_settings_byte_identical() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let dir = std::env::temp_dir().join(format!("ob-portable-off-{}", crate::unique_tag()));
    let prog = dir.join("Games/overbright");
    let data = prog.join("data");
    let share = dir.join("home/.local/share");
    let home = share.join("overbright");
    let put = |p: &std::path::Path, b: &[u8]| {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b).unwrap();
    };
    put(&data.join("game-root.txt"), b"../Need for Speed Underground 2\n");
    put(&data.join("options.txt"), b"portable options\n");
    put(&data.join("launcher.txt"), b"text 150\n");
    put(&data.join("NFS Underground 2/ABC/ABC"), b"save");
    put(&data.join("maps/m/map.obmap"), b"map");
    put(&data.join("cache/big.bin"), b"cache");
    put(&data.join("launcher.lock"), b"");
    let mine: [(&str, &[u8]); 3] = [
        ("game-root.txt", b"/media/games/Need for Speed Underground 2\n"),
        ("options.txt", b"my options\n"),
        ("launcher.txt", b"text 100\n"),
    ];
    for (n, b) in mine {
        put(&home.join(n), b);
    }
    let mut a = app(&env, &fonts, Screen::Storage, 1280, 720, 1.0);
    a.model.portable = super::model::PortableInfo {
        on: true,
        marker: true,
        changed: false,
        program: Some(prog.clone()),
        data: Some(data.clone()),
        home: home.clone(),
        home_saves: share.clone(),
        writable: true,
    };
    a.rebuild(&fonts);
    a.st.focus = Some("portable");
    key(&mut a, &fonts, Key::Space);
    assert_eq!(a.model.ask, Some(super::model::Dialog::PortableOff));
    let t = all_text(&a);
    assert!(t.contains("kept as they are: game-root.txt, launcher.txt, options.txt"), "{t}");
    assert!(
        t.contains("To copy: 2 files") && t.contains("maps (1 file)") && t.contains("NFS Underground 2 (1 file)"),
        "{t}"
    );
    assert!(a.ui.get("port_copy_back_replace").is_none(), "settings are never replaced: no replace offered");
    assert_eq!(focus(&a), "port_copy_back", "the safe copy is the default");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.message.as_ref().is_some_and(|(k, _)| *k == Mark::Ok), "{:?}", a.model.message);
    for (n, b) in mine {
        assert_eq!(std::fs::read(home.join(n)).unwrap(), b, "{n} untouched");
    }
    assert_eq!(
        std::fs::read_to_string(home.join("game-root.txt.from-portable")).unwrap(),
        format!("{}\n", dir.join("Games/Need for Speed Underground 2").display()),
        "the portable copy beside it, its path absolute"
    );
    assert_eq!(std::fs::read(home.join("options.txt.from-portable")).unwrap(), b"portable options\n");
    assert_eq!(std::fs::read(share.join("NFS Underground 2/ABC/ABC")).unwrap(), b"save");
    assert_eq!(std::fs::read(home.join("maps/m/map.obmap")).unwrap(), b"map");
    assert!(
        !home.join("cache").exists()
            && !home.join("NFS Underground 2").exists()
            && !home.join("launcher.lock").exists()
    );
    assert!(data.join("options.txt").is_file(), "nothing deleted");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The movies on the check screen: missing movies alone make "Ready, some
/// movies won't play" (not "changed game files"), the details open and
/// close with their button, and the summary's counts are the asset
/// report's groups (the same report as `--check-game`).
#[test]
fn check_screen_lists_the_movies() {
    use super::model::{Verdict, check_result, fake_assets};
    use crate::qol::setup;
    let fonts = Fonts::new();
    let env = FakeEnv;
    let root = FakeEnv::root();
    let report = fake_assets(&root, &["Scene05", "DriftTut"]);
    let c = setup::Check { assets: Some(report.clone()), ..Default::default() };
    let r = check_result(&root, &c, None);
    assert_eq!(r.verdict(), Verdict::Notes, "{r:?}");
    assert_eq!(r.state(), super::check::CheckState::MoviesMissing, "{r:?}");
    let s = r.summary.as_ref().unwrap();
    // The summary's counts are the report's groups.
    for (g, (_, line)) in s.groups.iter().zip(report.groups()) {
        let want = if g.missing.is_empty() {
            format!("{}: all {} found", g.group.words(), g.total)
        } else {
            format!("{}: {} of {} found; missing: {}", g.group.words(), g.found, g.total, g.missing.join(", "))
        };
        assert!(line.starts_with(&want), "{line} / {want}");
    }
    assert_eq!(s.groups.len(), report.groups().len());
    assert_eq!(s.movies_missing(), 2);
    let fine = setup::Check { assets: Some(fake_assets(&root, &[])), ..Default::default() };
    assert_eq!(check_result(&root, &fine, None).verdict(), Verdict::Ready);
    for (w, h) in SIZES {
        let a = super::headless::state_app(&env, &fonts, "check-details", w, h, 1.0).unwrap();
        let t = all_text(&a);
        assert!(t.contains("some movies won't play") && t.contains("Story movies"), "{w}x{h}: {t}");
        let mut ui = a.ui.clone();
        ui.view.h = ui.content_h.max(ui.view.h);
        check_layout(&ui, &fonts, &format!("check-details {w}x{h}"));
    }
    let mut a = super::headless::state_app(&env, &fonts, "check-movies-missing", 1280, 720, 1.0).unwrap();
    assert!(!all_text(&a).contains("Story movies"), "the details are closed at first");
    assert!(a.ui.get("details").is_some());
    click(&mut a, &fonts, "details");
    assert!(all_text(&a).contains("Story movies") && all_text(&a).contains("Hide details"));
    assert_eq!(focus(&a), "details_list", "the list takes the focus");
    key(&mut a, &fonts, Key::ActionY);
    assert!(!all_text(&a).contains("Story movies"), "Y / R closes them again");
}

/// Each state of the game check: its title, the focused button (only
/// the error states and the unrecognised copy move it off Use), no
/// engineer's words, and the details by pad at the smallest size.
#[test]
fn check_states_words_and_focus() {
    use super::check;
    let fonts = Fonts::new();
    let env = FakeEnv;
    let states = [
        ("check-ok", check::READY, "use"),
        ("check-movies-missing", check::MOVIES_MISSING, "use"),
        ("check-details", check::MOVIES_MISSING, "use"),
        ("check-changed", check::CHANGED, "use"),
        ("check-unknown-edition", check::UNKNOWN, "another"),
        ("check-no-english", "The game's text file is missing", "another"),
        ("err-missing", "Some game files are missing", "recheck"),
    ];
    // Words a player cannot act on: they stay in `--check-game` and the
    // support file.
    let banned = ["layout", "suffix", "SPEED2.EXE", "fallback", "known release", "edition's names", "_fr", "vp6"];
    let hex = |t: &str| t.split(|c: char| !c.is_ascii_hexdigit()).any(|w| w.len() >= 16);
    for (state, title, focused) in states {
        for (w, h) in SIZES {
            let mut a = super::headless::state_app(&env, &fonts, state, w, h, 1.0).unwrap();
            let t = all_text(&a);
            assert!(t.contains(title), "{state} {w}x{h}: {t}");
            assert_eq!(focus(&a), focused, "{state} {w}x{h}");
            if state.starts_with("check-movies") || state == "check-details" {
                assert!(!t.contains("discs"), "{state}: no advice to copy from discs");
            }
            assert!(!t.contains("English for now"), "{state}: no promise");
            if a.ui.get("details").is_some() && !a.model.show_details {
                click(&mut a, &fonts, "details");
            }
            let t = all_text(&a);
            for b in banned {
                assert!(!t.contains(b), "{state} {w}x{h}: shows {b:?}: {t}");
            }
            assert!(!hex(&t), "{state}: a hash on the screen: {t}");
        }
    }
    // The unrecognised copy is asked about: Use it anyway, not by default.
    let a = super::headless::state_app(&env, &fonts, "check-unknown-edition", 1280, 720, 1.0).unwrap();
    assert!(a.ui.get("use").is_some() && a.ui.get("support").is_some());
    // The text file is named; the copy's language is said, and it is one
    // the game cannot show.
    let a = super::headless::state_app(&env, &fonts, "check-no-english", 1280, 720, 1.0).unwrap();
    let t = all_text(&a);
    assert!(t.contains("LANGUAGES/English.bin") && t.contains("Korean") && t.contains("isn't supported"), "{t}");
    assert!(a.ui.get("use").is_none(), "the game does not start without it");
    // The details list scrolls to its last row by pad at the smallest size.
    let mut a = super::headless::state_app(&env, &fonts, "check-details", 640, 480, 1.0).unwrap();
    a.st.focus = Some("details_list");
    let n = a.model.check.as_ref().unwrap().summary.as_ref().unwrap().details().len();
    for _ in 0..n + 2 {
        key(&mut a, &fonts, Key::Down);
    }
    assert_eq!(a.model.details_sel, n - 1, "the last row reached");
    let last = a.model.check.as_ref().unwrap().summary.as_ref().unwrap().details()[n - 1].1.clone();
    assert!(all_text(&a).contains(&last), "the last row is shown: {last}");
}

/// A summary row opens the details at its area; the main screen keeps
/// the summary behind a closed drop-down.
#[test]
fn summary_rows_open_their_area() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = super::headless::state_app(&env, &fonts, "check-movies-missing", 1280, 720, 1.0).unwrap();
    click(&mut a, &fonts, "sum_movies");
    assert!(a.model.show_details);
    let s = a.model.check.as_ref().unwrap().summary.clone().unwrap();
    assert!(s.details()[a.model.details_sel].1.starts_with("Start movies"), "{:?}", s.details());
    let mut a = app(&env, &fonts, Screen::Main, 1280, 720, 1.0);
    assert!(!all_text(&a).contains("Game files"), "closed by default");
    click(&mut a, &fonts, "details");
    let t = all_text(&a);
    assert!(t.contains("Game files") && t.contains("Movies") && t.contains("Edition"), "{t}");
}

/// The check's summary: the known release, a changed one (a mod: some
/// files still the known release's) and one not recognised (none are);
/// the text file alone missing is its own refusal.
#[test]
fn check_summary_release_and_text() {
    use super::check::{CheckState, Release};
    let ok = super::headless::fake_check(&[], &[], &[], false);
    assert_eq!(ok.summary.as_ref().unwrap().release, Release::Known);
    assert_eq!(ok.state(), CheckState::Ready);
    let changed = super::headless::fake_check(&[], &["GLOBAL/GlobalB.lzc"], &[], false);
    assert_eq!(changed.summary.as_ref().unwrap().release, Release::Changed(vec!["GLOBAL/GlobalB.lzc".into()]));
    assert_eq!(changed.state(), CheckState::Changed);
    let unknown = super::headless::fake_check(&[], &[], &[], true);
    assert_eq!(unknown.state(), CheckState::UnknownEdition);
    assert!(unknown.usable, "asked about, not refused");
    // A copy with a language's text that is not supported only: the game
    // folder's look, no text file the game can use.
    let dir = std::env::temp_dir().join(format!("ob-check-text-{}", std::process::id()));
    for d in ["GLOBAL", "FRONTEND", "TRACKS", "LANGUAGES"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    for f in ["GLOBAL/GlobalB.lzc", "FRONTEND/FrontB.lzc", "LANGUAGES/Korean.bin"] {
        std::fs::write(dir.join(f), b"").unwrap();
    }
    let c = crate::qol::setup::Check {
        missing: vec!["LANGUAGES/English.bin".into()],
        different: Vec::new(),
        assets: Some(crate::game_assets::scan(&dir)),
        ..Default::default()
    };
    let r = super::model::check_result(&dir, &c, None);
    assert_eq!(r.problem, Some(super::model::Problem::NoText), "{r:?}");
    assert_eq!(r.issue(), Some(super::model::Issue::NoText { others: vec!["Korean".into()] }));
    let _ = std::fs::remove_dir_all(&dir);
}

/// docs/troubleshooting.md's game check table names the screen's titles.
#[test]
fn troubleshooting_table_matches_the_check_screen() {
    use super::check;
    // Read at run time (the public tree's docs, not built into the test).
    let doc = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs").join("troubleshooting.md"))
        .expect("docs/troubleshooting.md");
    let at = doc.find("## What the game check says").expect("the game check section");
    let section = &doc[at..];
    let section = &section[..section[3..].find("\n## ").map_or(section.len(), |i| i + 3)];
    let titles: Vec<String> = section
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| What") && !l.starts_with("|---"))
        .map(|l| l.trim_start_matches("| ").split(" |").next().unwrap().trim().to_string())
        .collect();
    let want: Vec<String> = [
        check::READY.to_string(),
        check::MOVIES_MISSING.to_string(),
        check::MENUS_MISSING.to_string(),
        check::CHANGED.to_string(),
        check::UNKNOWN.to_string(),
        super::model::Issue::NoText { others: Vec::new() }.words().title,
        super::model::Issue::Missing { files: Vec::new() }.words().title,
        super::model::Issue::NoGame.words().title,
        super::model::Issue::OwnFolder.words().title,
        super::model::Issue::Unreadable.words().title,
    ]
    .to_vec();
    assert_eq!(titles, want, "the table's first column, in the screen's order");
}

/// Export… of a map mod with textures from the original game's pixels asks
/// first ("Made from the original game"); Cancel exports nothing, Export
/// anyway exports.
#[test]
fn map_export_asks_before_derived_textures() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Mods, 1280, 720, 1.0);
    a.model.mods.map_sel = Some("open-docks".into());
    a.model.browse.purpose = super::model::BrowseFor::MapExport;
    a.use_folder(std::path::PathBuf::from("/exports"));
    a.rebuild(&fonts);
    assert!(matches!(&a.model.ask, Some(super::model::Dialog::MapExportDerived(f, _, 5)) if f == "open-docks"));
    let t = all_text(&a);
    assert!(t.contains("Made from the original game") && t.contains("5 textures are made from"), "{t}");
    a.event(&Event::Activate("map_export_cancel"));
    assert!(a.model.ask.is_none() && a.model.mods.results.is_empty(), "nothing exported");
    a.use_folder(std::path::PathBuf::from("/exports"));
    a.event(&Event::Activate("map_export_yes"));
    assert_eq!(a.model.mods.results[0].1, "Exported: /exports/open-docks.zip");
    // A mod without such textures exports straight away.
    a.model.mods.map_sel = Some("drift-park".into());
    a.use_folder(std::path::PathBuf::from("/exports"));
    assert!(a.model.ask.is_none());
    assert_eq!(a.model.mods.results[0].1, "Exported: /exports/drift-park.zip");
}

/// The own copy (`game_data`): offered after the game check with Make copy chosen, made in the background
/// while the launcher stays usable, then shown as ready in Settings › Advanced.
#[test]
fn game_data_flow() {
    use super::game_data::{State, Status};
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Check, 960, 720, 1.0);
    assert_eq!(focus(&a), "use");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.screen, Screen::GameData);
    assert_eq!(a.model.game_data.state, State::Offer);
    assert_eq!(a.model.game_data.size, 1_801_794_145);
    // One confirm starts it; Stop is then the focus.
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.game_data.running());
    assert!(a.game_data_busy());
    assert_eq!(focus(&a), "stop");
    // Back leaves it running behind the main screen, whose state line says so.
    key(&mut a, &fonts, Key::Escape);
    assert_eq!(a.screen, Screen::Main);
    assert!(super::screens::game_state(&a.model).1.contains("making your own copy"));
    assert!(a.poll_jobs(&fonts));
    assert!(!a.game_data_busy());
    assert!(matches!(a.model.game_data.status, Status::Ready { known: true, .. }));
    assert!(matches!(a.model.game_data.state, State::Done { known: true, .. }));
    // Settings › Advanced › Game data: ready, Check copy offered; Back returns to Advanced.
    a.go(Screen::Advanced);
    a.rebuild(&fonts);
    let row = a.ui.widgets.iter().flat_map(|w| &w.texts).any(|t| t.text == "Your own copy: ready");
    assert!(row, "the Advanced row says the copy is ready");
    a.event(&super::ui::Event::Activate("game_data"));
    a.rebuild(&fonts);
    assert_eq!(a.screen, Screen::GameData);
    assert_eq!(focus(&a), "continue");
    key(&mut a, &fonts, Key::Escape);
    assert_eq!(a.screen, Screen::Advanced);
    assert_eq!(focus(&a), "game_data");
    // With a copy there, the first-run step is not offered again.
    assert!(!a.offer_game_data());
}

/// Stop, then a failure: Try again runs the same again; a damaged copy offers Repair.
#[test]
fn game_data_stop_fail_and_repair() {
    use super::game_data::{End, Job, Kind, Msg, State, Status};
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Main, 960, 720, 1.0);
    assert!(a.offer_game_data(), "no copy: offered");
    a.rebuild(&fonts);
    key(&mut a, &fonts, Key::Enter);
    a.game_data_job = Some(Job::scripted(vec![Msg::End(End::Stopped { done: 5, of: 10 })]));
    a.event(&super::ui::Event::Activate("stop"));
    assert!(a.model.game_data.stopping);
    a.poll_jobs(&fonts);
    assert_eq!(a.model.game_data.state, State::Stopped { done: 5, of: 10 });
    assert_eq!(a.model.game_data.status, Status::Partial);
    assert_eq!(focus(&a), "import", "Go on");
    a.event(&super::ui::Event::Activate("import"));
    a.game_data_job = Some(Job::scripted(vec![Msg::End(End::Failed("A file could not be read".into()))]));
    a.poll_jobs(&fonts);
    assert!(matches!(a.model.game_data.state, State::Failed(_)));
    assert_eq!(focus(&a), "retry");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.game_data.running() && a.model.game_data.last == Kind::Import);
    a.game_data_job = Some(Job::scripted(vec![Msg::End(End::NoSpace { need: 2, free: 1, place: "/home".into() })]));
    a.poll_jobs(&fonts);
    assert!(matches!(a.model.game_data.state, State::NoSpace { .. }));
    // A damaged copy: Repair chosen.
    a.model.game_data.status = Status::Damaged(2);
    a.open_game_data(true);
    a.rebuild(&fonts);
    assert_eq!(a.model.game_data.state, State::Damaged(2));
    assert_eq!(focus(&a), "repair");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.game_data.last, Kind::Repair);
}

/// The crash report window: the reason in words, the system's record and the
/// run's logs on the failure screen, "Copy for a bug report" puts the
/// redacted text on the clipboard; a failure without a run (not started) and
/// every other state show none of it.
#[test]
fn the_crash_report_window() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    for (state, word) in
        [("crash-panic", "internal error"), ("crash-signal", "segmentation fault"), ("crash-windows", "access violation")]
    {
        let mut a = super::headless::state_app(&env, &fonts, state, 1280, 720, 1.0).unwrap();
        let t = all_text(&a);
        assert!(t.contains(word) && t.contains("Logs of this run"), "{state}: {t}");
        assert!(a.ui.get("copy_report").is_some() && a.ui.get("open_logs").is_some(), "{state}");
        assert!(!t.contains("Asking the system"), "{state}");
        super::model::COPIED.with(|c| *c.borrow_mut() = None);
        click(&mut a, &fonts, "copy_report");
        let copied = super::model::COPIED.with(|c| c.borrow().clone()).expect("the clipboard got the text");
        assert!(copied.contains("Overbright crash report") && copied.contains(word), "{state}: {copied}");
        assert!(copied.contains("===== system errors =====") && copied.contains("not sent anywhere"));
        assert!(all_text(&a).contains("Copied"), "{state}");
        assert!(a.ui.get("copy_report").is_some());
    }
    // No run, no window: the earlier states are unchanged.
    for state in ["start-failed", "no-game-window", "failure-details"] {
        let a = super::headless::state_app(&env, &fonts, state, 1280, 720, 1.0).unwrap();
        assert!(a.ui.get("copy_report").is_none() && !all_text(&a).contains("Logs of this run"), "{state}");
    }
    // The system's record is asked for while it is read.
    let mut a = super::headless::state_app(&env, &fonts, "crash-signal", 1280, 720, 1.0).unwrap();
    a.model.failure.as_mut().unwrap().report.as_mut().unwrap().asking = true;
    a.rebuild(&fonts);
    assert!(all_text(&a).contains("Asking the system"));
    // A clean end never reaches the screen: the run's verdict.
    let clean = super::run::Ended {
        code: Some(0),
        signal: None,
        pid: 1,
        started_at: std::time::SystemTime::UNIX_EPOCH,
        secs: 600.0,
        panicked: false,
        no_window: false,
        named: None,
        tail: Vec::new(),
        log: None,
    };
    assert!(!clean.failed());
}

/// The Mods page's "Look inside" shows the lines about the chosen pack and closes again; "Try it once" starts the game
/// with that pack as the run's trial and asks for nothing else; the copy that a trial leaves is kept or discarded.
#[test]
fn mods_look_inside_try_it_once_and_the_saves_copy() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Mods, 1280, 720, 1.0);
    // Nothing chosen yet: nothing opens and nothing starts.
    a.event(&Event::Activate("pack_inside"));
    assert!(a.model.mods.inside.is_none());
    assert_ne!(a.event(&Event::Activate("pack_try")), Outcome::Play);
    a.model.mods.pack_sel = Some("cone-pack".into());
    a.rebuild(&fonts);
    a.event(&Event::Activate("pack_inside"));
    let (name, lines) = a.model.mods.inside.clone().unwrap();
    assert_eq!(name, "Cone pack");
    assert!(lines.iter().any(|l| l.starts_with("Changes")));
    a.rebuild(&fonts);
    assert!(a.ui.get("inside_close").is_some());
    a.event(&Event::Activate("inside_close"));
    assert!(a.model.mods.inside.is_none());
    // Try it once: the pack is named for the run; order.txt (the fake env has none) is not asked about.
    assert_eq!(a.event(&Event::Activate("pack_try")), Outcome::Play);
    assert_eq!(a.model.mods.try_pack.as_deref(), Some("cone-pack"));
    // After a trial: the copy is offered, Keep leaves it, Discard refuses a folder that is not a trial copy.
    let tries = super::trial::tries_dir();
    let t = super::trial::Trial { pack: "cone-pack".into(), dir: tries.join("cone-pack-x") };
    a.model.mods.try_pack = None;
    a.model.mods.tried = Some(("Cone pack".into(), t.clone()));
    a.rebuild(&fonts);
    assert!(a.ui.get("try_keep").is_some() && a.ui.get("try_discard").is_some());
    a.event(&Event::Activate("try_keep"));
    assert!(a.model.mods.tried.is_none());
    assert!(a.model.mods.results[0].1.contains("kept"));
    let stray = super::trial::Trial { pack: "x".into(), dir: std::env::temp_dir().join("ob-not-a-trial-copy") };
    a.model.mods.tried = Some(("X".into(), stray));
    a.event(&Event::Activate("try_discard"));
    assert_eq!(a.model.mods.results[0].0, Mark::Error);
}

/// The Mods page's one list: every kind of mod in it (texture packs, mod packs, the HUD mod, map mods), each row
/// naming its kind; the filter chips narrow it by kind; the actions under it follow the chosen row's kind.
#[test]
fn mods_one_list_with_filters_and_chosen_actions() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Mods, 1280, 720, 1.0);
    let lines = super::screens::mod_lines(&a.model);
    let kinds: Vec<_> = lines.iter().map(|l| l.kind).collect();
    for k in super::model::ListKind::ALL {
        assert!(kinds.contains(&k) || k == super::model::ListKind::Hud, "{k:?} is in the list");
        assert!(lines.iter().filter(|l| l.kind == k).all(|l| l.detail.starts_with(k.one())), "{k:?} named in its rows");
    }
    for id in ["pk0", "mk0", "mp0"] {
        assert!(a.ui.get(id).is_some(), "{id} in the one list");
    }
    assert!(all_text(&a).contains("Mod pack · #1 · version 1.2.0"), "{}", all_text(&a));
    assert!(all_text(&a).contains("MOUNTED NOW"), "what is mounted is said");
    // The chosen row is a texture pack: the pack and map actions wait.
    a.st.focus = Some("pk0");
    key(&mut a, &fonts, Key::Down);
    assert!(!a.ui.get("pack_inside").unwrap().enabled && !a.ui.get("map_open").unwrap().enabled);
    // A mod pack chosen: its actions; a filter hides the other kinds.
    a.st.focus = Some("mk0");
    key(&mut a, &fonts, Key::Down);
    assert_eq!(a.model.mods.cur, Some("mk1"));
    assert!(a.ui.get("pack_inside").unwrap().enabled && a.ui.get("pack_up").unwrap().enabled);
    a.st.focus = Some("kf_packs");
    key(&mut a, &fonts, Key::Enter);
    assert_eq!(a.model.mods.filter, Some(super::model::ListKind::Pack));
    assert!(a.ui.get("pk0").is_none() && a.ui.get("mp0").is_none() && a.ui.get("mk1").is_some());
    assert_eq!(focus(&a), "kf_packs");
    a.st.focus = Some("kf_maps");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.ui.get("mk0").is_none() && a.ui.get("mp1").is_some());
    a.st.focus = Some("mp0");
    key(&mut a, &fonts, Key::Down);
    assert!(a.ui.get("map_open").unwrap().enabled && !a.ui.get("pack_up").unwrap().enabled);
    a.st.focus = Some("kf_all");
    key(&mut a, &fonts, Key::Enter);
    assert!(a.model.mods.filter.is_none() && a.ui.get("pk0").is_some());
    // No filter chips when there is one kind or none.
    let a = super::headless::state_app(&env, &fonts, "mods-empty", 1280, 720, 1.0).unwrap();
    assert!(a.ui.get("kf_all").is_none());
}

/// Moving a mod pack in the load order: the move keys (Alt+Up / Alt+Down, the pad's triggers) and a drag with the
/// mouse send the same move; a click without dragging still switches the row; the row stays focused where it went.
#[test]
fn mods_pack_reorder_by_keys_and_drag() {
    use super::ui::{Drag, handle};
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Mods, 1280, 720, 1.0);
    assert_eq!(a.ui.reorder.iter().filter(|r| r.starts_with("mk")).count(), 2, "the pack rows can be moved");
    assert!(!a.ui.reorder.contains(&"mp0") && !a.ui.reorder.contains(&"hud"));
    // The keys.
    a.st.focus = Some("mk0");
    let ev = handle(&a.ui, &mut a.st.clone(), &Input::Key(Key::MoveDown));
    assert_eq!(ev, [Event::Move("mk0", "mk1")]);
    assert!(handle(&a.ui, &mut a.st.clone(), &Input::Key(Key::MoveUp)).is_empty(), "not past the top");
    let order = |a: &App| a.model.mods.packs.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
    assert_eq!(order(&a), ["docks-remix", "cone-pack"]);
    key(&mut a, &fonts, Key::MoveDown);
    assert_eq!(focus(&a), "mk1", "the row stays focused where it went");
    assert_eq!(a.model.mods.pack_sel.as_deref(), Some("docks-remix"), "the moved pack stays chosen");
    assert_eq!(order(&a), ["cone-pack", "docks-remix"], "Alt+Down / the right trigger: one place weaker");
    // The pad's triggers are the same keys; at the end nothing moves.
    key(&mut a, &fonts, Key::MoveDown);
    assert_eq!(order(&a), ["cone-pack", "docks-remix"], "not past the bottom");
    key(&mut a, &fonts, Key::MoveUp);
    assert_eq!(order(&a), ["docks-remix", "cone-pack"], "Alt+Up / the left trigger: one place stronger");
    assert_eq!(focus(&a), "mk0");
    // A real drag through the app: the first pack row dropped on the second.
    let (r0, r1) = (on_window(&a, "mk0"), on_window(&a, "mk1"));
    let x = r0.cx() - 100.0;
    a.input(&Input::MouseMove(x, r0.cy()), &fonts);
    a.input(&Input::MouseDown(x, r0.cy()), &fonts);
    a.input(&Input::MouseMove(x, r1.cy()), &fonts);
    a.input(&Input::MouseUp(x, r1.cy()), &fonts);
    assert_eq!(order(&a), ["cone-pack", "docks-remix"], "dropped on the second row: moved down");
    assert_eq!(focus(&a), "mk1");
    assert_eq!(a.model.mods.pack_sel.as_deref(), Some("docks-remix"));
    // Dragged back up.
    let (r0, r1) = (on_window(&a, "mk0"), on_window(&a, "mk1"));
    a.input(&Input::MouseMove(x, r1.cy()), &fonts);
    a.input(&Input::MouseDown(x, r1.cy()), &fonts);
    a.input(&Input::MouseMove(x, r0.cy()), &fonts);
    a.input(&Input::MouseUp(x, r0.cy()), &fonts);
    assert_eq!(order(&a), ["docks-remix", "cone-pack"], "dragged back up");
    // A drag: down on the first pack row, over the second, up.
    let r0 = a.ui.get("mk0").unwrap().rect;
    let r1 = a.ui.get("mk1").unwrap().rect;
    let mut st = a.st.clone();
    st.scroll = 0.0;
    assert!(handle(&a.ui, &mut st, &Input::MouseDown(r0.cx() - 100.0, r0.cy())).is_empty());
    assert!(matches!(st.drag, Some(Drag::Row { live: false, .. })));
    handle(&a.ui, &mut st, &Input::MouseMove(r0.cx() - 100.0, r1.cy()));
    assert!(matches!(st.drag, Some(Drag::Row { id: "mk0", over: "mk1", live: true, .. })));
    let ev = handle(&a.ui, &mut st, &Input::MouseUp(r0.cx() - 100.0, r1.cy()));
    assert_eq!(ev, [Event::Move("mk0", "mk1")], "dropped: moved, not switched");
    assert!(st.drag.is_none());
    // A click: no drag went live, the row is switched.
    let mut st = a.st.clone();
    handle(&a.ui, &mut st, &Input::MouseDown(r0.cx(), r0.cy()));
    let ev = handle(&a.ui, &mut st, &Input::MouseUp(r0.cx(), r0.cy()));
    assert_eq!(ev, [Event::Toggle("mk0")]);
    // The drag is drawn: a line where the row would land.
    let mut st = a.st.clone();
    st.drag = Some(Drag::Row { id: "mk0", y0: r0.cy(), over: "mk1", live: true });
    let mut c = super::draw::Canvas::new(1280, 720);
    let mut c2 = super::draw::Canvas::new(1280, 720);
    super::ui::render(&a.ui, &st, &fonts, &mut c);
    super::ui::render(&a.ui, &super::ui::UiState::default(), &fonts, &mut c2);
    assert_ne!(c.pixmap.data(), c2.pixmap.data(), "the landing line is drawn");
}

/// Texture packs move within their own kind only: an imported package dropped on an Overbright pack (or the other
/// way round) is refused with a note and nothing changes; two imported packages swap; the move keys stop at the end of their kind.
#[test]
fn mods_texture_pack_reorder_never_crosses_kinds() {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let mut a = app(&env, &fonts, Screen::Mods, 1280, 720, 1.0);
    let ids = |a: &App| -> (Vec<String>, Vec<String>) {
        (a.model.mods.own.iter().map(|o| o.id.clone()).collect(), a.model.mods.list.iter().map(|(e, _)| e.id.clone()).collect())
    };
    let was = ids(&a);
    assert_eq!(was.0, ["night-roads"]);
    assert_eq!(was.1, ["roads-hd", "garage-redux"]);
    let n = a.model.mods.own.len();
    assert!(a.ui.reorder.contains(&super::screens::PACK_IDS[n]) && a.ui.reorder.contains(&super::screens::PACK_IDS[n + 1]));
    let drag = |a: &mut App, from: &'static str, to: &'static str| {
        let (r0, r1) = (on_window(a, from), on_window(a, to));
        let x = r0.cx() - 100.0;
        a.input(&Input::MouseMove(x, r0.cy()), &fonts);
        a.input(&Input::MouseDown(x, r0.cy()), &fonts);
        a.input(&Input::MouseMove(x, r1.cy()), &fonts);
        a.input(&Input::MouseUp(x, r1.cy()), &fonts);
    };
    // Imported onto Overbright: refused.
    drag(&mut a, super::screens::PACK_IDS[n], super::screens::PACK_IDS[0]);
    assert_eq!(ids(&a), was, "imported onto Overbright: never swapped");
    drag(&mut a, super::screens::PACK_IDS[0], super::screens::PACK_IDS[n]);
    assert_eq!(ids(&a), was, "Overbright onto imported: never swapped");
    a.st.focus = Some(super::screens::PACK_IDS[n]);
    key(&mut a, &fonts, Key::MoveUp);
    assert_eq!(ids(&a), was, "the move key stops at the top of its own kind");
    // Imported onto imported: moved, and the row stays where it went.
    drag(&mut a, super::screens::PACK_IDS[n], super::screens::PACK_IDS[n + 1]);
    assert_eq!(ids(&a).1, ["garage-redux", "roads-hd"]);
    assert_eq!(ids(&a).0, ["night-roads"]);
    assert_eq!(focus(&a), super::screens::PACK_IDS[n + 1]);
}

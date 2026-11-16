//! A screen drawn into an image without a window (the renders the tests
//! compare, `overbright --render`). Uses [`super::model::FakeEnv`]'s fixed
//! data: no game files, no settings read. [`STATES`] are named variants
//! of that data (no game found, a refused folder, the check still running,
//! long German paths, …) for the states a player can meet.

use std::path::PathBuf;

use super::draw::Canvas;
use super::font::Fonts;
use super::model::{Env, FakeEnv, Model, PackStatus, ProbeState};
use super::screens::{App, Screen};
use super::ui::{Mark, render as draw_ui};

/// The app for `screen` at `w` × `h` with the fake data.
pub fn app<'e>(env: &'e FakeEnv, fonts: &Fonts, screen: Screen, w: u32, h: u32, dpi: f32) -> App<'e> {
    App::new(env, env.load(), screen, (w, h), dpi, fonts)
}

/// The screen's pixels.
pub fn render(screen: Screen, w: u32, h: u32, dpi: f32) -> image::RgbaImage {
    let fonts = Fonts::new();
    let env = FakeEnv;
    let a = app(&env, &fonts, screen, w, h, dpi);
    draw(&a, &fonts)
}

/// A built app's pixels.
pub fn draw(a: &App, fonts: &Fonts) -> image::RgbaImage {
    let mut c = Canvas::new(a.size.0, a.size.1);
    draw_ui(&a.ui, &a.st, fonts, &mut c);
    c.to_image()
}

/// A built app's pixels over the backdrop with `picture` (the game's,
/// read with [`super::backdrop::load`]; `overbright --render … --game`).
pub fn draw_with(a: &App, fonts: &Fonts, picture: Option<&super::backdrop::Picture>) -> image::RgbaImage {
    let mut c = Canvas::new(a.size.0, a.size.1);
    let page = super::backdrop::compose(a.size.0, a.size.1, picture, 1.0);
    super::ui::render_on(&a.ui, &a.st, fonts, &mut c, &page);
    c.to_image()
}

/// A screen or a state (by name) built with the fake data.
pub fn named_app<'e>(env: &'e FakeEnv, fonts: &Fonts, name: &str, w: u32, h: u32, dpi: f32) -> Option<App<'e>> {
    match Screen::parse(name) {
        Some(s) => Some(app(env, fonts, s, w, h, dpi)),
        None => state_app(env, fonts, name, w, h, dpi),
    }
}

/// The named states: (name, the screen it is shown on). The fake data of
/// each comes from [`state_model`].
pub const STATES: [(&str, Screen); 31] = [
    ("no-game", Screen::FirstRun),
    ("check-ok", Screen::Check),
    ("wrong-version", Screen::Check),
    ("not-a-game-folder", Screen::Check),
    ("files-missing", Screen::Check),
    ("checking", Screen::Main),
    ("first-start", Screen::Main),
    ("probe-failed", Screen::Main),
    ("no-vulkan", Screen::Main),
    ("long-values", Screen::Main),
    ("skip-movies", Screen::Main),
    ("unofficial", Screen::About),
    ("start-failed", Screen::Failure),
    ("unreadable-folder", Screen::Browse),
    ("import-done", Screen::Import),
    ("import-empty", Screen::Import),
    ("mods-empty", Screen::Mods),
    ("mods-busy", Screen::Mods),
    ("mods-results", Screen::Mods),
    ("mods-remove", Screen::Mods),
    ("mods-hud-complete", Screen::Mods),
    ("mods-hud-incomplete", Screen::Mods),
    ("mods-hud-waiting", Screen::Mods),
    ("mods-hud-converting", Screen::Mods),
    ("mods-hud-set-up", Screen::Mods),
    ("mods-hud-not-the-mod", Screen::Mods),
    ("browse-hud", Screen::Browse),
    ("browse-many", Screen::Browse),
    ("prepare-running", Screen::Prepare),
    ("prepare-done", Screen::Prepare),
    ("prepare-failed", Screen::Prepare),
];

/// The fake data for the state `name` (None: not a state).
pub fn state_model(name: &str) -> Option<(Screen, Model)> {
    let screen = STATES.iter().find(|s| s.0 == name)?.1;
    let mut m = FakeEnv.load();
    let check = |m: &mut Model, root: Option<&str>, lines: Vec<(Mark, &str)>, usable: bool| {
        let c = m.check.as_mut().expect("the fake data has a check");
        if let Some(r) = root {
            c.root = PathBuf::from(r);
        }
        c.lines = lines.into_iter().map(|(k, t)| (k, t.to_string())).collect();
        c.usable = usable;
    };
    match name {
        "no-game" => {
            m.found.clear();
            m.root = None;
        }
        "check-ok" => check(
            &mut m,
            None,
            vec![
                (Mark::Ok, "Version: the known release (sizes and checksums match)"),
                (Mark::Ok, "All 10 files Overbright needs are present"),
            ],
            true,
        ),
        "wrong-version" => check(
            &mut m,
            None,
            vec![
                (
                    Mark::Error,
                    "This SPEED2.EXE is version 1.0: its data tables differ from the supported 1.2 release; \
                     Overbright does not start with it",
                ),
                (Mark::Ok, "All 10 files Overbright needs are present"),
            ],
            false,
        ),
        "not-a-game-folder" => check(
            &mut m,
            Some("/data/player/Downloads"),
            vec![(
                Mark::Error,
                "10 of the 10 files Overbright needs are missing (SPEED2.EXE, GLOBAL/GLOBALB.BUN, …): \
                 this is not a complete game folder",
            )],
            false,
        ),
        "files-missing" => check(
            &mut m,
            None,
            vec![
                (Mark::Ok, "Version: the known release (sizes and checksums match)"),
                (Mark::Error, "Missing: TRACKS/STREAML4RA.BUN — reinstall or copy the full game folder"),
                (Mark::Error, "Missing: FRONTEND/FrontB.lzc — reinstall or copy the full game folder"),
            ],
            false,
        ),
        "checking" => {
            m.probe = ProbeState::Pending;
            m.settings_file = false;
        }
        "first-start" => {
            // Before the game's first run, the probe done: the display rows
            // show the values the game's first start would pick.
            m.settings_file = false;
        }
        "no-vulkan" => {
            // The probe found no adapter: an Ubuntu machine with the Vulkan
            // loader but no driver for its card (`platform::vulkan`).
            let mut p = FakeEnv::probe();
            p.adapter = None;
            p.first_run = None;
            p.prepared = None;
            p.prepared_programs = None;
            p.gpu_class.clear();
            p.error = Some(crate::platform::vulkan::line_for(true, Some("ID=ubuntu\nID_LIKE=debian\n")));
            m.probe = ProbeState::Ready(p);
            m.settings_file = false;
        }
        "probe-failed" => {
            m.probe = ProbeState::Failed("the game program was not found next to the launcher".into());
            m.settings_file = false;
            m.controller = None;
        }
        "long-values" => {
            m.root = Some(PathBuf::from(
                "/data/player/.wine/drive_c/Program Files (x86)/EA GAMES/Need for Speed Underground 2 - Deutsche Version",
            ));
            m.languages = [
                "English",
                "German",
                "French",
                "Italian",
                "Spanish",
                "Dutch",
                "Swedish",
                "Danish",
                "Norwegian",
                "Finnish",
                "Polish",
                "Russian",
                "Czech",
                "Portuguese",
            ]
            .map(String::from)
            .to_vec();
            m.choices.texture_pack = "/data/player/Spiele/Texturpakete/Hochauflösende Benutzeroberfläche".into();
            m.pack_status = PackStatus::Incomplete;
            m.choices.graphics = super::model::Graphics::Custom;
            m.controller = Some("Sony Interactive Entertainment Wireless Controller (DualSense Edge)".into());
        }
        // Skip Intro Movies on, the title screen kept (the two boxes are
        // independent).
        "skip-movies" => {
            m.choices.skip_movies = true;
            m.saved.skip_movies = true;
        }
        "unofficial" => m.source = None,
        "start-failed" => {
            let f = m.failure.as_mut().expect("the fake data has a failure");
            f.tail.clear();
            f.how = "the game program could not be started: No such file or directory (os error 2)".into();
            f.log = None;
        }
        "unreadable-folder" => {
            m.browse.error = Some("That folder cannot be read: /root: Permission denied (os error 13)".into());
        }
        "import-done" => {
            m.import.done = Some(vec!["NIGHTOWL/NIGHTOWL".into(), "NIGHTOWL DVD 1/NIGHTOWL DVD 1".into()]);
        }
        "import-empty" => m.import.scan = None,
        "mods-empty" => m.mods.list.clear(),
        "prepare-running" => {
            m.prepare.state = super::prepare::State::Running { n: 2, of: 5, what: "shader programs".into() }
        }
        "prepare-done" => m.prepare.state = super::prepare::State::Done,
        "prepare-failed" => m.prepare.state = super::prepare::State::Failed("it ended with code 1".into()),
        "mods-busy" => {
            m.mods.busy = Some("Importing NFSU2 Redux HD Textures.tpf (1 of 2): 212 of 430 textures converted…".into())
        }
        "mods-results" => {
            m.mods.results = vec![
                (
                    Mark::Ok,
                    "NFSU2 Redux HD Textures.tpf: 412 of 430 textures replace the game's; 18 not found in your \
                     game's files (left out)"
                        .into(),
                ),
                (
                    Mark::Error,
                    "sa-roads.tpf: not imported: it was made for another game (GTA-SA.EXE), not Need for Speed \
                     Underground 2"
                        .into(),
                ),
            ]
        }
        "mods-remove" => m.mods.confirm = Some("roads-hd".into()),
        "mods-hud-complete" | "mods-hud-incomplete" => {
            m.choices.texture_pack = "/data/player/Mods/HD HUD and maps".into();
            m.choices.hud_mod = true;
            let complete = name == "mods-hud-complete";
            m.hud = Some(super::model::HudCheck {
                is_pack: true,
                listed: 710,
                present: if complete { 710 } else { 708 },
                missing: if complete { Vec::new() } else { vec!["4A7C21F0.png".into(), "4A7C21F1.png".into()] },
                maps: complete,
                complete,
            });
        }
        "mods-hud-waiting" => {
            m.mods.hud_parts.hud =
                Some(crate::hud_mod::Source::Dir("/data/player/mods/hud-part".into()));
            m.mods.hud_results = vec![(
                Mark::Info,
                crate::hud_mod::waiting_line(&m.mods.hud_parts).expect("one part"),
            )];
        }
        "mods-hud-converting" => {
            m.mods.hud_busy = Some("Converting the HUD mod: 315 of 710 images…".into());
        }
        "mods-hud-set-up" => {
            let out = FakeEnv.hud_convert(&Default::default(), &|_| {});
            m.choices.texture_pack = out.dir.as_ref().expect("a folder").display().to_string();
            m.hud = FakeEnv.hud(&m.choices.texture_pack);
            m.mods.hud_results = out.lines;
        }
        "mods-hud-not-the-mod" => {
            let mut parts = crate::hud_mod::Parts::default();
            FakeEnv.hud_add(&mut parts, std::path::Path::new("/data/player/mods/hud-part.7z"));
            m.mods.hud_results = parts.notes.into_iter().map(|t| (Mark::Error, t)).collect();
        }
        "browse-hud" => {
            m.browse.purpose = super::model::BrowseFor::HudMod;
            m.browse.dir = PathBuf::from("/data/player/mods");
            m.browse.entries = FakeEnv.list_dirs(&m.browse.dir).unwrap_or_default();
            m.browse.entries.extend(FakeEnv.list_archives(&m.browse.dir));
            m.browse.sel = 4;
        }
        // A folder of 40: the list scrolled by the wheel to its middle (the
        // scroll bar's thumb there), a row under the pointer, the chosen
        // row out of view above.
        "browse-many" => {
            m.browse.dir = PathBuf::from("/data/player/Games/Steam library/steamapps/common");
            m.browse.entries = (1..=40)
                .map(|i| super::model::DirEntry {
                    name: format!("Game {i:02}"),
                    path: m.browse.dir.join(format!("Game {i:02}")),
                    note: if i == 21 { "game folder".into() } else { String::new() },
                })
                .collect();
            m.browse.sel = 2;
        }
        _ => return None,
    }
    Some((screen, m))
}

/// The state `name` built at `w` × `h` (None: not a state).
pub fn state_app<'e>(env: &'e FakeEnv, fonts: &Fonts, name: &str, w: u32, h: u32, dpi: f32) -> Option<App<'e>> {
    let (screen, m) = state_model(name)?;
    let mut a = App::new(env, m, screen, (w, h), dpi, fonts);
    if name == "browse-many" {
        a.st.set_first("dirs", 16);
        if let Some(r) = a.ui.get("dirs").map(|w| w.rect) {
            a.st.pointer = Some((r.cx(), r.y + r.h / 2.0));
        }
        a.st.hover_row = Some(("dirs", 19));
        a.rebuild(fonts);
    }
    Some(a)
}

/// The state's pixels.
pub fn render_state(name: &str, w: u32, h: u32, dpi: f32) -> Option<image::RgbaImage> {
    let fonts = Fonts::new();
    let env = FakeEnv;
    state_app(&env, &fonts, name, w, h, dpi).map(|a| draw(&a, &fonts))
}

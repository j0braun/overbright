//! A screen drawn into an image without a window (the renders the tests
//! compare, `overbright --render`). Uses [`super::model::FakeEnv`]'s fixed
//! data: no game files, no settings read. [`STATES`] are named variants
//! of that data (no game found, a refused folder, the check still running,
//! long German paths, …) for the states a player can meet.

use std::path::PathBuf;

use super::draw::Canvas;
use super::font::Fonts;
use super::model::{Env, FakeEnv, Graphics, Model, PackStatus, ProbeState, Problem};
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
    let page = super::backdrop::backdrop_a(a.size.0, a.size.1, a.ui.scene, picture);
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
pub const STATES: [(&str, Screen); 73] = [
    ("no-game", Screen::FirstRun),
    ("check-ok", Screen::Check),
    ("check-movies", Screen::Check),
    ("check-movies-missing", Screen::Check),
    ("check-details", Screen::Check),
    ("check-changed", Screen::Check),
    ("check-unknown-edition", Screen::Check),
    ("check-no-english", Screen::Check),
    ("not-a-game-folder", Screen::Check),
    ("files-missing", Screen::Check),
    ("own-folder", Screen::Check),
    ("checking", Screen::Main),
    ("first-start", Screen::Main),
    ("probe-failed", Screen::Main),
    ("no-vulkan", Screen::Main),
    ("long-values", Screen::Main),
    ("language-unsupported", Screen::Advanced),
    ("skip-movies", Screen::Main),
    ("unofficial", Screen::About),
    ("portable", Screen::About),
    ("start-failed", Screen::Failure),
    ("no-game-window", Screen::Failure),
    ("failure-details", Screen::Failure),
    ("unreadable-folder", Screen::Browse),
    ("saves-empty", Screen::Saves),
    ("saves-found", Screen::Saves),
    ("saves-conflict", Screen::Saves),
    ("saves-detail-covers", Screen::Saves),
    ("saves-backups", Screen::Saves),
    ("saves-ask-delete", Screen::Saves),
    ("saves-undo", Screen::Saves),
    ("saves-busy", Screen::Saves),
    ("saves-bad-file", Screen::Saves),
    ("saves-long-names", Screen::Saves),
    ("mods-empty", Screen::Mods),
    ("mods-busy", Screen::Mods),
    ("mods-results", Screen::Mods),
    ("mods-remove", Screen::Mods),
    ("mods-map-editor-open", Screen::Mods),
    ("mods-map-editor-failed", Screen::Mods),
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
    ("first-none", Screen::FirstRun),
    ("first-several", Screen::FirstRun),
    ("err-own", Screen::Check),
    ("err-missing", Screen::Check),
    ("main-checking", Screen::Main),
    ("main-novulkan", Screen::Main),
    ("main-gone", Screen::Main),
    ("main-starting", Screen::Main),
    ("main-quit", Screen::Main),
    ("main-prepare", Screen::Main),
    ("settings-reset", Screen::Settings),
    ("mods-off", Screen::Mods),
    ("main-first", Screen::Main),
    ("storage-portable", Screen::Storage),
    ("storage-custom", Screen::Storage),
    ("storage-missing", Screen::Storage),
    ("storage-copy-back", Screen::Storage),
    ("main-shaders", Screen::Main),
    ("main-shaders-ready", Screen::Main),
    ("settings-suggested", Screen::Settings),
    ("mods-conflict", Screen::Mods),
    ("mods-order", Screen::Mods),
];

/// A checked fake install: the key files `missing` and `different`, the
/// movies `movies_missing`; `unknown`: no data file is the known
/// release's (another release).
pub fn fake_check(missing: &[&str], different: &[&str], movies_missing: &[&str], unknown: bool) -> super::model::CheckResult {
    use crate::fingerprint::{FILES, FilePrint, Fingerprint, State};
    let root = FakeEnv::root();
    let c = crate::qol::setup::Check {
        missing: missing.iter().map(|f| f.to_string()).collect(),
        different: different.iter().map(|f| f.to_string()).collect(),
        assets: Some(super::model::fake_assets(&root, movies_missing)),
        ..Default::default()
    };
    let fp = Fingerprint {
        root: root.clone(),
        files: FILES
            .iter()
            .map(|&file| FilePrint {
                file,
                state: State::Read { size: 1, sha256: "0".repeat(64) },
                edition: (!unknown && !different.contains(&file)).then_some("known"),
            })
            .collect(),
        mods: Vec::new(),
        exe_protected: false,
    };
    super::model::check_result(&root, &c, Some(&fp))
}

/// The fake data for the state `name` (None: not a state).
pub fn state_model(name: &str) -> Option<(Screen, Model)> {
    let screen = STATES.iter().find(|s| s.0 == name)?.1;
    let mut m = FakeEnv.load();
    // The wireframes' names for states that exist under another name.
    let name = match name {
        "first-none" => "no-game",
        "err-own" => "own-folder",
        "err-missing" => "files-missing",
        "main-checking" => "checking",
        "main-novulkan" => "no-vulkan",
        n => n,
    };
    let check = |m: &mut Model, root: Option<&str>, lines: Vec<(Mark, &str)>, usable: bool| {
        let c = m.check.as_mut().expect("the fake data has a check");
        if let Some(r) = root {
            c.root = PathBuf::from(r);
        }
        c.lines = lines.into_iter().map(|(k, t)| (k, t.to_string())).collect();
        c.usable = usable;
        let missing = c.lines.iter().filter(|(k, t)| *k == Mark::Error && t.starts_with("Missing")).count();
        c.problem = match () {
            _ if usable => None,
            _ if missing > 0 => Some(Problem::Missing(missing)),
            _ => Some(Problem::NoGame),
        };
    };
    match name {
        "no-game" => {
            m.found.clear();
            m.root = None;
        }
        // The game check's states (docs/launcher.md, "The game check").
        "check-ok" => m.check = Some(fake_check(&[], &[], &[], false)),
        // The details open: a US install (no language suffix), all found.
        "check-movies" => {
            m.check = Some(fake_check(&[], &[], &[], false));
            m.show_details = true;
        }
        "check-movies-missing" => m.check = Some(fake_check(&[], &[], &["Scene05", "Scene06", "DriftTut"], false)),
        "check-details" => {
            m.check = Some(fake_check(&[], &[], &["Scene05", "Scene06", "DriftTut"], false));
            m.show_details = true;
        }
        "check-changed" => {
            m.check = Some(fake_check(&[], &["GLOBAL/GlobalB.lzc", "FRONTEND/FrontB.lzc"], &[], false))
        }
        "check-unknown-edition" => m.check = Some(fake_check(&[], &[], &[], true)),
        "check-no-english" => {
            let mut c = fake_check(&["LANGUAGES/English.bin"], &[], &[], false);
            // A copy with Korean text only, which the game cannot show (a
            // French-only copy starts in French; `check_result` names the
            // problem by the folder's look, the fake folder is not there).
            c.usable = false;
            c.problem = Some(Problem::NoText);
            if let Some(s) = c.summary.as_mut() {
                s.languages = vec!["Korean".into()];
            }
            m.check = Some(c);
        }
        "not-a-game-folder" => check(
            &mut m,
            Some("/data/player/Downloads"),
            vec![(
                Mark::Error,
                "9 of the 9 files Overbright needs are missing (GLOBAL/GLOBALB.BUN, GLOBAL/GlobalB.lzc, …): \
                 this is not a complete game folder",
            )],
            false,
        ),
        "files-missing" => {
            let mut c = fake_check(&["TRACKS/STREAML4RA.BUN", "FRONTEND/FrontB.lzc"], &[], &[], false);
            c.usable = false;
            c.problem = Some(Problem::Missing(2));
            m.check = Some(c);
        }
        "own-folder" => {
            check(&mut m, Some("/data/player/Games/overbright-0.3.0-linux-x86_64"), Vec::new(), false);
            m.check.as_mut().expect("the fake data has a check").problem = Some(Problem::OwnFolder);
        }
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
        "language-unsupported" => {
            // The languages the engine cannot show, all installed: the
            // longest "Not supported yet" line.
            m.languages = ["English", "French", "Korean", "Japanese", "Chinese", "Thailand"].map(String::from).to_vec();
        }
        "long-values" => {
            m.root = Some(PathBuf::from(
                "/data/player/.wine/drive_c/Program Files (x86)/EA GAMES/Need for Speed Underground 2 - Deutsche Version",
            ));
            m.check.as_mut().expect("the fake data has a check").root = m.root.clone().unwrap_or_default();
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
        // A portable install: the header says so, the folder is beside
        // the programs.
        "portable" => {
            m.folders.portable = true;
            m.folders.data = PathBuf::from("/media/player/SSD/Overbright/data");
        }
        "start-failed" => {
            let f = m.failure.as_mut().expect("the fake data has a failure");
            f.tail.clear();
            f.how = "the game program could not be started: No such file or directory (os error 2)".into();
            f.log = None;
            f.kind = crate::launcher::run::ErrorKind::NoProgram;
        }
        "no-game-window" => {
            let f = m.failure.as_mut().expect("the fake data has a failure");
            f.kind = crate::launcher::run::ErrorKind::NoWindow;
            f.how = "no game window, exit code 0".into();
            f.tail = vec!["could not open a window: the display refused the size".into()];
        }
        "failure-details" => m.failure_details = true,
        "first-several" => {}
        "main-gone" => {
            let gone = PathBuf::from("/media/player/USB/Need for Speed Underground 2");
            m.root = Some(gone.clone());
            let c = m.check.as_mut().expect("the fake data has a check");
            c.root = gone;
            c.usable = false;
            c.problem = Some(Problem::NoGame);
        }
        "main-starting" => m.starting = true,
        "main-first" => m.just_found = true,
        "storage-portable" => {
            m.portable.on = true;
            let data = PathBuf::from("/data/player/Games/overbright-0.3.0/data");
            for r in &mut m.locations {
                r.path = r.path.as_ref().map(|p| {
                    let rest = p.strip_prefix("/data/player/.local/share/overbright").or_else(|_| p.strip_prefix("/data/player/.local/share"));
                    match rest {
                        Ok(rest) => data.join(rest),
                        Err(_) => data.join("cache"),
                    }
                });
            }
        }
        "storage-custom" => {
            use crate::platform::dirs::Location as L;
            for r in &mut m.locations {
                if r.loc == L::Screenshots {
                    r.path = Some(PathBuf::from("/data/player/Pictures/NFSU2"));
                    r.custom = true;
                }
            }
        }
        "storage-missing" => {
            use crate::platform::dirs::Location as L;
            for r in &mut m.locations {
                if r.loc == L::Cache {
                    r.path = Some(PathBuf::from("/media/player/USB-SSD/overbright-cache"));
                    r.custom = true;
                    r.missing = true;
                }
            }
        }
        "main-shaders" => {
            if let ProbeState::Ready(p) = &mut m.probe {
                p.prepared = Some(false);
            }
        }
        "main-shaders-ready" => {
            m.prepare.state = super::prepare::State::Done;
            m.prepare.done_at = Some(std::time::Instant::now());
        }
        "settings-suggested" => {
            // The probe's choice in force: the Graphics row carries the tag.
            if let Some(f) = m.probe.first_run() {
                m.choices.graphics = if f.detail == 0 { Graphics::Original } else { Graphics::Enhanced };
            }
        }
        "main-quit" => m.ask = Some(super::model::Dialog::Quit),
        "main-prepare" => {
            m.prepare.background = true;
            m.prepare.state = super::prepare::State::Running { n: 3, of: 5, what: "shader programs".into() };
            m.prepare.started = std::time::Instant::now().checked_sub(std::time::Duration::from_secs(12));
        }
        "settings-reset" => m.ask = Some(super::model::Dialog::Reset),
        "storage-copy-back" => {
            // Portable mode off with files to copy back, some different
            // from those already in the home folder.
            m.portable.on = true;
            m.portable.marker = true;
            let item = |shown: &str, len: u64, settings: bool| super::transfer::Item {
                from: PathBuf::from("/data/player/Games/overbright-0.3.0/data").join(shown),
                to: PathBuf::from("/data/player/.local/share/overbright").join(shown),
                shown: shown.into(),
                len,
                settings,
            };
            m.copy_plan = Some(super::transfer::Plan {
                copy: vec![
                    item("texture-mods.txt", 120, true),
                    item("NFS Underground 2/RYO/RYO", 210_000, false),
                    item("maps/docks/map.obmap", 3_400, false),
                    item("maps/docks/editor.toml", 90, false),
                    item("Screenshots/photo-1.png", 2_400_000, false),
                ],
                same: vec![item("logs/game-1.log", 4_000, false)],
                differ: vec![
                    item("game-root.txt", 60, true),
                    item("options.txt", 900, true),
                    item("NFS Underground 2/MIA/MIA", 210_000, false),
                ],
                source: "portable".into(),
                paths: super::transfer::Paths::default(),
            });
            m.ask = Some(super::model::Dialog::PortableOff);
        }
        "mods-off" => {
            m.mods.paused = true;
            m.mods.list.iter_mut().for_each(|(e, _)| e.on = false);
        }
        "mods-conflict" => {
            m.mods.own[0].on = true;
            m.mods.conflict = Some(super::model::Conflict {
                first: "Night Roads".into(),
                second: "City roads HD".into(),
                textures: 214,
                same_list: false,
            });
        }
        "mods-order" => {
            m.mods.list.iter_mut().for_each(|(e, _)| e.on = true);
            m.mods.conflict = Some(super::model::Conflict {
                first: "City roads HD".into(),
                second: "Garage and shop redux".into(),
                textures: 12,
                same_list: true,
            });
            m.mods.show_order = true;
            m.mods.sel = 2;
        }
        "unreadable-folder" => {
            m.browse.error = Some("That folder cannot be read: /root: Permission denied (os error 13)".into());
        }
        "saves-empty" => {
            m.saves.rows.clear();
            m.saves.backups.clear();
            m.saves.found = Some(Vec::new());
        }
        "saves-found" => {
            m.saves.rows.clear();
            m.saves.backups.clear();
        }
        "saves-conflict" => {
            m.saves.plan = Some(super::saves_page::fake_plan(PathBuf::from(super::model::FAKE_SAVES)))
        }
        "saves-detail-covers" => {
            let r = &mut m.saves.rows[0];
            r.s.covers = (1..=8)
                .map(|n| super::saves::summary::Cover {
                    name: format!("NIGHTOWL Magazine {n}"),
                    dvd: false,
                    n,
                    bytes: 1_048_612,
                })
                .collect();
            r.thumbs = (1..=4).map(|n| super::saves_page::fake_cover(2 * n)).collect();
            r.s.payload_name = Some("NightOwl".into());
        }
        "saves-backups" => {
            m.saves.show_backups = true;
            m.saves.actions = true;
        }
        "saves-ask-delete" => m.ask = Some(super::model::Dialog::SaveDelete("NIGHTOWL".into())),
        "saves-undo" => {
            m.saves.rows.remove(0);
            m.saves.undo = Some(super::saves::store::Undo {
                remove: None,
                restore: Some(PathBuf::from(
                    "/data/player/.local/share/NFS Underground 2 Backups/NIGHTOWL/launcher-20261118-120000",
                )),
                said: "Deleted NIGHTOWL".into(),
            });
        }
        "saves-busy" => m.saves.busy = Some("Backing up, then deleting…".into()),
        "saves-bad-file" => {
            m.saves.sel = 1;
            m.saves.rows[1].s.problem = Some("header checksum".into());
        }
        "saves-long-names" => {
            let r = &mut m.saves.rows[0];
            r.s.name = "THEFINISHEDCAREERSAVEFROMOLDERBUILDS".into();
            r.car = "MITSUBISHI LANCEREVO8".into();
            r.s.cash = 1_000_006_300;
            if let Some(f) = m.saves.found.as_mut().and_then(|f| f.first_mut()) {
                f.source.label = "Steam (Proton) prefix 1234567890 in /media/player/Spiele-SSD/SteamLibrary".into();
            }
        }
        "mods-empty" => {
            m.mods.list.clear();
            m.mods.own.clear();
            m.mods.maps.clear();
        }
        "prepare-running" => {
            m.prepare.state = super::prepare::State::Running { n: 2, of: 5, what: "shader programs".into() }
        }
        "prepare-done" => m.prepare.state = super::prepare::State::Done,
        "prepare-failed" => m.prepare.state = super::prepare::State::Failed("it ended with code 1".into()),
        "mods-map-editor-open" => {
            m.mods.map_sel = Some("drift-park".into());
            m.mods.editor = super::model::EditorRun::Open(Some("drift-park".into()));
        }
        "mods-map-editor-failed" => {
            m.mods.editor = super::model::EditorRun::Failed(
                "the map editor could not be started (overbright-editor): No such file or directory".into(),
            )
        }
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
            m.mods.hud_parts.hud = Some(crate::hud_mod::Source::Dir("/data/player/mods/hud-part".into()));
            m.mods.hud_results = vec![(Mark::Info, crate::hud_mod::waiting_line(&m.mods.hud_parts).expect("one part"))];
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

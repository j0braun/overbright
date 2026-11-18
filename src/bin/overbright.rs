//! `overbright`: the launcher (src/launcher/, docs/launcher.md).
//!
//! ```text
//! overbright [--launcher] [--screen <name>] [-- game args]
//!                                              the window; Play starts the game with the game args
//! overbright --render <screen|state> --size WxH [--scale F] out.png
//!                                              a screen (or a named state of one: no game found, a
//!                                              refused folder, …) drawn with fixed fake data into a PNG;
//!                                              `--game <folder>`: over that game's title picture
//! overbright --list-screens                    the screens, then the states
//! overbright --version          the version line and where the source is
//! overbright --list-folder <dir> [game|packages|hud]
//!                                              the folder browser's rows for that folder
//! overbright --add-packages <file|folder>…     the Mods page's "Add package…" without the window
//! overbright --add-hud-mod <file|folder>…      the Mods page's "Add HUD mod…" without the window
//! ```
//! The last three do what the window does with the same code (the
//! player's real folders) and print its lines; for scripts, the Windows
//! smoke and maintainers.
//! With "Start the game directly next time" (`launcher.skip`) the game
//! starts at once, unless Shift (or a pad's Back / Select) is held or
//! `--launcher` is given; a failed start then opens the window on the
//! failure screen. One launcher at a time. Arguments the launcher does not
//! know go to the game, as do all after `--`.

// A window program on Windows (no console window from Explorer); a start
// from a terminal attaches to it (`console::attach`).
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::process::ExitCode;

use nfsu2::launcher::{
    dirs, headless, model, run,
    screens::{Outcome, Screen},
    start_log, window,
};

const USAGE: &str = "usage: overbright [--launcher] [--screen <name>] [-- game args]\n       overbright --render <screen|state> --size WxH [--scale F] out.png\n       overbright --list-screens\n       overbright --version\n       overbright --list-folder <dir> [game|packages|hud]\n       overbright --add-packages <file|folder>…\n       overbright --add-hud-mod <file|folder>…";

/// `--list-folder`: the browser's rows (folders, then the package or
/// archive files the purpose lists), one `name<TAB>note` line each.
fn list_folder(dir: &str, kind: Option<&str>) -> Result<(), String> {
    use model::Env;
    let env = model::RealEnv;
    let dir = std::path::Path::new(dir);
    let mut rows = env.list_dirs(dir)?;
    match kind {
        None | Some("game") => {}
        Some("packages") => {
            let packages = env.list_packages(dir);
            rows.retain(|r| !packages.iter().any(|p| p.path == r.path));
            rows.extend(packages);
        }
        Some("hud") => rows.extend(env.list_archives(dir)),
        Some(k) => return Err(format!("--list-folder: {k}: game, packages or hud\n{USAGE}")),
    }
    for r in rows {
        println!("{}\t{}", r.name, r.note);
    }
    Ok(())
}

/// `--add-packages`: texture packages (TexMod, TexWizard) imported for the
/// kept game folder; a folder that is no package stands for the TexWizard
/// packs in it.
fn add_packages(paths: &[String]) -> Result<(), String> {
    let paths = model::expand_packages(paths.iter().map(std::path::PathBuf::from).collect());
    let root = nfsu2::qol::setup::find();
    let out = model::import_paths(&paths, root.as_deref(), &|t| eprintln!("{t}"));
    let mut ok = !out.is_empty();
    for o in &out {
        println!("{} {}", if o.ok { "ok:" } else { "error:" }, o.line);
        ok &= o.ok;
    }
    if ok { Ok(()) } else { Err("not every package was imported".into()) }
}

/// `--add-hud-mod`: the HUD mod's downloads converted; the result kept
/// as the HUD mod folder, as the window does.
fn add_hud_mod(paths: &[String]) -> Result<(), String> {
    let mut parts = nfsu2::hud_mod::Parts::default();
    for p in paths {
        nfsu2::hud_mod::add(&mut parts, std::path::Path::new(p));
        for n in parts.notes.drain(..) {
            println!("error: {n}");
        }
    }
    if !parts.complete() {
        if let Some(t) = nfsu2::hud_mod::waiting_line(&parts) {
            println!("info: {t}");
        }
        return Err("both parts of the HUD mod are needed".into());
    }
    let out = model::convert_hud(&parts, &|t| eprintln!("{t}"));
    let mut ok = true;
    for (mark, line) in &out.lines {
        let word = match mark {
            nfsu2::launcher::ui::Mark::Ok => "ok:",
            nfsu2::launcher::ui::Mark::Warn => "warning:",
            nfsu2::launcher::ui::Mark::Info => "info:",
            nfsu2::launcher::ui::Mark::Error => "error:",
        };
        ok &= *mark == nfsu2::launcher::ui::Mark::Ok;
        println!("{word} {line}");
    }
    if let Some(dir) = &out.dir {
        nfsu2::qol::setup::save_texture_pack(Some(dir)).map_err(|e| format!("the HUD mod folder was not kept: {e}"))?;
        println!("folder: {}", dir.display());
    }
    if ok { Ok(()) } else { Err("the HUD mod was not set up".into()) }
}

fn main() -> ExitCode {
    nfsu2::console::attach();
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The launcher's log from the first line (`start_log`, docs/launcher.md
    // "When the launcher does not open"), and a panic written to it.
    let logged = start_log::window_run(&args);
    // One launcher at a time (the lock is held until the process ends);
    // taken before the log, so a second start leaves the first one's log
    // alone.
    let mut lock_error = None;
    let _lock = if logged {
        match run::single_instance(&dirs::lock_file()) {
            Ok(Some(l)) => Some(l),
            Ok(None) => {
                nfsu2::console::fatal("Overbright is already running.");
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                lock_error = Some(e);
                None
            }
        }
    } else {
        None
    };
    if logged {
        start_log::start(&args);
        if let Some(e) = lock_error {
            log::warn!("no launcher lock ({e}); going on");
        }
    } else {
        let _ = env_logger::Builder::from_default_env().try_init();
    }
    start_log::install_panic_hook();
    let result = std::panic::catch_unwind(|| run_cli(&args));
    let log = start_log::path();
    let told = |what: &str| if logged { start_log::failure_text(what, &log) } else { what.to_string() };
    match result {
        Ok(Ok(())) => {
            start_log::finish();
            ExitCode::SUCCESS
        }
        Ok(Err(e)) => {
            start_log::line(&format!("end: the launcher stopped: {e}"));
            nfsu2::console::fatal(&told(&format!("overbright: {e}")));
            ExitCode::from(2)
        }
        Err(_) => {
            let what = start_log::panic_message().unwrap_or_else(|| "(no message)".into());
            start_log::line("end: the launcher stopped after an internal error");
            nfsu2::console::fatal(&told(&format!("The launcher stopped after an internal error: {what}")));
            ExitCode::from(101)
        }
    }
}

fn parse_size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once(['x', 'X'])?;
    let (w, h) = (w.parse().ok()?, h.parse().ok()?);
    (16..=16384).contains(&w).then_some(())?;
    (16..=16384).contains(&h).then_some((w, h))
}

fn run_cli(args: &[String]) -> Result<(), String> {
    let (ours, after) = match args.iter().position(|a| a == "--") {
        Some(i) => (&args[..i], &args[i + 1..]),
        None => (args, &[][..]),
    };
    let mut game_args: Vec<String> = Vec::new();
    let mut screen: Option<Screen> = None;
    let mut render: Option<String> = None;
    let mut size = None;
    let mut scale = 1.0f32;
    let mut out = None;
    let mut force = false;
    let mut picture_from: Option<String> = None;
    let mut it = ours.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--render" => {
                let name = it
                    .next()
                    .filter(|s| Screen::parse(s).is_some() || headless::state_model(s).is_some())
                    .ok_or(format!("--render <screen|state> (--list-screens)\n{USAGE}"))?;
                render = Some(name.clone())
            }
            "--screen" => {
                screen = Some(it.next().and_then(|s| Screen::parse(s)).ok_or(format!("--screen <screen>\n{USAGE}"))?)
            }
            "--size" => size = Some(it.next().and_then(|s| parse_size(s)).ok_or(format!("--size WxH\n{USAGE}"))?),
            "--scale" => {
                scale = it
                    .next()
                    .and_then(|s| s.parse().ok())
                    .filter(|v: &f32| (0.5..=4.0).contains(v))
                    .ok_or(format!("--scale F (0.5..4)\n{USAGE}"))?
            }
            "--launcher" => force = true,
            "--game" if render.is_some() => {
                picture_from = Some(it.next().ok_or(format!("--game <folder>\n{USAGE}"))?.clone())
            }
            "--version" => {
                // The about screen's two facts (the release check reads the
                // second: a published build names its source).
                println!("{}", nfsu2::build_info::version_line());
                match model::source_url() {
                    Some(u) => println!("source: {u}"),
                    None => println!("{}", nfsu2::launcher::screens::UNOFFICIAL),
                }
                return Ok(());
            }
            "--list-screens" => {
                for s in Screen::ALL {
                    println!("{}", s.name());
                }
                for (name, s) in headless::STATES {
                    println!("{name} (state of {})", s.name());
                }
                return Ok(());
            }
            "--list-folder" => {
                let dir = it.next().ok_or(format!("--list-folder <dir>\n{USAGE}"))?;
                return list_folder(dir, it.next().map(String::as_str));
            }
            "--add-packages" | "--add-hud-mod" => {
                let rest: Vec<String> = it.by_ref().cloned().collect();
                if rest.is_empty() {
                    return Err(format!("{a} <file|folder>…\n{USAGE}"));
                }
                return if a == "--add-packages" { add_packages(&rest) } else { add_hud_mod(&rest) };
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            x if render.is_some() && !x.starts_with("--") => out = Some(x.to_string()),
            // Anything else is the game's (design rule 6).
            x => game_args.push(x.to_string()),
        }
    }
    game_args.extend(after.iter().cloned());
    if let Some(s) = render {
        let (w, h) = size.ok_or(format!("--render needs --size WxH\n{USAGE}"))?;
        let out = out.ok_or(format!("--render needs an output file\n{USAGE}"))?;
        let fonts = nfsu2::launcher::font::Fonts::new();
        let env = model::FakeEnv;
        let app = headless::named_app(&env, &fonts, &s, w, h, scale).ok_or(format!("{s}: no such screen or state"))?;
        let picture = match &picture_from {
            Some(root) => Some(
                nfsu2::launcher::backdrop::load(std::path::Path::new(root))
                    .ok_or(format!("{root}: no title picture found"))?,
            ),
            None => None,
        };
        let img = headless::draw_with(&app, &fonts, picture.as_ref());
        img.save(&out).map_err(|e| format!("{out}: {e}"))?;
        return Ok(());
    }
    // (One launcher at a time: `main` holds the lock.)
    let root = nfsu2::qol::setup::find();
    let usable = root.as_ref().is_some_and(|r| nfsu2::qol::setup::check(r, false).ok());
    log::info!(
        "start: game folder {} ({}); game program {}",
        root.as_ref().map_or("none found".to_string(), |r| r.display().to_string()),
        if usable { "usable" } else { "not usable" },
        dirs::game_exe().display()
    );
    let game = window::GameStart {
        exe: dirs::game_exe(),
        args: game_args,
        logs: Some(dirs::logs()),
        root: root.clone().filter(|_| usable),
    };
    let mut failure = None;
    // The skip rule: straight into the game, unless asked for the window.
    if screen.is_none() && usable && model::load_choices().0.skip && !force {
        if run::shift_held() || run::pad_back_held() {
            log::info!("Shift / Back held: the launcher window");
        } else {
            match game.start() {
                Ok(r) => {
                    let e = r.wait();
                    if !e.failed() {
                        return Ok(());
                    }
                    eprintln!("overbright: showing the failure screen ({})", e.how());
                    failure = Some(window::failure_of(&e));
                }
                Err(e) => failure = Some(model::Failure { during_start: true, how: e, tail: Vec::new(), log: None }),
            }
        }
    }
    let first = match (screen, &failure) {
        (Some(s), _) => s,
        (None, Some(_)) => Screen::Failure,
        (None, None) if usable => Screen::Main,
        _ => Screen::FirstRun,
    };
    log::info!("start: the window, screen {}", first.name());
    let env = model::RealEnv;
    let start = window::Start { screen: first, root: root.filter(|_| usable), failure, probe: true, game };
    match window::run_with_root(&env, start)? {
        Outcome::Play | Outcome::Quit | Outcome::Stay => Ok(()),
    }
}

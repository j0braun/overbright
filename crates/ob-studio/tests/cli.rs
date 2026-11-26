//! The command-line tool's golden tests: each case runs the tool (in this process) on a mod written here and
//! compares its `--json` document with the kept one in `tests/cli/<case>.json` (paths shown as `<tmp>/…`, times
//! as 0), its exit code included. `OB_STUDIO_BLESS=1` rewrites the kept documents; review the diff.
//!
//! Also: every command of the registry is on the command line, every exit code is reached, an edit's flags and
//! the same edit as an operations-file line give the same request, and an edit while Studio holds the mod goes
//! through Studio's inbox.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use glam::{Vec2, Vec3};
use serde_json::Value;

use nfsu2::studio_api::map::assets::{GivenAssets, TrackInfo};
use nfsu2::studio_api::world::World;
use ob_formats::formats::roads::{RoadNet, Segment};
use ob_map::tools::map::format::{CustomRace, MapMod, RaceKind, Spot};
use ob_map::tools::map::store;
use ob_studio::cli::{self, Env};
use ob_studio::history::{self, History, disk};
use ob_studio::registry;
use ob_testkit::Scratch;

/// A straight road along +x, 0..400 m, in two segments.
fn straight() -> RoadNet {
    let xs = [0.0, 100.0, 200.0, 200.0, 300.0, 400.0];
    let mut net = RoadNet {
        nodes: xs.iter().enumerate().map(|(i, &x)| (Vec2::new(x, 0.0), i / 3)).collect(),
        links: vec![(0xFF, 0, [-1; 3]); 6],
        along: vec![0.0; 6],
        ..RoadNet::default()
    };
    let seg = |first, exits: [Vec<(usize, u8)>; 2]| Segment {
        first,
        end: first + 3,
        length: 200.0,
        spline_length: 200.0,
        exits,
        barriers: vec![],
        tracks: Default::default(),
    };
    net.segments = vec![seg(0, [vec![], vec![(1, 0)]]), seg(3, [vec![(0, 1)], vec![]])];
    net
}

fn sprint(id: &str, points: &[(f32, f32)]) -> CustomRace {
    let mut r = CustomRace::new(id, 4001, RaceKind::Sprint);
    r.name = "Sprint".into();
    r.checkpoints = points.iter().map(|&(x, y)| Vec2::new(x, y)).collect();
    r.start = Some(Spot { position: Vec3::new(30.0, 0.0, 0.0), heading: 0.0 });
    r.finish = Some(Spot { position: Vec3::new(390.0, 0.0, 0.0), heading: 0.0 });
    r
}

/// A mod folder `docks` with one race on the road (`broken`: a race with one checkpoint beside it).
fn fixture(tag: &str, broken: bool) -> (Scratch, PathBuf) {
    let s = Scratch::new(tag);
    let dir = s.path().join("docks");
    std::fs::create_dir_all(&dir).unwrap();
    let mut races = vec![sprint("sprint", &[(20.0, 0.0), (180.0, 0.0), (380.0, 1.0)])];
    if broken {
        races.push(sprint("short", &[(20.0, 0.0)]));
    }
    let map = MapMod { name: "Docks".into(), author: "Tester".into(), races, ..MapMod::default() };
    std::fs::write(dir.join(store::FILE), map.write()).unwrap();
    (s, dir)
}

fn env() -> Env {
    Env {
        game: Some(PathBuf::from("game")),
        game_source: "NFSU2_ROOT",
        world: Box::new(|_, _| Ok(World::default())),
        assets: Box::new(|_| {
            Box::new(GivenAssets {
                tracks: vec![TrackInfo { number: 4001, name: "Docks".into() }],
                nets: HashMap::from([(4001, straight())]),
                free_roam: Vec::new(),
            })
        }),
        poll: Duration::from_millis(5),
    }
}

fn no_game() -> Env {
    Env { game: None, world: Box::new(|_, _| Err("no game".into())), ..env() }
}

fn argv(words: &[&str], dir: &Path) -> Vec<String> {
    words.iter().map(|w| w.replace("<mod>", &dir.to_string_lossy())).collect()
}

/// The tool's run: (exit code, stdout, stderr).
fn run(words: &[&str], dir: &Path, env: &Env) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = cli::run(&argv(words, dir), env, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// Paths under the scratch folder as `<tmp>/…` (with `/` on every system), times as 0, the build line and a pack's size
/// (which holds the build line) as placeholders.
fn normal(v: &mut Value, tmp: &str) {
    match v {
        Value::String(s) => {
            let t = s.replace(tmp, "<tmp>");
            if t != *s {
                *s = t.replace('\\', "/");
            }
            // The holder's process id.
            if let Some(i) = s.find("(process ") {
                let end = s[i..].find(')').map_or(s.len(), |e| i + e);
                s.replace_range(i..end, "(process <pid>");
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| normal(x, tmp)),
        Value::Object(o) => {
            // A written pack's size depends on the build line inside it, so it is checked against the file on disk
            // (the document must tell the truth) and kept out of the golden.
            if let (Some(to), Some(n)) = (o.get("to").and_then(Value::as_str), o.get("bytes").and_then(Value::as_u64)) {
                if to.ends_with(".obpk") {
                    let len = std::fs::metadata(to).unwrap_or_else(|e| panic!("{to}: {e}")).len();
                    assert_eq!(n, len, "{to}: \"bytes\" is the written file's size");
                    o.insert("bytes".into(), Value::from("<bytes>"));
                }
            }
            for (k, x) in o.iter_mut() {
                if k == "time_ms" {
                    *x = Value::from(0);
                } else if k == "build" {
                    // The build line differs with every build.
                    *x = Value::from("<build>");
                } else {
                    normal(x, tmp);
                }
            }
        }
        _ => {}
    }
}

/// Run `words --json` and compare its document with the kept `tests/cli/<case>.json`; the exit code.
fn golden(case: &str, words: &[&str], s: &Scratch, dir: &Path, env: &Env) -> i32 {
    let mut w = words.to_vec();
    w.push("--json");
    let (code, out, err) = run(&w, dir, env);
    assert!(err.is_empty(), "{case}: --json writes nothing to stderr: {err}");
    let mut doc: Value =
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("{case}: not one JSON document ({e}): {out}"));
    assert_eq!(doc["exit"], code, "{case}: the document's exit code");
    assert_eq!(doc["format"], 1, "{case}");
    normal(&mut doc, &s.path().to_string_lossy());
    let now = serde_json::to_string_pretty(&doc).unwrap() + "\n";
    let path = format!("{}/tests/cli/{case}.json", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("OB_STUDIO_BLESS").is_some() {
        std::fs::create_dir_all(Path::new(&path).parent().unwrap()).unwrap();
        std::fs::write(&path, &now).unwrap();
    }
    let kept = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
    assert!(kept == now, "{case}: differs from tests/cli/{case}.json (OB_STUDIO_BLESS=1 rewrites it):\n{now}");
    code
}

fn text(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(store::FILE)).unwrap()
}

#[test]
fn help_and_usage_errors() {
    let (s, dir) = fixture("cli-usage", false);
    let e = env();
    let cases: &[(&str, &[&str], i32)] = &[
        ("help", &["help"], cli::OK),
        ("help-race-point-add", &["help", "race", "point", "add"], cli::OK),
        ("unknown-command", &["frobnicate", "--mod", "<mod>"], cli::USAGE),
        ("unknown-flag", &["race", "delete", "--mod", "<mod>", "--race", "sprint", "--colour", "red"], cli::USAGE),
        ("missing-argument", &["race", "delete", "--mod", "<mod>"], cli::USAGE),
        ("bad-value", &["race", "set", "--mod", "<mod>", "--race", "sprint", "--laps", "many"], cli::USAGE),
        ("bad-enum", &["race", "set", "--mod", "<mod>", "--race", "sprint", "--kind", "rally"], cli::USAGE),
        ("no-mod-flag", &["race", "delete", "--race", "sprint"], cli::USAGE),
        ("no-command", &[], cli::USAGE),
        ("dry-run-of-a-read", &["checks", "run", "--mod", "<mod>", "--dry-run"], cli::USAGE),
        ("not-a-mod", &["race", "delete", "--mod", "<mod>/nothing", "--race", "sprint"], cli::FAILED),
    ];
    for (case, words, code) in cases {
        assert_eq!(golden(case, words, &s, &dir, &e), *code, "{case}");
    }
    // Nothing above changed the mod or made its editor folder.
    assert!(!dir.join(store::EDITOR_DIR).exists());
}

#[test]
fn every_registered_command_is_on_the_command_line() {
    let (s, dir) = fixture("cli-commands", false);
    for c in registry::COMMANDS {
        let mut words: Vec<&str> = vec!["help"];
        words.extend(c.name.split('.'));
        let (code, out, _) = run(&[&words[..], &["--json"]].concat(), &dir, &env());
        assert_eq!(code, cli::OK, "{}", c.name);
        let doc: Value = serde_json::from_str(&out).unwrap();
        let flags: Vec<&str> =
            doc["commands"][0]["flags"].as_array().unwrap().iter().map(|f| f["flag"].as_str().unwrap()).collect();
        let want: Vec<String> = c.args.iter().map(|a| format!("--{}", a.key)).collect();
        assert_eq!(flags, want, "{}", c.name);
    }
    drop(s);
}

#[test]
fn every_exit_code_is_documented() {
    let doc =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/modding/studio-cli.md")).unwrap();
    for (code, kind, _) in cli::EXIT_CODES {
        assert!(doc.contains(&format!("| {code} | `{kind}` |")), "docs/modding/studio-cli.md lacks exit code {code}");
    }
    for c in registry::COMMANDS {
        assert!(
            doc.contains(&format!("`{}`", c.name.replace('.', " "))),
            "docs/modding/studio-cli.md lacks `{}`",
            c.name
        );
    }
}

#[test]
fn edits_dry_run_make_steps_and_refuse() {
    let (s, dir) = fixture("cli-edits", false);
    let e = env();
    let before = text(&dir);
    let set = ["race", "set", "--mod", "<mod>", "--race", "sprint", "--name", "Docks run", "--laps", "1"];
    assert_eq!(golden("race-set-dry-run", &[&set[..], &["--dry-run"]].concat(), &s, &dir, &e), cli::OK);
    assert_eq!(text(&dir), before, "a dry run writes nothing");
    assert!(!dir.join(store::EDITOR_DIR).exists(), "a dry run makes no editor folder");

    assert_eq!(golden("race-set", &set, &s, &dir, &e), cli::OK);
    assert!(text(&dir).contains("name = Docks run"));
    assert_eq!(golden("race-set-unchanged", &set, &s, &dir, &e), cli::OK);
    assert_eq!(
        golden(
            "race-create",
            &["race", "create", "--mod", "<mod>", "--track", "4001", "--id", "second", "--kind", "circuit"],
            &s,
            &dir,
            &e
        ),
        cli::OK
    );
    assert_eq!(
        golden(
            "race-point-add",
            &[
                "race",
                "point",
                "add",
                "--mod",
                "<mod>",
                "--race",
                "second",
                "--part",
                "checkpoint",
                "--position",
                "50,0,0"
            ],
            &s,
            &dir,
            &e
        ),
        cli::OK
    );
    assert_eq!(
        golden(
            "race-point-move-later",
            &[
                "race",
                "point",
                "move",
                "--mod",
                "<mod>",
                "--race",
                "sprint",
                "--part",
                "checkpoint",
                "--index",
                "0",
                "--later"
            ],
            &s,
            &dir,
            &e
        ),
        cli::OK
    );
    assert_eq!(
        golden("race-delete-refused", &["race", "delete", "--mod", "<mod>", "--race", "nope"], &s, &dir, &e),
        cli::FAILED
    );
    assert_eq!(
        golden(
            "pack-details",
            &["pack", "details", "--mod", "<mod>", "--description", "Night at the docks"],
            &s,
            &dir,
            &e
        ),
        cli::OK
    );
    // A prop edit reads the world: refused on an empty one, exit 4 without the game.
    assert_eq!(
        golden("props-delete-refused", &["props", "delete", "--mod", "<mod>", "--key", "added:a9"], &s, &dir, &e),
        cli::FAILED
    );
    assert_eq!(
        golden(
            "props-delete-no-game",
            &["props", "delete", "--mod", "<mod>", "--key", "added:a9"],
            &s,
            &dir,
            &no_game()
        ),
        cli::NO_GAME
    );
    assert_eq!(golden("history-show", &["history", "show", "--mod", "<mod>"], &s, &dir, &e), cli::OK);
    // The steps are the tool's, in the mod's history, and undo takes them back in Studio.
    let mut h =
        History::open(&dir, &dir.join(store::EDITOR_DIR), &[store::FILE], "test", std::sync::Arc::new(disk::Real))
            .unwrap();
    assert_eq!(h.steps().len(), 5);
    assert!(h.steps().iter().all(|s| s.author == cli::AUTHOR));
    assert_eq!(h.undo("test").unwrap().as_deref(), Some("Mod details"));
}

#[test]
fn an_operations_file_is_one_step_and_its_lines_are_the_flags_requests() {
    let (s, dir) = fixture("cli-ops", false);
    let e = env();
    // The request a command's flags make, as the tool prints it …
    let (_, out, _) =
        run(&["race", "set", "--mod", "<mod>", "--race", "sprint", "--laps", "1", "--dry-run", "--json"], &dir, &e);
    let line = serde_json::to_string(&serde_json::from_str::<Value>(&out).unwrap()["request"]).unwrap();
    // … is an operations file's line.
    let ops = s.path().join("ops.jsonl");
    std::fs::write(
        &ops,
        format!("# two edits\n{line}\n{{\"op\":\"rename_race\",\"race\":\"sprint\",\"name\":\"Docks run\"}}\n"),
    )
    .unwrap();
    let file = ops.to_string_lossy().to_string();
    let apply = ["ops", "apply", "--mod", "<mod>", "--file", &file, "--label", "Shorter sprint"];
    assert_eq!(golden("ops-apply-dry-run", &[&apply[..], &["--dry-run"]].concat(), &s, &dir, &e), cli::OK);
    assert_eq!(golden("ops-apply", &apply, &s, &dir, &e), cli::OK);
    let h = History::open_read(&dir, &dir.join(store::EDITOR_DIR), &[store::FILE], std::sync::Arc::new(disk::Real))
        .unwrap();
    assert_eq!(h.steps().len(), 1, "one step for the whole file");
    assert_eq!(h.steps()[0].label, "Shorter sprint");

    std::fs::write(&ops, "{\"op\":\"rename_race\",\"race\":\"sprint\",\"name\":\"x\"}\n{\"op\":\"frob\"}\n").unwrap();
    assert_eq!(
        golden("ops-apply-bad-line", &["ops", "apply", "--mod", "<mod>", "--file", &file], &s, &dir, &e),
        cli::USAGE
    );
}

#[test]
fn the_checks_run_and_exit_3_on_an_error() {
    let (s, dir) = fixture("cli-checks", false);
    assert_eq!(golden("checks-ok", &["checks", "run", "--mod", "<mod>"], &s, &dir, &env()), cli::OK);
    assert_eq!(golden("checks-no-game", &["checks", "run", "--mod", "<mod>"], &s, &dir, &no_game()), cli::NO_GAME);
    // An empty --game is not a game folder: the lookup goes on (NFSU2_ROOT, the launcher's folder) and says how.
    let (code, _, err) = run(&["checks", "run", "--mod", "<mod>", "--game", ""], &dir, &no_game());
    assert_eq!(code, cli::NO_GAME, "{err}");
    assert!(err.contains("set NFSU2_ROOT"), "{err}");
    let (s, dir) = fixture("cli-checks-broken", true);
    assert_eq!(golden("checks-errors", &["checks", "run", "--mod", "<mod>"], &s, &dir, &env()), cli::CHECKS);
    let (code, out, _) = run(&["checks", "run", "--mod", "<mod>"], &dir, &env());
    assert_eq!(code, cli::CHECKS);
    assert!(out.lines().last().unwrap().starts_with("1 error(s)"), "{out}");
}

#[test]
fn while_studio_has_the_mod_the_edit_goes_through_its_inbox() {
    let (s, dir) = fixture("cli-inbox", false);
    let edir = dir.join(store::EDITOR_DIR);
    let mut studio =
        History::open(&dir, &edir, &[store::FILE], history::STUDIO, std::sync::Arc::new(disk::Real)).unwrap();
    assert!(studio.writable());
    let e = env();
    let set = ["race", "set", "--mod", "<mod>", "--race", "sprint", "--name", "Other"];
    assert_eq!(golden("inbox-busy", &[&set[..], &["--no-inbox"]].concat(), &s, &dir, &e), cli::BUSY);
    assert_eq!(golden("inbox-timed-out", &[&set[..], &["--wait", "0.05"]].concat(), &s, &dir, &e), cli::TIMED_OUT);
    // The request stays: Studio makes it at its next look.
    let t = studio.tick(Instant::now()).unwrap();
    assert_eq!(t.answered.len(), 1);
    assert_eq!(studio.steps().len(), 1);

    // Studio looking at its inbox while the tool waits.
    let words = argv(&["race", "set", "--mod", "<mod>", "--race", "sprint", "--name", "Docks run", "--json"], &dir);
    let tool = std::thread::spawn(move || {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = cli::run(&words, &env(), &mut out, &mut err);
        (code, String::from_utf8(out).unwrap())
    });
    while !tool.is_finished() {
        studio.tick(Instant::now()).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    let (code, out) = tool.join().unwrap();
    assert_eq!(code, cli::OK, "{out}");
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert_eq!((doc["via"].as_str(), doc["step"].as_u64()), (Some("inbox"), Some(2)));
    assert_eq!(studio.steps().len(), 2);
    assert!(studio.steps()[1].label.ends_with("Rename race"), "{}", studio.steps()[1].label);
    assert!(studio.doc(store::FILE).unwrap().contains("name = Docks run"));
}

/// The game for Looks: a world with one texture, `ROAD` (4×4 grey).
fn looks_env() -> Env {
    use ob_formats::formats::texture::{Blend, PixelFormat, Scroll, Texture};
    Env {
        world: Box::new(|_, _| {
            let mut w = World::default();
            let t = Texture {
                name: "ROAD".into(),
                hash: 0x0000_0ABC,
                width: 4,
                height: 4,
                format: PixelFormat::Rgba8,
                mip_count: 1,
                data: [90u8, 90, 90, 255].repeat(16).into(),
                palette: vec![],
                alpha_test: false,
                blend: Blend::None,
                rendering_order: 0,
                late_buckets: false,
                scroll: Scroll::default(),
            };
            w.textures.insert(t.hash, t);
            Ok(w)
        }),
        ..env()
    }
}

#[test]
fn looks_open_import_and_list() {
    let (s, dir) = fixture("cli-looks", false);
    let e = looks_env();
    let open = ["looks", "open", "--mod", "<mod>", "--texture", "world:0xABC"];
    assert_eq!(golden("looks-open", &open, &s, &dir, &e), cli::OK);
    let work = dir.join("work/looks/world/ROAD.png");
    assert!(work.is_file());
    assert_eq!(golden("looks-blank-over-paint", &[&open[..], &["--blank"]].concat(), &s, &dir, &e), cli::FAILED);
    assert_eq!(
        golden("looks-bad-texture", &["looks", "open", "--mod", "<mod>", "--texture", "ui:1"], &s, &dir, &e),
        cli::USAGE
    );
    // The paint program saves; `looks import` takes it in as one step.
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([250, 10, 10, 255]));
    img.save(&work).unwrap();
    let import = ["looks", "import", "--mod", "<mod>", "--texture", "world:0x00000ABC"];
    assert_eq!(golden("looks-import", &import, &s, &dir, &e), cli::OK);
    assert!(dir.join("textures/world/00000ABC.png").is_file(), "the pack data");
    assert_eq!(golden("looks-list", &["looks", "list", "--mod", "<mod>"], &s, &dir, &e), cli::OK);
    // `pack export`: the texture started from the game's pixels, so the export needs the yes.
    let zip = s.path().join("shared/docks.zip");
    let to = zip.to_string_lossy().to_string();
    assert_eq!(
        golden("pack-export-derived", &["pack", "export", "--mod", "<mod>", "--to", &to], &s, &dir, &e),
        cli::FAILED
    );
    assert!(!zip.exists());
    let (code, out, _) = run(&["pack", "export", "--mod", "<mod>", "--to", &to, "--derived-ok", "--json"], &dir, &e);
    assert_eq!(code, cli::OK, "{out}");
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["derived"], 1);
    assert_eq!(doc["build"], ob_studio::share::build());
    let fresh = s.path().join("fresh-maps");
    let f = ob_studio::share::import(&fresh, &zip).unwrap();
    assert!(fresh.join(&f).join("textures/world/00000ABC.png").is_file());
    assert!(!fresh.join(&f).join("work").exists());
    // `modpack export`: the same yes for the same texture; the pack holds the race and the texture as their own files.
    let packs = s.path().join("packs");
    let pto = packs.to_string_lossy().to_string();
    assert_eq!(
        golden("modpack-export-derived", &["modpack", "export", "--mod", "<mod>", "--to", &pto], &s, &dir, &e),
        cli::FAILED
    );
    assert!(!packs.exists());
    let (code, out, _) = run(
        &["modpack", "export", "--mod", "<mod>", "--to", &pto, "--derived-ok", "--version", "1.2.0", "--json"],
        &dir,
        &e,
    );
    assert_eq!(code, cli::OK, "{out}");
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["id"], "docks");
    let file = packs.join("docks-1.2.0.obpk");
    assert_eq!(doc["to"], file.to_string_lossy().as_ref());
    let b = ob_asset_format::bundle::Bundle::open(&file, ob_asset_format::bundle::Kind::PackFile).unwrap();
    b.verify().unwrap();
    for f in ["pack.toml", "provenance.toml", "races/sprint.toml", "world/_any/textures/images/0x00000ABC.png"] {
        assert!(b.get(f).is_some(), "{f}");
    }
    assert!(b.get("map.obmap").is_none());
    assert_eq!(
        run(&["modpack", "export", "--mod", "<mod>", "--to", &pto, "--derived-ok", "--version", "one"], &dir, &e).0,
        cli::USAGE
    );
    // `modpack convert`: the plain mod (its zip from `pack export`) becomes a pack, refused when it is neither kind.
    let conv = s.path().join("converted");
    let (cto, czip) = (conv.to_string_lossy().to_string(), zip.to_string_lossy().to_string());
    let words = ["modpack", "convert", "--from", &czip, "--to", &cto, "--id", "docks-old", "--derived-ok"];
    assert_eq!(golden("modpack-convert", &words, &s, &dir, &e), cli::OK);
    let b = ob_asset_format::bundle::Bundle::open(
        &conv.join("docks-old-1.0.0.obpk"),
        ob_asset_format::bundle::Kind::PackFile,
    )
    .unwrap();
    let prov = String::from_utf8(b.read(b.get("provenance.toml").unwrap()).unwrap().into_owned()).unwrap();
    assert!(prov.contains("converted = true"), "{prov}");
    let nothing = s.path().join("nothing");
    std::fs::create_dir_all(&nothing).unwrap();
    let nothing = nothing.to_string_lossy().to_string();
    let (code, _, err) = run(&["modpack", "convert", "--from", &nothing, "--to", &cto], &dir, &e);
    assert_eq!(code, cli::FAILED);
    assert!(err.contains("neither a map mod"), "{err}");
    let h =
        History::open_read(&dir, &dir.join(store::EDITOR_DIR), ob_studio::MOD_DOCS, std::sync::Arc::new(disk::Real))
            .unwrap();
    assert_eq!(h.steps().last().map(|s| s.label.as_str()), Some("Paint ROAD"));
    // `looks watch` for a moment: nothing new saved, nothing made.
    let (code, out, _) = run(&["looks", "watch", "--mod", "<mod>", "--seconds", "0.2", "--json"], &dir, &e);
    assert_eq!(code, cli::OK);
    assert_eq!(serde_json::from_str::<Value>(&out).unwrap()["saves"], serde_json::json!([]));
}

/// Every edit the tool reports made leaves a mod that reads: a value the file format refuses (more than one lap
/// for a sprint, a rotation that is not one) is refused before anything is written, the dry run says the same,
/// and the mod still opens afterwards. Each case once wrote a `map.obmap` that no command could read again.
#[test]
fn an_edit_never_leaves_a_mod_that_does_not_read() {
    let s = Scratch::new("cli-unreadable");
    let dir = s.path().join("m");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(store::FILE), "overbright-map 1\nname = L\n").unwrap();
    let e = env();
    let ok = |words: &[&str]| {
        let (code, out, err) = run(words, &dir, &e);
        assert_eq!(code, cli::OK, "{words:?}: {out}{err}");
    };
    ok(&["race", "create", "--mod", "<mod>", "--track", "4001", "--id", "s1"]);
    let ops = s.path().join("ops.jsonl");
    std::fs::write(
        &ops,
        "{\"op\":\"set_kind\",\"race\":\"s1\",\"kind\":\"sprint\"}\n{\"op\":\"set_laps\",\"race\":\"s1\",\"laps\":5}\n",
    )
    .unwrap();
    let file = ops.to_string_lossy().to_string();
    let to = r#"{"id":"s1","name":"s1","track":4001,"backward":false,"kind":"sprint","laps":7,"checkpoints":[],"grid":[],"start":null,"finish":null}"#;
    let zero = "0,0,0,0,0,0,0,0,0";
    let bad: [&[&str]; 6] = [
        &["race", "set", "--mod", "<mod>", "--race", "s1", "--laps", "3"],
        &["race", "set", "--mod", "<mod>", "--race", "s1", "--laps", "3", "--kind", "sprint"],
        &["race", "put", "--mod", "<mod>", "--race", "s1", "--to", to],
        &["ops", "apply", "--mod", "<mod>", "--file", &file],
        &["props", "move", "--mod", "<mod>", "--key", "added:a1", "--position", "1,1,1", "--rotation", zero],
        &["props", "add", "--mod", "<mod>", "--solid", "BOX", "--position", "1,2,3", "--rotation", zero],
    ];
    let before = text(&dir);
    for words in bad {
        for dry in [true, false] {
            let mut w = words.to_vec();
            if dry {
                w.push("--dry-run");
            }
            w.push("--json");
            let (code, out, _) = run(&w, &dir, &e);
            let doc: Value = serde_json::from_str(&out).unwrap();
            assert_eq!(code, cli::FAILED, "{w:?} is refused: {out}");
            assert_eq!(doc["error"]["reason"], "bad_value", "{w:?}: {out}");
            let detail = doc["error"]["detail"].as_str().unwrap_or_default();
            assert!(detail.contains("one lap") || detail.contains("unit axes"), "{w:?} says why: {out}");
            assert_eq!(text(&dir), before, "{w:?} wrote nothing");
            let (code, out, err) = run(&["history", "show", "--mod", "<mod>"], &dir, &e);
            assert_eq!(code, cli::OK, "the mod still reads after {w:?}: {out}{err}");
        }
    }
    // The same race as a circuit takes its laps.
    ok(&["race", "set", "--mod", "<mod>", "--race", "s1", "--kind", "circuit", "--laps", "3"]);
    assert!(MapMod::parse(&text(&dir)).unwrap().races[0].laps == 3);
}

/// A mod an older tool left unreadable (a sprint with 3 laps) says why on every command and how to repair it;
/// `pack repair` makes the one safe repair as one step and the mod reads again. A file with no safe repair is
/// refused as it is.
#[test]
fn a_mod_left_unreadable_says_so_and_pack_repair_mends_it() {
    let s = Scratch::new("cli-repair");
    let dir = s.path().join("b");
    std::fs::create_dir_all(&dir).unwrap();
    let broken = "overbright-map 1\nname = B\n\n[race]\nid = s1\ntrack = 4001\nmode = sprint\nlaps = 3\n";
    std::fs::write(dir.join(store::FILE), broken).unwrap();
    let e = env();
    let (code, out, _) = run(&["history", "show", "--mod", "<mod>", "--json"], &dir, &e);
    assert_eq!(code, cli::FAILED);
    let doc: Value = serde_json::from_str(&out).unwrap();
    let msg = doc["error"]["message"].as_str().unwrap();
    assert!(msg.contains("line 8: a sprint race runs one lap") && msg.contains("pack repair"), "{msg}");
    assert_eq!(doc["error"]["repair"]["fixes"].as_array().unwrap().len(), 1, "{out}");
    let (code, _, _) = run(&["pack", "repair", "--mod", "<mod>", "--dry-run"], &dir, &e);
    assert_eq!((code, text(&dir).as_str()), (cli::OK, broken), "a dry run writes nothing");
    let (code, out, err) = run(&["pack", "repair", "--mod", "<mod>"], &dir, &e);
    assert_eq!(code, cli::OK, "{out}{err}");
    let m = MapMod::parse(&text(&dir)).expect("reads again");
    assert_eq!((m.races[0].kind, m.races[0].laps), (RaceKind::Sprint, 1));
    let (code, out, _) = run(&["pack", "repair", "--mod", "<mod>"], &dir, &e);
    assert!(code == cli::OK && out.contains("Nothing to repair"), "{out}");
    // No safe repair: refused, the file kept.
    let zero =
        "overbright-map 1\n\n[prop.add]\nregion = L4RA\nsolid = X\nposition = 0 0 0\nrotation = 0 0 0 0 0 0 0 0 0\n";
    std::fs::write(dir.join(store::FILE), zero).unwrap();
    let (code, _, err) = run(&["pack", "repair", "--mod", "<mod>"], &dir, &e);
    assert!(code == cli::FAILED && err.contains("no safe repair"), "{err}");
    assert_eq!(text(&dir), zero);
}

#[test]
fn pack_create_makes_a_new_mod_and_refuses_clashes_and_bad_names() {
    let (s, _) = fixture("cli-create", false);
    let maps = s.path().to_path_buf();
    let e = no_game();
    let cases: &[(&str, &[&str], i32)] = &[
        ("pack-create", &["pack", "create", "--name", "Harbour edits", "--maps", "<mod>"], cli::OK),
        (
            "pack-create-folder",
            &["pack", "create", "--name", "Night lights", "--folder", "night_2", "--maps", "<mod>"],
            cli::OK,
        ),
        ("pack-create-folder-used", &["pack", "create", "--name", "Harbour edits", "--maps", "<mod>"], cli::FAILED),
        (
            "pack-create-name-used",
            &["pack", "create", "--name", "docks", "--folder", "d2", "--maps", "<mod>"],
            cli::FAILED,
        ),
        ("pack-create-no-name", &["pack", "create", "--name", "  ", "--maps", "<mod>"], cli::USAGE),
        (
            "pack-create-bad-folder",
            &["pack", "create", "--name", "X", "--folder", "../up", "--maps", "<mod>"],
            cli::USAGE,
        ),
        ("pack-create-missing-name", &["pack", "create", "--maps", "<mod>"], cli::USAGE),
    ];
    for (case, words, code) in cases {
        assert_eq!(golden(case, words, &s, &maps, &e), *code, "{case}");
    }
    assert_eq!(store::list(&maps), ["docks", "harbour-edits", "night_2"]);
    let made = store::read_file(&maps.join("harbour-edits").join(store::FILE)).unwrap();
    assert_eq!(made.name, "Harbour edits");
    assert!(made.props.is_empty() && made.races.is_empty());
    // The new mod is one the other commands take.
    let (code, out, _) = run(&["history", "show", "--mod", "<mod>/harbour-edits"], &maps, &e);
    assert_eq!(code, cli::OK, "{out}");
    // The plain text says where it is and how to edit it.
    let (code, out, _) = run(&["pack", "create", "--name", "Third", "--maps", "<mod>"], &maps, &e);
    assert_eq!(code, cli::OK);
    assert!(out.contains("Made Third in ") && out.contains("--mod "), "{out}");
}

/// A start written at a height with no road (z = 0 under the road, or none given) takes the road's height when
/// the game's world is at hand; a point with no floor under it at all is an error of the checks.
#[test]
fn race_points_take_the_road_height_and_the_checks_look_under_them() {
    let (_s, dir) = fixture("cli-floor", false);
    // A flat floor at z = 0 for 200 m around the origin; the sprint's finish (x = 390) is off it.
    let flat = Env { world: Box::new(|_, _| Ok(World::flat(200.0))), ..env() };
    let point = |verb: &str, part: &str, at: &str| {
        let w = ["race", "point", verb, "--mod", "<mod>", "--race", "sprint", "--part", part, "--position", at];
        run(&w, &dir, &flat)
    };
    let (code, _, err) = point("add", "start", "30,0,-30");
    assert_eq!(code, cli::OK, "{err}");
    let map = store::read_file(&dir.join(store::FILE)).unwrap();
    assert_eq!(map.races[0].start.unwrap().position, Vec3::new(30.0, 0.0, 0.0), "on the road, not 30 m under it");
    // The default grid's first slot moved past the floor's edge.
    let (code, _, err) = point("move", "grid", "300,0,-4");
    assert_eq!(code, cli::OK, "{err}");
    let map = store::read_file(&dir.join(store::FILE)).unwrap();
    assert_eq!(map.races[0].grid[0].position, Vec3::new(300.0, 0.0, -4.0), "no road there: kept as given");
    let (code, out, _) = run(&["checks", "run", "--mod", "<mod>"], &dir, &flat);
    assert_eq!(code, cli::CHECKS, "{out}");
    assert!(out.contains("grid slot 1 has no floor under it") && out.contains("fall through the world"), "{out}");
    assert!(out.contains("the finish line has no floor under it"), "{out}");
    assert!(!out.contains("start line has no floor"), "{out}");
}

/// The game for the list commands: a region with cones, a barrier and a sign placed, one texture named ROAD.
fn lists_env() -> Env {
    use ob_formats::formats::solid::Solid;
    let solid = |name: &str| Solid {
        name: name.into(),
        hash: 0,
        bbox_min: [-0.5, -0.5, 0.0],
        bbox_max: [0.5, 0.5, 1.0],
        textures: vec![0x0000_0ABC],
        vertices: Vec::new(),
        normals: Vec::new(),
        light_materials: Vec::new(),
        indices: vec![0; 6],
        materials: vec![ob_formats::formats::solid::Material {
            texture_slot: 0,
            light_material: 0xFF,
            first_index: 0,
            index_count: 3,
            flags: 0,
        }],
        mesh_flags: 0,
        markers: Vec::new(),
        light_markers: Vec::new(),
    };
    Env {
        world: Box::new(move |root, _| {
            let mut w = (looks_env().world)(root, "L4RA")?;
            w.solids = vec![solid("XC_CONE_01"), solid("XB_BARRIER_03"), solid("XS_SIGN_STOP")];
            let place = |solid| nfsu2::studio_api::world::Placement {
                solid,
                low_solid: None,
                mid_solid: None,
                transform: glam::Mat4::IDENTITY,
                section: 1,
                flags: 0,
                cull_radius: 5.0,
            };
            w.placements = vec![place(0), place(0), place(1), place(2)];
            Ok(w)
        }),
        ..env()
    }
}

#[test]
fn the_list_and_find_commands_read_what_the_window_reads() {
    let (s, dir) = fixture("cli-lists", false);
    let e = lists_env();
    let cases: &[(&str, &[&str], i32)] = &[
        ("props-solids", &["props", "solids"], cli::OK),
        ("props-solids-filter", &["props", "solids", "--filter", "barrier"], cli::OK),
        ("props-solids-category", &["props", "solids", "--category", "signs"], cli::OK),
        ("props-solids-near-miss", &["props", "solids", "--filter", "barier"], cli::OK),
        ("props-solids-bad-category", &["props", "solids", "--category", "barier"], cli::USAGE),
        ("race-tracks", &["race", "tracks"], cli::OK),
        ("race-tracks-other-region", &["race", "tracks", "--region", "L4RB"], cli::OK),
        ("looks-find", &["looks", "find", "road"], cli::OK),
        ("looks-find-hash", &["looks", "find", "--text", "0x0ABC"], cli::OK),
        ("looks-find-near-miss", &["looks", "find", "raod"], cli::OK),
        ("looks-find-no-text", &["looks", "find"], cli::USAGE),
        ("game-find", &["game", "find", "--game", "<mod>"], cli::OK),
        ("game-find-missing-folder", &["game", "find", "--game", "<mod>/nowhere"], cli::NO_GAME),
        ("unknown-command-near-miss", &["looks", "fnd"], cli::USAGE),
    ];
    for (case, words, code) in cases {
        assert_eq!(golden(case, words, &s, &dir, &e), *code, "{case}");
    }
    // The prop browser's list and the command give the same solids.
    let (_, out, _) = run(&["props", "solids", "--category", "knock-over", "--json"], &dir, &e);
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["count"], 0, "nothing in the fixture world is knocked over");
    // Without the game the lists say so and exit 4; the tool needs no mod for them.
    for w in [&["props", "solids"][..], &["race", "tracks"], &["looks", "find", "road"], &["game", "find"]] {
        assert_eq!(run(w, &dir, &no_game()).0, cli::NO_GAME, "{w:?}");
    }
    // The plain text names the near miss.
    let (code, out, _) = run(&["props", "solids", "--filter", "barier"], &dir, &e);
    assert_eq!(code, cli::OK);
    assert!(out.contains("Did you mean `barrier`?"), "{out}");
}

/// Run files in the mod's editor folder, as the window's Runs workspace keeps them.
fn put_runs(dir: &Path) {
    use ob_studio::runs::{fixtures, library};
    let runs = dir.join(store::EDITOR_DIR).join("runs");
    let text = |seed, prop_crash| -> String {
        fixtures::lines(seed, prop_crash, false).iter().map(|l| l.write() + "\n").collect()
    };
    library::save(&runs, "seed1", &text(1, true)).unwrap();
    library::save(&runs, "seed2", &text(2, false)).unwrap();
}

#[test]
fn the_runs_commands_read_and_change_the_runs_the_window_lists() {
    let (s, dir) = fixture("cli-runs", false);
    let e = env();
    put_runs(&dir);
    let cases: &[(&str, &[&str], i32)] = &[
        ("runs-list", &["runs", "list", "--mod", "<mod>"], cli::OK),
        ("runs-show", &["runs", "show", "--mod", "<mod>", "--run", "seed1"], cli::OK),
        ("runs-telemetry", &["runs", "telemetry", "--mod", "<mod>", "--run", "seed1", "--car", "2"], cli::OK),
        ("runs-telemetry-no-car", &["runs", "telemetry", "--mod", "<mod>", "--run", "seed1", "--car", "9"], cli::FAILED),
        ("runs-compare", &["runs", "compare", "--mod", "<mod>", "--run", "seed1", "--with", "seed2"], cli::OK),
        ("runs-show-missing", &["runs", "show", "--mod", "<mod>", "--run", "nothing"], cli::FAILED),
        ("runs-show-bad-name", &["runs", "show", "--mod", "<mod>", "--run", "../x"], cli::USAGE),
        ("runs-rename", &["runs", "rename", "--mod", "<mod>", "--run", "seed2", "--to", "other"], cli::OK),
        ("runs-rename-taken", &["runs", "rename", "--mod", "<mod>", "--run", "seed1", "--to", "other"], cli::FAILED),
        ("runs-delete", &["runs", "delete", "--mod", "<mod>", "--run", "other"], cli::OK),
        ("runs-delete-gone", &["runs", "delete", "--mod", "<mod>", "--run", "other"], cli::FAILED),
        ("runs-dry-run-refused", &["runs", "delete", "--mod", "<mod>", "--run", "seed1", "--dry-run"], cli::USAGE),
    ];
    for (case, words, code) in cases {
        assert_eq!(golden(case, words, &s, &dir, &e), *code, "{case}");
    }
    // The telemetry is the window's: speed from the file, the racing line from the mod's race.
    let (_, out, _) = run(&["runs", "telemetry", "--mod", "<mod>", "--run", "seed1", "--car", "2", "--json"], &dir, &e);
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert!(doc["lateral"].is_null(), "the run's race `r` is not in the mod: no line to measure from");
    assert_eq!(doc["speed"]["max"], 20.0);
    // Deleting left the other run alone.
    assert!(dir.join(store::EDITOR_DIR).join("runs").join("seed1.obrun").is_file());
}

#[test]
fn the_data_commands_read_the_grid_and_write_cells_as_undo_steps() {
    let (s, dir) = fixture("cli-data", true);
    let e = env();
    let key = "docks:sprint";
    let set = |v: &'static str| ["data", "set", "--mod", "<mod>", "--kind", "race", "--row", key, "--field", "name", "--value", v];
    let cases: &[(&str, &[&str], i32)] = &[
        ("data-kinds", &["data", "kinds", "--mod", "<mod>"], cli::OK),
        ("data-show-race", &["data", "show", "--mod", "<mod>", "--kind", "race"], cli::OK),
        ("data-show-search", &["data", "show", "--mod", "<mod>", "--kind", "race", "--search", "short"], cli::OK),
        ("data-show-sorted", &["data", "show", "--mod", "<mod>", "--kind", "race", "--sort", "name", "--desc"], cli::OK),
        ("data-show-filter", &["data", "show", "--mod", "<mod>", "--kind", "race", "--filter", "name=sprint"], cli::OK),
        ("data-show-bad-kind", &["data", "show", "--mod", "<mod>", "--kind", "rac"], cli::USAGE),
        ("data-show-bad-column", &["data", "show", "--mod", "<mod>", "--kind", "race", "--sort", "colour"], cli::USAGE),
        ("data-set-dry-run", &[&set("Docks run")[..], &["--dry-run"]].concat(), cli::OK),
        ("data-set", &set("Docks run"), cli::OK),
        ("data-set-unchanged", &set("Docks run"), cli::OK),
        ("data-set-read-only-column", &["data", "set", "--mod", "<mod>", "--kind", "race", "--row", key, "--field", "checkpoints", "--value", "3"], cli::FAILED),
        ("data-set-bad-value", &["data", "set", "--mod", "<mod>", "--kind", "race", "--row", key, "--field", "laps", "--value", "many"], cli::FAILED),
    ];
    for (case, words, code) in cases {
        assert_eq!(golden(case, words, &s, &dir, &e), *code, "{case}");
    }
    // The cell is the race's name, in one history step the window shows and can undo.
    assert!(text(&dir).contains("name = Docks run"));
    let (_, out, _) = run(&["history", "show", "--mod", "<mod>", "--json"], &dir, &e);
    assert!(out.contains("Docks run") || out.contains("Rename"), "{out}");
}

#[test]
fn the_browser_commands_read_the_regions_index() {
    let (s, dir) = fixture("cli-browser", false);
    let e = lists_env();
    let cases: &[(&str, &[&str], i32)] = &[
        ("browser-search", &["browser", "search"], cli::OK),
        ("browser-search-words", &["browser", "search", "cone"], cli::OK),
        ("browser-search-models", &["browser", "search", "--text", "x", "--kind", "models", "--category", "signs"], cli::OK),
        ("browser-search-textures", &["browser", "search", "--kind", "textures", "--textures", "unused"], cli::OK),
        ("browser-search-bad-kind", &["browser", "search", "--kind", "model"], cli::USAGE),
        ("browser-used", &["browser", "used", "cone"], cli::OK),
        ("browser-used-none", &["browser", "used", "zebra"], cli::OK),
        ("browser-used-no-text", &["browser", "used"], cli::USAGE),
    ];
    for (case, words, code) in cases {
        assert_eq!(golden(case, words, &s, &dir, &e), *code, "{case}");
    }
    for w in [&["browser", "search"][..], &["browser", "used", "cone"]] {
        assert_eq!(run(w, &dir, &no_game()).0, cli::NO_GAME, "{w:?}");
    }
}

#[test]
fn collision_show_counts_what_each_mode_of_the_layer_draws() {
    let (s, dir) = fixture("cli-collision", false);
    let e = lists_env();
    let cases: &[(&str, &[&str], i32)] = &[
        ("collision-show", &["collision", "show"], cli::OK),
        ("collision-show-invisible", &["collision", "show", "--mode", "invisible"], cli::OK),
        ("collision-show-off", &["collision", "show", "--mode", "off", "--mod", "<mod>"], cli::OK),
        ("collision-show-bad-mode", &["collision", "show", "--mode", "some"], cli::USAGE),
    ];
    for (case, words, code) in cases {
        assert_eq!(golden(case, words, &s, &dir, &e), *code, "{case}");
    }
    assert_eq!(run(&["collision", "show"], &dir, &no_game()).0, cli::NO_GAME);
}

/// A region with one model that has real geometry (two parts) for the export and import commands.
fn model_env() -> Env {
    use ob_formats::formats::solid::{Material, Solid, Vertex};
    let v = |p: [f32; 3], uv: [f32; 2]| Vertex { position: p, color: [255; 4], uv };
    let solid = Solid {
        name: "XC_CONE_01".into(),
        hash: 0x55,
        bbox_min: [0.0; 3],
        bbox_max: [1.0, 1.0, 2.0],
        textures: vec![0x0000_0ABC, 0x0000_0ABD],
        vertices: vec![
            v([0.0, 0.0, 0.0], [0.0, 0.0]),
            v([1.0, 0.0, 0.0], [1.0, 0.0]),
            v([1.0, 1.0, 2.0], [1.0, 1.0]),
            v([0.0, 1.0, 2.0], [0.0, 1.0]),
        ],
        normals: Vec::new(),
        light_materials: Vec::new(),
        indices: vec![0, 1, 2, 0, 2, 3],
        materials: vec![
            Material { texture_slot: 0, light_material: 0xFF, first_index: 0, index_count: 3, flags: 0 },
            Material { texture_slot: 1, light_material: 0xFF, first_index: 3, index_count: 3, flags: 0 },
        ],
        mesh_flags: 0,
        markers: Vec::new(),
        light_markers: Vec::new(),
    };
    Env {
        world: Box::new(move |root, _| {
            let mut w = (looks_env().world)(root, "L4RA")?;
            w.solids = vec![solid.clone()];
            let place = nfsu2::studio_api::world::Placement {
                solid: 0,
                low_solid: None,
                mid_solid: None,
                transform: glam::Mat4::IDENTITY,
                section: 1,
                flags: 0,
                cull_radius: 5.0,
            };
            w.placements = vec![place];
            Ok(w)
        }),
        ..env()
    }
}

#[test]
fn a_model_goes_out_as_gltf_and_an_edited_one_comes_back_as_a_mod_model() {
    let (s, dir) = fixture("cli-model", false);
    let e = model_env();
    // Out: the reference lands in the mod's work folder, marked; the game folder is never a target.
    assert_eq!(golden("browser-export", &["browser", "export", "cone", "--mod", "<mod>"], &s, &dir, &e), cli::OK);
    let reference = dir.join(".editor/reference");
    assert!(reference.join("XC_CONE_01.gltf").is_file() && reference.join("LOCAL-ONLY.txt").is_file());
    let r = run(&["browser", "export", "cone", "--mod", "<mod>", "--to", "game/models"], &dir, &e); assert_eq!(r.0, cli::FAILED, "{r:?}");
    assert_eq!(run(&["browser", "export", "zebra", "--mod", "<mod>"], &dir, &e).0, cli::FAILED);
    assert_eq!(run(&["browser", "export", "cone"], &dir, &e).0, cli::USAGE);

    // Back: the unedited file maps onto the two parts and stays on this computer.
    let file = reference.join("XC_CONE_01.gltf").to_string_lossy().into_owned();
    let imp = ["browser", "import", "--mod", "<mod>", "--file", file.as_str()];
    assert_eq!(golden("browser-import", &imp, &s, &dir, &e), cli::OK);
    assert!(dir.join(".editor/models/XC_CONE_01.gltf").is_file());
    assert!(dir.join(".editor/models/XC_CONE_01.model.toml").is_file());
    assert!(!dir.join("models").exists(), "a model from the game's files is never shared");
    // Own work: refused while the shape is the game's own and the file says so; accepted once edited.
    let own = [&imp[..], &["--own"]].concat();
    assert_eq!(golden("browser-import-own-refused", &own, &s, &dir, &e), cli::FAILED);
    let bin = reference.join("XC_CONE_01.bin");
    let mut b = std::fs::read(&bin).unwrap();
    b[0..4].copy_from_slice(&5.0f32.to_le_bytes());
    std::fs::write(&bin, b).unwrap();
    let mut g: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    g["asset"]["extras"] = serde_json::json!({ "model": "XC_CONE_01" });
    std::fs::write(&file, serde_json::to_vec_pretty(&g).unwrap()).unwrap();
    assert_eq!(golden("browser-import-own", &own, &s, &dir, &e), cli::OK);
    assert!(dir.join("models/XC_CONE_01.gltf").is_file());
    // Refusals say why: a model the file is not for, a part that is not the model's, no model named.
    assert_eq!(golden("browser-import-other-model", &[&imp[..], &["--model", "zebra"]].concat(), &s, &dir, &e), cli::FAILED);
    g["materials"][0]["name"] = serde_json::json!("CHROME");
    std::fs::write(&file, serde_json::to_vec_pretty(&g).unwrap()).unwrap();
    assert_eq!(golden("browser-import-unmapped", &imp, &s, &dir, &e), cli::FAILED);
    g["asset"].as_object_mut().unwrap().remove("extras");
    std::fs::write(&file, serde_json::to_vec_pretty(&g).unwrap()).unwrap();
    assert_eq!(run(&imp, &dir, &e).0, cli::USAGE);
    assert_eq!(run(&["browser", "import", "--mod", "<mod>"], &dir, &e).0, cli::USAGE);
    assert_eq!(run(&[&imp[..], &["--model", "cone"]].concat(), &dir, &no_game()).0, cli::NO_GAME);
}

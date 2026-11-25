//! Studio's command-line tool (`overbright-studio-cli`, docs/modding/studio-cli.md): every command of the
//! registry ([`crate::registry::COMMANDS`]) is a verb (`props move` runs `props.move`), each of its arguments a
//! flag, and the help, the flags and their checks come from the registry, so a new command is on the command line
//! without code here beyond the edit it makes ([`ops::request`]).
//!
//! - **Output**: plain text, or with `--json` one JSON document on stdout (`"format": 1`, its `exit` code
//!   included); messages for people go to stderr.
//! - **Exit codes** are fixed ([`EXIT_CODES`]): unknown words and flags are usage errors, never ignored, and a
//!   failure is never 0.
//! - **Edits** are the map edits the editor makes, as the same JSON ([`ops`]): the tool builds the edit's JSON
//!   from its flags (or reads an operations file, one edit per line), prints it as `request`, runs it on the mod
//!   and writes the result as one step of the mod's history ([`crate::history`]), named after the tool. With
//!   `--dry-run` it shows what would change (items and lines) and writes nothing. When another program (Studio)
//!   has the mod open, the edit goes to that program's inbox and the tool waits for its answer (`--wait`), or
//!   fails at once with `--no-inbox`.
//! - **Checks**: `checks run` runs the map checks ([`crate::validate`]) and exits 3 when one finds an error.

pub mod args;
pub mod diff;
pub mod looks;
pub mod ops;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::history::{self, History, Outcome, disk, inbox};
use crate::registry::{self, Command, CommandKind};
use crate::session::{Level, MapSession};
use crate::validate;
use nfsu2::studio_api::map::assets::{GameAssets, MapAssets};
use nfsu2::studio_api::world::World;
use ob_map::tools::map::format::MapMod;
use ob_map::tools::map::ops::{Op, RacePart};
use ob_map::tools::map::store;

/// The program's name.
pub const PROGRAM: &str = "overbright-studio-cli";
/// The author of the steps the tool makes, in the mod's history.
pub const AUTHOR: &str = "overbright-studio (command line)";
/// The output's format version (`"format"`).
pub const FORMAT: u32 = 1;
/// The region a mod is edited on unless `--region` says another.
pub const REGION: &str = "L4RA";
/// How long the tool waits for Studio's answer unless `--wait` says otherwise.
pub const WAIT: Duration = Duration::from_secs(10);

pub const OK: i32 = 0;
pub const FAILED: i32 = 1;
pub const USAGE: i32 = 2;
pub const CHECKS: i32 = 3;
pub const NO_GAME: i32 = 4;
pub const CRASHED: i32 = 5;
pub const TIMED_OUT: i32 = 6;
pub const BUSY: i32 = 7;
pub const CANCELLED: i32 = 130;

/// Every exit code: (code, its word in the JSON's `error.kind`, what it means).
pub const EXIT_CODES: &[(i32, &str, &str)] = &[
    (OK, "ok", "done"),
    (FAILED, "failed", "the command failed: an edit refused, a conflict, a file that does not read"),
    (USAGE, "usage", "the command line is wrong: an unknown command or flag, a missing or bad value"),
    (CHECKS, "checks", "the checks found an error"),
    (NO_GAME, "no_game", "the game's data is missing or unsupported (give --game)"),
    (CRASHED, "crashed", "the game, run as a child, crashed"),
    (TIMED_OUT, "timed_out", "the program that has the mod open did not answer in time"),
    (BUSY, "busy", "another program has the mod open and --no-inbox was given"),
    (CANCELLED, "cancelled", "cancelled (Ctrl+C)"),
];

fn kind_of(code: i32) -> &'static str {
    EXIT_CODES.iter().find(|c| c.0 == code).map_or("failed", |c| c.1)
}

/// What the tool reads of the game: the region's world (for prop edits) and the road networks (for the checks).
pub struct Env {
    /// The game folder unless `--game` gives one: `NFSU2_ROOT`, else the one the launcher set (as the editor and
    /// the game find it).
    pub game: Option<PathBuf>,
    /// The region `.1` of the game folder `.0` as loaded without map mods.
    pub world: Box<dyn Fn(&Path, &str) -> Result<World, String>>,
    /// The game data the checks read, from the game folder.
    pub assets: Box<dyn Fn(&Path) -> Box<dyn MapAssets>>,
    /// How often an answer from the inbox is looked for.
    pub poll: Duration,
}

impl Env {
    /// The game as the player set it up.
    pub fn system() -> Env {
        Env {
            game: nfsu2::studio_api::qol::setup::game_root(),
            world: Box::new(|root, region| World::load(root, region).map_err(|e| format!("{e:#}"))),
            assets: Box::new(|root| Box::new(GameAssets::new(root.to_path_buf()))),
            poll: Duration::from_millis(50),
        }
    }
}

/// A command's end: its exit code, its JSON document and the lines for people.
struct Done {
    code: i32,
    doc: Value,
    text: Vec<String>,
}

fn fail(code: i32, message: impl Into<String>) -> Done {
    let message = message.into();
    Done {
        code,
        text: vec![format!("error: {message}")],
        doc: json!({ "error": { "kind": kind_of(code), "message": message } }),
    }
}

/// Run the tool on `argv` (without the program's name): the exit code. The JSON goes to `out`, the text to `out`
/// (results) and `err` (errors).
pub fn run(argv: &[String], env: &Env, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let as_json = argv.iter().any(|a| a == "--json");
    let (name, d) = match args::parse(argv) {
        Err(e) => (None, fail(USAGE, format!("{e} (see `{PROGRAM} help`)"))),
        Ok(line) => (line.command.map(|c| c.name), dispatch(&line, env)),
    };
    if as_json {
        let mut doc = serde_json::Map::new();
        doc.insert("format".into(), json!(FORMAT));
        if let Some(n) = name {
            doc.insert("command".into(), json!(n));
        }
        doc.insert("ok".into(), json!(d.code == OK));
        doc.insert("exit".into(), json!(d.code));
        if let Value::Object(m) = d.doc {
            doc.extend(m);
        }
        let _ = writeln!(out, "{}", serde_json::to_string_pretty(&Value::Object(doc)).unwrap_or_default());
    } else {
        let to: &mut dyn Write = if d.code == OK || d.code == CHECKS { out } else { err };
        for l in &d.text {
            let _ = writeln!(to, "{l}");
        }
    }
    d.code
}

fn dispatch(line: &args::Line, env: &Env) -> Done {
    if let Some(words) = &line.help_for {
        return help(args::command_of_words(words), words);
    }
    let Some(cmd) = line.command else {
        return if line.switch("help") {
            help(None, &[])
        } else {
            fail(USAGE, format!("no command given (see `{PROGRAM} help`)"))
        };
    };
    if line.switch("help") {
        return help(Some(cmd), &[]);
    }
    if line.switch("dry-run") && cmd.kind != CommandKind::Edit {
        return fail(USAGE, format!("--dry-run is for edits; `{}` changes nothing", words(cmd)));
    }
    match cmd.name {
        "schema.export" => {
            let doc: Value = serde_json::from_str(&registry::export().pretty()).unwrap_or_default();
            Done { code: OK, text: vec![registry::export().pretty()], doc: json!({ "schema": doc }) }
        }
        "pack.create" => create(line),
        "checks.run" => with_mod(line, |m| checks(line, env, m)),
        "history.show" => with_mod(line, history_show),
        "pack.export" => with_mod(line, |m| export(line, m)),
        "looks.open" | "looks.import" | "looks.watch" | "looks.list" => {
            with_mod(line, |m| crate::cli::looks::run(cmd.name, line, env, m))
        }
        _ if cmd.kind == CommandKind::Edit => with_mod(line, |m| edit(line, cmd, env, m)),
        other => fail(USAGE, format!("`{other}` is not on the command line yet")),
    }
}

fn words(cmd: &Command) -> String {
    cmd.name.replace('.', " ")
}

/// The mod a command works on.
struct Mod {
    dir: PathBuf,
    edir: PathBuf,
    folder: String,
}

fn with_mod(line: &args::Line, f: impl FnOnce(&Mod) -> Done) -> Done {
    let Some(dir) = line.flag("mod").filter(|d| !d.is_empty()) else {
        return fail(USAGE, "--mod is needed: the mod's folder (the one holding map.obmap)");
    };
    let dir = PathBuf::from(dir);
    if !dir.join(store::FILE).is_file() {
        return fail(FAILED, format!("no {} in {}", store::FILE, dir.display()));
    }
    let folder = store::folder_of_file(&dir.join(store::FILE));
    f(&Mod { edir: dir.join(store::EDITOR_DIR), dir, folder })
}

fn open_read(m: &Mod) -> Result<(History, MapMod), Done> {
    let h = History::open_read(&m.dir, &m.edir, crate::MOD_DOCS, Arc::new(disk::Real))
        .map_err(|e| fail(FAILED, format!("{}: {e}", m.dir.display())))?;
    let map = read_map(&h)?;
    Ok((h, map))
}

fn read_map(h: &History) -> Result<MapMod, Done> {
    let text = h.doc(store::FILE).unwrap_or_default();
    MapMod::parse(text).map_err(|e| {
        let mut message = format!("{} does not read: {e}", store::FILE);
        let fixes = repairs(text);
        if !fixes.is_empty() {
            message.push_str(&format!(". {}", repair_hint(h.dir())));
        }
        let mut d = fail(FAILED, message);
        if !fixes.is_empty() {
            d.doc["error"]["repair"] = json!({ "command": "pack repair", "fixes": fixes });
            d.text.extend(fixes.iter().map(|f| format!("  repair: {f}")));
        }
        d
    })
}

/// The safe repairs a map file that does not read has ([`MapMod::parse_repairing`]); empty when it reads as it
/// is or has none (then it is refused as it is).
pub fn repairs(text: &str) -> Vec<String> {
    match (MapMod::parse(text), MapMod::parse_repairing(text)) {
        (Err(_), Ok((_, fixes))) => fixes,
        _ => Vec::new(),
    }
}

/// What to run to repair the mod in `dir`.
pub fn repair_hint(dir: &Path) -> String {
    format!("It can be repaired safely: {PROGRAM} pack repair --mod {}", dir.display())
}

fn game_root(line: &args::Line, env: &Env) -> Result<PathBuf, Done> {
    match line.flag("game").filter(|g| !g.is_empty()).map(PathBuf::from).or_else(|| env.game.clone()) {
        Some(p) => Ok(p),
        None => Err(fail(
            NO_GAME,
            "the game folder is not known: give --game <folder>, set NFSU2_ROOT, or choose it once in the launcher",
        )),
    }
}

// ---- edits --------------------------------------------------------------------------------------------------------

/// Whether running `op` reads the region's world (a prop edit does).
fn needs_world(op: &Op) -> bool {
    match op {
        Op::Place { .. }
        | Op::Delete { .. }
        | Op::Reset { .. }
        | Op::Add { .. }
        | Op::Duplicate { .. }
        | Op::DropToGround { .. } => true,
        Op::Group { ops, .. } => ops.iter().any(needs_world),
        _ => false,
    }
}

/// Whether `op` writes a start or finish line or a grid slot, whose height comes from the road under it when the
/// game's world is at hand (it is read when the game is known; without it the height stays as given).
fn wants_floor(op: &Op) -> bool {
    match op {
        Op::AddPoint { part, .. } => *part != RacePart::Checkpoint,
        Op::MovePoint { point, .. } => point.part != RacePart::Checkpoint,
        Op::PutRace { .. } => true,
        Op::Group { ops, .. } => ops.iter().any(wants_floor),
        _ => false,
    }
}

/// `NoPlacement` → `no_placement`.
fn snake(debug: &str) -> String {
    let mut s = String::new();
    for (i, c) in debug.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                s.push('_');
            }
            s.push(c.to_ascii_lowercase());
        } else {
            s.push(c);
        }
    }
    s
}

fn edit(line: &args::Line, cmd: &Command, env: &Env, m: &Mod) -> Done {
    match try_edit(line, cmd, env, m) {
        Ok(d) | Err(d) => d,
    }
}

fn try_edit(line: &args::Line, cmd: &Command, env: &Env, m: &Mod) -> Result<Done, Done> {
    let dry = line.switch("dry-run");
    let wait = match line.flag("wait") {
        None => WAIT,
        Some(t) => Duration::from_secs_f64(
            t.parse::<f64>()
                .ok()
                .filter(|s| s.is_finite() && *s >= 0.0)
                .ok_or_else(|| fail(USAGE, format!("--wait: expected seconds, got `{t}`")))?,
        ),
    };
    let region = line.flag("region").unwrap_or(REGION).to_string();
    // A dry run only reads; an edit takes the mod for writing, unless another program has it.
    let mut h = if dry {
        History::open_read(&m.dir, &m.edir, crate::MOD_DOCS, Arc::new(disk::Real))
    } else {
        History::open(&m.dir, &m.edir, crate::MOD_DOCS, AUTHOR, Arc::new(disk::Real))
    }
    .map_err(|e| fail(FAILED, format!("{}: {e}", m.dir.display())))?;
    let before = h.doc(store::FILE).unwrap_or_default().to_string();
    let repair = cmd.name == "pack.repair";
    let (map, request) = if repair {
        // The mod as it reads with its safe repairs, put in place as one step from an empty session (the file
        // as it is does not read, so nothing starts from it).
        let (fixed, fixes) = MapMod::parse_repairing(&before)
            .map_err(|e| fail(FAILED, format!("{} does not read and has no safe repair: {e}", store::FILE)))?;
        if fixes.is_empty() {
            let doc = json!({ "outcome": "unchanged", "fixes": [] });
            return Ok(Done {
                code: OK,
                doc,
                text: vec![format!("Nothing to repair: {} reads as it is", store::FILE)],
            });
        }
        let request =
            json!({ "op": "put_mod", "map": serde_json::to_value(&fixed).map_err(|e| fail(FAILED, e.to_string()))? });
        (MapMod::default(), request)
    } else {
        let map = read_map(&h)?;
        let request = ops::request(cmd.name, &line.args, &map, Path::new("")).map_err(|e| fail(USAGE, e))?;
        (map, request)
    };
    let op = ops::read(&request).map_err(|e| fail(USAGE, e))?;
    let world = if needs_world(&op) {
        let root = game_root(line, env)?;
        (env.world)(&root, &region).map_err(|e| fail(NO_GAME, format!("the game's world does not load: {e}")))?
    } else if wants_floor(&op) {
        game_root(line, env).ok().and_then(|root| (env.world)(&root, &region).ok()).unwrap_or_default()
    } else {
        World::default()
    };
    let mut session = MapSession::new(&region, &m.folder, map, world);
    // `props move --heading`: the prop's own rotation turned to the heading, its tilt kept (the Properties panel's
    // Heading); an upright one when the world does not have the prop (the session then refuses the move).
    let op = match (op, line.args.get("heading").and_then(Value::as_f64)) {
        (Op::Place { key, position, rotation }, Some(h)) if cmd.name == "props.move" => {
            let rotation = session
                .placement_of(&key)
                .and_then(|i| nfsu2::studio_api::map::edit::placement_parts(&session.world, i))
                .map_or(rotation, |(_, r)| nfsu2::studio_api::map::edit::with_heading(r, h as f32));
            Op::Place { key, position, rotation }
        }
        (op, _) => op,
    };
    let mut change = match session.run(op) {
        Ok(c) => c,
        Err(r) => {
            let why = session.refused_why.clone();
            let mut d =
                fail(FAILED, format!("refused: {r}{}", why.as_ref().map(|w| format!(": {w}")).unwrap_or_default()));
            d.doc["error"]["reason"] = json!(snake(&format!("{r:?}")));
            if let Some(w) = why {
                d.doc["error"]["detail"] = json!(w);
            }
            d.doc["request"] = request;
            return Err(d);
        }
    };
    if repair {
        change.label = "Repair mod".into();
    }
    let after = session.map.write();
    // The writer never leaves a file its reader refuses (the session checks every edit; this is the last word).
    if let Err(e) = MapMod::parse(&after) {
        let mut d = fail(FAILED, format!("refused: the edit would leave {} unreadable: {e}", store::FILE));
        d.doc["error"]["reason"] = json!("bad_value");
        d.doc["request"] = request;
        return Err(d);
    }
    let mut doc = json!({
        "request": request,
        "label": change.label,
        "change": serde_json::to_value(&change).unwrap_or_default(),
    });
    if !change.changed || after == before {
        doc["outcome"] = json!("unchanged");
        return Ok(Done { code: OK, doc, text: vec![format!("Nothing to change: {}", change.label)] });
    }
    let items = diff::items(store::FILE, &before, &after);
    let lines = diff::lines(&before, &after, 2);
    doc["diff"] = json!({ "items": items, "lines": lines });
    let mut text = Vec::new();
    let summary: Vec<String> = items
        .iter()
        .map(|i| format!("  {} {}", i["change"].as_str().unwrap_or(""), i["item"].as_str().unwrap_or("")))
        .collect();
    if dry {
        doc["outcome"] = json!("dry_run");
        text.push(format!("Dry run, nothing written. {} would change {} item(s):", change.label, items.len()));
        text.extend(summary);
        text.extend(lines.iter().map(|l| format!("    {l}")));
        return Ok(Done { code: OK, doc, text });
    }
    let docs = BTreeMap::from([(store::FILE.to_string(), Some(after))]);
    if h.writable() {
        let seq = match h.transact(AUTHOR, &change.label, None, &docs, None) {
            Ok(Outcome::Made(seq)) => seq,
            Ok(Outcome::Unchanged) => h.seq(),
            Ok(Outcome::Refused(why)) => return Err(fail(FAILED, format!("refused: {why}"))),
            Ok(o) => return Err(fail(FAILED, format!("not made: {o:?}"))),
            Err(e) => return Err(fail(FAILED, e.to_string())),
        };
        h.save().map_err(|e| fail(FAILED, format!("not saved: {e}")))?;
        doc["outcome"] = json!("made");
        doc["via"] = json!("file");
        doc["step"] = json!(seq);
        text.push(format!("Made step {seq}: {}", change.label));
        text.extend(summary);
        return Ok(Done { code: OK, doc, text });
    }
    // Another program has the mod open: ask it.
    let owner = match h.read_only_by() {
        Some(Some(o)) => format!("{} (process {})", o.program, o.pid),
        _ => "another program".to_string(),
    };
    if line.switch("no-inbox") {
        let mut d = fail(BUSY, format!("{owner} has the mod open; nothing was changed"));
        d.doc["held_by"] = json!(owner);
        return Err(d);
    }
    let req = inbox::Request {
        id: inbox::new_id(),
        author: AUTHOR.into(),
        label: change.label.clone(),
        base: Some(h.seq()),
        docs,
    };
    inbox::submit(&m.edir, &req).map_err(|e| fail(FAILED, format!("the request was not sent: {e}")))?;
    let start = Instant::now();
    let reply = loop {
        if let Some(r) = inbox::reply(&m.edir, &req.id) {
            inbox::forget(&m.edir, &req.id);
            break r;
        }
        if start.elapsed() >= wait {
            let mut d = fail(
                TIMED_OUT,
                format!(
                    "{owner} has the mod open and did not answer within {:.1} s; the request stays in its inbox and is \
                     made when it next looks",
                    wait.as_secs_f64()
                ),
            );
            d.doc["held_by"] = json!(owner);
            return Err(d);
        }
        std::thread::sleep(env.poll);
    };
    doc["via"] = json!("inbox");
    doc["held_by"] = json!(owner);
    match reply {
        inbox::Reply::Ok { seq } => {
            doc["outcome"] = json!("made");
            doc["step"] = json!(seq);
            text.push(format!("Made step {seq} through {owner}: {}", change.label));
            text.extend(summary);
            Ok(Done { code: OK, doc, text })
        }
        inbox::Reply::Conflict { items } => {
            let mut d = fail(FAILED, format!("conflict: changed meanwhile in another way: {}", items.join(", ")));
            d.doc["error"]["reason"] = json!("conflict");
            d.doc["error"]["items"] = json!(items);
            Err(d)
        }
        inbox::Reply::Refused { reason } => Err(fail(FAILED, format!("refused by {owner}: {reason}"))),
    }
}

// ---- a new mod ----------------------------------------------------------------------------------------------------

fn create(line: &args::Line) -> Done {
    let text = |k: &str| line.args.get(k).and_then(Value::as_str).map(str::to_string);
    let name = text("name").unwrap_or_default();
    let maps = text("maps").filter(|m| !m.is_empty()).map(PathBuf::from).unwrap_or_else(store::maps_dir);
    let region = line.flag("region").unwrap_or(crate::create::REGION).to_string();
    let word = text("template").unwrap_or_else(|| "empty".into());
    let Some(template) = crate::create::Template::from_word(&word) else {
        let words: Vec<&str> = crate::create::Template::ALL.iter().map(|t| t.word()).collect();
        let mut d = fail(USAGE, format!("no template {word}: use {}", words.join(", ")));
        d.doc["error"]["reason"] = json!("bad_template");
        return d;
    };
    let made = crate::create::plan(&maps, &name, text("folder").as_deref()).and_then(|folder| {
        crate::create::create_from(&maps, &folder, &name, &region, template).map(|dir| (folder, dir))
    });
    match made {
        Ok((folder, dir)) => Done {
            code: OK,
            text: vec![
                format!("Made {} in {}", name.trim(), dir.display()),
                format!("Edit it with --mod {}", dir.display()),
            ],
            doc: json!({
                "name": name.trim(),
                "folder": folder,
                "dir": dir.to_string_lossy(),
                "region": region.trim().to_ascii_uppercase(),
                "template": template.word(),
            }),
        },
        Err(r) => {
            let mut d = fail(if r.is_input() { USAGE } else { FAILED }, r.to_string());
            d.doc["error"]["reason"] = json!(r.reason());
            d
        }
    }
}

// ---- sharing ------------------------------------------------------------------------------------------------------

fn export(line: &args::Line, m: &Mod) -> Done {
    let to = PathBuf::from(line.args.get("to").and_then(Value::as_str).unwrap_or_default());
    let ok = line.args.get("derived-ok").and_then(Value::as_bool).unwrap_or(false);
    match crate::share::export_dir(&m.dir, &m.folder, &to, ok) {
        Ok(e) => Done {
            code: OK,
            text: vec![format!("Exported {} to {} ({} files)", m.folder, e.to.display(), e.files)],
            doc: json!({
                "to": e.to.to_string_lossy(),
                "files": e.files,
                "bytes": e.bytes,
                "derived": e.derived,
                "build": crate::share::build(),
            }),
        },
        Err(crate::share::Refused::Derived(d)) => {
            let message = format!("{} Give --derived-ok to export them.", crate::share::derived_warning(d.len()));
            let list: Vec<Value> = d.iter().map(|d| json!({ "texture": d.id.to_string(), "name": d.name })).collect();
            let mut done = fail(FAILED, message);
            done.doc["error"]["reason"] = json!("derived");
            done.doc["derived"] = json!(list);
            done
        }
        Err(e) => fail(FAILED, e.to_string()),
    }
}

// ---- reads --------------------------------------------------------------------------------------------------------

fn level_word(l: Level) -> &'static str {
    match l {
        Level::Error => "error",
        Level::Warning => "warning",
        Level::Info => "info",
    }
}

fn checks(line: &args::Line, env: &Env, m: &Mod) -> Done {
    let (_, map) = match open_read(m) {
        Ok(x) => x,
        Err(d) => return d,
    };
    let region = line.flag("region").unwrap_or(REGION);
    let mut nets = std::collections::HashMap::new();
    let mut tracks = Vec::new();
    if !map.races.is_empty() {
        let root = match game_root(line, env) {
            Ok(r) => r,
            Err(d) => return d,
        };
        let assets = (env.assets)(&root);
        tracks = assets.tracks(region).iter().map(|t| t.number).collect();
        for t in tracks.iter().copied().chain(map.races.iter().map(|r| r.track)) {
            nets.entry(t).or_insert_with(|| assets.road_net(t));
        }
        if tracks.is_empty() && nets.values().all(Option::is_none) {
            return fail(NO_GAME, format!("the game's race tracks do not read from {}", root.display()));
        }
    }
    let tested = validate::Tested::load(&m.edir);
    let mut found = validate::checks(&map, &nets, &tracks, &tested);
    // The floor under the start, finish and grid and the mod's drawn and removed collision, from the region's
    // world with the mod on it (its drawn ground is a floor), read when the game is at hand.
    let spots = map.races.iter().any(|r| r.start.is_some() || r.finish.is_some() || !r.grid.is_empty());
    if (spots || !map.collision.is_empty())
        && let Ok(root) = game_root(line, env)
        && let Ok(mut world) = (env.world)(&root, region)
        && world.has_floors()
    {
        if !map.collision.is_empty() {
            world.apply_map_mods(region, &[(m.folder.clone(), map.clone())]);
        }
        let under = |p| world.road_height_below(p);
        found.extend(validate::floor_checks(&map, &under));
        let c = &world.map_overlay.collision;
        found.extend(validate::collision_checks(&map, &under, &c.unmatched_at, &c.outside_at));
        found.sort_by_key(|c| (std::cmp::Reverse(c.level), c.ok));
    }
    let count = |l: Level| found.iter().filter(|c| c.level == l && !c.ok).count();
    let (errors, warnings) = (count(Level::Error), count(Level::Warning));
    let list: Vec<Value> = found
        .iter()
        .map(|c| {
            let mut o = json!({ "level": level_word(c.level), "passed": c.ok, "text": c.text });
            if let Some(k) = c.race {
                o["race"] = json!(map.races.get(k).map(|r| r.id.clone()));
            }
            if let Some(p) = c.point {
                o["point"] = serde_json::to_value(p).unwrap_or_default();
            }
            if let Some(at) = c.at {
                o["at"] = json!([at.x, at.y]);
            }
            if let Some(f) = &c.fix {
                o["fix"] = json!(f.label());
            }
            o
        })
        .collect();
    let mut text: Vec<String> = found
        .iter()
        .map(|c| {
            let race = c.race.and_then(|k| map.races.get(k)).map(|r| format!("[{}] ", r.id)).unwrap_or_default();
            let fix = c.fix.as_ref().map(|f| format!(" (fix: {})", f.label())).unwrap_or_default();
            format!("{:<7} {race}{}{fix}", if c.ok { "ok" } else { level_word(c.level) }, c.text)
        })
        .collect();
    text.push(format!("{errors} error(s), {warnings} warning(s)"));
    Done {
        code: if errors > 0 { CHECKS } else { OK },
        doc: json!({ "errors": errors, "warnings": warnings, "checks": list }),
        text,
    }
}

fn history_show(m: &Mod) -> Done {
    let (h, _) = match open_read(m) {
        Ok(x) => x,
        Err(d) => return d,
    };
    let step = |s: &history::Step| {
        let mut o = json!({ "step": s.seq, "time_ms": s.at, "author": s.author, "label": s.label });
        if let Some(r) = &s.request {
            o["request"] = json!(r);
        }
        o
    };
    let mut text: Vec<String> =
        h.steps().iter().map(|s| format!("{:>4}  {}  ({})", s.seq, s.label, s.author)).collect();
    if text.is_empty() {
        text.push("No steps kept yet.".into());
    }
    Done {
        code: OK,
        doc: json!({
            "step": h.seq(),
            "steps": h.steps().iter().map(step).collect::<Vec<_>>(),
            "undone": h.undone().iter().map(step).collect::<Vec<_>>(),
            "versions": h.versions().iter().map(|v| json!({ "name": v.name, "time_ms": v.at })).collect::<Vec<_>>(),
            "unsaved": h.unsaved(),
        }),
        text,
    }
}

// ---- help ---------------------------------------------------------------------------------------------------------

fn help(cmd: Option<&'static Command>, words: &[String]) -> Done {
    if !words.is_empty() && cmd.is_none() {
        return fail(USAGE, format!("unknown command `{}`", words.join(" ")));
    }
    let mut text = Vec::new();
    match cmd {
        None => {
            text.push(format!("{PROGRAM} <command> [flags]: Overbright Studio from the command line."));
            text.push(String::new());
            text.push("Commands:".into());
            for c in registry::COMMANDS {
                text.push(format!("  {:<20} {:<12} {}", words_of(c), c.kind.word(), c.summary));
            }
            text.push(String::new());
            text.push("Flags of every command:".into());
            for (f, what) in args::COMMON {
                text.push(format!("  --{f:<12} {what}"));
            }
            text.push(String::new());
            text.push("Exit codes:".into());
            for (code, _, what) in EXIT_CODES {
                text.push(format!("  {code:>3}  {what}"));
            }
            text.push(String::new());
            text.push(format!("`{PROGRAM} help <command>` lists a command's flags."));
        }
        Some(c) => {
            text.push(format!("{PROGRAM} {} [flags]: {} ({}).", words_of(c), c.summary, c.kind.word()));
            for f in c.args {
                let req =
                    if matches!(f.req, registry::Req::Yes | registry::Req::YesAs(_)) { "needed" } else { "optional" };
                let words = registry::enum_words(&f.ty).map(|w| format!(": {}", w.join(" | "))).unwrap_or_default();
                text.push(format!("  --{:<12} {:<6} {req:<8} {}{words}", f.key, registry::type_name(&f.ty), f.label));
            }
        }
    }
    let commands: Vec<Value> = match cmd {
        Some(c) => vec![command_json(c)],
        None => registry::COMMANDS.iter().map(command_json).collect(),
    };
    let codes: Vec<Value> = EXIT_CODES.iter().map(|(c, k, w)| json!({ "code": c, "kind": k, "meaning": w })).collect();
    let common: Vec<Value> = args::COMMON.iter().map(|(f, w)| json!({ "flag": f, "meaning": w })).collect();
    Done { code: OK, text, doc: json!({ "commands": commands, "flags": common, "exit_codes": codes }) }
}

fn words_of(c: &Command) -> String {
    words(c)
}

fn command_json(c: &Command) -> Value {
    let args: Vec<Value> = c
        .args
        .iter()
        .map(|f| {
            let mut o = json!({
                "flag": format!("--{}", f.key),
                "type": registry::type_name(&f.ty),
                "needed": matches!(f.req, registry::Req::Yes | registry::Req::YesAs(_)),
                "label": f.label,
            });
            if let Some(w) = registry::enum_words(&f.ty) {
                o["enum"] = json!(w);
            }
            o
        })
        .collect();
    json!({ "command": c.name, "words": words(c), "kind": c.kind.word(), "summary": c.summary, "flags": args })
}

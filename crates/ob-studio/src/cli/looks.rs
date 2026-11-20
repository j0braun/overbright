//! Studio Looks on the command line (docs/modding/studio-looks.md): `looks open` writes a texture's work file,
//! `looks import` takes it in as one step, `looks watch` takes each save in as it comes, `looks list` shows the
//! mod's textures and work files with where their pixels started.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{AUTHOR, Done, Env, FAILED, Mod, NO_GAME, OK, USAGE, args, fail, game_root};
use crate::history::{History, disk};
use crate::looks::work::{Imported, Workbench};
use crate::looks::{self, Looks, TextureId};

/// How often `looks watch` looks at the work files.
pub const POLL: Duration = Duration::from_millis(50);

pub(super) fn run(name: &str, line: &args::Line, env: &Env, m: &Mod) -> Done {
    let r = match name {
        "looks.open" => open(line, env, m),
        "looks.import" => import(line, m),
        "looks.watch" => watch(line, m),
        _ => list(m),
    };
    match r {
        Ok(d) | Err(d) => d,
    }
}

fn texture(line: &args::Line) -> Result<TextureId, Done> {
    let t = line.args.get("texture").and_then(Value::as_str).unwrap_or_default();
    t.parse().map_err(|e: String| fail(USAGE, format!("--texture: {e}")))
}

fn read_looks(h: &History) -> Result<Looks, Done> {
    Looks::parse(h.doc(looks::FILE).unwrap_or_default()).map_err(|e| fail(FAILED, e))
}

fn open_read(m: &Mod) -> Result<History, Done> {
    History::open_read(&m.dir, &m.edir, crate::MOD_DOCS, Arc::new(disk::Real))
        .map_err(|e| fail(FAILED, format!("{}: {e}", m.dir.display())))
}

fn open_write(m: &Mod) -> Result<History, Done> {
    History::open(&m.dir, &m.edir, crate::MOD_DOCS, AUTHOR, Arc::new(disk::Real))
        .map_err(|e| fail(FAILED, format!("{}: {e}", m.dir.display())))
}

fn rel(m: &Mod, p: &Path) -> String {
    p.strip_prefix(&m.dir).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

fn open(line: &args::Line, env: &Env, m: &Mod) -> Result<Done, Done> {
    let id = texture(line)?;
    let blank = line.args.get("blank").and_then(Value::as_bool).unwrap_or(false);
    let looks = read_looks(&open_read(m)?)?;
    let mut w = Workbench::open(&m.dir, &m.edir);
    // The game is read only when the work file is made from it.
    let needs_game = w.file(id).is_none_or(|f| !w.path(f).is_file()) && (blank || !looks.entries.contains_key(&id));
    let world = if needs_game {
        let root = game_root(line, env)?;
        let region = line.flag("region").unwrap_or(super::REGION);
        Some((env.world)(&root, region).map_err(|e| fail(NO_GAME, format!("the game's world does not load: {e}")))?)
    } else {
        None
    };
    let none = crate::studio_api::world::World::default();
    let f = w.start(&looks, id, world.as_ref().unwrap_or(&none), blank).map_err(|e| fail(FAILED, e))?;
    let path = w.path(&f);
    let mut text =
        vec![format!("Open {} in your paint program; each save is taken in by `looks watch`.", path.display())];
    if f.origin.derived() {
        text.push("Made from the game's own pixels: the result is derived (shared only with a warning).".into());
    }
    Ok(Done {
        code: OK,
        doc: json!({
            "texture": id.to_string(),
            "name": f.name,
            "work_file": path.to_string_lossy(),
            "origin": f.origin.word(),
            "derived": f.origin.derived(),
        }),
        text,
    })
}

fn outcome(r: &Imported, doc: &mut Value) -> String {
    match r {
        Imported::Step(seq) => {
            doc["outcome"] = json!("made");
            doc["step"] = json!(seq);
            format!("made step {seq}")
        }
        Imported::Unchanged => {
            doc["outcome"] = json!("unchanged");
            "unchanged".into()
        }
        Imported::Sent(id) => {
            doc["outcome"] = json!("sent");
            doc["request"] = json!(id);
            format!("sent to the program that has the mod open (request {id})")
        }
    }
}

fn import(line: &args::Line, m: &Mod) -> Result<Done, Done> {
    let id = texture(line)?;
    let mut w = Workbench::open(&m.dir, &m.edir);
    let saved = w.read_now(id).map_err(|e| fail(FAILED, e))?;
    let mut doc = json!({ "texture": id.to_string() });
    if line.switch("dry-run") {
        doc["outcome"] = json!("dry_run");
        doc["image"] = json!(looks::image_hash(&saved.png));
        return Ok(Done { code: OK, doc, text: vec![format!("Dry run, nothing written: {id} would be taken in")] });
    }
    let mut h = open_write(m)?;
    let r = w.import(&mut h, AUTHOR, &saved).map_err(|e| fail(FAILED, e))?;
    let words = outcome(&r, &mut doc);
    Ok(Done { code: OK, doc, text: vec![format!("{id}: {words}")] })
}

fn watch(line: &args::Line, m: &Mod) -> Result<Done, Done> {
    let until = match line.args.get("seconds").and_then(Value::as_f64) {
        Some(s) if s.is_finite() && s >= 0.0 => Some(Instant::now() + Duration::from_secs_f64(s)),
        Some(s) => return Err(fail(USAGE, format!("--seconds: {s} is not a time"))),
        None => None,
    };
    let mut h = open_write(m)?;
    let mut w = Workbench::open(&m.dir, &m.edir);
    if w.files().is_empty() {
        return Err(fail(FAILED, "no work files: open a texture first (looks open)"));
    }
    let mut events = Vec::new();
    let mut text = Vec::new();
    w.watch(&mut h, AUTHOR, POLL, until, &|| false, &mut |id, r| {
        let mut e = json!({ "texture": id.to_string() });
        let words = match &r {
            Ok(r) => outcome(r, &mut e),
            Err(why) => {
                e["outcome"] = json!("refused");
                e["reason"] = json!(why);
                format!("refused: {why}")
            }
        };
        text.push(format!("{id}: {words}"));
        events.push(e);
    })
    .map_err(|e| fail(FAILED, e))?;
    Ok(Done { code: OK, doc: json!({ "saves": events }), text })
}

fn list(m: &Mod) -> Result<Done, Done> {
    let looks = read_looks(&open_read(m)?)?;
    let w = Workbench::open(&m.dir, &m.edir);
    let textures: Vec<Value> = looks
        .entries
        .values()
        .map(|e| {
            json!({
                "texture": e.id.to_string(),
                "name": e.name,
                "origin": e.origin.word(),
                "derived": e.origin.derived(),
                "size": [e.width, e.height],
                "image": e.image,
                "pack_file": format!("{}/{}", looks::PACK_DIR, e.id.pack_file()),
            })
        })
        .collect();
    let work: Vec<Value> = w
        .files()
        .iter()
        .map(|f| json!({ "texture": f.id.to_string(), "name": f.name, "work_file": rel(m, &w.path(f)), "origin": f.origin.word() }))
        .collect();
    let mut text: Vec<String> = looks
        .entries
        .values()
        .map(|e| {
            format!(
                "{}  {:<24} {}x{}  {}",
                e.id,
                e.name,
                e.width,
                e.height,
                if e.origin.derived() { "derived" } else { "own" }
            )
        })
        .collect();
    if text.is_empty() {
        text.push("No textures changed yet.".into());
    }
    text.extend(w.files().iter().map(|f| format!("work: {} ({})", rel(m, &w.path(f)), f.id)));
    Ok(Done { code: OK, doc: json!({ "textures": textures, "work": work }), text })
}

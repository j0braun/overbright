//! The list and find commands, which read the game and no mod: `props solids`, `race tracks`, `looks find` and
//! `game find`. Each runs the function the window runs (the prop browser's list [`crate::browse`], the editor's
//! track list [`MapAssets::tracks`], the game folder lookup of the editor and the game), and a near miss says
//! what was meant ([`crate::browse::did_you_mean`]).

use std::path::PathBuf;

use serde_json::{Value, json};

use super::{Done, Env, FAILED, NO_GAME, OK, REGION, USAGE, args, fail, game_root};
use crate::browse::{self, Filter};

pub(super) fn run(name: &str, line: &args::Line, env: &Env) -> Done {
    match name {
        "props.solids" => solids(line, env),
        "race.tracks" => tracks(line, env),
        "looks.find" => find_texture(line, env),
        _ => game(line, env),
    }
}

fn region(line: &args::Line) -> String {
    line.flag("region").unwrap_or(REGION).to_string()
}

fn text(line: &args::Line, key: &str) -> String {
    line.args.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// The words of a name (`XC_CONE_01` is `XC`, `CONE`, `01`), for "did you mean" on a search.
fn name_words<'a>(names: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut v: Vec<String> = names
        .flat_map(|n| n.split(|c: char| !c.is_ascii_alphanumeric()).map(str::to_ascii_lowercase).collect::<Vec<_>>())
        .filter(|w| w.len() >= 3)
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The "Did you mean" line for a search that found nothing: the nearest word of the search, from `words`.
fn mean(search: &str, words: &[String]) -> Option<String> {
    let fixed: Vec<String> = search
        .split_whitespace()
        .map(|w| browse::did_you_mean(w, words.iter().map(String::as_str)).unwrap_or(w).to_string())
        .collect();
    let fixed = fixed.join(" ");
    (!fixed.eq_ignore_ascii_case(search.trim())).then_some(fixed)
}

fn world(line: &args::Line, env: &Env) -> Result<nfsu2::studio_api::world::World, Done> {
    let root = game_root(line, env)?;
    (env.world)(&root, &region(line)).map_err(|e| fail(NO_GAME, format!("the game's world does not load: {e}")))
}

fn solids(line: &args::Line, env: &Env) -> Done {
    let category = text(line, "category");
    let filter = if category.is_empty() {
        Filter::All
    } else {
        match browse::filter_from_word(&category) {
            Some(f) => f,
            None => {
                let words: Vec<&str> = Filter::ALL.iter().map(|f| browse::filter_word(*f)).collect();
                let mut m = format!("--category: no category `{category}`: use {}", words.join(", "));
                if let Some(n) = browse::did_you_mean(&category, words.iter().copied()) {
                    m.push_str(&format!(". Did you mean `{n}`?"));
                }
                let mut d = fail(USAGE, m);
                d.doc["error"]["reason"] = json!("bad_category");
                return d;
            }
        }
    };
    let world = match world(line, env) {
        Ok(w) => w,
        Err(d) => return d,
    };
    let search = text(line, "filter");
    let all = browse::entries(&world);
    let shown = browse::filtered(&all, filter, &search);
    let mut lines: Vec<String> = shown
        .iter()
        .map(|e| {
            format!("{:<28} {:<10} {} placed", e.name, if e.knock_over { "knock-over" } else { "static" }, e.placed)
        })
        .collect();
    let mut doc = json!({
        "region": region(line),
        "count": shown.len(),
        "solids": shown.iter().map(|e| json!({
            "name": e.name,
            "knock_over": e.knock_over,
            "placed": e.placed,
            "categories": browse::categories(e),
        })).collect::<Vec<_>>(),
    });
    if shown.is_empty() {
        lines.push(format!("No solid matches in {}", region(line)));
        if let Some(m) = mean(&search, &name_words(all.iter().map(|e| e.name.as_str()))) {
            lines.push(format!("Did you mean `{m}`?"));
            doc["did_you_mean"] = json!(m);
        }
    } else {
        lines.push(format!("{} solids", shown.len()));
    }
    Done { code: OK, text: lines, doc }
}

fn tracks(line: &args::Line, env: &Env) -> Done {
    let root = match game_root(line, env) {
        Ok(r) => r,
        Err(d) => return d,
    };
    let region = region(line);
    let list = (env.assets)(&root).tracks(&region);
    let mut lines: Vec<String> = list.iter().map(|t| format!("{:>5}  {}", t.number, t.name)).collect();
    let mut doc = json!({
        "region": region,
        "count": list.len(),
        "tracks": list.iter().map(|t| json!({ "number": t.number, "name": t.name })).collect::<Vec<_>>(),
    });
    if list.is_empty() {
        lines.push(format!("No base tracks in region {region}"));
        let known = regions(&root);
        if let Some(n) = browse::did_you_mean(&region, known.iter().map(String::as_str)) {
            lines.push(format!("Did you mean --region {n}?"));
            doc["did_you_mean"] = json!(n);
        }
    }
    Done { code: OK, text: lines, doc }
}

/// The regions the game folder has (`TRACKS/ROUTES<region>`), for "did you mean" on `--region`.
fn regions(root: &std::path::Path) -> Vec<String> {
    let dir = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("TRACKS"))
        .map(|e| e.path())
        .unwrap_or_else(|| root.join("TRACKS"));
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.to_ascii_uppercase().strip_prefix("ROUTES").map(str::to_string))
        .collect();
    v.sort();
    v
}

fn find_texture(line: &args::Line, env: &Env) -> Done {
    use crate::browser::{Hit, Index, Kind, Query};
    let search = text(line, "text");
    let world = match world(line, env) {
        Ok(w) => w,
        Err(d) => return d,
    };
    // The Browser's own index and search ([`crate::browser`]): one answer in Studio and here.
    let index = Index::build(&region(line), &world);
    let q = Query { text: search.clone(), kind: Kind::Textures, ..Default::default() };
    let mut found: Vec<usize> =
        index.search(&q).into_iter().filter_map(|h| if let Hit::Texture(i) = h { Some(i) } else { None }).collect();
    found.sort_by(|&a, &b| {
        let (a, b) = (&index.textures[a], &index.textures[b]);
        a.name.cmp(&b.name).then(a.hash.cmp(&b.hash))
    });
    let mut lines: Vec<String> = found
        .iter()
        .map(|&i| {
            let t = &index.textures[i];
            format!("{:<28} {}  {}x{}  {}", t.name, t.id(), t.width, t.height, index.texture_used(i))
        })
        .collect();
    let mut doc = json!({
        "region": region(line),
        "text": search,
        "count": found.len(),
        "textures": found.iter().map(|&i| {
            let t = &index.textures[i];
            json!({
                "texture": t.id().to_string(),
                "name": t.name,
                "hash": format!("0x{:08X}", t.hash),
                "width": t.width,
                "height": t.height,
                "animated": t.animated,
                "materials": t.materials(),
                "used": index.texture_used(i),
                "used_by": t.used_by.iter().map(|&(m, n)| json!({
                    "model": index.models[m].name, "materials": n,
                })).collect::<Vec<_>>(),
                "sections": index.texture_sections(i).iter().map(|&(s, n)| json!({
                    "section": s, "placements": n,
                })).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    });
    if found.is_empty() {
        lines.push(format!("No texture matches `{search}` in {}", region(line)));
        if let Some(m) = mean(&search, &name_words(world.textures.values().map(|t| t.name.as_str()))) {
            lines.push(format!("Did you mean `{m}`?"));
            doc["did_you_mean"] = json!(m);
        }
    }
    Done { code: OK, text: lines, doc }
}

/// Where the game folder came from, as the tool looks for it: the flag, `NFSU2_ROOT`, the launcher's choice.
fn game(line: &args::Line, env: &Env) -> Done {
    let from_flag = line.flag("game").filter(|g| !g.is_empty()).map(PathBuf::from);
    let (root, source) = match (&from_flag, &env.game) {
        (Some(p), _) => (p.clone(), "--game"),
        (None, Some(p)) => (p.clone(), env.game_source),
        (None, None) => return game_root(line, env).err().unwrap_or_else(|| fail(FAILED, "no game folder")),
    };
    if !root.is_dir() {
        let mut d = fail(NO_GAME, format!("the game folder {} (from {source}) does not exist", root.display()));
        d.doc["error"]["source"] = json!(source);
        return d;
    }
    Done {
        code: OK,
        text: vec![format!("{} (from {source})", root.display())],
        doc: json!({ "game": root.to_string_lossy(), "source": source }),
    }
}

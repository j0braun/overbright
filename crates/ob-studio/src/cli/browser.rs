//! The Browser workspace on the command line: `browser search` and `browser used` read the region's asset index
//! ([`crate::browser::Index`]) the window's Browser reads, so the same words give the same answer.

use serde_json::{Value, json};

use super::{Done, Env, OK, REGION, USAGE, args, fail, game_root, NO_GAME};
use crate::browse::{self, Filter};
use crate::browser::{Hit, Index, Kind, Query, TexFilter};

fn text(line: &args::Line, key: &str) -> String {
    line.args.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn word(name: &str) -> String {
    name.to_ascii_lowercase().replace(' ', "-")
}

fn bad(flag: &str, given: &str, words: &[String]) -> Done {
    let mut m = format!("--{flag}: no `{given}`: use {}", words.join(", "));
    if let Some(n) = browse::did_you_mean(given, words.iter().map(String::as_str)) {
        m.push_str(&format!(". Did you mean `{n}`?"));
    }
    let mut d = fail(USAGE, m);
    d.doc["error"]["reason"] = json!(format!("bad_{flag}"));
    d
}

fn index(line: &args::Line, env: &Env) -> Result<(String, Index), Done> {
    let region = line.flag("region").unwrap_or(REGION).to_string();
    let root = game_root(line, env)?;
    let world = (env.world)(&root, &region).map_err(|e| fail(NO_GAME, format!("the game's world does not load: {e}")))?;
    let index = Index::build(&region, &world);
    Ok((region, index))
}

fn hit_json(ix: &Index, h: Hit) -> Value {
    match h {
        Hit::Model(i) => {
            let m = &ix.models[i];
            json!({
                "type": "model", "name": m.name, "hash": format!("0x{:08X}", m.hash), "knock_over": m.knock_over,
                "placed": m.placed(), "shown": m.shown, "sections": m.sections.len(), "triangles": m.triangles,
                "textures": m.textures.len(), "used": ix.model_used(i),
            })
        }
        Hit::Texture(i) => {
            let t = &ix.textures[i];
            json!({
                "type": "texture", "name": t.name, "texture": t.id().to_string(), "width": t.width,
                "height": t.height, "animated": t.animated, "models": t.used_by.len(), "materials": t.materials(),
                "used": ix.texture_used(i),
            })
        }
    }
}

fn hit_line(ix: &Index, h: Hit) -> String {
    match h {
        Hit::Model(i) => format!("model    {:<28} {}", ix.models[i].name, ix.model_used(i)),
        Hit::Texture(i) => {
            let t = &ix.textures[i];
            format!("texture  {:<28} {}x{}  {}", t.name, t.width, t.height, ix.texture_used(i))
        }
    }
}

pub(super) fn run(name: &str, line: &args::Line, env: &Env) -> Done {
    if name == "browser.used" { used(line, env) } else { search(line, env) }
}

fn search(line: &args::Line, env: &Env) -> Done {
    let kinds: Vec<String> = Kind::ALL.iter().map(|k| word(k.name())).collect();
    let tex: Vec<String> = TexFilter::ALL.iter().map(|k| word(k.name())).collect();
    let cats: Vec<String> = Filter::ALL.iter().map(|f| browse::filter_word(*f).to_string()).collect();
    let mut q = Query { text: text(line, "text"), ..Query::default() };
    let kind = text(line, "kind");
    if !kind.is_empty() {
        match Kind::ALL.iter().find(|k| word(k.name()) == kind.to_ascii_lowercase()) {
            Some(k) => q.kind = *k,
            None => return bad("kind", &kind, &kinds),
        }
    }
    let t = text(line, "textures");
    if !t.is_empty() {
        match TexFilter::ALL.iter().find(|k| word(k.name()) == t.to_ascii_lowercase()) {
            Some(k) => q.textures = *k,
            None => return bad("textures", &t, &tex),
        }
    }
    let c = text(line, "category");
    if !c.is_empty() {
        match browse::filter_from_word(&c) {
            Some(f) => q.models = f,
            None => return bad("category", &c, &cats),
        }
    }
    let (region, ix) = match index(line, env) {
        Ok(x) => x,
        Err(d) => return d,
    };
    let hits = ix.search(&q);
    let (models, textures) = ix.counts(&q.text);
    let mut lines: Vec<String> = hits.iter().map(|h| hit_line(&ix, *h)).collect();
    if hits.is_empty() {
        lines.push(format!("Nothing matches `{}` in {region}", q.text));
    }
    let doc = json!({
        "region": region, "text": q.text, "count": hits.len(),
        "found": { "models": models, "textures": textures },
        "hits": hits.iter().map(|h| hit_json(&ix, *h)).collect::<Vec<_>>(),
    });
    Done { code: OK, text: lines, doc }
}

fn used(line: &args::Line, env: &Env) -> Done {
    let q = Query { text: text(line, "text"), ..Query::default() };
    let (region, ix) = match index(line, env) {
        Ok(x) => x,
        Err(d) => return d,
    };
    let hits = ix.search(&q);
    let mut lines = Vec::new();
    let mut docs = Vec::new();
    for h in &hits {
        let mut d = hit_json(&ix, *h);
        lines.push(hit_line(&ix, *h));
        match *h {
            Hit::Model(i) => {
                let m = &ix.models[i];
                let tex: Vec<Value> = m
                    .textures
                    .iter()
                    .map(|&(hash, n)| {
                        let name = ix.texture_of_hash(hash).map_or(String::new(), |t| ix.textures[t].name.clone());
                        lines.push(format!("  draws {name} ({n} materials)"));
                        json!({ "texture": format!("world:0x{hash:08X}"), "name": name, "materials": n })
                    })
                    .collect();
                for (s, n) in &m.sections {
                    lines.push(format!("  section {s}: {n} placed"));
                }
                d["placements"] = json!(m.placements);
                d["as_lod"] = json!(m.as_lod);
                d["section_list"] = json!(m.sections.iter().map(|(s, n)| json!({ "section": s, "placed": n })).collect::<Vec<_>>());
                d["texture_list"] = json!(tex);
            }
            Hit::Texture(i) => {
                let t = &ix.textures[i];
                let models: Vec<Value> = t
                    .used_by
                    .iter()
                    .map(|&(m, n)| {
                        lines.push(format!("  drawn by {} ({n} materials)", ix.models[m].name));
                        json!({ "model": ix.models[m].name, "materials": n })
                    })
                    .collect();
                let secs = ix.texture_sections(i);
                for (s, n) in &secs {
                    lines.push(format!("  section {s}: {n} placed"));
                }
                d["model_list"] = json!(models);
                d["section_list"] = json!(secs.iter().map(|(s, n)| json!({ "section": s, "placed": n })).collect::<Vec<_>>());
            }
        }
        docs.push(d);
    }
    if hits.is_empty() {
        lines.push(format!("Nothing matches `{}` in {region}", q.text));
    }
    Done { code: OK, text: lines, doc: json!({ "region": region, "text": q.text, "count": docs.len(), "found": docs }) }
}

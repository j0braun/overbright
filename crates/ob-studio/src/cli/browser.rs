//! The Browser workspace on the command line: `browser search` and `browser used` read the region's asset index
//! ([`crate::browser::Index`]) the window's Browser reads, so the same words give the same answer.

use serde_json::{Value, json};

use super::{Done, Env, OK, REGION, USAGE, args, fail, game_root, NO_GAME};
use crate::browse::{self, Filter};
use crate::browser::{Hit, Index, Kind, Query, TexFilter, media};

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
        Hit::Sound(i) => {
            let s = &ix.sounds.as_deref().unwrap_or_default()[i];
            json!({
                "type": "sound", "name": s.name, "bank": s.bank, "samples": s.samples, "rate": s.rate,
                "frames": s.frames, "seconds": s.seconds(), "playable": s.playable,
            })
        }
        Hit::Line(i) => {
            let l = &ix.lines.as_deref().unwrap_or_default()[i];
            json!({ "type": "string", "hash": format!("0x{:08X}", l.hash), "text": l.text, "language": ix.language })
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
        Hit::Sound(i) => {
            let s = &ix.sounds.as_deref().unwrap_or_default()[i];
            format!("sound    {:<28} {}", s.name, s.bank)
        }
        Hit::Line(i) => {
            let l = &ix.lines.as_deref().unwrap_or_default()[i];
            format!("{:08X}  {}", l.hash, l.text)
        }
        Hit::Texture(i) => {
            let t = &ix.textures[i];
            format!("texture  {:<28} {}x{}  {}", t.name, t.width, t.height, ix.texture_used(i))
        }
    }
}

pub(super) fn run(name: &str, line: &args::Line, env: &Env) -> Done {
    match name {
        "browser.used" => used(line, env),
        "browser.sounds" => search_kind(line, env, Some(Kind::Sounds)),
        "browser.strings" => search_kind(line, env, Some(Kind::Strings)),
        "browser.wav" => wav(line, env),
        "browser.mesh" => mesh(line, env),
        "browser.area" => area(line, env),
        "browser.export" => export(line, env),
        _ => search_kind(line, env, None),
    }
}

/// The game's sounds and language lines the command line reads, as the window's Browser does ([`media::load`]).
fn read_media(line: &args::Line, env: &Env, ix: &mut Index, kind: Kind) -> Result<(), Done> {
    let root = game_root(line, env)?;
    let m = (env.media)(&root);
    media::load(ix, &*m, kind, &text(line, "language")).map_err(|e| {
        let mut d = fail(USAGE, e);
        d.doc["error"]["reason"] = json!("bad_language");
        d
    })
}

fn search_kind(line: &args::Line, env: &Env, preset: Option<Kind>) -> Done {
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
    if let Some(k) = preset {
        q.kind = k;
    }
    // Sounds and lines come from the game's files, not from a region's world: no world is loaded for them.
    let (region, ix) = if matches!(q.kind, Kind::Sounds | Kind::Strings) {
        let mut ix = Index::empty(line.flag("region").unwrap_or(REGION));
        if let Err(d) = read_media(line, env, &mut ix, q.kind) {
            return d;
        }
        (ix.region.clone(), ix)
    } else {
        match index(line, env) {
            Ok(x) => x,
            Err(d) => return d,
        }
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
        "language": if q.kind == Kind::Strings { json!(ix.language) } else { Value::Null },
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
            Hit::Sound(_) | Hit::Line(_) => {}
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

/// The one model `text` names in the region's index: an exact name, else the only model the words find.
fn model_named(ix: &Index, text: &str) -> Result<usize, Done> {
    let q = Query { text: text.to_string(), kind: Kind::Models, ..Query::default() };
    let hits: Vec<usize> = ix.search(&q).into_iter().filter_map(|h| if let Hit::Model(i) = h { Some(i) } else { None }).collect();
    if let Some(&i) = hits.iter().find(|&&i| ix.models[i].name.eq_ignore_ascii_case(text)) {
        return Ok(ix.models[i].solid);
    }
    match hits.as_slice() {
        [i] => Ok(ix.models[*i].solid),
        [] => Err(fail(super::FAILED, format!("no model matches `{text}`"))),
        many => {
            let names: Vec<&str> = many.iter().take(6).map(|&i| ix.models[i].name.as_str()).collect();
            Err(fail(super::FAILED, format!("`{text}` matches {} models ({}…): give the whole name", many.len(), names.join(", "))))
        }
    }
}

/// `browser export`: the model as a glTF reference, as the window's Export glTF writes it.
pub(super) fn export(line: &args::Line, env: &Env) -> Done {
    use crate::browser::gltf;
    let name = text(line, "text");
    let to = text(line, "to");
    let dir =    match (Some(to).filter(|t| !t.is_empty()), line.flag("mod").filter(|t| !t.is_empty())) {
        (Some(t), _) => std::path::PathBuf::from(t),
        (None, Some(m)) => gltf::reference_dir(std::path::Path::new(m)),
        (None, None) => return fail(USAGE, "--to (a folder) or --mod is needed: where the glTF is written"),
    };
    let root = match game_root(line, env) {
        Ok(r) => r,
        Err(d) => return d,
    };
    let region = line.flag("region").unwrap_or(REGION).to_string();
    let world = match (env.world)(&root, &region) {
        Ok(w) => w,
        Err(e) => return fail(NO_GAME, format!("the game's world does not load: {e}")),
    };
    let ix = Index::build(&region, &world);
    let solid = match model_named(&ix, &name) {
        Ok(s) => s,
        Err(d) => return d,
    };
    match gltf::write_reference(&dir, Some(&root), &world.solids[solid], &world.textures, &region) {
        Ok(r) => Done {
            code: OK,
            text: vec![format!("{} written: from your game files, local-only (see {})", r.gltf.display(), gltf::NOTE_FILE)],
            doc: json!({ "region": region, "model": world.solids[solid].name, "gltf": r.gltf, "bin": r.bin, "textures": r.textures, "note": r.note, "origin": "from-game-files", "share": "local-only" }),
        },
        Err(e) => fail(super::FAILED, e),
    }
}

/// `browser import`: an edited glTF mapped onto the original model's parts and kept in the mod.
pub(super) fn import(line: &args::Line, env: &Env, mod_dir: &std::path::Path) -> Done {
    use crate::browser::import;
    let file = std::path::PathBuf::from(text(line, "file"));
    let own = line.args.get("own").and_then(Value::as_bool).unwrap_or(false);
    let foreign = match crate::browser::worker::read(&file) {
        Ok(f) => f,
        Err(r) => return refused(&r),
    };
    let wanted = match Some(text(line, "model")).filter(|m| !m.is_empty()).or_else(|| foreign.model.clone()) {
        Some(m) => m,
        None => return fail(USAGE, "--model is needed: the file does not say which model it was exported from"),
    };
    let root = match game_root(line, env) {
        Ok(r) => r,
        Err(d) => return d,
    };
    let region = line.flag("region").unwrap_or(REGION).to_string();
    let world = match (env.world)(&root, &region) {
        Ok(w) => w,
        Err(e) => return fail(NO_GAME, format!("the game's world does not load: {e}")),
    };
    let ix = Index::build(&region, &world);
    let solid = match model_named(&ix, &wanted) {
        Ok(s) => s,
        Err(d) => return d,
    };
    let orig = &world.solids[solid];
    let imp = match import::map_onto(&foreign, orig, &world.textures) {
        Ok(i) => i,
        Err(r) => return refused(&r),
    };
    if own && let Err(r) = import::check_own(&foreign, &imp, orig) {
        return refused(&r);
    }
    match import::write_model(mod_dir, &imp, &world.textures, &region, own) {
        Ok(k) => Done {
            code: OK,
            text: vec![
                format!("{} kept as a mod model in {}: {} vertices, {} triangles", orig.name, k.dir.display(), imp.solid.vertices.len(), imp.solid.indices.len() / 3),
                if own { "shared with the mod (your own work)".into() } else { "from your game files: it stays on this computer and is not in the shared copy".to_string() },
            ],
            doc: json!({
                "region": region, "model": orig.name, "dir": k.dir, "gltf": k.gltf, "record": k.record, "shared": k.shared,
                "origin": if own { "own" } else { "from-game-files" },
                "vertices": imp.solid.vertices.len(), "triangles": imp.solid.indices.len() / 3,
                "parts": imp.parts.iter().map(|(n, t)| json!({ "name": n, "triangles": t })).collect::<Vec<_>>(),
            }),
        },
        Err(e) => fail(super::FAILED, e),
    }
}

fn refused(r: &crate::browser::import::Refusal) -> Done {
    let mut d = fail(super::FAILED, format!("not imported: {r}"));
    d.text = r.0.iter().map(|w| format!("not imported: {w}")).collect();
    d.doc["error"]["reasons"] = json!(r.0);
    d
}

/// The sound `text` names among the game's: an exact event name, else the only one the words find.
fn sound_named(all: &[media::Sound], text: &str) -> Result<usize, Done> {
    let found = media::find_sounds(all, text);
    if let Some(&i) = found.iter().find(|&&i| all[i].name.eq_ignore_ascii_case(text)) {
        if found.iter().filter(|&&i| all[i].name.eq_ignore_ascii_case(text)).count() == 1 {
            return Ok(i);
        }
    }
    match found.as_slice() {
        [i] => Ok(*i),
        [] => {
            let mut m = format!("no sound matches `{text}`");
            if let Some(n) = browse::did_you_mean(text, all.iter().map(|s| s.name.as_str())) {
                m.push_str(&format!(". Did you mean `{n}`?"));
            }
            Err(fail(super::FAILED, m))
        }
        many => {
            let names: Vec<String> = many.iter().take(6).map(|&i| format!("{} ({})", all[i].name, all[i].bank)).collect();
            Err(fail(super::FAILED, format!("`{text}` matches {} sounds ({}…): add words of the bank", many.len(), names.join(", "))))
        }
    }
}

/// `browser wav`: a sound written as a WAV file, marked as from the game files like a model's export.
fn wav(line: &args::Line, env: &Env) -> Done {
    use crate::browser::gltf;
    let name = text(line, "text");
    let to = text(line, "to");
    let dir = match (Some(to).filter(|t| !t.is_empty()), line.flag("mod").filter(|t| !t.is_empty())) {
        (Some(t), _) => std::path::PathBuf::from(t),
        (None, Some(m)) => gltf::reference_dir(std::path::Path::new(m)),
        (None, None) => return fail(USAGE, "--to (a folder) or --mod is needed: where the WAV is written"),
    };
    let root = match game_root(line, env) {
        Ok(r) => r,
        Err(d) => return d,
    };
    let m = (env.media)(&root);
    let all = m.sounds();
    let s = match sound_named(&all, &name) {
        Ok(i) => &all[i],
        Err(d) => return d,
    };
    match media::write_wav(&dir, Some(&root), &*m, s) {
        Ok(f) => Done {
            code: OK,
            text: vec![format!("{} written ({:.2} s): from your game files, local-only (see {})", f.display(), s.seconds(), gltf::NOTE_FILE)],
            doc: json!({
                "sound": s.name, "bank": s.bank, "wav": f, "rate": s.rate, "seconds": s.seconds(),
                "origin": "from-game-files", "share": "local-only",
            }),
        },
        Err(e) => fail(super::FAILED, e),
    }
}

/// `browser mesh`: a model's geometry in numbers, as the window's viewer shows it.
fn mesh(line: &args::Line, env: &Env) -> Done {
    let name = text(line, "text");
    let (region, world) = match world(line, env) {
        Ok(x) => x,
        Err(d) => return d,
    };
    let ix = Index::build(&region, &world);
    let solid = match model_named(&ix, &name) {
        Ok(s) => s,
        Err(d) => return d,
    };
    let f = crate::browser::mesh::facts(&world.solids[solid], &world.textures);
    let mut lines = vec![format!("{}  0x{:08X}", f.name, f.hash)];
    lines.extend(f.lines());
    Done {
        code: OK,
        text: lines,
        doc: json!({
            "region": region, "model": f.name, "hash": format!("0x{:08X}", f.hash), "vertices": f.vertices,
            "triangles": f.triangles, "lit": f.lit, "min": f.min, "max": f.max, "size": f.size, "markers": f.markers,
            "parts": f.parts.iter().map(|p| json!({
                "name": p.name, "triangles": p.triangles, "texture": p.texture.map(|h| format!("world:0x{h:08X}")),
            })).collect::<Vec<_>>(),
        }),
    }
}

fn world(line: &args::Line, env: &Env) -> Result<(String, nfsu2::studio_api::world::World), Done> {
    let root = game_root(line, env)?;
    let region = line.flag("region").unwrap_or(REGION).to_string();
    let world = (env.world)(&root, &region).map_err(|e| fail(NO_GAME, format!("the game's world does not load: {e}")))?;
    Ok((region, world))
}

/// `x,y,z` (metres, the game's Z up).
pub fn parse_point(text: &str) -> Option<glam::Vec3> {
    let v: Vec<f32> = text.split(',').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    match v.as_slice() {
        [x, y, z] if v.iter().all(|c| c.is_finite()) => Some(glam::Vec3::new(*x, *y, *z)),
        _ => None,
    }
}

/// `browser area`: the area around a point (the camera) as one glTF reference, as the window's Export area writes it.
fn area(line: &args::Line, env: &Env) -> Done {
    use crate::browser::gltf;
    let at = text(line, "at");
    let Some(centre) = parse_point(&at) else {
        return fail(USAGE, "--at x,y,z is needed: the game's coordinates in metres, Z up (the camera's place)");
    };
    let radius = line.args.get("radius").and_then(Value::as_f64).map_or(gltf::AREA_RADIUS, |r| r as f32);
    let to = text(line, "to");
    let dir = match (Some(to).filter(|t| !t.is_empty()), line.flag("mod").filter(|t| !t.is_empty())) {
        (Some(t), _) => std::path::PathBuf::from(t),
        (None, Some(m)) => gltf::reference_dir(std::path::Path::new(m)),
        (None, None) => return fail(USAGE, "--to (a folder) or --mod is needed: where the glTF is written"),
    };
    let root = match game_root(line, env) {
        Ok(r) => r,
        Err(d) => return d,
    };
    let (region, world) = match world(line, env) {
        Ok(x) => x,
        Err(d) => return d,
    };
    match gltf::write_area(&dir, Some(&root), &world.placements, &world.solids, &world.textures, &region, centre, radius) {
        Ok(r) => Done {
            code: OK,
            text: vec![format!(
                "{} written: {} placements of {} models within {radius} m, from your game files, local-only (see {})",
                r.files.gltf.display(), r.placements, r.models, gltf::NOTE_FILE
            )],
            doc: json!({
                "region": region, "gltf": r.files.gltf, "bin": r.files.bin, "textures": r.files.textures, "note": r.files.note,
                "at": [centre.x, centre.y, centre.z], "radius": radius, "placements": r.placements, "models": r.models,
                "skipped": r.skipped, "origin": "from-game-files", "share": "local-only",
            }),
        },
        Err(e) => fail(super::FAILED, e),
    }
}

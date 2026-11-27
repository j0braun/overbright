//! `collision show`: what the World view's collision layer shows in each mode (View ▸ Collision: off, invisible
//! barriers, all collision), counted on the region's collision as the window builds it ([`crate::collision_view`]),
//! with the mod's removed and drawn collision applied when `--mod` is given.

use std::sync::Arc;

use serde_json::{Value, json};

use super::{Done, Env, FAILED, NO_GAME, OK, REGION, USAGE, args, fail, game_root};
use crate::collision_view::{Show, layer_of};
use crate::history::{History, disk};
use crate::session::MapSession;
use ob_map::tools::map::format::MapMod;
use ob_map::tools::map::store;

pub(super) fn show(line: &args::Line, env: &Env) -> Done {
    let word = line.args.get("mode").and_then(Value::as_str).unwrap_or("all");
    let Some(mode) = Show::parse(word) else {
        let mut d = fail(USAGE, format!("--mode: no mode `{word}`: use off, invisible or all"));
        d.doc["error"]["reason"] = json!("bad_mode");
        return d;
    };
    let region = line.flag("region").unwrap_or(REGION).to_string();
    let (map, folder) = match line.flag("mod").filter(|d| !d.is_empty()) {
        None => (MapMod::default(), String::new()),
        Some(dir) => {
            let dir = std::path::PathBuf::from(dir);
            let edir = dir.join(store::EDITOR_DIR);
            let h = match History::open_read(&dir, &edir, crate::MOD_DOCS, Arc::new(disk::Real)) {
                Ok(h) => h,
                Err(e) => return fail(FAILED, format!("{}: {e}", dir.display())),
            };
            match MapMod::parse(h.doc(store::FILE).unwrap_or_default()) {
                Ok(m) => (m, store::folder_of_file(&dir.join(store::FILE))),
                Err(e) => return fail(FAILED, format!("{} does not read: {e}", store::FILE)),
            }
        }
    };
    let root = match game_root(line, env) {
        Ok(r) => r,
        Err(d) => return d,
    };
    let world = match (env.world)(&root, &region) {
        Ok(w) => w,
        Err(e) => return fail(NO_GAME, format!("the game's world does not load: {e}")),
    };
    let session = MapSession::new(&region, &folder, map, world);
    let layer = layer_of(&session.world, &session.base.placements);
    let counts = layer.counts();
    let off = layer.floors.iter().filter(|f| f.not_ground).count();
    let rows = mode.rows(&counts, off);
    let mut text = vec![format!("Collision in {region}: {}", mode.label())];
    text.extend(rows.iter().map(|(what, n)| format!("  {what}: {n}")));
    if rows.is_empty() {
        text.push("  nothing is drawn".into());
    }
    let doc = json!({
        "region": region, "mode": word, "label": mode.label(), "mod": line.flag("mod"),
        "faces_read": layer.read,
        "counts": rows.iter().map(|(what, n)| json!({ "what": what, "count": n })).collect::<Vec<_>>(),
        "all": {
            "invisible": counts.invisible, "barriers": counts.barriers, "visible": counts.visible,
            "floors": counts.floors, "floors_not_ground": off,
        },
    });
    Done { code: OK, text, doc }
}

//! The edit report (the launcher's Look inside, `overbright-studio-cli modpack report`): one text a player can read that
//! says what a mod pack changes compared with the game, in words: who made it and for which game copy, what it
//! credits, each prop it moves, removes or adds, its races, the textures it replaces, and what it overrules among the
//! packs that are on. Read only: nothing is mounted or switched on.

use std::path::Path;

use ob_asset_format::modpack::{self, BaseId};

use super::inside::{Inside, inside, inside_of};

/// Each list is cut after this many lines; the rest is counted.
const LIST: usize = 40;

fn list(out: &mut String, title: &str, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    out.push_str(&format!("\n{title}\n"));
    for l in lines.iter().take(LIST) {
        out.push_str(&format!("  - {l}\n"));
    }
    if lines.len() > LIST {
        out.push_str(&format!("  - and {} more\n", lines.len() - LIST));
    }
}

/// The report text for `i`.
pub fn text(i: &Inside) -> String {
    let mut out = format!("Edit report: {} ({} {})\n", i.name, i.id, i.version);
    for l in i.lines().into_iter().filter(|l| !l.starts_with("Changes ") && !l.starts_with("Regions ") && !l.starts_with("Credit:")) {
        out.push_str(&format!("{l}\n"));
    }
    out.push_str("\nCompared with the game\n");
    let changes = i.lines().into_iter().find(|l| l.starts_with("Changes ")).unwrap_or_default();
    out.push_str(&format!("  {changes}\n"));
    if !i.regions.is_empty() {
        out.push_str(&format!("  Regions {}\n", i.regions.join(", ")));
    }
    list(&mut out, "Props", &i.prop_lines);
    list(&mut out, "Races", &i.race_lines);
    list(&mut out, "Textures it replaces", &i.texture_ids);
    let credits: Vec<String> = i.credits.iter().map(|c| c.line()).collect();
    list(&mut out, "Credits", &credits);
    out
}

/// The report of the installed pack `id`; None when it is not installed.
pub fn of_id(id: &str, base: Option<&BaseId>) -> Option<String> {
    inside(id, base).map(|i| text(&i))
}

/// The report of the pack at `path` (a `.obpk` file or a pack folder), installed or not.
pub fn of_path(path: &Path, base: Option<&BaseId>) -> Result<String, String> {
    let p = modpack::open(path)?;
    let installed = modpack::scan(&super::mods_dir()).packs.iter().any(|q| q.id == p.id && q.source.path() == path);
    Ok(text(&inside_of(&p, base, installed)))
}

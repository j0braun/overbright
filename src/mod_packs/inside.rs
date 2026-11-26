//! "Look inside" (the launcher's Mods page): what a mod pack changes, read from its `pack.toml`, its map data and the
//! layer stack. Read only: nothing is mounted, written or switched on.

use std::collections::BTreeSet;

use ob_asset_format::modpack::{self, BaseId};

use crate::tools::map::format::PropEdit;

/// What a pack holds and how it stands among the packs that are on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inside {
    pub id: String,
    pub name: String,
    pub version: String,
    pub authors: Vec<String>,
    pub licence: Option<String>,
    pub description: String,
    /// The base sets it was made for ("any copy of the game" when it names none).
    pub made_for: String,
    /// The packs it needs.
    pub needs: Vec<String>,
    pub races: usize,
    pub moved: usize,
    pub deleted: usize,
    pub added: usize,
    /// The map regions its prop edits are in.
    pub regions: Vec<String>,
    pub textures: usize,
    /// Collision edits (walls and grounds drawn, faces taken out).
    pub collision: usize,
    /// Each prop edit in words (for the edit report).
    pub prop_lines: Vec<String>,
    /// Each race in words.
    pub race_lines: Vec<String>,
    /// The textures it replaces, as `world:0x1A2B3C4D` / `car:…`.
    pub texture_ids: Vec<String>,
    /// Who made third-party assets in it (the credits in its provenance).
    pub credits: Vec<super::credits::Credit>,
    /// Where a converted pack came from (its provenance), when it was made by `modpack convert`.
    pub converted_from: Option<String>,
    /// Why it cannot be used with the base set in use.
    pub refused: Option<String>,
    /// What it overrules or is overruled by among the packs that are on: (pack, it wins, values).
    pub clashes: Vec<(String, bool, usize)>,
}

fn n(c: usize, one: &str, many: &str) -> String {
    if c == 1 { format!("1 {one}") } else { format!("{c} {many}") }
}

impl Inside {
    /// The lines the page shows.
    pub fn lines(&self) -> Vec<String> {
        let mut v = vec![format!("Version {}{}", self.version, if self.authors.is_empty() { String::new() } else { format!(" by {}", self.authors.join(", ")) })];
        if let Some(l) = &self.licence {
            v.push(format!("Licence {l}"));
        }
        if !self.description.trim().is_empty() {
            v.push(self.description.trim().to_string());
        }
        v.push(format!("Made for {}", self.made_for));
        for c in &self.credits {
            v.push(format!("Credit: {}", c.line()));
        }
        if let Some(f) = &self.converted_from {
            v.push(format!("Provenance: converted from {f}"));
        }
        if !self.needs.is_empty() {
            v.push(format!("Needs {}", self.needs.join(", ")));
        }
        let kinds: Vec<String> = [
            (self.races, "race", "races"),
            (self.moved, "moved prop", "moved props"),
            (self.deleted, "deleted prop", "deleted props"),
            (self.collision, "collision edit", "collision edits"),
            (self.added, "added prop", "added props"),
            (self.textures, "texture", "textures"),
        ]
        .into_iter()
        .filter(|(c, ..)| *c > 0)
        .map(|(c, a, b)| n(c, a, b))
        .collect();
        v.push(if kinds.is_empty() { "Changes nothing the game reads".into() } else { format!("Changes {}", kinds.join(", ")) });
        if !self.regions.is_empty() {
            v.push(format!("Regions {}", self.regions.join(", ")));
        }
        for (other, wins, c) in &self.clashes {
            v.push(format!("{} {other} ({})", if *wins { "Overrules" } else { "Overruled by" }, n(*c, "value", "values")));
        }
        if self.clashes.is_empty() {
            v.push("No clash with the packs that are on".into());
        }
        if let Some(r) = &self.refused {
            v.push(format!("Not usable: {r}"));
        }
        v
    }
}

/// The report for pack `id` against `base`; None when it is not installed.
pub fn inside(id: &str, base: Option<&BaseId>) -> Option<Inside> {
    let dir = super::mods_dir();
    let scan = modpack::scan(&dir);
    let p = scan.packs.iter().find(|p| p.id == id)?;
    Some(inside_of(p, base, true))
}

/// The report for the pack `p`, installed in the mods folder or not (a file the player was given: no order, so no
/// clashes with the packs that are on).
pub fn inside_of(p: &modpack::Installed, base: Option<&BaseId>, installed: bool) -> Inside {
    let (id, dir) = (p.id.as_str(), super::mods_dir());
    let m = &p.manifest;
    let mut out = Inside {
        id: id.into(),
        name: m.name.clone(),
        version: m.version.to_string(),
        authors: m.authors.clone(),
        licence: m.licence.clone(),
        description: m.description.clone(),
        needs: m.dependencies.keys().cloned().collect(),
        ..Default::default()
    };
    let bases: Vec<&str> = m.game_bases.iter().flatten().map(String::as_str).filter(|b| *b != "*").collect();
    out.made_for = if !bases.is_empty() {
        format!("base set {}", bases.join(", "))
    } else if m.game_fingerprint.as_ref().is_some_and(|f| !f.is_empty()) {
        "one copy of the game".into()
    } else {
        "any copy of the game".into()
    };
    out.refused = if installed {
        modpack::plan_with(&dir, base, &super::engine(), Some(id)).refused.into_iter().find(|(r, _)| r == id).map(|(_, why)| why)
    } else {
        modpack::check_base(m, base).err()
    };
    let prov = provenance_text(p);
    out.converted_from = prov.as_deref().and_then(converted_from);
    out.credits = prov.as_deref().map(super::credits::parse).unwrap_or_default();
    if let Ok(root) = super::materialize(p) {
        if let Ok(Some(map)) = super::content::read(&root) {
            out.races = map.races.len();
            out.collision = map.collision.len();
            out.race_lines = map
                .races
                .iter()
                .map(|r| {
                    let laps = if r.kind.loops() { format!(", {}", n(r.laps as usize, "lap", "laps")) } else { String::new() };
                    format!("{} ({}{laps}, {})", if r.name.trim().is_empty() { &r.id } else { r.name.trim() }, r.kind.name(), n(r.checkpoints.len(), "checkpoint", "checkpoints"))
                })
                .collect();
            let mut regions = BTreeSet::new();
            for e in &map.props {
                match e {
                    PropEdit::Move { target, position, .. } => {
                        out.moved += 1;
                        out.prop_lines.push(format!(
                            "Moves {} in {} by {:.1} m",
                            target.solid,
                            target.region,
                            position.distance(target.at)
                        ));
                        regions.insert(target.region.clone());
                    }
                    PropEdit::Delete { target } => {
                        out.deleted += 1;
                        out.prop_lines.push(format!("Removes {} in {}", target.solid, target.region));
                        regions.insert(target.region.clone());
                    }
                    PropEdit::Add { region, solid, .. } => {
                        out.added += 1;
                        out.prop_lines.push(format!("Adds {solid} in {region}"));
                        regions.insert(region.clone());
                    }
                }
            }
            out.regions = regions.into_iter().collect();
        }
        let mut seen = BTreeSet::new();
        for car in [false, true] {
            for d in super::content::texture_dirs(&root, car) {
                for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                    let path = e.path();
                    if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("png"))
                        && let Some(stem) = path.file_stem().and_then(|n| n.to_str())
                    {
                        seen.insert((car, stem.to_ascii_lowercase()));
                    }
                }
            }
        }
        out.textures = seen.len();
        out.texture_ids = seen.iter().map(|(car, s)| format!("{}:{s}", if *car { "car" } else { "world" })).collect();
    }
    if installed {
        out.clashes = super::clash_pairs_with(base, Some(id))
        .into_iter()
        .filter_map(|(weaker, stronger, c)| {
            if stronger == id {
                Some((weaker, true, c))
            } else if weaker == id {
                Some((stronger, false, c))
            } else {
                None
            }
        })
        .collect();
    }
    out
}

/// The text of a pack's `provenance.toml` (a folder's file, or the entry in the pack file).
pub(super) fn provenance_text(p: &modpack::Installed) -> Option<String> {
    use ob_asset_format::bundle::{self, Bundle};
    let name = crate::tools::map::share::PROVENANCE;
    match &p.source {
        modpack::Source::Folder(d) => std::fs::read_to_string(d.join(name)).ok(),
        modpack::Source::File(f) => {
            let b = Bundle::open(f, bundle::Kind::PackFile).ok()?;
            let e = b.get(name)?;
            String::from_utf8(b.read(e).ok()?.into_owned()).ok()
        }
    }
}

/// `converted_from` of a `provenance.toml`: set when `modpack convert` made the pack.
fn converted_from(text: &str) -> Option<String> {
    let t: toml::Table = text.parse().ok()?;
    if t.get("converted").and_then(toml::Value::as_bool) != Some(true) {
        return None;
    }
    Some(t.get("converted_from").and_then(toml::Value::as_str).unwrap_or("an older mod").to_string())
}

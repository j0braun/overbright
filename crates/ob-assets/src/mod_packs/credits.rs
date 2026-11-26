//! Credits of a pack (docs/modding/share-a-pack.md): who made third-party assets the pack holds, as `[[credit]]`
//! tables in its `provenance.toml`. A mod folder lists them in `credits.toml` (the same tables; it is shared with the
//! mod like any file); an export or conversion carries them into the pack's provenance, and the launcher's Look inside
//! and the edit report show them. A converted texture pack of someone else's adds its author's credit by itself.

use std::path::Path;

/// The credits file of a mod folder.
pub const FILE: &str = "credits.toml";
/// At most this many credits are read from one file.
pub const MAX: usize = 200;

/// One credit line: an asset (or a group of them), who made it, and the terms.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Credit {
    pub name: String,
    pub by: String,
    pub licence: Option<String>,
    pub source: Option<String>,
}

impl Credit {
    /// The words the launcher and the report show: `Name by Someone, CC-BY-4.0, https://…`.
    pub fn line(&self) -> String {
        let mut s = format!("{} by {}", self.name, self.by);
        for x in [&self.licence, &self.source].into_iter().flatten() {
            s.push_str(", ");
            s.push_str(x);
        }
        s
    }
}

fn clean(v: Option<&toml::Value>, max: usize) -> Option<String> {
    let t: String = v?.as_str()?.chars().filter(|c| !c.is_control()).take(max).collect();
    let t = t.trim().to_string();
    (!t.is_empty()).then_some(t)
}

/// The `[[credit]]` tables of a `credits.toml` or `provenance.toml` text. An entry with no name or no author is left
/// out; text that does not read as TOML gives none.
pub fn parse(text: &str) -> Vec<Credit> {
    let Ok(t) = text.parse::<toml::Table>() else { return Vec::new() };
    let Some(list) = t.get("credit").and_then(toml::Value::as_array) else { return Vec::new() };
    let mut out: Vec<Credit> = Vec::new();
    for c in list.iter().take(MAX) {
        let (Some(name), Some(by)) = (clean(c.get("name"), 120), clean(c.get("by"), 120)) else { continue };
        let credit = Credit { name, by, licence: clean(c.get("licence"), 200), source: clean(c.get("source"), 300) };
        if !out.contains(&credit) {
            out.push(credit);
        }
    }
    out
}

/// A mod folder's own credits (`credits.toml`); none when it has no such file.
pub fn of_dir(dir: &Path) -> Vec<Credit> {
    std::fs::read_to_string(dir.join(FILE)).map(|t| parse(&t)).unwrap_or_default()
}

/// The `[[credit]]` tables for a provenance text.
pub fn toml(credits: &[Credit]) -> String {
    let mut s = String::new();
    for c in credits {
        s += &format!("\n[[credit]]\nname = {}\nby = {}\n", super::pack_export::q(&c.name), super::pack_export::q(&c.by));
        if let Some(l) = &c.licence {
            s += &format!("licence = {}\n", super::pack_export::q(l));
        }
        if let Some(x) = &c.source {
            s += &format!("source = {}\n", super::pack_export::q(x));
        }
    }
    s
}

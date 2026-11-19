//! The specification's generated parts and the check that keeps the pages and the code in step (versioning.md
//! §6). Generated from this crate:
//!
//! - every page's rule table (between `<!-- generated:rules -->` and `<!-- /generated -->`), from [`crate::rules`];
//! - the manifest's field tables (`<!-- generated:fields pack -->`, `<!-- generated:fields pack.base -->`), from
//!   [`crate::manifest::FIELDS`] and [`crate::manifest::BASE_FIELDS`];
//! - `schemas/pack.schema.json` and `schemas/pack-migrations.schema.json`, from the same field tables.
//!
//! [`check`] regenerates all of it in memory and lists every difference, every rule table outside its markers,
//! every rule id a page names that the registry lacks (and every registry rule no page shows), and every kind of
//! the README's registry a reference could not name. The example `asset_format_docs` writes or checks the tree;
//! the crate's test `docs_in_step` runs the check, so a change to one side without the other fails `cargo test`.

use std::collections::BTreeMap;
use std::path::Path;

use crate::fields;
use crate::manifest;
use crate::names;
use crate::rules::{CROSS_REFS, RESERVED, Rule};

/// The specification's folder, relative to the repository.
pub const DIR: &str = "docs/asset-format";

/// The pages and schemas as text, by path relative to [`DIR`] (`conventions.md`, `schemas/pack.schema.json`).
pub type Tree = BTreeMap<String, String>;

/// Reads the pages (`*.md`) and the schemas (`schemas/**/*.json`) under `dir`.
pub fn load(dir: &Path) -> std::io::Result<Tree> {
    let mut tree = Tree::new();
    fn walk(base: &Path, dir: &Path, tree: &mut Tree) -> std::io::Result<()> {
        for e in std::fs::read_dir(dir)? {
            let p = e?.path();
            let rel = p.strip_prefix(base).expect("under base").to_string_lossy().replace('\\', "/");
            if p.is_dir() {
                if rel == "schemas" || rel.starts_with("schemas/") {
                    walk(base, &p, tree)?;
                }
            } else if rel.ends_with(".md") || (rel.starts_with("schemas/") && rel.ends_with(".json")) {
                tree.insert(rel, std::fs::read_to_string(&p)?);
            }
        }
        Ok(())
    }
    walk(dir, dir, &mut tree)?;
    Ok(tree)
}

const END: &str = "<!-- /generated -->";

/// A page's rule table.
pub fn rule_table(page: &str) -> String {
    let mut out = String::from("| Rule | Text |\n|---|---|\n");
    for r in Rule::ALL.iter().filter(|r| r.page() == page) {
        out.push_str(&format!("| {} | {} |\n", r.id(), r.text()));
    }
    for (_, r, text) in CROSS_REFS.iter().filter(|(p, _, _)| *p == page) {
        out.push_str(&format!("| {} | {} |\n", r.id(), text));
    }
    out
}

/// What a marker's block holds.
fn block(page: &str, what: &str) -> Option<String> {
    match what {
        "rules" => Some(rule_table(page)),
        "fields pack" => Some(fields::doc_table(manifest::FIELDS)),
        "fields pack.base" => Some(fields::doc_table(manifest::BASE_FIELDS)),
        _ => None,
    }
}

/// A page with every generated block rewritten; `Err` names an unknown or unclosed marker.
pub fn regenerate_page(page: &str, text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    // A marker starts a line (a page may name the markers in its text).
    let find = |s: &str| {
        if s.starts_with("<!-- generated:") { Some(0) } else { s.find("\n<!-- generated:").map(|i| i + 1) }
    };
    while let Some(start) = find(rest) {
        let open_end = rest[start..].find("-->").ok_or_else(|| format!("{page}: an unclosed marker"))? + start + 3;
        let what = rest[start + "<!-- generated:".len()..open_end - 3].trim();
        let body = block(page, what).ok_or_else(|| format!("{page}: unknown marker `generated:{what}`"))?;
        let close = rest[open_end..]
            .find(&format!("\n{END}"))
            .map(|i| i + 1)
            .ok_or_else(|| format!("{page}: `generated:{what}` without `{END}`"))?
            + open_end;
        out.push_str(&rest[..open_end]);
        out.push('\n');
        out.push_str(&body);
        out.push_str(END);
        rest = &rest[close + END.len()..];
    }
    out.push_str(rest);
    Ok(out)
}

/// The generated schemas, by path.
pub fn schemas() -> Vec<(&'static str, String)> {
    vec![
        ("schemas/pack.schema.json", manifest::schema().pretty()),
        ("schemas/pack-migrations.schema.json", manifest::migrations_schema().pretty()),
    ]
}

/// Every file of `tree` whose generated content differs, with its new content.
pub fn generate(tree: &Tree) -> Result<Tree, String> {
    let mut changed = Tree::new();
    for (path, text) in tree.iter().filter(|(p, _)| p.ends_with(".md")) {
        let new = regenerate_page(path, text)?;
        if &new != text {
            changed.insert(path.clone(), new);
        }
    }
    for (path, text) in schemas() {
        if tree.get(path) != Some(&text) {
            changed.insert(path.to_string(), text);
        }
    }
    Ok(changed)
}

/// Rule ids a text names in backticks (`` `CONV.9` ``): two to four capitals, a dot, a number.
fn rule_mentions(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in text.split('`').skip(1).step_by(2) {
        if let Some((a, n)) = part.split_once('.')
            && (2..=4).contains(&a.len())
            && a.bytes().all(|b| b.is_ascii_uppercase())
            && !n.is_empty()
            && n.bytes().all(|b| b.is_ascii_digit())
        {
            out.push(part.to_string());
        }
    }
    out
}

/// The kinds the README's registry names (the first column's `` `name` ``s and the "Additional short names").
fn readme_kinds(readme: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(sec) = readme.split("## 3. Kind registry").nth(1) else { return out };
    let sec = sec.split("**Not yet owned by a kind**").next().unwrap_or(sec);
    for line in sec.lines() {
        let cells: &str = if let Some(row) = line.strip_prefix("| ") {
            row.split(" | ").next().unwrap_or("")
        } else if line.starts_with("**Additional short names**") || line.starts_with('`') {
            line
        } else {
            continue;
        };
        for part in cells.split('`').skip(1).step_by(2) {
            if part.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') && !part.is_empty() {
                out.push(part.to_string());
            }
        }
    }
    out
}

/// Every way the tree and the code disagree; empty when they are in step.
pub fn check(tree: &Tree) -> Vec<String> {
    let mut problems = Vec::new();
    match generate(tree) {
        Err(e) => problems.push(e),
        Ok(changed) => {
            for (path, new) in &changed {
                let old = tree.get(path).map(String::as_str).unwrap_or("");
                let line = old
                    .lines()
                    .zip(new.lines())
                    .position(|(a, b)| a != b)
                    .unwrap_or(old.lines().count().min(new.lines().count()))
                    + 1;
                problems.push(format!(
                    "{path}: differs from the code from line {line} on (run the example asset_format_docs --write)"
                ));
            }
        }
    }
    let mut shown = std::collections::HashSet::new();
    for (path, text) in tree.iter().filter(|(p, _)| p.ends_with(".md")) {
        // Every rule table is generated.
        let mut inside = false;
        for (n, line) in text.lines().enumerate() {
            if line.starts_with("<!-- generated:rules") {
                inside = true;
            } else if line.starts_with(END) {
                inside = false;
            } else if line == "| Rule | Text |" && !inside {
                problems.push(format!("{path}:{}: a rule table outside `<!-- generated:rules -->` markers", n + 1));
            }
        }
        if text.contains("<!-- generated:rules") {
            shown.insert(path.as_str());
        }
        for id in rule_mentions(text) {
            if Rule::from_id(&id).is_none() && !RESERVED.contains(&id.as_str()) {
                problems.push(format!("{path}: names the rule `{id}`, which the code does not define"));
            }
        }
    }
    for r in Rule::ALL {
        if !tree.contains_key(r.page()) {
            problems.push(format!("rule {} belongs to {}, which does not exist", r.id(), r.page()));
        } else if !shown.contains(r.page()) {
            problems.push(format!("rule {} belongs to {}, which has no generated rule table", r.id(), r.page()));
        }
    }
    if let Some(readme) = tree.get("README.md") {
        let kinds = readme_kinds(readme);
        if kinds.len() < 20 {
            problems.push("README.md: the kind registry was not found".into());
        }
        for k in kinds {
            if !names::KINDS.contains(&k.as_str()) {
                problems.push(format!("README.md: the kind `{k}` is not in names::KINDS"));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions() {
        assert_eq!(rule_mentions("see `CONV.9` and `P.1`, `MRG.3`; `a.b`, `TEX.x`"), ["CONV.9", "MRG.3"]);
    }

    #[test]
    fn markers() {
        let page = "a\n<!-- generated:rules -->\nold\n<!-- /generated -->\nb\n";
        let new = regenerate_page("layout.md", page).unwrap();
        assert!(new.starts_with("a\n<!-- generated:rules -->\n| Rule | Text |\n|---|---|\n| LAY.1 |"));
        assert!(new.ends_with("<!-- /generated -->\nb\n"));
        assert_eq!(regenerate_page("layout.md", &new).unwrap(), new);
        assert!(regenerate_page("x.md", "<!-- generated:nope -->\n<!-- /generated -->").is_err());
        assert!(regenerate_page("x.md", "<!-- generated:rules -->\n").is_err());
    }
}

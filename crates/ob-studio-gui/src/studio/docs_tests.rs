//! docs/modding/studio.md against the code: its "Keyboard and pad" table is the key table ([`KEYS`]) both ways, and
//! every chord the page names (Ctrl+K, Shift+F6, F5, ...) is one Studio's table or the map editor's table has.

use super::nav::KEYS;
use crate::editor::menu::KEYS as EDITOR_KEYS;

fn doc() -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/modding/studio.md");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The cells of the table's "Keys" column: (pad cell, keys cell, does cell).
fn table_rows(doc: &str) -> Vec<(String, String, String)> {
    let at = doc.find("\n## Keyboard and pad\n").expect("a Keyboard and pad section");
    let rest = &doc[at + 1..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |e| e + 3);
    rest[..end]
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| Pad") && !l.starts_with("|---"))
        .map(|l| {
            let c: Vec<&str> = l.trim_matches('|').split(" | ").map(str::trim).collect();
            assert_eq!(c.len(), 3, "a key table row with three cells: {l}");
            (c[0].to_string(), c[1].to_string(), c[2].to_string())
        })
        .collect()
}

/// A comma inside a chord (`Ctrl+,`) must not split the list.
const COMMA: &str = "Ctrl+COMMA";

/// The chords of a keys cell: split on `/`, `;` and `, `, without parentheses.
fn chords(cell: &str) -> Vec<String> {
    let mut s = cell.replace("Ctrl+,", COMMA);
    while let (Some(a), Some(b)) = (s.find('('), s.find(')')) {
        s.replace_range(a..=b, "");
    }
    s.split(['/', ';', ',']).map(|c| c.trim().to_string()).filter(|c| !c.is_empty() && c != "–").collect()
}

/// Where the page and the table word the same key differently.
fn alias(c: &str) -> &str {
    match c {
        "Right mouse button + WASD" => "RMB + WASD",
        "Menu key" => "Menu key",
        other => other,
    }
}

#[test]
fn studio_key_table_matches_the_doc_both_ways() {
    let doc = doc();
    let rows = table_rows(&doc);
    assert!(rows.len() >= 10, "the table has its rows");
    let doc_keys: Vec<String> = rows.iter().flat_map(|r| chords(&r.1)).map(|c| alias(&c).to_string()).collect();
    let mut code_keys = Vec::new();
    for k in KEYS.iter() {
        for c in chords(k.keys) {
            assert!(doc_keys.contains(&c), "the key table has `{c}` ({}), the Keyboard and pad table does not", k.word);
            code_keys.push(c);
        }
    }
    for c in &doc_keys {
        assert!(code_keys.contains(c), "the Keyboard and pad table names `{c}`, which the key table does not have");
    }
}

/// Chords written in the page's prose: `Ctrl+K`, `Shift+F6`, `Alt+←`, a lone `F5`.
fn prose_chords(doc: &str) -> Vec<String> {
    let mut out = Vec::new();
    let words = doc.split(|c: char| c.is_whitespace() || "(),;:`".contains(c) && c != '+');
    for w in words {
        let w = w.trim_matches(|c: char| ".*|".contains(c));
        let has_mod = ["Ctrl+", "Alt+", "Shift+"].iter().any(|m| w.starts_with(m));
        let fkey = w.len() >= 2
            && w.len() <= 3
            && w.starts_with('F')
            && w[1..].chars().all(|c| c.is_ascii_digit())
            && w[1..].parse::<u8>().is_ok_and(|n| (1..=12).contains(&n));
        if has_mod && w.len() > 4 || fkey {
            out.push(w.to_string());
        }
    }
    out
}

#[test]
fn every_chord_the_studio_page_names_exists_in_the_code() {
    let doc = doc();
    let mut known = String::new();
    for k in KEYS.iter() {
        known.push_str(k.keys);
        known.push('\n');
    }
    for b in EDITOR_KEYS {
        known.push_str(b.keys);
        known.push('\n');
    }
    // Keys only a page handles: Changes' multi-select (Shift+↓) and Runs' rename (F2) and Browser's export (Ctrl+E), area export (Ctrl+Shift+E) and bring back (Ctrl+I), in `shell.rs`.
    known.push_str("Shift+↓ F2 Shift+Tab Ctrl+E Ctrl+Shift+E Ctrl+I");
    for c in prose_chords(&doc) {
        if c.ends_with("number")
            || (c.len() == 6 && c.starts_with("Ctrl+") && c.ends_with(|d: char| d.is_ascii_digit()))
        {
            continue; // the rail's numbers, in the table as Ctrl+1 … Ctrl+0
        }
        let c = c.trim_end_matches(['.', '→']).to_string();
        let ok = known.contains(&c) || known.contains(c.trim_end_matches(['←', '→']));
        assert!(ok, "studio.md names the key `{c}`, which no key table has");
    }
}

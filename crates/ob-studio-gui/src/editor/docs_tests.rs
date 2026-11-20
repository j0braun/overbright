//! docs/modding/map-editor.md against the code: its Controls table is the key
//! table ([`KEYS`] and the tools' keys, both ways), every menu item the
//! menu bar draws has a `**Menu ▸ Item**` line and every such line names an
//! item that exists, every [`Command`] is in a menu, and the toolbar's
//! tools and the bottom tabs are named.

use super::app::Panels;
use super::menu::{Command, KEYS, Tool, controls};

fn read(rel: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn doc() -> String {
    read("../../docs/modding/map-editor.md")
}

/// The rows of the table under `## Controls`: (keys without backticks, what).
fn doc_controls(doc: &str) -> Vec<(String, String)> {
    let at = doc.find("\n## Controls\n").expect("a Controls section");
    let rest = &doc[at + 1..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |e| e + 3);
    rest[..end]
        .lines()
        .filter(|l| l.starts_with("| `"))
        .map(|l| {
            let cells: Vec<&str> = l.trim_matches('|').split(" | ").map(str::trim).collect();
            assert_eq!(cells.len(), 2, "a Controls row with two cells: {l}");
            (cells[0].trim_matches('`').to_string(), cells[1].to_string())
        })
        .collect()
}

/// The first string literal of a line.
fn literal(line: &str) -> Option<&str> {
    let a = line.find('"')? + 1;
    let b = line[a..].find('"')? + a;
    Some(&line[a..b])
}

/// A drawn item's name: its first string literal, or the shared word it
/// names (`words::SAVE`, crates/ob-look) when that comes first.
fn item_name(line: &str) -> Option<String> {
    let lit = line.find('"');
    if let Some(i) = line.find("words::")
        && lit.is_none_or(|l| i < l)
    {
        let id: String = line[i + 7..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        return ob_look::words::ALL.iter().find(|w| w.to_uppercase() == id).map(|w| w.to_string());
    }
    literal(line).map(str::to_string)
}

/// The menu bar's items as the source of `menu::bar` draws them:
/// ("Menu ▸ Item", the command it posts if any).
fn menu_items() -> Vec<(String, Option<String>)> {
    let src = read("src/editor/menu.rs");
    let start = src.find("pub fn bar(").expect("menu::bar");
    let end = start + src[start..].find("\n}\n").expect("the end of menu::bar");
    // The open menus with their lines' indents (a submenu nests: "View ▸
    // Collision ▸ Off").
    let mut menus: Vec<(usize, &str)> = Vec::new();
    let mut out = Vec::new();
    for line in src[start..end].lines() {
        let indent = line.len() - line.trim_start().len();
        if !line.trim().is_empty() {
            while menus.last().is_some_and(|&(i, _)| indent <= i) {
                menus.pop();
            }
        }
        if line.contains("menu_button(") {
            menus.push((indent, literal(line).expect("a menu's name")));
            continue;
        }
        let menu = menus.iter().map(|m| m.1).collect::<Vec<_>>().join(" ▸ ");
        // The collision layer's choices are drawn from their own table.
        if line.contains("collision::Show::ALL") {
            out.extend(super::collision::Show::ALL.map(|c| (format!("{menu} ▸ {}", c.label()), None)));
            continue;
        }
        let drawn = ["item(ui, \"", "item(ui, words::", "command(ui, s, \"", "command(ui, s, words::", "Button::selectable(", "ui.button(\""];
        if !drawn.iter().any(|d| line.contains(d)) {
            continue;
        }
        let Some(name) = item_name(line) else { continue };
        let cmd = line
            .find("Command::")
            .map(|i| line[i + 9..].chars().take_while(char::is_ascii_alphanumeric).collect::<String>());
        out.push((format!("{menu} ▸ {name}"), cmd));
    }
    out
}

#[test]
fn the_controls_table_is_the_key_table() {
    let doc = doc_controls(&doc());
    let code: Vec<(String, String)> =
        controls().into_iter().flat_map(|(_, rows)| rows).map(|(k, w)| (k, w.to_string())).collect();
    assert!(code.len() > 20, "{code:?}");
    let missing: Vec<_> = code.iter().filter(|r| !doc.contains(r)).collect();
    assert!(missing.is_empty(), "keys the editor has but docs/modding/map-editor.md's Controls table lacks: {missing:?}");
    let stale: Vec<_> = doc.iter().filter(|r| !code.contains(r)).collect();
    assert!(stale.is_empty(), "Controls rows in docs/modding/map-editor.md the key table does not have: {stale:?}");
    assert_eq!(doc.len(), code.len(), "every row once");
    assert!(KEYS.iter().all(|b| doc.iter().any(|(k, _)| k == b.keys)));
}

#[test]
fn every_menu_item_and_command_is_documented_and_exists() {
    let doc = doc();
    let items = menu_items();
    assert!(items.len() >= 15, "the scan found the menus: {items:?}");
    // Every command has a menu item (and so a doc line below).
    for c in Command::ALL {
        let name = format!("{c:?}");
        assert!(items.iter().any(|(_, k)| k.as_deref() == Some(name.as_str())), "{name} is in no menu");
    }
    // Every item drawn has its line in the doc.
    let missing: Vec<_> = items.iter().filter(|(p, _)| !doc.contains(&format!("**{p}**"))).map(|(p, _)| p).collect();
    assert!(missing.is_empty(), "menu items without a **Menu ▸ Item** line in docs/modding/map-editor.md: {missing:?}");
    // Every `**Menu ▸ Item**` in the doc names an item that exists (Undo
    // and Redo are drawn with their step's name).
    let mut stale = Vec::new();
    for path in doc.split("**").skip(1).step_by(2) {
        let path = path.split_whitespace().collect::<Vec<_>>().join(" ");
        let path = path.as_str();
        if !path.contains(" ▸ ") {
            continue;
        }
        let known = items.iter().any(|(p, _)| p == path) || ["Edit ▸ Undo", "Edit ▸ Redo"].contains(&path);
        if !known {
            stale.push(path.to_string());
        }
    }
    stale.dedup();
    assert!(stale.is_empty(), "docs/modding/map-editor.md names menu items the editor does not have: {stale:?}");
}

#[test]
fn toolbar_tools_and_tabs_are_named() {
    let doc = doc();
    for t in Tool::ALL {
        assert!(doc.contains(&format!("**{}** ({})", t.name(), t.key().name())), "{t:?} with its key");
    }
    for name in ["Snap", "Undo", "Redo", "Save", "Simulate", "Drive here", "Test race", "Work light", "Game look"] {
        assert!(doc.contains(&format!("**{name}**")), "{name}");
    }
    let tabs: Vec<&str> = Panels::standard().tabs.iter().map(|t| t.1).chain(["Log"]).collect();
    for t in tabs {
        assert!(doc.contains(&format!("**{t}**")), "the {t} tab");
    }
}

#[test]
fn the_three_test_modes_say_who_drives_in_the_doc_and_the_ui() {
    use super::modes::{DRIVE_HERE, SIMULATE, TEST_RACE};
    let doc = doc();
    for (m, who) in [(SIMULATE, "the AI"), (TEST_RACE, "you"), (DRIVE_HERE, "you")] {
        let key = if m.key.is_empty() { String::new() } else { format!(" ({})", m.key) };
        assert!(doc.contains(&format!("| **{}**{key} | {who} |", m.name)), "{} in the testing table", m.name);
    }
    assert!(doc.contains(&format!("**Help ▸ {}**", super::modes::HELP_TITLE)));
    // The menu items and the toolbar buttons carry the same words.
    assert_eq!(Command::TestRace.tip(), Some(TEST_RACE.tooltip()));
    assert_eq!(Command::DriveHere.tip(), Some(DRIVE_HERE.tooltip()));
    let menu = read("src/editor/menu.rs");
    assert!(menu.contains("on_hover_text(super::modes::TEST_RACE.tooltip())"));
    assert!(menu.contains("on_hover_text(super::modes::DRIVE_HERE.tooltip())"));
    assert!(read("src/editor/sim/panel.rs").contains("SIMULATE.tooltip()"));
}

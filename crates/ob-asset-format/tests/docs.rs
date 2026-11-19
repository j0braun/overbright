//! The specification and the code agree (versioning.md §6), and a planted drift is caught.
use std::path::PathBuf;

use ob_asset_format::docs;

/// The specification's folder (tests run in the package's folder).
fn dir() -> PathBuf {
    PathBuf::from("../..").join(docs::DIR)
}

fn tree() -> docs::Tree {
    docs::load(&dir()).expect("docs/asset-format")
}

#[test]
fn page_examples_match_the_example_files() {
    // manifest.md §1 and §2 show the two manifest examples; the §2 one lists all eight languages.
    let page = std::fs::read_to_string(dir().join("manifest.md")).unwrap();
    let blocks: Vec<&str> = page.split("```toml\n").skip(1).map(|b| b.split("```").next().unwrap()).collect();
    assert_eq!(blocks.len(), 2);
    let example = |p: &str| std::fs::read_to_string(dir().join("examples/manifest").join(p)).unwrap();
    assert_eq!(blocks[0], example("mod/pack.toml"));
    let parse = |t: &str| {
        ob_asset_format::manifest::Manifest::parse(t.as_bytes(), ob_asset_format::manifest::Container::Unknown)
            .unwrap()
            .0
    };
    let base = parse(blocks[1]);
    assert_eq!(base.base.as_ref().unwrap().languages.len(), 8);
    assert_eq!(base.to_toml(), blocks[1]);
    // layering.md §2's order.txt and layout.md §1's active.txt line.
    let layering = std::fs::read_to_string(dir().join("layering.md")).unwrap();
    let order = layering.split("```text\n").nth(2).unwrap().split("```").next().unwrap();
    let (o, notices) = ob_asset_format::order::Order::parse(order.as_bytes()).unwrap();
    assert!(notices.is_empty());
    assert_eq!(o.render(), order);
}

#[test]
fn docs_in_step() {
    let problems = docs::check(&tree());
    assert!(
        problems.is_empty(),
        "the pages and the code differ (run `cargo run -p ob-asset-format --example asset_format_docs -- --write`):\n{}",
        problems.join("\n")
    );
}

/// Changes one file of the tree and expects the check to name it.
fn planted(path: &str, from: &str, to: &str) -> Vec<String> {
    let mut t = tree();
    let text = t.get_mut(path).unwrap_or_else(|| panic!("{path}"));
    assert!(text.contains(from), "{path} holds {from:?}");
    *text = text.replacen(from, to, 1);
    docs::check(&t)
}

#[test]
fn planted_drift_fails() {
    // A rule's text edited on its page only.
    let p = planted("layout.md", "| LAY.2 | Two bundles", "| LAY.2 | Three bundles");
    assert!(p.iter().any(|l| l.starts_with("layout.md:")), "{p:?}");
    // A field's meaning edited in the manifest's table only.
    let p = planted("manifest.md", "Display name.", "The display name.");
    assert!(p.iter().any(|l| l.starts_with("manifest.md:")), "{p:?}");
    // A schema edited by hand.
    let p = planted("schemas/pack.schema.json", "\"maxItems\": 32", "\"maxItems\": 33");
    assert!(p.iter().any(|l| l.starts_with("schemas/pack.schema.json:")), "{p:?}");
    // A rule id a page names that the code lacks.
    let p = planted("world.md", "(`WLD.11`", "(`WLD.99`");
    assert!(p.iter().any(|l| l.contains("WLD.99")), "{p:?}");
    // A rule table outside its markers.
    let p = planted("text.md", "<!-- generated:rules -->\n", "");
    assert!(p.iter().any(|l| l.contains("outside")), "{p:?}");
    // A kind added to the README's registry without the code knowing it.
    let p = planted("README.md", "| `region` |", "| `region`, `district` |");
    assert!(p.iter().any(|l| l.contains("district")), "{p:?}");
}

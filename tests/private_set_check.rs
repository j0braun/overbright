//! The checks' rule for the private set (scripts/lib/common.sh `ck_private_here`, scripts/checks/checks.sh
//! `skip_private`): a tree that holds the set accepts no skip for lack of it. The harness' own side of the
//! rule (a binary built with the set fails when the set is gone) is tested in src/bin/game/dev/private.rs.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository: cargo runs an integration test in the package's folder.
fn repo() -> PathBuf {
    std::env::current_dir().expect("the test's working folder")
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-private-check-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `ck_private_here TREE` with OB_PRIVATE set to `ob_private` (None: unset).
fn private_here(tree: &Path, ob_private: Option<&Path>) -> bool {
    let lib = repo().join("scripts/lib/common.sh");
    let mut c = Command::new("bash");
    c.arg("-c").arg(r#"set -u; . "$1"; ck_private_here "$2""#).arg("bash").arg(&lib).arg(tree);
    c.env_remove("OB_PRIVATE");
    if let Some(p) = ob_private {
        c.env("OB_PRIVATE", p);
    }
    c.status().expect("bash runs").success()
}

#[test]
fn a_public_tree_holds_no_private_set() {
    let tree = temp_dir("public");
    assert!(!private_here(&tree, None));
    std::fs::remove_dir_all(&tree).ok();
}

#[test]
fn a_tree_with_the_saves_holds_the_private_set() {
    let tree = temp_dir("saves");
    std::fs::create_dir_all(tree.join("tests/fixtures/native_saves")).unwrap();
    assert!(private_here(&tree, None));
    std::fs::remove_dir_all(&tree).ok();
}

#[test]
fn ob_private_decides_when_set() {
    let tree = temp_dir("env");
    let set = temp_dir("env-set");
    assert!(private_here(&tree, Some(&set)), "OB_PRIVATE names a folder: the set is here");
    assert!(!private_here(&tree, Some(&tree.join("gone"))), "OB_PRIVATE names no folder: not here");
    std::fs::remove_dir_all(&tree).ok();
    std::fs::remove_dir_all(&set).ok();
}

/// Every skip for lack of the private set in checks.sh goes through `skip_private` (which fails in a tree
/// that holds the set), none straight into the skipped list.
#[test]
fn checks_route_every_private_skip_through_the_rule() {
    let text = std::fs::read_to_string(repo().join("scripts/checks/checks.sh")).unwrap();
    assert!(text.contains("ck_private_here \"$REPO\""), "checks.sh asks whether the tree holds the set");
    for line in text.lines() {
        if line.contains("SKIPPED+=") {
            assert!(!line.contains("OB_PRIVATE") && !line.contains("selftest: "), "a private skip bypasses skip_private: {line}");
        }
    }
    assert_eq!(text.matches("skip_private \"").count(), 5, "selftest, shots, flows, the release check and the rate gate's flows");
}

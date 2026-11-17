//! The maintainer's private set for the developer harness (docs/testing.md
//! "Private data"): the original's own saves that some shots, flows and
//! selftest steps start from. They are read when the harness runs, never
//! compiled in, so the public tree builds without them. Where the set is
//! missing, the step prints `SKIP: <step>: needs the maintainer's private
//! set (OB_PRIVATE)`; a shot or flow that cannot run at all then exits with
//! status 3, which scripts/checks/checks.sh records as skipped (not passed,
//! not failed). That holds for a harness built without the set (a public
//! tree). A harness built with it (`ob_private`, build.rs) whose set is
//! gone or moved fails instead: the error names the path looked in and says
//! to set OB_PRIVATE, and the process ends with status 1.

use std::path::PathBuf;

/// The reason a step could not run.
pub(crate) const NEEDS: &str = "needs the maintainer's private set (OB_PRIVATE)";

/// The exit status of a shot or flow skipped for lack of the private set.
pub(crate) const SKIP_STATUS: i32 = 3;

/// `$OB_PRIVATE`, else (until the set moves out) the repository the
/// harness was built from, as the test kit's `private_root`.
fn root() -> PathBuf {
    std::env::var_os("OB_PRIVATE")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

/// Whether this harness was built with the private set (build.rs sets
/// `ob_private` when it found the set).
const BUILT_WITH_SET: bool = cfg!(ob_private);

/// A file of the private set by its path under the set's root; `None` when
/// it is not there and the harness was built without the set. Any other
/// read error, and a missing file in a harness built with the set, is an
/// error.
pub(crate) fn read(rel: &str) -> anyhow::Result<Option<Vec<u8>>> {
    read_at(&path_of(&root(), rel), BUILT_WITH_SET)
}

/// `rel` (`/`-separated) under `root`, rebuilt from their parts so the
/// path has the system's one separator (a Windows harness built in the
/// release container has the root `/src`: `\src\tests\…`, not
/// `/src\tests/…`), and a Windows root such as `Z:\home\…` (Wine's view
/// of a Linux folder) works as given.
fn path_of(root: &std::path::Path, rel: &str) -> PathBuf {
    let base: PathBuf = root.components().collect();
    rel.split(['/', '\\']).filter(|c| !c.is_empty()).fold(base, |p, c| p.join(c))
}

/// [`read`] of `path`; `built_with_set`: the harness was built with the
/// private set, so a missing file means the set is gone or moved.
fn read_at(path: &std::path::Path, built_with_set: bool) -> anyhow::Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && !built_with_set => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(anyhow::anyhow!(
            "private file {} is missing, but this harness was built with the private set: \
             the set is gone or moved; set OB_PRIVATE to its folder (docs/testing.md \"Private data\")",
            path.display()
        )),
        Err(e) => Err(anyhow::anyhow!("private file {}: {e}", path.display())),
    }
}

/// The skip line for `what`.
pub(crate) fn skip_line(what: &str) -> String {
    format!("SKIP: {what}: {NEEDS}")
}

/// [`read`], or, when the file is missing, the skip line and the process
/// ends with [`SKIP_STATUS`] (a shot or flow that has nothing to run
/// without it).
pub(crate) fn require(rel: &str, what: &str) -> anyhow::Result<Vec<u8>> {
    match read(rel)? {
        Some(b) => Ok(b),
        // In a test (reported_issues' finished-career starts) a missing file fails that
        // test: such tests carry the OB_PRIVATE ignore (docs/testing.md).
        None if cfg!(test) => panic!("{}", skip_line(what)),
        None => {
            println!("{}", skip_line(what));
            std::process::exit(SKIP_STATUS)
        }
    }
}

/// The original's save of the maintainer's finished career (stage 6, every
/// shop entered): the start of the `finished_career_*` shots, `garage_neon`,
/// `--flow rain` / `outrun` / `engine`.
pub(crate) const FINISHED_CAREER: &str = "tests/fixtures/native_saves/career_complete.sav";

/// The profile name a flow files a private save under: the name stored in
/// the save itself, upper-cased (the profile folder's spelling), so no
/// profile name is written into the harness. A name with nothing a folder
/// name can carry (empty, or only other characters) is `PLAYER`.
pub(crate) fn profile_name(save: &nfsu2::formats::native_save::NativeSave) -> String {
    folder_name(&save.name())
}

/// [`profile_name`] of a save named `name`.
fn folder_name(name: &str) -> String {
    let name: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'))
        .collect::<String>()
        .trim()
        .to_ascii_uppercase();
    if name.is_empty() { "PLAYER".to_string() } else { name }
}

#[cfg(test)]
mod tests {
    /// One kind of separator in the path looked in.
    #[test]
    fn the_path_has_one_separator() {
        let p = super::path_of(std::path::Path::new("/src"), "tests/fixtures/native_saves/career_start.sav");
        let shown = p.display().to_string();
        let other = if std::path::MAIN_SEPARATOR == '/' { '\\' } else { '/' };
        assert!(!shown.contains(other), "{shown}");
        assert!(shown.ends_with(&format!("native_saves{}career_start.sav", std::path::MAIN_SEPARATOR)), "{shown}");
    }

    use super::*;

    /// The profile folder's name from the save's: upper-cased, only
    /// characters a folder name carries, `PLAYER` when none is left.
    #[test]
    fn the_profile_name_is_read_from_the_save() {
        assert_eq!(folder_name("Racer 7"), "RACER 7");
        assert_eq!(folder_name("a/b"), "AB");
        assert_eq!(folder_name(""), "PLAYER");
        assert_eq!(folder_name(" ?? "), "PLAYER");
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ob-private-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_missing_file_is_a_skip_without_the_set() {
        let d = temp_dir("skip");
        assert!(read_at(&d.join("no-such.sav"), false).unwrap().is_none());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_missing_file_fails_with_the_path_and_the_hint_when_built_with_the_set() {
        let d = temp_dir("fail");
        let path = d.join("no-such.sav");
        let e = read_at(&path, true).unwrap_err().to_string();
        assert!(e.contains(&path.display().to_string()), "{e}");
        assert!(e.contains("set OB_PRIVATE"), "{e}");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_present_file_is_read_either_way() {
        let d = temp_dir("read");
        let path = d.join("a.sav");
        std::fs::write(&path, b"abc").unwrap();
        for built in [false, true] {
            assert_eq!(read_at(&path, built).unwrap().as_deref(), Some(&b"abc"[..]));
        }
        std::fs::remove_dir_all(&d).ok();
    }
}

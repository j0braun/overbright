//! The version stamp of this build (build.rs): `OB_VERSION` and `OB_COMMIT`
//! as the release build sets them, else the package version and "unknown".

/// The version: `git describe` of the released commit, or the package
/// version for an ordinary build.
pub const VERSION: &str = env!("OB_VERSION");

/// The full commit id the build was made from, or "unknown".
pub const COMMIT: &str = env!("OB_COMMIT");

/// `game --version`'s line: name, version, commit and target.
pub fn version_line() -> String {
    format!("Overbright {VERSION} (commit {COMMIT}, {}-{})", std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_line_names_version_and_commit() {
        let line = super::version_line();
        assert!(line.starts_with("Overbright "), "{line}");
        assert!(line.contains(super::VERSION) && line.contains(super::COMMIT), "{line}");
        assert!(!super::VERSION.is_empty() && !super::COMMIT.is_empty());
    }
}

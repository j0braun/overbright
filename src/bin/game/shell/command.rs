//! The command line, parsed once into a
//! [`Command`]; the headless modes listed in one place; the data folders
//! the headless modes make for themselves removed at the end.

/// What the command line asks for (the first argument; `--no-boot` may
/// stand anywhere).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    /// `--version`: the version line.
    Version,
    /// `--help` / `-h`: the usage, exit code 0 (in every build, so asking
    /// a binary for its options never opens the game's window).
    Help,
    /// `--probe`: the launcher's hardware check (`shell::probe`).
    Probe,
    /// `--support-file [folder]`: the support file for a bug report
    /// (`nfsu2::diag::support`), with no window.
    SupportFile,
    /// `--list-adapters`: the graphics adapters, for the support file.
    ListAdapters,
    /// A flag this build does not know (`dev_hooks::unknown_flag`): the
    /// usage, exit code 2.
    Unknown(String),
    /// `--set-root <game folder>`.
    SetRoot(Option<String>),
    /// `--check-game [game folder]`.
    CheckGame(Option<String>),
    /// `--crash-test`: a deliberate panic with Crash Reports on.
    CrashTest,
    /// `--verify-cache [track ...]`.
    VerifyCache(Vec<String>),
    /// `--prepare`: the caches a first drive needs, made ahead with no
    /// window (`shell::prepare`).
    Prepare,
    /// `--dev-harness`: "dev-harness: on".
    #[cfg(feature = "dev-harness")]
    DevHarness,
    /// `--shot-list`.
    #[cfg(feature = "dev-harness")]
    ShotList,
    /// `--playthrough-all [seeds]`.
    #[cfg(feature = "dev-harness")]
    PlaythroughAll(Option<String>),
    /// `--playthrough`.
    #[cfg(feature = "dev-harness")]
    Playthrough,
    /// `--selftest`.
    #[cfg(feature = "dev-harness")]
    Selftest,
    /// `--selftest-showcase`.
    #[cfg(feature = "dev-harness")]
    SelftestShowcase,
    /// `--flow [name]`.
    #[cfg(feature = "dev-harness")]
    Flow(Option<String>),
    /// `--photo [dir] [screen]`.
    #[cfg(feature = "dev-harness")]
    Photo(Option<String>, Option<String>),
    /// `--bench [scene] [frames]`.
    #[cfg(feature = "dev-harness")]
    Bench(Option<String>, Option<String>),
    /// `--shot <target> <out.png>`: the whole command line (the target and
    /// the output are `args[1]` and `args[2]`, as before).
    #[cfg(feature = "dev-harness")]
    Shot(Vec<String>),
    /// The game in its window; `--no-boot` skips the boot screens,
    /// `--editor` turns the modding tools on (docs/texture-editor.md).
    Window { no_boot: bool },
}

impl Command {
    /// The command line (without the program name), in the order the start
    /// tested it: `--version`, `--help` and `--probe` first, then any flag this
    /// build does not know, then the first argument.
    pub(crate) fn parse(args: &[String]) -> Command {
        let first = args.first().map(String::as_str);
        let arg = |i: usize| args.get(i).cloned();
        match first {
            Some("--version") => return Command::Version,
            Some("--help" | "-h") => return Command::Help,
            Some("--probe") => return Command::Probe,
            Some("--support-file") => return Command::SupportFile,
            Some("--list-adapters") => return Command::ListAdapters,
            _ => {}
        }
        if let Some(flag) = crate::dev_hooks::unknown_flag(args) {
            return Command::Unknown(flag.to_string());
        }
        match first {
            #[cfg(feature = "dev-harness")]
            Some("--dev-harness") => Command::DevHarness,
            #[cfg(feature = "dev-harness")]
            Some("--shot-list") => Command::ShotList,
            Some("--set-root") => Command::SetRoot(arg(1)),
            Some("--check-game") => Command::CheckGame(arg(1)),
            #[cfg(feature = "dev-harness")]
            Some("--playthrough-all") => Command::PlaythroughAll(arg(1)),
            Some("--crash-test") => Command::CrashTest,
            Some("--verify-cache") => Command::VerifyCache(args[1..].to_vec()),
            Some("--prepare") => Command::Prepare,
            #[cfg(feature = "dev-harness")]
            Some("--selftest-showcase") => Command::SelftestShowcase,
            #[cfg(feature = "dev-harness")]
            Some("--selftest") => Command::Selftest,
            #[cfg(feature = "dev-harness")]
            Some("--flow") => Command::Flow(arg(1)),
            #[cfg(feature = "dev-harness")]
            Some("--playthrough") => Command::Playthrough,
            #[cfg(feature = "dev-harness")]
            Some("--photo") => Command::Photo(arg(1), arg(2)),
            #[cfg(feature = "dev-harness")]
            Some("--bench") => Command::Bench(arg(1), arg(2)),
            #[cfg(feature = "dev-harness")]
            Some("--shot") => Command::Shot(args.to_vec()),
            _ => Command::Window { no_boot: args.iter().any(|a| a == "--no-boot") },
        }
    }

    /// The headless modes (no window, no audio device): every harness
    /// mode `run` dispatches. `deterministic`: those whose images and
    /// state must repeat (the bench and the photo check left out), which
    /// build the garage's part changes in the frame.
    #[cfg(feature = "dev-harness")]
    pub(crate) fn headless(&self, deterministic: bool) -> bool {
        match self {
            Command::Shot(_)
            | Command::Selftest
            | Command::SelftestShowcase
            | Command::Flow(_)
            | Command::Playthrough => true,
            Command::Bench(..) | Command::Photo(..) => !deterministic,
            _ => false,
        }
    }
}

/// A data folder a headless mode made for itself (`XDG_DATA_HOME`): removed
/// when the start's `main` ends (not on `process::exit`). A folder the
/// caller named (NFSU2_SHOT_DATA, NFSU2_FLOW_DATA) is not one.
pub(crate) struct TempData(pub(crate) Option<std::path::PathBuf>);

impl Drop for TempData {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Command;

    fn parse(args: &[&str]) -> Command {
        Command::parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn the_command_line_parses_in_the_starts_order() {
        assert_eq!(parse(&["--version", "--bogus"]), Command::Version);
        assert_eq!(parse(&["--help"]), Command::Help);
        assert_eq!(parse(&["-h", "--no-boot"]), Command::Help);
        assert_eq!(parse(&["--probe"]), Command::Probe);
        assert_eq!(parse(&["--support-file", "/x"]), Command::SupportFile);
        assert_eq!(parse(&["--list-adapters"]), Command::ListAdapters);
        assert_eq!(parse(&[]), Command::Window { no_boot: false });
        assert_eq!(parse(&["--no-boot"]), Command::Window { no_boot: true });
        assert_eq!(parse(&["--set-root", "x"]), Command::SetRoot(Some("x".into())));
        assert_eq!(parse(&["--check-game"]), Command::CheckGame(None));
        assert_eq!(parse(&["--verify-cache", "a", "b"]), Command::VerifyCache(vec!["a".into(), "b".into()]));
        assert_eq!(parse(&["--crash-test"]), Command::CrashTest);
        #[cfg(feature = "dev-harness")]
        {
            assert_eq!(parse(&["--bench", "race"]), Command::Bench(Some("race".into()), None));
            assert!(parse(&["--shot", "main", "o.png"]).headless(true));
            assert!(!parse(&["--bench"]).headless(true) && parse(&["--bench"]).headless(false));
            assert!(!parse(&["--no-boot"]).headless(false));
        }
        #[cfg(not(feature = "dev-harness"))]
        assert_eq!(parse(&["--shot", "main"]), Command::Unknown("--shot".into()));
    }
}

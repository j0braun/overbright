//! How the game names what stopped it, for the launcher's failure screen
//! (docs/launcher.md): its `error.kind=<word>` line before the error text,
//! and the window line the launcher recognises.

/// The game's line when its window cannot open (`shell::window`).
pub const NO_WINDOW: &str = "could not open a window";
/// The start of the game's machine line naming what went wrong
/// ([`ErrorKind::line`]), printed on its own line before the error text.
pub const KIND_PREFIX: &str = "error.kind=";

/// What kind of problem stopped the game, as the game names it on its
/// `error.kind=` line (or as the launcher infers it from the end): the
/// failure screen shows a title, one sentence and one fixing button for
/// each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ErrorKind {
    /// The game folder is missing or incomplete (`files-missing`).
    GameFiles,
    /// No graphics adapter or driver the game can draw with (`gpu`).
    NoGpu,
    /// The game window could not open (`no-window`).
    NoWindow,
    /// The game crashed (a panic; `crash`).
    Crash,
    /// The game program itself could not be started (the launcher's own
    /// kind: `game` missing beside the launcher, or not runnable).
    NoProgram,
    /// Anything else.
    #[default]
    Other,
}

impl ErrorKind {
    pub const ALL: [ErrorKind; 6] = [
        ErrorKind::GameFiles,
        ErrorKind::NoGpu,
        ErrorKind::NoWindow,
        ErrorKind::Crash,
        ErrorKind::NoProgram,
        ErrorKind::Other,
    ];

    /// The word on the `error.kind=` line.
    pub fn word(self) -> &'static str {
        match self {
            ErrorKind::GameFiles => "files-missing",
            ErrorKind::NoGpu => "gpu",
            ErrorKind::NoWindow => "no-window",
            ErrorKind::Crash => "crash",
            ErrorKind::NoProgram => "no-program",
            ErrorKind::Other => "other",
        }
    }

    /// The kind a line names (`error.kind=gpu`), if it is such a line.
    pub fn parse(line: &str) -> Option<ErrorKind> {
        let w = line.trim().strip_prefix(KIND_PREFIX)?;
        ErrorKind::ALL.into_iter().find(|k| k.word() == w.trim())
    }

    /// The line the game prints before its error text.
    pub fn line(self) -> String {
        format!("{KIND_PREFIX}{}", self.word())
    }

    /// The kind of an error that stopped the game's window from opening
    /// (its whole chain as text): no adapter or driver, or the window.
    pub fn of_window_error(text: &str) -> ErrorKind {
        let t = text.to_ascii_lowercase();
        if ["adapter", "vulkan", "dx12", "gpu", "device"].iter().any(|w| t.contains(w)) {
            ErrorKind::NoGpu
        } else {
            ErrorKind::NoWindow
        }
    }
}

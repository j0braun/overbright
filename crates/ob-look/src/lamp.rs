//! The lamps: the round status marks in front of a line (ok, warning, note,
//! error, busy), drawn in the look's `ok`, `warn`, `info` and `err` tokens.

/// A status line's mark (`qol::setup::Mark` and the like).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Ok,
    Warn,
    Info,
    Error,
    /// Something is being checked (a spinner).
    Busy,
}

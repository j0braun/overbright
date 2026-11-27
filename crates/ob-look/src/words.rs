//! The words on buttons, menu items and legends that the launcher, the map
//! editor and Studio share, so one action reads the same everywhere
//! (docs/STYLE.md: plain words, one word per action).
//!
//! A screen names its own words where an action is its own ("Rename…",
//! "Test race"); a word the programs share comes from here.

/// Leave this screen or dialog (the pad's B, Escape).
pub const BACK: &str = "Back";
/// Drop what the dialog would do.
pub const CANCEL: &str = "Cancel";
pub const SHARE: &str = "Share";
/// The question before sharing while a test run leaves props out (Studio, Test without selected props).
pub fn left_out_line(n: usize) -> String {
    format!("{n} prop{} left out of the running test. They stay in what you share.", if n == 1 { " is" } else { "s are" })
}
/// Close a message.
pub const OK: &str = "OK";
/// Take the item or place the cursor is on (the pad's A, Enter).
pub const SELECT: &str = "Select";
/// Open a file, a folder or a picked item.
pub const OPEN: &str = "Open";
/// Write the changes.
pub const SAVE: &str = "Save";
/// Close the program.
pub const QUIT: &str = "Quit";
/// Take back the last step.
pub const UNDO: &str = "Undo";
/// Make the step Undo took back again.
pub const REDO: &str = "Redo";
/// Remove the item (asks first where it cannot be undone).
pub const DELETE: &str = "Delete";
/// Make a copy of the item.
pub const DUPLICATE: &str = "Duplicate";
/// Give the item a new name.
pub const RENAME: &str = "Rename";
/// Make the new thing a dialog describes (a mod, a race).
pub const CREATE: &str = "Create";
/// Bring back what was kept (a backup, unsaved changes).
pub const RESTORE: &str = "Restore";

/// Every shared word, in the order above.
pub const ALL: &[&str] =
    &[BACK, CANCEL, OK, SELECT, OPEN, SAVE, QUIT, UNDO, REDO, DELETE, DUPLICATE, RENAME, CREATE, RESTORE];

/// The question before a mod with textures from the original game's pixels is exported (Studio's Mod page,
/// the launcher's Map mods export): its title and its yes.
pub const DERIVED_TITLE: &str = "Made from the original game";
pub const EXPORT_ANYWAY: &str = "Export anyway";

#[cfg(test)]
mod tests {
    use super::ALL;

    #[test]
    fn each_word_is_one_short_capitalised_word() {
        for (i, w) in ALL.iter().enumerate() {
            assert!(!w.is_empty() && w.len() <= 12 && !w.contains(' '), "{w}");
            assert!(w.chars().next().unwrap().is_ascii_uppercase(), "{w}");
            assert!(!ALL[..i].contains(w), "{w} twice");
        }
    }
}

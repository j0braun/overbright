//! The shell's words, in one table (docs/STYLE.md: plain words, one word per action). The words the programs share
//! (Back, Select, Undo …) come from `ob_look::words`.

pub use ob_look::words::QUIT;

pub const STUDIO: &str = "Overbright Studio";
pub const TOO_SMALL: &str = "Studio needs at least 960 × 600";
pub const FIND_HINT: &str = "Find anything…";
pub const NO_MOD_TITLE: &str = "No mod open";
pub const NO_MOD_LINE: &str = "Open one of your mods, or start a new one.";
pub const OPEN_A_MOD: &str = "Open a mod";
pub const READY_TITLE: &str = "Your new mod is ready";
pub const READY_LINE: &str =
    "Nothing changed yet. Move or add things in World, or make a race. Studio saves your work automatically.";
/// The Changes page's focus menu.
pub const RESET_TO_ORIGINAL: &str = "Reset to original";
pub const SHOW_IN_WORLD: &str = "Show in World";
pub const CHANGED_LINE: &str = "Studio saves your work automatically. Changes lists what differs from the original.";
pub const READ_ONLY_TITLE: &str = "Open to look at";
pub const GO_TO_WORLD: &str = "Go to World";
pub const MAKE_A_RACE: &str = "Make a race";
pub const OFF_IN_GAME: &str = "Off · Turn on in the launcher";
pub const SAVED: &str = "Saved automatically";
pub const SAVING_BEHIND: &str = "Saving is behind";
pub const SAVING: &str = "Saving…";
pub const NO_MOD_STATUS: &str = "No mod open";
pub const MOD_PAGE: &str = "Sharing your mod comes here: check it, pack it and make it ready for others.";
pub const SETTINGS_PAGE: &str = "Studio's settings come here: UI size, keys and pads.";
pub const TOOLS_COME: &str = "Open a mod to edit its world";
pub const VIEW_PLACEHOLDER: &str = "Open a mod on Home or in the Mod menu";
pub const INSPECTOR_EMPTY: &str = "Pick something in the view or the tree.";
pub const LEGEND_NOTE_KEYS: &str = "Keyboard: the legend follows the device you used last";
pub const LEGEND_NOTE_PAD: &str = "Pad: the legend follows the device you used last";

pub const NO_PROBLEMS: &str = "No problems found";
pub const NOTHING_LOGGED: &str = "Nothing logged yet";
pub const NO_JOBS: &str = "No jobs running";
pub const NO_STEPS: &str = "No steps yet";
pub const NO_MODS_YET: &str = "No mods yet";
pub const ONLY_THIS_WINDOW: &str = "Changes · only this window writes it";
pub const IN_GAME_ON: &str = "In your game: on";
pub const IN_GAME_OFF: &str = "In your game: off · ";
pub const TURN_ON_IN_LAUNCHER: &str = "Turn on in the launcher";
pub const RESET_LAYOUT: &str = "Reset layout";
pub const COPY_ADDRESS: &str = "Copy address";
pub const TEST_HERE: &str = "Test here";
pub const NOTHING_SELECTED: &str = "Nothing selected";

/// Every word above.
pub const ALL: [&str; 38] = [
    NO_PROBLEMS,
    NOTHING_LOGGED,
    NO_JOBS,
    NO_STEPS,
    NO_MODS_YET,
    ONLY_THIS_WINDOW,
    IN_GAME_ON,
    IN_GAME_OFF,
    TURN_ON_IN_LAUNCHER,
    RESET_LAYOUT,
    COPY_ADDRESS,
    TEST_HERE,
    NOTHING_SELECTED,
    QUIT,
    STUDIO,
    TOO_SMALL,
    FIND_HINT,
    NO_MOD_TITLE,
    NO_MOD_LINE,
    OPEN_A_MOD,
    READY_TITLE,
    READY_LINE,
    CHANGED_LINE,
    READ_ONLY_TITLE,
    GO_TO_WORLD,
    MAKE_A_RACE,
    OFF_IN_GAME,
    SAVED,
    SAVING_BEHIND,
    SAVING,
    NO_MOD_STATUS,
    MOD_PAGE,
    SETTINGS_PAGE,
    TOOLS_COME,
    VIEW_PLACEHOLDER,
    INSPECTOR_EMPTY,
    LEGEND_NOTE_KEYS,
    LEGEND_NOTE_PAD,
];

/// The characters beyond ASCII Studio's words and glyphs use (the faces must have them).
pub const USED_CHARS: &str = "…·×›←→";

#[cfg(test)]
mod tests {
    use super::*;

    /// Words a player should not have to know.
    const JARGON: [&str; 10] =
        ["commit", "journal", "repository", "serialize", "widget", "viewport", "payload", "uri", "url", "toml"];

    #[test]
    fn words_are_plain() {
        for w in ALL.iter().chain(super::super::nav::KEYS.iter().flat_map(|k| [&k.word, &k.help])) {
            let low = w.to_ascii_lowercase();
            for j in JARGON {
                assert!(!low.split(|c: char| !c.is_ascii_alphanumeric()).any(|p| p == j), "{w:?} says {j:?}");
            }
            assert!(w.chars().next().is_some_and(|c| c.is_uppercase() || !c.is_alphabetic()), "{w:?}");
            for c in w.chars().filter(|c| !c.is_ascii()) {
                assert!(USED_CHARS.contains(c), "{w:?}: {c:?} is not in USED_CHARS");
            }
        }
    }

    #[test]
    fn the_faces_have_every_character_used() {
        use ab_glyph::{Font, FontRef};
        for data in [
            ob_look::fonts::BODY,
            ob_look::fonts::BODY_SEMI,
            ob_look::fonts::LABEL_ITALIC,
            ob_look::fonts::TITLE_ITALIC,
        ] {
            let font = FontRef::try_from_slice(data).unwrap();
            for c in USED_CHARS.chars() {
                assert_ne!(font.glyph_id(c).0, 0, "a face lacks {c:?}");
            }
        }
    }
}

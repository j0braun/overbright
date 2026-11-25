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
pub const SETTINGS_PAGE: &str = "Studio's settings come here: UI size, keys and pads.";
pub const TOOLS_COME: &str = "Open a mod to edit its world";
pub const VIEW_PLACEHOLDER: &str = "Open a mod on Home or in the Mod menu";
pub const INSPECTOR_EMPTY: &str = "Pick something in the view or the tree.";
pub const LEGEND_NOTE_KEYS: &str = "Keyboard: the legend follows the device you used last";
pub const LEGEND_NOTE_PAD: &str = "Pad: the legend follows the device you used last";

pub const NO_PROBLEMS: &str = "No problems found";
pub const NOTHING_LOGGED: &str = "Nothing logged yet";
pub const NO_JOBS: &str = "No jobs running";
pub const PROBLEMS: &str = "Problems";
pub const SIMULATING: &str = "Simulating…";
pub const FIND_GO_TO: &str = "GO TO";
pub const FIND_COMMANDS: &str = "COMMANDS";
pub const FIND_MODS: &str = "YOUR MODS";
pub const PLACES_YOU_WERE: &str = "PLACES YOU WERE";
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

/// The Mod page.
pub const MOD_PAGE_NO_MOD: &str = "Open a mod to see its details, check it and share it.";
pub const MOD_DETAILS: &str = "Mod details";
pub const IN_GAME_ON_SHORT: &str = "On";
pub const READY_TO_SHARE: &str = "READY TO SHARE";
pub const EXPORT: &str = "Export";
pub const EXPORT_ANYWAY: &str = ob_look::words::EXPORT_ANYWAY;
pub const DERIVED_TITLE: &str = ob_look::words::DERIVED_TITLE;
pub const SHOW: &str = "Show";
pub const HIDE: &str = "Hide";
pub const NOT_SET: &str = "Not set";

/// Making a mod: Home's "Start something new", File ▸ New mod… and its dialog.
pub const NEW_MOD: &str = "New mod…";
pub const NEW_MOD_TITLE: &str = "New mod";
pub const NEW_MOD_LINE: &str = "Move, add and hide props and make races. The game's own files stay as they are.";
pub const NAME: &str = "Name";
pub const FOLDER: &str = "Folder";
pub const NAME_HINT: &str = "Docks remix";
pub const NEW_MOD_GOES: &str = "Goes in the folder";
pub const NEW_MOD_NAME_FIRST: &str = "Type a name; the folder is made from it";
pub const START_NEW: &str = "START SOMETHING NEW";
pub const EMPTY_MAP_MOD: &str = "Empty map mod";
pub const EMPTY_MAP_MOD_LINE: &str = "Props and races, nothing else";

/// Home's start and the tutorials (`guide`).
pub const START_HERE: &str = "START HERE";
pub const PLACES: &str = "WHAT EACH PLACE DOES";
pub const TUTORIAL: &str = "Tutorial";
pub const TUTORIAL_ROW: &str = "Your first mod";
pub const TUTORIAL_ROW_LINE: &str = "Tutorial · make a mod, place a prop, drive it, share it";
pub const END_TUTORIAL: &str = "End tutorial";
pub const GO_TO_RACES: &str = "Go to Races";
pub const RACE_TUTORIAL_ROW: &str = "Make your first race";
pub const RACE_TUTORIAL_ROW_LINE: &str = "Tutorial · base track, start, race points, test, share";
pub const OPEN_SAMPLE: &str = "Open the sample";

/// Home's templates and the sample (`ob_studio::create::Template`).
pub const SPRINT_RACE: &str = "Sprint race";
pub const SPRINT_RACE_LINE: &str = "One sprint along the docks, ready to change";
pub const CIRCUIT_RACE: &str = "Circuit race";
pub const CIRCUIT_RACE_LINE: &str = "Three laps downtown, ready to change";
pub const SAMPLE: &str = "SAMPLE";
pub const SAMPLE_RACE_PACK: &str = "Sample race pack";
pub const SAMPLE_LINE: &str = "Three races with start gates and signs";
pub const STARTS_WITH: &str = "Starts with";

/// Every word above.
pub const ALL: [&str; 82] = [
    RACE_TUTORIAL_ROW,
    RACE_TUTORIAL_ROW_LINE,
    OPEN_SAMPLE,
    SPRINT_RACE,
    SPRINT_RACE_LINE,
    CIRCUIT_RACE,
    CIRCUIT_RACE_LINE,
    SAMPLE,
    SAMPLE_RACE_PACK,
    SAMPLE_LINE,
    STARTS_WITH,
    START_HERE,
    PLACES,
    TUTORIAL,
    TUTORIAL_ROW,
    TUTORIAL_ROW_LINE,
    END_TUTORIAL,
    GO_TO_RACES,
    NEW_MOD,
    NEW_MOD_TITLE,
    NEW_MOD_LINE,
    NAME,
    FOLDER,
    NAME_HINT,
    NEW_MOD_GOES,
    NEW_MOD_NAME_FIRST,
    START_NEW,
    EMPTY_MAP_MOD,
    EMPTY_MAP_MOD_LINE,
    MOD_PAGE_NO_MOD,
    MOD_DETAILS,
    IN_GAME_ON_SHORT,
    READY_TO_SHARE,
    EXPORT,
    EXPORT_ANYWAY,
    DERIVED_TITLE,
    SHOW,
    HIDE,
    NOT_SET,
    NO_PROBLEMS,
    NOTHING_LOGGED,
    NO_JOBS,
    PROBLEMS,
    SIMULATING,
    FIND_GO_TO,
    FIND_COMMANDS,
    FIND_MODS,
    PLACES_YOU_WERE,
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

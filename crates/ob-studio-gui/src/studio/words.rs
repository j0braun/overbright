//! The shell's words, in one table (docs/STYLE.md: plain words, one word per action). The words the programs share
//! (Back, Select, Undo …) come from `ob_look::words`.

pub use ob_look::words::QUIT;

/// Edit hygiene (the map editor): the snap mode shown in the hint line, and the status after Esc puts a drag back.
pub const DRAG_CANCELLED: &str = "Drag cancelled: everything is back where it was";
pub const SNAP_ROAD: &str = "Snap: to the road · Alt: free";
pub const SNAP_OFF: &str = "Snap: off · Alt: free";
/// "Snap: steps of 0.25 m and 15° · Alt: free".
pub fn snap_steps(metres: &str, degrees: &str) -> String {
    format!("Snap: steps of {metres} m and {degrees}° · Alt: free")
}

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
pub const SETTINGS_PAGE: &str = "Studio's settings: the world view first; UI size, keys and pads come next.";
pub const WORLD_VIEW: &str = "WORLD VIEW";
pub const WORLD_VIEW_LINE: &str = "Auto draws the lighter picture on integrated and software graphics. Weak GPU draws smaller, shorter and without shadows or reflections. Only Studio's view changes, never the game.";
pub const TOOLS_COME: &str = "Open a mod to edit its world";
pub const VIEW_PLACEHOLDER: &str = "Open a mod on Home or in the Mod menu";
pub const INSPECTOR_EMPTY: &str = "Pick something in the view or the tree.";
pub const LEGEND_NOTE_KEYS: &str = "Keyboard: the legend follows the device you used last";
pub const LEGEND_NOTE_PAD: &str = "Pad: the legend follows the device you used last";

/// The Runs workspace.
pub const RUNS: &str = "Runs";
pub const NO_RUNS: &str = "No runs yet";
pub const NO_RUNS_HOW: &str = "Simulate a race with the Simulate button and its run is kept here.";

pub const NO_PROBLEMS: &str = "No problems found";
pub const NOTHING_LOGGED: &str = "Nothing logged yet";
pub const NO_JOBS: &str = "No jobs running";
pub const PROBLEMS: &str = "Problems";
pub const SIMULATING: &str = "Simulating…";
pub const FIND_GO_TO: &str = "GO TO";
pub const FIND_COMMANDS: &str = "COMMANDS";
pub const FIND_MODS: &str = "YOUR MODS";
pub const PLACES_YOU_WERE: &str = "PLACES YOU WERE";
pub const FIND_PLACES: &str = "PLACES";
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

/// Moved here from the pages: every plain word Studio draws is in this table (the test `every_word_comes_from_the_table`).
pub const BROWSER: &str = "Browser";
pub const NO_MOD_OPEN_OPEN_ONE_ON: &str = "No mod open: open one on Home to browse the game's models and textures of its region";
pub const SEARCH_MODELS_AND_TEXTURES_BY_NAME: &str = "Search models and textures by name, 0x hash or section:1407";
pub const CHOOSE_MODELS_OR_TEXTURES_TO_FILTER: &str = "Choose Models or Textures to filter them";
pub const NOTHING_IN_THIS_REGION: &str = "Nothing in this region";
pub const DID_YOU_MEAN: &str = "Did you mean";
pub const NO_MODEL_OR_TEXTURE_MATCHES: &str = "No model or texture matches";
pub const CHOOSE_A_RESULT_TO_SEE_IT: &str = "Choose a result to see it and where it is used";
pub const OPEN_IN_LOOKS: &str = "Open in Looks";
pub const DRAWING: &str = "Drawing…";
pub const NO_PICTURE: &str = "No picture";
pub const USED_BY_MODELS: &str = "Used by models";
pub const TEXTURES: &str = "Textures";
pub const EXPORT_GLTF: &str = "Export glTF";
pub const BRING_BACK: &str = "Bring back";
pub const THIS_MOD: &str = "This mod";
pub const HISTORY_STEP: &str = "History step";
pub const COMPARE_VERSIONS: &str = "Compare versions";
pub const CLASHES: &str = "Clashes";
pub const POSITION: &str = "Position";
pub const HEADING: &str = "Heading";
pub const HIDDEN: &str = "Hidden";
pub const PROP: &str = "Prop";
pub const RACE_TYPE: &str = "Race type";
pub const LAPS: &str = "Laps";
pub const BASE_TRACK: &str = "Base track";
pub const BACKWARD: &str = "Backward";
pub const CHECKPOINTS: &str = "Checkpoints";
pub const START_GRID: &str = "Start grid";
pub const RACE: &str = "Race";
pub const IMAGE: &str = "Image";
pub const RACE_ADDED_BY_YOU: &str = "Race · added by you";
pub const NOTHING_TO_RESET_HERE_PICK_A: &str = "Nothing to reset here: pick a value of yours";
pub const UNDONE: &str = "Undone";
pub const REDONE: &str = "Redone";
pub const NOTHING_TO_TAKE_BACK: &str = "Nothing to take back";
pub const SHOW_IN_RACES: &str = "Show in Races";
pub const SHOW_IN_HISTORY: &str = "Show in History";
pub const WORLD: &str = "← World";
pub const ESC: &str = "Esc";
pub const CHANGES: &str = "Changes";
pub const NO_MOD_OPEN_OPEN_ONE_ON_2: &str = "No mod open: open one on Home to see what it changes";
pub const EVERYTHING_THIS_MOD_CHANGES: &str = "Everything this mod changes";
pub const ONLY_WHAT_LOSES: &str = "Only what loses";
pub const ALL_VALUES: &str = "All values";
pub const ONLY_EDITED: &str = "Only edited";
pub const RESET: &str = "Reset";
pub const REMOVE: &str = "Remove";
pub const SHOW_IN_CLASHES: &str = "Show in Clashes";
pub const NOTHING_OF_YOURS_LOSES_EVERY_VALUE: &str = "Nothing of yours loses: every value you give is used in your game";
pub const NOTHING_CHANGED_YET_THE_ORIGINAL_EVERYWHERE: &str = "Nothing changed yet: the original everywhere";
pub const NO_VALUES_DIFFER: &str = "No values differ";
pub const THE_STACK_AS_COLUMNS_IN_YOUR: &str = "· the stack as columns, in your game's order";
pub const VALUE: &str = "Value";
pub const IN_YOUR_GAME: &str = "In your game";
pub const INSPECTOR: &str = "Inspector";
pub const ORIGINAL: &str = "Original";
pub const YOURS: &str = "Yours";
pub const CHANGED_IN: &str = "Changed in";
pub const TOUCHES: &str = "Touches";
pub const RETURN_TO_ORIGINAL: &str = "Return to original";
pub const PICK_SEVERAL_ROWS_SHIFT_OR_A: &str = "Pick several rows (Shift+↓, or A on each with the pad) to reset them in one step.";
pub const RACES: &str = "Races";
pub const PROPS: &str = "Props";
pub const DATA: &str = "Data";
pub const SEARCH: &str = "Search";
pub const NO_MOD_OPEN_OPEN_ONE_ON_3: &str = "No mod open: open one on Home to see its data";
pub const THIS_MOD_GIVES_NO_VALUES_OF: &str = "This mod gives no values of this kind yet";
pub const NO_ROW_MATCHES_THE_SEARCH: &str = "No row matches the search";
pub const FILL: &str = "Fill";
pub const CELLS: &str = "Cells";
pub const PASTE: &str = "Paste";
pub const NOT_CHANGED: &str = "Not changed";
pub const NOT_PASTED: &str = "Not pasted";
pub const NOT_IMPORTED: &str = "Not imported";
pub const COPIED: &str = "Copied";
pub const NOTHING_TO_CHANGE: &str = "Nothing to change";
pub const TABLE_WRITTEN_TO: &str = "Table written to";
pub const EXPORT_TABLE: &str = "Export table";
pub const IMPORT_TABLE: &str = "Import table";
pub const TAKES_IT_BACK: &str = "Ctrl+Z takes it back";
pub const NO_TABLE_FILE_YET: &str = "Export the table first: the file it brings back is not there yet";
pub const SELECT_MODE_ON: &str = "Selecting: arrows extend the selection, Back ends it";
pub const SELECT_MODE_OFF: &str = "Selecting ended";
pub const IN_CHANGES: &str = "In Changes";
pub const MAKE_A_MOD: &str = "Make a mod";
pub const GO_TO_A_STREET: &str = "Go to a street";
pub const PLACE_A_PROP: &str = "Place a prop";
pub const DRIVE_IT: &str = "Drive it";
pub const CHOOSE_A_BASE_TRACK: &str = "Choose a base track";
pub const PLACE_THE_START: &str = "Place the start";
pub const ADD_RACE_POINTS: &str = "Add race points";
pub const TEST_THE_RACE: &str = "Test the race";
pub const SHARE_IT: &str = "Share it";
pub const YOUR_FIRST_RACE_IS_DONE: &str = "Your first race is done";
pub const YOUR_FIRST_MOD_IS_DONE: &str = "Your first mod is done";
pub const TAKE_ME_THERE: &str = "Take me there";
pub const PLACE_A_CONE: &str = "Place a cone";
pub const DRIVE_HERE: &str = "Drive here";
pub const GO_TO_THE_DOCKS: &str = "Go to the docks";
pub const ADD_THEM_AHEAD: &str = "Add them ahead";
pub const TEST_RACE: &str = "Test race";
pub const GO_TO_MOD: &str = "Go to Mod";
pub const CLOSE: &str = "Close";
pub const MY_FIRST_MOD: &str = "My first mod";
pub const MY_FIRST_RACE: &str = "My first race";
pub const HERE_STREET_LEVEL_A_PLACE_TO: &str = "Here: street level, a place to start";
pub const MAKE_A_RACE_FIRST_GO_TO: &str = "Make a race first: Go to the docks, or Test › New race";
pub const START_LINE_PLACED_THE_CARS_LINE: &str = "Start line placed: the cars line up behind it, facing down the road";
pub const PLACE_THE_START_LINE_FIRST: &str = "Place the start line first";
pub const NO_GROUND_IN_VIEW_PRESS_TAKE: &str = "No ground in view: press Take me there, or fly down to a street";
pub const NO_CONE_IN_THIS_CITY_DRAG: &str = "No cone in this city: drag a prop from the Prop browser";
pub const NONE_YET_THE_ORIGINAL_EVERYWHERE: &str = "None yet · the original everywhere";
pub const AUTHOR: &str = "Author";
pub const DESCRIPTION: &str = "Description";
pub const FILES: &str = "Files";
pub const EXPORT_TO: &str = "Export to";
pub const YOURS_WINS: &str = "Yours wins";
pub const THE_ORIGINAL_STAYS: &str = "The original stays";
pub const NOT_EXPORTED: &str = "Not exported";
pub const NOT_READY_TO_SHARE_YET_SEE: &str = "Not ready to share yet: see the list";
pub const SAY_WHERE_TO_EXPORT_TO: &str = "Say where to export to";
pub const MOD: &str = "Mod";
pub const NONE_WITH_THE_MODS_IN_YOUR: &str = "None with the mods in your game";
pub const RAIL: &str = "Rail";
pub const ADDRESS: &str = "Address";
pub const TOOLBAR: &str = "Toolbar";
pub const LEFT_PANEL: &str = "Left panel";
pub const VIEW: &str = "View";
pub const RIGHT_DOCK: &str = "Right dock";
pub const BOTTOM_DOCK: &str = "Bottom dock";
pub const ENTER: &str = "Enter";
pub const SELECT: &str = "Select";
pub const TAKE_THE_FOCUSED_ITEM: &str = "Take the focused item";
pub const BACK: &str = "Back";
pub const ONE_LEVEL_OUT_MENU_FIELD_ROW: &str = "One level out: menu, field, row, area, then the page's back link";
pub const SHIFT_F10_MENU_KEY: &str = "Shift+F10 / Menu key";
pub const MENU: &str = "Menu";
pub const THE_FOCUSED_THING_S_MENU_UNDO: &str = "The focused thing's menu: undo, redo, test here, reset (on Changes), copy address";
pub const CTRL_K: &str = "Ctrl+K";
pub const FIND: &str = "Find";
pub const FIND_NAMES_COMMANDS_PLACES_AND_PASTED: &str = "Find names, commands, places and pasted addresses";
pub const CTRL_PGUP_CTRL_PGDN: &str = "Ctrl+PgUp / Ctrl+PgDn";
pub const WORKSPACE: &str = "Workspace";
pub const THE_WORKSPACE_ABOVE_OR_BELOW_ON: &str = "The workspace above or below on the rail";
pub const CTRL_1_CTRL_0_CTRL: &str = "Ctrl+1 … Ctrl+0, Ctrl+,";
pub const A_WORKSPACE_BY_ITS_NUMBER_ON: &str = "A workspace by its number on the rail; Ctrl+, opens Settings";
pub const F6_SHIFT_F6: &str = "F6 / Shift+F6";
pub const AREA: &str = "Area";
pub const THE_NEXT_OR_PREVIOUS_AREA_RAIL: &str = "The next or previous area: rail, address, toolbar, left panel, view, right dock, bottom dock";
pub const TAB_ARROWS: &str = "Tab / arrows";
pub const MOVE: &str = "Move";
pub const BETWEEN_THE_CONTROLS_OF_AN_AREA: &str = "Between the controls of an area, in reading order";
pub const TEXT: &str = "← →";
pub const CHANGE: &str = "Change";
pub const CHANGE_A_VALUE_BY_ITS_NUDGE: &str = "Change a value by its nudge step (Shift: ten steps)";
pub const TEST: &str = "Test";
pub const TEST_WHAT_THE_TEST_BUTTON_NAMES: &str = "Test what the Test button names";
pub const ALT: &str = "Alt+←";
pub const LAST_PLACE: &str = "Last place";
pub const BACK_TO_THE_LAST_PLACE_FORWARD: &str = "Back to the last place (forward: Alt+→)";
pub const FORWARD_AGAIN: &str = "Forward again";
pub const NEXT_PLACE: &str = "Next place";
pub const ALT_2: &str = "Alt+→";
pub const CTRL_L: &str = "Ctrl+L";
pub const SHOW_THE_ADDRESS_TO_EDIT_OR: &str = "Show the address to edit or paste; Enter goes there";
pub const CTRL_SHIFT_C: &str = "Ctrl+Shift+C";
pub const COPY_THE_FOCUSED_ITEM_S_ADDRESS: &str = "Copy the focused item's address";
pub const CTRL_D: &str = "Ctrl+D";
pub const WHAT_THE_OPEN_MOD_CHANGES_VALUE: &str = "What the open mod changes, value by value, and which mod wins";
pub const CTRL_G: &str = "Ctrl+G";
pub const YOUR_PROPS_AND_RACES_AS_A: &str = "Your props and races as a table: sort, search, edit a cell";
pub const CTRL_N: &str = "Ctrl+N";
pub const MAKE_A_NEW_MAP_MOD_HOME: &str = "Make a new map mod (Home: Empty map mod)";
pub const DO_THE_TUTORIAL_S_STEP_HOME: &str = "Do the tutorial's step (Home's card holds it for the pad); starts the tutorial";
pub const CTRL_Z: &str = "Ctrl+Z";
pub const UNDO: &str = "Undo";
pub const TAKE_BACK_THE_LAST_STEP_DECK: &str = "Take back the last step (Deck: L4)";
pub const CTRL_Y: &str = "Ctrl+Y";
pub const REDO: &str = "Redo";
pub const MAKE_IT_AGAIN_DECK_R4: &str = "Make it again (Deck: R4)";
pub const RMB_WASD: &str = "RMB + WASD";
pub const FLY: &str = "Fly";
pub const MOVE_AND_LOOK_IN_THE_3D: &str = "Move and look in the 3D view while it has focus";
pub const THAT_RUN_IS_THE_ONE_SHOWN: &str = "That run is the one shown: pick another to compare it with";
pub const DRIVE_TO_RECORD: &str = "Drive to record";
pub const START_THE_GAME_ON_THE_CHOSEN: &str = "Start the game on the chosen race (or from the camera's spot), drive it yourself, and keep the drive here as a run";
pub const ENTER_OPEN_IT_C_COMPARE_WITH: &str = "Enter: open it · C: compare with the run shown · F2: rename · Delete: delete";
pub const RENAME: &str = "Rename";
pub const CANCEL: &str = "Cancel";
pub const DELETE: &str = "Delete";
pub const KEEP: &str = "Keep";
pub const SHOW_IT_IN_THE_VIEW_ENTER: &str = "Show it in the view (Enter)";
pub const OPEN: &str = "Open";
pub const COMPARE: &str = "Compare";
pub const STOP_COMPARING: &str = "Stop comparing";
pub const DRAW_IT_AS_GHOSTS_OVER_THE: &str = "Draw it as ghosts over the run shown (C)";
pub const HELP: &str = "Help";
pub const GO: &str = "Go";
pub const EDIT: &str = "Edit";
pub const FILE: &str = "File";
pub const DATA_SHOWS_A_MOD_OPEN_ONE: &str = "Data shows a mod: open one first";
pub const CHANGES_SHOWS_A_MOD_OPEN_ONE: &str = "Changes shows a mod: open one first";
pub const CHANGES_HAS_NO_ENTRY_FOR_THAT: &str = "Changes has no entry for that row";
pub const CHOOSE_A_MODEL_TO_EXPORT_IT: &str = "Choose a model to export it: textures have no glTF";
pub const CHOOSE_A_MODEL_TO_BRING_BACK: &str = "Choose a model to bring back: textures have no glTF";
pub const FIND_2: &str = "Find…";
pub const CONTROLS: &str = "Controls";
pub const MAP_EDITOR: &str = "Map editor";
pub const NONE_YET: &str = "None yet";
pub const TEST_F5: &str = "Test  F5";
pub const JOBS: &str = "Jobs";
pub const LOG: &str = "Log";
pub const HISTORY: &str = "History";
pub const SHIFT_F5: &str = "Shift+F5";
pub const COMPARE_WITH_THE_RUN_SHOWN: &str = "Compare with the run shown";
pub const HOME: &str = "Home";
pub const WORLD_2: &str = "World";
pub const SETTINGS: &str = "Settings";
pub const YOUR_MODS_AND_WHERE_TO_START: &str = "Your mods, and where to start";
pub const MOVE_ADD_AND_HIDE_PROPS_IN: &str = "Move, add and hide props in the city, and drive there";
pub const MAKE_A_RACE_START_CHECKPOINTS_FINISH: &str = "Make a race: start, checkpoints, finish; test and simulate it";
pub const FIND_THE_GAME_S_MODELS_AND: &str = "Find the game's models and textures by name, and where each is used";
pub const PLAY_A_RECORDED_RUN_BACK_READ: &str = "Play a recorded run back, read its telemetry, compare two runs";
pub const EVERYTHING_YOUR_MOD_CHANGES_AND_WHICH: &str = "Everything your mod changes, and which mod wins";
pub const ITS_DETAILS_WHETHER_IT_IS_READY: &str = "Its details, whether it is ready to share, and Export";
pub const STUDIO_S_OWN_SETTINGS: &str = "Studio's own settings";
pub const TELEMETRY: &str = "Telemetry";
pub const THE_MOD_IS_NOT_OPEN_IN: &str = "The mod is not open in the map editor";
pub const A_PROP_S_ADDRESS_IS_REGION: &str = "A prop's address is region/section/index";
pub const NO_SUCH_PROP_IN_THIS_REGION: &str = "No such prop in this region";
pub const NO_SUCH_RACE_IN_THIS_MOD: &str = "No such race in this mod";
pub const ROW_SAVED: &str = "Saved";
pub const ROW_OPEN_FOR: &str = "Open for";
pub const ROW_YOUR_MODS: &str = "Your mods";
pub const AUTOMATICALLY: &str = "Automatically";

/// The row under Open for when another program has the mod.
pub fn looking_only_row(who: &str) -> String {
    format!("Looking only · {who} has it")
}

/// Every word above, for the tests.
pub const SEARCH_SOUNDS_BY_BANK_OR_NAME: &str = "Search sounds by bank or name";
pub const SEARCH_LINES_BY_WORDS: &str = "Search a language's lines by words, 0x hash or label";
pub const ENTER_PLAYS_A_SOUND: &str = "Enter plays a sound, Ctrl+E writes it as a WAV";
pub const GAME_HAS_NO_SOUND_BANKS: &str = "The game has no sound banks here";
pub const NO_SOUND_MATCHES: &str = "No sound matches";
pub const NO_LINES_IN_THIS_LANGUAGE: &str = "No lines in this language file";
pub const NO_LINE_MATCHES: &str = "No line matches";
pub const CHOOSE_A_MODEL_OR_SOUND_TO_EXPORT: &str = "Choose a model or a sound to export it: textures and lines have no file";
pub const EXPORT_AREA: &str = "Export area";
pub const PLAY: &str = "Play";
pub const EXPORT_WAV: &str = "Export WAV";
pub const NOT_DECODABLE: &str = "Not decodable: only the banks' own codec plays";
pub const LANGUAGE_FILES_NEED_GAME: &str = "The language files are read when the game folder is set";
pub const GAME_FOLDER_NOT_SET: &str = "The game folder is not set: the launcher's setup has it";

pub const SOUND: &str = "Sound";
/// View menu: marks the road nearest the selection (else the camera) and goes to it.
pub const SHOW_NEAREST_ROAD: &str = "Show nearest road";

/// The collision layer: what a click names.
pub const COLLISION_SURFACE: &str = "Surface";
pub const COLLISION_BARRIER_GROUP: &str = "Barrier group";
pub const COLLISION_NEXT_TO: &str = "Next to the prop";
pub const COLLISION_NO_PROP: &str = "No prop nearby";
pub const COLLISION_ADDED_PROP: &str = "An added prop";
pub const COLLISION_MOVED_PROP: &str = "A moved prop";
pub const COLLISION_DRAWN_BY_MOD: &str = "Drawn by this mod";
pub const COLLISION_ROAD_FLOOR: &str = "A road floor";
pub const COLLISION_FLOOR_OFF: &str = "A floor that is not ground";


pub const ALL: &[&str] = &[
    QUIT,
    STUDIO,
    TOO_SMALL,
    FIND_HINT,
    NO_MOD_TITLE,
    NO_MOD_LINE,
    OPEN_A_MOD,
    READY_TITLE,
    READY_LINE,
    RESET_TO_ORIGINAL,
    SHOW_IN_WORLD,
    CHANGED_LINE,
    READ_ONLY_TITLE,
    GO_TO_WORLD,
    MAKE_A_RACE,
    OFF_IN_GAME,
    SAVED,
    SAVING_BEHIND,
    SAVING,
    SETTINGS_PAGE,
    WORLD_VIEW,
    WORLD_VIEW_LINE,
    TOOLS_COME,
    VIEW_PLACEHOLDER,
    INSPECTOR_EMPTY,
    LEGEND_NOTE_KEYS,
    LEGEND_NOTE_PAD,
    RUNS,
    NO_RUNS,
    NO_RUNS_HOW,
    NO_PROBLEMS,
    NOTHING_LOGGED,
    NO_JOBS,
    PROBLEMS,
    SIMULATING,
    FIND_GO_TO,
    FIND_COMMANDS,
    FIND_MODS,
    PLACES_YOU_WERE,
    FIND_PLACES,
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
    START_HERE,
    PLACES,
    TUTORIAL,
    TUTORIAL_ROW,
    TUTORIAL_ROW_LINE,
    END_TUTORIAL,
    GO_TO_RACES,
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
    BROWSER,
    NO_MOD_OPEN_OPEN_ONE_ON,
    SEARCH_MODELS_AND_TEXTURES_BY_NAME,
    CHOOSE_MODELS_OR_TEXTURES_TO_FILTER,
    NOTHING_IN_THIS_REGION,
    DID_YOU_MEAN,
    NO_MODEL_OR_TEXTURE_MATCHES,
    CHOOSE_A_RESULT_TO_SEE_IT,
    OPEN_IN_LOOKS,
    DRAWING,
    NO_PICTURE,
    USED_BY_MODELS,
    TEXTURES,
    EXPORT_GLTF,
    BRING_BACK,
    THIS_MOD,
    HISTORY_STEP,
    COMPARE_VERSIONS,
    CLASHES,
    POSITION,
    HEADING,
    HIDDEN,
    PROP,
    RACE_TYPE,
    LAPS,
    BASE_TRACK,
    BACKWARD,
    CHECKPOINTS,
    START_GRID,
    RACE,
    IMAGE,
    RACE_ADDED_BY_YOU,
    NOTHING_TO_RESET_HERE_PICK_A,
    UNDONE,
    REDONE,
    NOTHING_TO_TAKE_BACK,
    SHOW_IN_RACES,
    SHOW_IN_HISTORY,
    WORLD,
    ESC,
    CHANGES,
    NO_MOD_OPEN_OPEN_ONE_ON_2,
    EVERYTHING_THIS_MOD_CHANGES,
    ONLY_WHAT_LOSES,
    ALL_VALUES,
    ONLY_EDITED,
    RESET,
    REMOVE,
    SHOW_IN_CLASHES,
    NOTHING_OF_YOURS_LOSES_EVERY_VALUE,
    NOTHING_CHANGED_YET_THE_ORIGINAL_EVERYWHERE,
    NO_VALUES_DIFFER,
    THE_STACK_AS_COLUMNS_IN_YOUR,
    VALUE,
    IN_YOUR_GAME,
    INSPECTOR,
    ORIGINAL,
    YOURS,
    CHANGED_IN,
    TOUCHES,
    RETURN_TO_ORIGINAL,
    PICK_SEVERAL_ROWS_SHIFT_OR_A,
    RACES,
    PROPS,
    DATA,
    SEARCH,
    NO_MOD_OPEN_OPEN_ONE_ON_3,
    THIS_MOD_GIVES_NO_VALUES_OF,
    NO_ROW_MATCHES_THE_SEARCH,
    FILL,
    CELLS,
    PASTE,
    NOT_CHANGED,
    NOT_PASTED,
    NOT_IMPORTED,
    COPIED,
    NOTHING_TO_CHANGE,
    TABLE_WRITTEN_TO,
    EXPORT_TABLE,
    IMPORT_TABLE,
    TAKES_IT_BACK,
    NO_TABLE_FILE_YET,
    SELECT_MODE_ON,
    SELECT_MODE_OFF,
    IN_CHANGES,
    MAKE_A_MOD,
    GO_TO_A_STREET,
    PLACE_A_PROP,
    DRIVE_IT,
    CHOOSE_A_BASE_TRACK,
    PLACE_THE_START,
    ADD_RACE_POINTS,
    TEST_THE_RACE,
    SHARE_IT,
    YOUR_FIRST_RACE_IS_DONE,
    YOUR_FIRST_MOD_IS_DONE,
    TAKE_ME_THERE,
    PLACE_A_CONE,
    DRIVE_HERE,
    GO_TO_THE_DOCKS,
    ADD_THEM_AHEAD,
    TEST_RACE,
    GO_TO_MOD,
    CLOSE,
    MY_FIRST_MOD,
    MY_FIRST_RACE,
    HERE_STREET_LEVEL_A_PLACE_TO,
    MAKE_A_RACE_FIRST_GO_TO,
    START_LINE_PLACED_THE_CARS_LINE,
    PLACE_THE_START_LINE_FIRST,
    NO_GROUND_IN_VIEW_PRESS_TAKE,
    DRAG_CANCELLED,
    SNAP_ROAD,
    SNAP_OFF,
    NO_CONE_IN_THIS_CITY_DRAG,
    NONE_YET_THE_ORIGINAL_EVERYWHERE,
    AUTHOR,
    DESCRIPTION,
    FILES,
    EXPORT_TO,
    YOURS_WINS,
    THE_ORIGINAL_STAYS,
    NOT_EXPORTED,
    NOT_READY_TO_SHARE_YET_SEE,
    SAY_WHERE_TO_EXPORT_TO,
    MOD,
    NONE_WITH_THE_MODS_IN_YOUR,
    RAIL,
    ADDRESS,
    TOOLBAR,
    LEFT_PANEL,
    VIEW,
    RIGHT_DOCK,
    BOTTOM_DOCK,
    ENTER,
    SELECT,
    TAKE_THE_FOCUSED_ITEM,
    BACK,
    ONE_LEVEL_OUT_MENU_FIELD_ROW,
    SHIFT_F10_MENU_KEY,
    MENU,
    THE_FOCUSED_THING_S_MENU_UNDO,
    CTRL_K,
    FIND,
    FIND_NAMES_COMMANDS_PLACES_AND_PASTED,
    CTRL_PGUP_CTRL_PGDN,
    WORKSPACE,
    THE_WORKSPACE_ABOVE_OR_BELOW_ON,
    CTRL_1_CTRL_0_CTRL,
    A_WORKSPACE_BY_ITS_NUMBER_ON,
    F6_SHIFT_F6,
    AREA,
    THE_NEXT_OR_PREVIOUS_AREA_RAIL,
    TAB_ARROWS,
    MOVE,
    BETWEEN_THE_CONTROLS_OF_AN_AREA,
    TEXT,
    CHANGE,
    CHANGE_A_VALUE_BY_ITS_NUDGE,
    TEST,
    TEST_WHAT_THE_TEST_BUTTON_NAMES,
    ALT,
    LAST_PLACE,
    BACK_TO_THE_LAST_PLACE_FORWARD,
    FORWARD_AGAIN,
    NEXT_PLACE,
    ALT_2,
    CTRL_L,
    SHOW_THE_ADDRESS_TO_EDIT_OR,
    CTRL_SHIFT_C,
    COPY_THE_FOCUSED_ITEM_S_ADDRESS,
    CTRL_D,
    WHAT_THE_OPEN_MOD_CHANGES_VALUE,
    CTRL_G,
    YOUR_PROPS_AND_RACES_AS_A,
    CTRL_N,
    MAKE_A_NEW_MAP_MOD_HOME,
    DO_THE_TUTORIAL_S_STEP_HOME,
    CTRL_Z,
    UNDO,
    TAKE_BACK_THE_LAST_STEP_DECK,
    CTRL_Y,
    REDO,
    MAKE_IT_AGAIN_DECK_R4,
    RMB_WASD,
    FLY,
    MOVE_AND_LOOK_IN_THE_3D,
    THAT_RUN_IS_THE_ONE_SHOWN,
    DRIVE_TO_RECORD,
    START_THE_GAME_ON_THE_CHOSEN,
    ENTER_OPEN_IT_C_COMPARE_WITH,
    RENAME,
    CANCEL,
    DELETE,
    KEEP,
    SHOW_IT_IN_THE_VIEW_ENTER,
    OPEN,
    COMPARE,
    STOP_COMPARING,
    DRAW_IT_AS_GHOSTS_OVER_THE,
    HELP,
    GO,
    EDIT,
    FILE,
    DATA_SHOWS_A_MOD_OPEN_ONE,
    CHANGES_SHOWS_A_MOD_OPEN_ONE,
    CHANGES_HAS_NO_ENTRY_FOR_THAT,
    CHOOSE_A_MODEL_TO_EXPORT_IT,
    CHOOSE_A_MODEL_TO_BRING_BACK,
    FIND_2,
    CONTROLS,
    MAP_EDITOR,
    NONE_YET,
    TEST_F5,
    JOBS,
    LOG,
    HISTORY,
    SHIFT_F5,
    COMPARE_WITH_THE_RUN_SHOWN,
    HOME,
    WORLD_2,
    SETTINGS,
    YOUR_MODS_AND_WHERE_TO_START,
    MOVE_ADD_AND_HIDE_PROPS_IN,
    MAKE_A_RACE_START_CHECKPOINTS_FINISH,
    FIND_THE_GAME_S_MODELS_AND,
    PLAY_A_RECORDED_RUN_BACK_READ,
    EVERYTHING_YOUR_MOD_CHANGES_AND_WHICH,
    ITS_DETAILS_WHETHER_IT_IS_READY,
    STUDIO_S_OWN_SETTINGS,
    TELEMETRY,
    THE_MOD_IS_NOT_OPEN_IN,
    A_PROP_S_ADDRESS_IS_REGION,
    NO_SUCH_PROP_IN_THIS_REGION,
    NO_SUCH_RACE_IN_THIS_MOD,
    ROW_SAVED,
    ROW_OPEN_FOR,
    ROW_YOUR_MODS,
    AUTOMATICALLY,
    SOUND,
    SEARCH_SOUNDS_BY_BANK_OR_NAME,
    SEARCH_LINES_BY_WORDS,
    ENTER_PLAYS_A_SOUND,
    GAME_HAS_NO_SOUND_BANKS,
    NO_SOUND_MATCHES,
    NO_LINES_IN_THIS_LANGUAGE,
    NO_LINE_MATCHES,
    CHOOSE_A_MODEL_OR_SOUND_TO_EXPORT,
    EXPORT_AREA,
    PLAY,
    EXPORT_WAV,
    NOT_DECODABLE,
    LANGUAGE_FILES_NEED_GAME,
    GAME_FOLDER_NOT_SET,
    SHOW_NEAREST_ROAD,
    COLLISION_SURFACE,
    COLLISION_BARRIER_GROUP,
    COLLISION_NEXT_TO,
    COLLISION_NO_PROP,
    COLLISION_ADDED_PROP,
    COLLISION_MOVED_PROP,
    COLLISION_DRAWN_BY_MOD,
    COLLISION_ROAD_FLOOR,
    COLLISION_FLOOR_OFF,
];

/// The characters beyond ASCII Studio's words and glyphs use (the faces must have them).
pub const USED_CHARS: &str = "…·×›←→↓";

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

    /// The plain string literals of a page's code (not its tests): what Studio shows a player must be a word of
    /// this table, so a translation has one place to start from. A literal counts as a word when it starts with a
    /// capital (or ← ·), has a lowercase letter or a space, and has no `{` (a formatted line is built by a
    /// function of this file). Lines with assertions, panics, logs and attributes are not words.
    fn literals(src: &str) -> Vec<(usize, String)> {
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        let mut out = vec![];
        'lines: for (n, line) in body.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("//") {
                continue;
            }
            const NOT_WORDS: [&str; 12] = [
                "assert", "expect(", "panic!", "unreachable", "anyhow", "bail", "\"ob://", "#[", "println!", "eprintln!",
                "log::", "warn!(",
            ];
            if NOT_WORDS.iter().any(|p| line.contains(p)) {
                continue;
            }
            let b: Vec<char> = line.chars().collect();
            let mut i = 0;
            while i < b.len() {
                match b[i] {
                    '/' if b.get(i + 1) == Some(&'/') => break,
                    '\'' => {
                        // A char literal ('"', '\n') or a lifetime.
                        if b.get(i + 2) == Some(&'\'') {
                            i += 3;
                        } else if b.get(i + 1) == Some(&'\\') && b.get(i + 3) == Some(&'\'') {
                            i += 4;
                        } else {
                            i += 1;
                        }
                    }
                    'r' if matches!(b.get(i + 1), Some('"') | Some('#'))
                        && (i == 0 || !(b[i - 1].is_alphanumeric() || b[i - 1] == '_')) =>
                    {
                        continue 'lines;
                    }
                    '"' => {
                        let mut j = i + 1;
                        while j < b.len() && b[j] != '"' {
                            j += if b[j] == '\\' { 2 } else { 1 };
                        }
                        if j >= b.len() {
                            continue 'lines;
                        }
                        let s: String = b[i + 1..j].iter().collect();
                        let first = s.chars().next();
                        if s.chars().count() >= 2
                            && first.is_some_and(|c| c.is_ascii_uppercase() || c == '←' || c == '·')
                            && s.chars().any(|c| c.is_ascii_lowercase() || c == ' ')
                            && !s.contains(['{', '\\'])
                        {
                            out.push((n + 1, s));
                        }
                        i = j + 1;
                    }
                    _ => i += 1,
                }
            }
        }
        out
    }

    #[test]
    fn every_word_comes_from_the_table() {
        let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/studio"));
        let mut bad = vec![];
        let mut seen = 0;
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if !name.ends_with(".rs") || name.ends_with("_tests.rs") || ["words.rs", "matrix.rs", "tests.rs"].contains(&name.as_str()) {
                continue;
            }
            seen += 1;
            for (n, s) in literals(&std::fs::read_to_string(&p).unwrap()) {
                bad.push(format!("{name}:{n}: {s:?}"));
            }
        }
        assert!(seen >= 15, "{seen} files scanned");
        assert!(bad.is_empty(), "words outside the table (src/studio/words.rs):\n  {}", bad.join("\n  "));
    }

    #[test]
    fn the_scan_finds_a_loose_word() {
        let found = literals("fn f() { p.text(a, \"Hello there\"); let c = '\"'; x(\"id\"); }\n#[cfg(test)]\nmod t { \"Test only\"; }");
        assert_eq!(found, vec![(1, "Hello there".to_string())]);
        assert!(literals("let r = \"{n} items\"; // \"Comment\"").is_empty());
    }

    #[test]
    fn the_table_has_no_word_twice() {
        let mut v: Vec<&str> = ALL.to_vec();
        v.sort();
        let n = v.len();
        v.dedup();
        assert_eq!(v.len(), n, "a word is in the table twice");
    }
}

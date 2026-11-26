//! The window and the command line do the same things (docs/modding/studio-cli.md, "Same as the window"): every
//! command the window has (its workspaces, its menu actions, its menu commands, its key table) either has a
//! counterpart on the command line, or is marked window-only with the reason. A new window command does not
//! build until it is placed here, and a counterpart named here has to be a command of the registry.

use ob_studio::registry;

use super::nav::{self, KEYS};
use super::workspace::Workspace;
use crate::editor::collision::{Draw, Show};
use crate::editor::menu::{Action, Command, Tool};

/// What the command line has for a window command.
#[derive(Clone, Copy, Debug)]
enum Cli {
    /// These commands (words as typed: `race point add`; every one is in the registry).
    Has(&'static [&'static str]),
    /// None, and why: what the window command is only for.
    WindowOnly(&'static str),
}

use Cli::{Has, WindowOnly};

const VIEWING: &str =
    "a way of looking at the window's view (camera, layout, what is drawn); a script reads and writes the data itself";
const WINDOW_STATE: &str =
    "state of the open window (focus, areas, dialogs, history of places); a command runs once and has none";

fn workspace(w: Workspace) -> Cli {
    match w {
        Workspace::Home => Has(&["pack create", "game find"]),
        Workspace::World => Has(&["props move", "props add", "collision show", "collision wall", "checks run"]),
        Workspace::Races => Has(&["race create", "race set", "race point add", "race tracks", "checks run"]),
        Workspace::Browser => Has(&["browser search", "browser used", "browser export", "browser import", "props solids", "looks find"]),
        Workspace::Runs => Has(&["runs list", "runs show", "runs telemetry", "runs compare", "runs rename", "runs delete"]),
        Workspace::Mod => Has(&["pack details", "pack export", "modpack export", "modpack convert", "modpack report", "history show"]),
        Workspace::Settings => WindowOnly("Studio's own settings: layout, units and the look of the window"),
        Workspace::Changes => Has(&["data show", "history show"]),
        Workspace::Data => Has(&["data kinds", "data show", "data set"]),
    }
}

fn nav_command(c: nav::Command) -> Cli {
    use nav::Command as C;
    match c {
        C::Workspace(w) => workspace(w),
        C::Undo | C::Redo => WindowOnly(
            "walks the open window's history; each command is one step of the same history (`history show`), and an edit is taken back by the edit that reverses it",
        ),
        C::NewMod => Has(&["pack create"]),
        C::Test => WindowOnly("starts the game on the open mod and shows it; the command line has no game window"),
        C::Select
        | C::Back
        | C::Menu
        | C::Find
        | C::PrevWorkspace
        | C::NextWorkspace
        | C::NextArea
        | C::PrevArea
        | C::Move
        | C::Change
        | C::LastPlace
        | C::NextPlace
        | C::EditAddress
        | C::CopyAddress
        | C::Tutorial => WindowOnly(WINDOW_STATE),
    }
}

fn menu_command(c: Command) -> Cli {
    match c {
        Command::Duplicate => Has(&["props duplicate"]),
        Command::Delete => Has(&["props delete", "race point delete", "race delete"]),
        Command::DropToGround => Has(&["props drop"]),
        Command::NewRace => Has(&["race create"]),
        Command::SelectAllOfSolid => WindowOnly("a selection in the window; a script names the props it means (`props solids` lists where each is placed)"),
        Command::Rotate90 | Command::FaceCamera => Has(&["props move"]),
        Command::ResetCamera | Command::FrameSelection => WindowOnly(VIEWING),
        Command::TestRace | Command::DriveHere | Command::RestartTest | Command::RecordDrive => {
            WindowOnly("starts the game on the open mod and shows it; the command line has no game window")
        }
    }
}

fn action(a: &Action) -> Cli {
    match a {
        Action::NewMod => Has(&["pack create"]),
        Action::OpenMod | Action::OpenRecent(_) => {
            WindowOnly("picks which mod the window has open; a command names its mod with --mod")
        }
        Action::Save => WindowOnly("a command's step is written at once; there is nothing unsaved to save"),
        Action::Details => Has(&["pack details"]),
        Action::Import => Has(&["modpack convert", "pack put", "looks import"]),
        Action::Export => Has(&["pack export"]),
        Action::ExportPack => Has(&["modpack export"]),
        Action::Quit | Action::Controls | Action::HowToTest | Action::Hint(_) | Action::Tab(_) => {
            WindowOnly("closes or explains the window; `help` explains the command line")
        }
        Action::Undo | Action::Redo => WindowOnly("walks the open window's history (see Undo in the key table)"),
        Action::Tool(t) => match t {
            Tool::Select => WindowOnly("the pointer's job in the window; a command names what it acts on"),
            Tool::Move | Tool::Rotate => Has(&["props move", "race point move"]),
            Tool::Checkpoint | Tool::Start | Tool::Finish | Tool::Grid => Has(&["race point add"]),
        },
        Action::Roads => WindowOnly("a drawing over the window's top view; `checks run` reports the roads' own findings"),
        // The collision layer's three modes: `collision show --mode` counts what each draws.
        Action::Collision(s) => match s {
            Show::Off | Show::Invisible | Show::All => Has(&["collision show"]),
        },
        // What colours the layer: a view setting, like the camera views.
        Action::CollisionColour(_) => WindowOnly(VIEWING),
        // Whether other races' barriers are drawn in World: a view setting, like the camera views.
        Action::RaceBarriers => WindowOnly(VIEWING),
        Action::Goal(_) | Action::GoTo | Action::SaveView | Action::RecallView(_) | Action::DeleteView(_) => {
            WindowOnly(VIEWING)
        }
        Action::TestFocus => WindowOnly("picks the car and world clock the window's next test game starts with; a command has no game window"),
        Action::HideSelected | Action::IsolateSelected | Action::ShowAll | Action::ModOnly => WindowOnly(
            "hides or isolates props in the window's view only, never in the mod; a script reads and writes the data itself",
        ),
        Action::Draw(d) => match d {
            Draw::Wall { .. } => Has(&["collision wall"]),
            Draw::Ground { .. } => Has(&["collision ground"]),
        },
    }
}

/// Every claim of a counterpart is a command that exists; every window-only line says why.
fn check(what: &str, c: Cli) {
    match c {
        Has(words) => {
            assert!(!words.is_empty(), "{what}");
            for w in words {
                assert!(
                    registry::command(&w.replace(' ', ".")).is_some(),
                    "{what}: `{w}` is not a command of the registry"
                );
            }
        }
        WindowOnly(why) => assert!(why.len() >= 20, "{what}: a window-only command needs its reason"),
    }
}

#[test]
fn every_window_command_has_a_command_line_counterpart_or_a_reason() {
    for w in Workspace::ALL.into_iter().chain(Workspace::PAGES) {
        check(&format!("workspace {}", w.name()), workspace(w));
    }
    for k in KEYS.iter() {
        check(&format!("key {}", k.word), nav_command(k.command));
    }
    for c in Command::ALL {
        check(&format!("menu command {c:?}"), menu_command(c));
    }
    let samples = [
        Action::NewMod,
        Action::OpenMod,
        Action::OpenRecent(String::new()),
        Action::Save,
        Action::Details,
        Action::Import,
        Action::Export,
        Action::ExportPack,
        Action::Quit,
        Action::Undo,
        Action::Redo,
        Action::Roads,
        Action::Tab(""),
        Action::Controls,
        Action::HowToTest,
        Action::Hint(true),
        Action::Draw(Draw::Wall { first: None }),
        Action::Draw(Draw::Ground { points: Vec::new() }),
    ]
    .into_iter()
    .chain(Tool::ALL.map(Action::Tool))
    .chain(Show::ALL.map(Action::Collision));
    for a in samples {
        check(&format!("menu action {a:?}"), action(&a));
    }
}

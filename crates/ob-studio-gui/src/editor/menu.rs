//! The menu bar, the toolbar, the hint line, the status bar, the shell's
//! keys and the key table (Help > Controls is made from it).
//!
//! There are no modes: one toolbar with the same buttons in the same
//! order in every state ([`Tool`]: Select, Move, Rotate, Snap, then the
//! race tools in a fixed slot, Undo, Redo, Save; Simulate, Drive here and
//! Test race on the right). A race tool is armed explicitly and Esc goes
//! back to Select; the tools act on what is selected.
//!
//! Two kinds of request come out of them: [`Action`]s the shell runs
//! itself (files, undo, tools, panels), and [`Command`]s for the editor's parts
//! (Delete, Test race, …): a menu item or toolbar button posts a command
//! for this frame, and the part that handles it takes it with [`take`]
//! while drawing (its own key for the same thing goes through its own
//! input handling). A command that cannot run in the editor's state is
//! disabled with its reason ([`Command::unavailable`]); one posted that
//! nobody took is logged at the frame's end ([`clear_commands`]), and a
//! test runs every command for both kinds of selection and in both views
//! to keep that list empty.

use egui::{Key, KeyboardShortcut, Modifiers};
use ob_look::words;

use super::app::App;
use super::props::{self, Props};
use super::races::{self, Races};
use super::state::{EditorState, Focus, Level, Mode};
use super::theme;

/// What the shell does itself.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    NewMod,
    OpenMod,
    OpenRecent(String),
    Save,
    Details,
    Import,
    Export,
    /// File ▸ Export as mod pack… (a `.obpk`, docs/modding/asset-format/layout.md §4).
    ExportPack,
    Quit,
    Undo,
    Redo,
    /// Pick a toolbar tool (its key or its button).
    Tool(Tool),
    /// View ▸ Roads: the road network over the top view.
    Roads,
    /// View ▸ Show nearest road: mark the road nearest the selection (else the camera) and go to it.
    ShowNearestRoad,
    Tab(&'static str),
    Controls,
    /// Help ▸ How to test your race.
    HowToTest,
    /// Show or hide the hint line under the view.
    Hint(bool),
    /// The collision layer (View ▸ Collision).
    Collision(super::collision::Show),
    /// What colours the collision layer (View ▸ Collision ▸ By kind / By surface).
    CollisionColour(super::collision::Colour),
    /// Show or hide one kind of the layer (View ▸ Collision ▸ Kinds).
    CollisionKind(super::collision::Class),
    /// Every kind shown again.
    CollisionEveryKind,
    /// The layer's opacity in percent.
    CollisionOpacity(u8),
    CollisionWireframe,
    CollisionFullHeight,
    RaceBarriers,
    /// Arm a drawing tool (Edit ▸ Draw wall / Draw ground).
    Draw(super::collision::Draw),
    /// A camera preset (View ▸ Top view / Isometric view).
    Goal(super::views::Goal),
    /// View ▸ Go to…
    GoTo,
    /// Race ▸ Test car and clock…: what a test run starts with besides the place.
    TestFocus,
    /// H, Shift+H, Alt+H: the view's own hiding (never the mod's).
    HideSelected,
    IsolateSelected,
    ShowAll,
    /// View ▸ This mod's changes only.
    ModOnly,
    /// View ▸ Views ▸ Save view…
    SaveView,
    /// A saved view by position, to bring back or to delete.
    RecallView(usize),
    DeleteView(usize),
}

/// A request for one of the editor's parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    Duplicate,
    Delete,
    /// Edit ▸ Collision ▸ Remove wall (Del on the selected wall or drawn ground).
    RemoveWall,
    SelectAllOfSolid,
    DropToGround,
    Rotate90,
    FaceCamera,
    ResetCamera,
    FrameSelection,
    NewRace,
    TestRace,
    DriveHere,
    /// Start the open test game's run over.
    RestartTest,
    /// Drive to record: a test run whose drive is kept as a run (Runs).
    RecordDrive,
    /// Test race with the selected props' edits left out of the test layer, for this one run.
    TestWithout,
    /// Ask the open test game what it loaded (the opt-in probe).
    WhatLoaded,
}

impl Command {
    /// Every command, for the test that each one has a taker.
    pub const ALL: [Command; 16] = [
        Command::Duplicate,
        Command::Delete,
        Command::RemoveWall,
        Command::SelectAllOfSolid,
        Command::DropToGround,
        Command::Rotate90,
        Command::FaceCamera,
        Command::ResetCamera,
        Command::FrameSelection,
        Command::NewRace,
        Command::TestRace,
        Command::DriveHere,
        Command::RestartTest,
        Command::RecordDrive,
        Command::TestWithout,
        Command::WhatLoaded,
    ];

    /// What the menu item says when hovered (the test modes say who
    /// drives and where it shows).
    pub fn tip(self) -> Option<String> {
        match self {
            Command::TestRace => Some(super::modes::TEST_RACE.tooltip()),
            Command::DriveHere => Some(super::modes::DRIVE_HERE.tooltip()),
            _ => None,
        }
    }

    /// Why `self` cannot run on `s` now (the menu shows it disabled with
    /// this text), or None when a part takes and runs it.
    pub fn unavailable(self, s: &EditorState) -> Option<&'static str> {
        let props_only = matches!(
            self,
            Command::Duplicate
                | Command::SelectAllOfSolid
                | Command::DropToGround
                | Command::Rotate90
                | Command::FaceCamera
        );
        match self {
            _ if props_only && (s.mode != Mode::Props || s.selection.is_empty()) => Some("Select a prop first"),
            Command::Delete if s.mode == Mode::Props && s.selection.is_empty() && s.wall.is_none() => {
                Some("Select a prop first")
            }
            Command::Delete if s.mode == Mode::Races && s.point.is_none() => Some("Select a race point first"),
            Command::TestWithout if s.selection.is_empty() => Some("Select the props to leave out first"),
            Command::RemoveWall => match &s.wall {
                None => Some("Select a wall first (View ▸ Collision shows them)"),
                Some(p) if super::collision::is_barrier(p) => {
                    Some("A race's barriers are switched per race; they cannot be removed")
                }
                Some(_) => None,
            },
            _ => None,
        }
    }
}

fn commands_id() -> egui::Id {
    egui::Id::unique("editor.commands")
}

/// Post `c` for this frame.
pub fn post(ctx: &egui::Context, c: Command) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<Command>>(commands_id()).push(c));
}

fn hosted_id() -> egui::Id {
    egui::Id::new("ob-editor-hosted")
}

/// Mark the editor drawing on `ctx` as hosted by Studio, which keeps Ctrl+D for itself (Changes,
/// docs/modding/studio.md): the editor then neither reads nor shows Ctrl+D for Duplicate.
pub fn set_hosted(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(hosted_id(), true));
}

/// Studio hosts the editor drawing on `ctx` ([`set_hosted`]).
pub fn hosted(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(hosted_id()).unwrap_or(false))
}

/// Duplicate's key as shown: Ctrl+D in the editor's own window, none inside Studio (Ctrl+D is Changes there).
pub fn duplicate_key(ctx: &egui::Context) -> &'static str {
    if hosted(ctx) { "" } else { "Ctrl+D" }
}

/// Whether `c` was posted this frame (taking it: one part handles it).
pub fn take(ctx: &egui::Context, c: Command) -> bool {
    ctx.data_mut(|d| {
        let v = d.get_temp_mut_or_default::<Vec<Command>>(commands_id());
        match v.iter().position(|x| *x == c) {
            Some(i) => {
                v.remove(i);
                true
            }
            None => false,
        }
    })
}

/// Drop the commands nobody took (end of frame), logged, and return
/// them.
pub fn clear_commands(ctx: &egui::Context) -> Vec<Command> {
    let left = ctx.data_mut(|d| d.remove_temp::<Vec<Command>>(commands_id())).unwrap_or_default();
    for c in &left {
        log::warn!("map editor: nothing took the command {c:?}");
    }
    left
}

const fn ctrl(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, key)
}
pub const SAVE: KeyboardShortcut = ctrl(Key::S);
pub const UNDO: KeyboardShortcut = ctrl(Key::Z);
pub const REDO: KeyboardShortcut = ctrl(Key::Y);
pub const REDO_SHIFT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
pub const NEW: KeyboardShortcut = ctrl(Key::N);
pub const OPEN: KeyboardShortcut = ctrl(Key::O);
pub const QUIT: KeyboardShortcut = ctrl(Key::Q);

/// The toolbar's tools, in toolbar order: what the left button does in
/// the view. Select, Move and Rotate work on the selected props (Select
/// also picks race points); the race tools add to the selected race and
/// stay armed until Esc or another tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Select,
    Move,
    Rotate,
    Checkpoint,
    Start,
    Finish,
    Grid,
}

impl Tool {
    /// Every tool, in toolbar order.
    pub const ALL: [Tool; 7] =
        [Tool::Select, Tool::Move, Tool::Rotate, Tool::Checkpoint, Tool::Start, Tool::Finish, Tool::Grid];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Move => "Move",
            Tool::Rotate => "Rotate",
            Tool::Checkpoint => "Checkpoint",
            Tool::Start => "Start",
            Tool::Finish => "Finish",
            Tool::Grid => "Grid",
        }
    }

    /// Its key (no modifier).
    pub fn key(self) -> Key {
        match self {
            Tool::Select => Key::Q,
            Tool::Move => Key::W,
            Tool::Rotate => Key::E,
            Tool::Checkpoint => Key::C,
            Tool::Start => Key::S,
            Tool::Finish => Key::X,
            Tool::Grid => Key::G,
        }
    }

    /// What it does (the key table, the button's tooltip).
    pub fn what(self) -> &'static str {
        match self {
            Tool::Select => "Select tool: a click selects a prop or a race point, never adds",
            Tool::Move => "Move tool: drag an arrow to move the selected props",
            Tool::Rotate => "Rotate tool: drag the ring to turn the selected props (Alt: turn freely)",
            Tool::Checkpoint => "Checkpoint tool: a click on the road adds a checkpoint",
            Tool::Start => "Start tool: a click on the road puts the start line",
            Tool::Finish => "Finish tool: a click on the road puts the finish line",
            Tool::Grid => "Grid tool: a click on the road adds a grid slot",
        }
    }

    /// The race tool behind it.
    pub fn race(self) -> Option<races::Tool> {
        match self {
            Tool::Checkpoint => Some(races::Tool::Checkpoint),
            Tool::Start => Some(races::Tool::Start),
            Tool::Finish => Some(races::Tool::Finish),
            Tool::Grid => Some(races::Tool::Grid),
            _ => None,
        }
    }

    /// A tool that adds on a click (Esc goes back to Select).
    pub fn armed(self) -> bool {
        self.race().is_some()
    }

    /// Why it cannot be picked now.
    pub fn unavailable(self, s: &EditorState) -> Option<&'static str> {
        (self.armed() && s.race.is_none_or(|k| k >= s.session.map.races.len()))
            .then_some("Make a race first: Race > New race")
    }
}

/// The tool in force: a race tool while one is armed for a selected race,
/// else the props' tool.
pub fn current_tool(s: &EditorState, p: &Props, r: &Races) -> Tool {
    if s.mode == Mode::Races {
        match r.tool {
            races::Tool::Checkpoint => return Tool::Checkpoint,
            races::Tool::Start => return Tool::Start,
            races::Tool::Finish => return Tool::Finish,
            races::Tool::Grid => return Tool::Grid,
            races::Tool::Select => {}
        }
    }
    match p.tool {
        props::Tool::Select => Tool::Select,
        props::Tool::Move => Tool::Move,
        props::Tool::Rotate => Tool::Rotate,
    }
}

/// Pick tool `t`. A race tool works on the selected race (the race's
/// side of the selection comes forward); Move and Rotate on the props.
pub fn set_tool(s: &mut EditorState, p: &mut Props, r: &mut Races, t: Tool) {
    if t.unavailable(s).is_some() {
        return;
    }
    match t {
        Tool::Select => {
            p.tool = props::Tool::Select;
            r.tool = races::Tool::Select;
        }
        Tool::Move | Tool::Rotate => {
            p.tool = if t == Tool::Move { props::Tool::Move } else { props::Tool::Rotate };
            r.tool = races::Tool::Select;
            s.mode = Mode::Props;
        }
        _ => {
            r.tool = t.race().unwrap_or(races::Tool::Select);
            s.mode = Mode::Races;
        }
    }
}

/// One row of the key table.
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    /// Where it applies (the Controls dialog's heading).
    pub group: &'static str,
    /// The keys as shown.
    pub keys: &'static str,
    pub what: &'static str,
    /// The keys it reads (a test checks that every key the editor reads
    /// is in the table).
    pub reads: &'static [Key],
}

const fn b(group: &'static str, keys: &'static str, what: &'static str, reads: &'static [Key]) -> Binding {
    Binding { group, keys, what, reads }
}

/// The key table: Help > Controls lists it, with the tools' keys
/// ([`Tool::ALL`]) under "Tools".
pub const KEYS: &[Binding] = &[
    b("View", "RMB + W A S D", "Fly", &[Key::W, Key::A, Key::S, Key::D]),
    b("View", "RMB + Q / E  (F / R)", "Fly down / up", &[Key::Q, Key::E, Key::F, Key::R]),
    b("View", "RMB + wheel", "Fly speed", &[]),
    b("View", "Shift / Ctrl while flying", "Fast / slow", &[]),
    b("View", "Wheel", "Dolly; in the overview: zoom", &[]),
    b("View", "F", "Frame the selection", &[Key::F]),
    b("View", "T", "Top-down overview on / off", &[Key::T]),
    b("View", "Home", "Fit the overview to the region", &[Key::Home]),
    b("View", "H", "Hide the selected props (this view only)", &[Key::H]),
    b("View", "Shift+H", "Show only the selected props", &[Key::H]),
    b("View", "Alt+H", "Show every prop again", &[Key::H]),
    b("View", "L", "Collision layer: off / invisible barriers / all collision", &[Key::L]),
    b("Tools", "Esc", "Back to Select (a race tool armed); else clear the selection", &[Key::Escape]),
    b("Edit", "LMB / Ctrl+LMB", "Select / add to the selection", &[]),
    b("Edit", "Shift + drag an arrow", "Copy the selection and move the copy", &[]),
    b("Edit", "Alt while dragging or placing", "No snapping", &[]),
    b("Edit", "Shift + click on the route", "Insert a checkpoint there", &[]),
    b("Edit", "[ / ]", "Previous / next checkpoint", &[Key::OpenBracket, Key::CloseBracket]),
    b("Edit", "Ctrl+D", words::DUPLICATE, &[Key::D]),
    b("Edit", "Del", words::DELETE, &[Key::Delete]),
    b("Edit", "B / Shift+B", "Draw wall / Draw ground (Enter or the first corner finishes the outline)", &[Key::B]),
    b("Edit", "Esc, Backspace", "In a drawing tool: Esc drops the shape, again puts the tool down; Backspace the last corner", &[Key::Backspace]),
    b("Edit", "End", "Drop to the ground", &[Key::End]),
    b("Edit", "RMB on a prop / Shift+F10", "The prop menu: Rotate 90°, Rotate…, Face camera", &[Key::F10]),
    b("Edit", "Ctrl+Z, Ctrl+Y (Ctrl+Shift+Z)", "Undo, redo", &[Key::Z, Key::Y]),
    b("File", "Ctrl+N, Ctrl+O", "New map mod, open a map mod", &[Key::N, Key::O]),
    b("File", "Ctrl+S", words::SAVE, &[Key::S]),
    b("File", "Ctrl+Q", words::QUIT, &[Key::Q]),
    b("File", "Enter", "Confirm a dialog", &[Key::Enter]),
    b("Run", "F5 / Shift+F5", "Test race / Drive here", &[Key::F5]),
    b("Run", "Ctrl+F5", "Restart race (the open test game)", &[Key::F5]),
    b("Run", "Space", "Play / pause the simulation (its timeline under the view)", &[Key::Space]),
    b(
        "Run",
        "Left / Right, Shift+Left / Right",
        "The simulation a recorded frame back / on, a second with Shift",
        &[Key::ArrowLeft, Key::ArrowRight],
    ),
    b(
        "Run",
        "Up / Down",
        "Show the line above / below in What went wrong (Simulate tab)",
        &[Key::ArrowUp, Key::ArrowDown],
    ),
];

/// The Controls dialog's rows, grouped: the tools first (from
/// [`Tool::ALL`]), then the key table.
pub fn controls() -> Vec<(&'static str, Vec<(String, &'static str)>)> {
    let mut out: Vec<(&'static str, Vec<(String, &'static str)>)> = Vec::new();
    let mut add = |group: &'static str, keys: String, what: &'static str| match out.iter_mut().find(|g| g.0 == group) {
        Some(g) => g.1.push((keys, what)),
        None => out.push((group, vec![(keys, what)])),
    };
    for t in Tool::ALL {
        add("Tools", t.key().name().to_string(), t.what());
    }
    for k in KEYS {
        add(k.group, k.keys.to_string(), k.what);
    }
    out
}

/// The shell's keys (not while a text field has the keyboard for the
/// single keys): files, undo, the view, the tools' keys and Esc while a
/// race tool is armed (not while the camera flies: WASD QE are its keys
/// then). Tab is left to egui (keyboard focus).
pub fn shortcuts(app: &App, ctx: &egui::Context, out: &mut Vec<Action>) {
    let open = app.state.is_some();
    let typing = ctx.egui_wants_keyboard_input();
    let flying = super::app::flying(ctx);
    let armed = app
        .state
        .as_ref()
        .is_some_and(|s| current_tool(s, &app.panels.props.borrow(), &app.panels.races.borrow()).armed())
        // Esc during a drag only puts the drag back; the tool stays.
        && !app.panels.races.borrow().is_dragging();
    ctx.input_mut(|i| {
        if i.consume_shortcut(&NEW) {
            out.push(Action::NewMod);
        }
        if i.consume_shortcut(&OPEN) {
            out.push(Action::OpenMod);
        }
        if i.consume_shortcut(&QUIT) {
            out.push(Action::Quit);
        }
        if !open {
            return;
        }
        if i.consume_shortcut(&SAVE) {
            out.push(Action::Save);
        }
        if i.consume_shortcut(&REDO_SHIFT) || i.consume_shortcut(&REDO) {
            out.push(Action::Redo);
        }
        if i.consume_shortcut(&UNDO) {
            out.push(Action::Undo);
        }
        if typing {
            return;
        }
        if i.consume_key(Modifiers::SHIFT, Key::H) {
            out.push(Action::IsolateSelected);
        }
        if i.consume_key(Modifiers::ALT, Key::H) {
            out.push(Action::ShowAll);
        }
        if i.consume_key(Modifiers::SHIFT, Key::B) {
            out.push(Action::Draw(super::collision::Draw::Ground { points: Vec::new() }));
        }
        if flying || i.modifiers.any() {
            return;
        }
        if i.consume_key(Modifiers::NONE, Key::B) {
            out.push(Action::Draw(super::collision::Draw::Wall { first: None }));
        }
        if i.consume_key(Modifiers::NONE, Key::H) {
            out.push(Action::HideSelected);
        }
        if i.consume_key(Modifiers::NONE, Key::L) {
            let now = app.state.as_ref().map_or(super::collision::Show::Off, |s| s.collision);
            out.push(Action::Collision(now.next()));
        }
        for t in Tool::ALL {
            if i.consume_key(Modifiers::NONE, t.key()) {
                out.push(Action::Tool(t));
            }
        }
        if armed && i.consume_key(Modifiers::NONE, Key::Escape) {
            out.push(Action::Tool(Tool::Select));
        }
    });
    if open && !typing {
        if ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::F5)) {
            post(ctx, Command::DriveHere);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F5)) {
            post(ctx, Command::RestartTest);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F5)) {
            post(ctx, Command::TestRace);
        }
    }
}

fn item(ui: &mut egui::Ui, text: &str, key: Option<&KeyboardShortcut>, enabled: bool) -> bool {
    let mut b = egui::Button::new(text);
    if let Some(k) = key {
        b = b.shortcut_text(ui.ctx().format_shortcut(k));
    }
    ui.add_enabled(enabled, b).clicked()
}

/// A menu item that says why it cannot act while `why` is set.
fn item_why(ui: &mut egui::Ui, text: &str, key: &str, why: Option<&str>) -> bool {
    let r = ui.add_enabled(why.is_none(), egui::Button::new(text).shortcut_text(key));
    match why {
        Some(w) => r.on_disabled_hover_text(w).clicked(),
        None => r.clicked(),
    }
}

/// A menu item that posts `c`: disabled with the reason while it cannot
/// run.
fn command(ui: &mut egui::Ui, s: Option<&EditorState>, text: &str, key: &str, c: Command) -> bool {
    let why = match s {
        Some(s) => c.unavailable(s),
        None => Some("Open a map mod first"),
    };
    let r = ui.add_enabled(why.is_none(), egui::Button::new(text).shortcut_text(key));
    let r = match (why, c.tip()) {
        (Some(why), _) => r.on_disabled_hover_text(why),
        (None, Some(tip)) => r.on_hover_text(tip),
        (None, None) => r,
    };
    if r.clicked() {
        post(ui.ctx(), c);
        return true;
    }
    false
}

/// The menu bar.
pub fn bar(app: &App, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if item(ui, "New map mod…", Some(&NEW), true) {
                out.push(Action::NewMod);
            }
            if item(ui, "Open map mod…", Some(&OPEN), true) {
                out.push(Action::OpenMod);
            }
            let recent = super::app::recent(&app.maps);
            if !recent.is_empty() {
                ui.separator();
                ui.weak("Recent");
                for f in recent.into_iter().take(5) {
                    if ui.button(&f).clicked() {
                        out.push(Action::OpenRecent(f));
                    }
                }
            }
            ui.separator();
            mod_file_items(app, ui, out);
            ui.separator();
            if item(ui, words::QUIT, Some(&QUIT), true) {
                out.push(Action::Quit);
            }
        });
        ui.menu_button("Edit", |ui| edit_items(app, ui, out));
        ui.menu_button("View", |ui| view_items(app, ui, out));
        ui.menu_button("Race", |ui| race_items(app, ui, out));
        ui.menu_button("Help", |ui| help_items(app, ui, out));
        // The mod and region at the right; a dot while unsaved.
        if let Some(s) = app.state.as_ref() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                if s.session.dirty {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.5, theme::warn());
                }
                let name =
                    if s.session.map.name.is_empty() { s.session.folder.as_str() } else { s.session.map.name.as_str() };
                let text =
                    egui::RichText::new(format!("{name} · {}", s.session.region)).font(theme::font(theme::SEMI, 14.0));
                ui.add(egui::Label::new(text).truncate());
            });
        }
    });
}

/// File's items for the open mod: Save, Mod details, Import, Export (Studio's
/// File menu has them too).
pub fn mod_file_items(app: &App, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let open = app.state.is_some();
    if item(ui, words::SAVE, Some(&SAVE), open) {
        out.push(Action::Save);
    }
    if item(ui, "Mod details…", None, open) {
        out.push(Action::Details);
    }
    ui.separator();
    if item(ui, "Import…", None, true) {
        out.push(Action::Import);
    }
    if item(ui, "Export…", None, open) {
        out.push(Action::Export);
    }
    if item(ui, "Export as mod pack…", None, open) {
        out.push(Action::ExportPack);
    }
}

/// The Edit menu's items.
pub fn edit_items(app: &App, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let (u, r) = app.state.as_ref().map_or((None, None), |s| {
        (s.session.undo_label().map(str::to_string), s.session.redo_label().map(str::to_string))
    });
    let ut = u.as_ref().map_or(words::UNDO.to_string(), |l| format!("Undo: {l}"));
    if item(ui, &ut, Some(&UNDO), u.is_some()) {
        out.push(Action::Undo);
    }
    let rt = r.as_ref().map_or(words::REDO.to_string(), |l| format!("Redo: {l}"));
    if item(ui, &rt, Some(&REDO), r.is_some()) {
        out.push(Action::Redo);
    }
    ui.separator();
    let s = app.state.as_ref();
    let dup = duplicate_key(ui.ctx());
    command(ui, s, words::DUPLICATE, dup, Command::Duplicate);
    command(ui, s, words::DELETE, "Del", Command::Delete);
    command(ui, s, "Select all of this solid", "", Command::SelectAllOfSolid);
    command(ui, s, "Drop to ground", "End", Command::DropToGround);
    command(ui, s, "Rotate 90°", "", Command::Rotate90);
    command(ui, s, "Face camera", "", Command::FaceCamera);
    ui.separator();
    // One group for the collision tools: the same selection (a click on a wall), the same Del, Esc and Alt.
    let open = s.is_some();
    ui.menu_button("Collision", |ui| {
        if item_why(ui, "Draw wall", "B", (!open).then_some("Open a map mod first")) {
            out.push(Action::Draw(super::collision::Draw::Wall { first: None }));
            ui.close();
        }
        if item_why(ui, "Draw ground", "Shift+B", (!open).then_some("Open a map mod first")) {
            out.push(Action::Draw(super::collision::Draw::Ground { points: Vec::new() }));
            ui.close();
        }
        command(ui, s, "Remove wall", "Del", Command::RemoveWall);
    });
}

/// The View menu's items.
pub fn view_items(app: &App, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let open = app.state.is_some();
    let roads = app.state.as_ref().is_some_and(|s| s.roads);
    if ui
        .add_enabled(open, egui::Button::selectable(roads, "Roads"))
        .on_hover_text("The road network over the top view (T)")
        .clicked()
    {
        out.push(Action::Roads);
    }
    let none = if open { None } else { Some("Open a map mod first") };
    if item_why(ui, "Show nearest road", "", none) {
        out.push(Action::ShowNearestRoad);
    }
    ui.separator();
    let s = app.state.as_ref();
    command(ui, s, "Frame selection", "F", Command::FrameSelection);
    command(ui, s, "Reset camera", "", Command::ResetCamera);
    if item_why(ui, "Top view", "T", none) {
        out.push(Action::Goal(super::views::Goal::Top));
    }
    if item_why(ui, "Isometric view", "", none) {
        out.push(Action::Goal(super::views::Goal::Isometric));
    }
    if item_why(ui, "Go to…", "", none) {
        out.push(Action::GoTo);
    }
    ui.add_enabled_ui(open, |ui| {
        ui.menu_button("Views", |ui| {
            if item_why(ui, "Save view…", "", none) {
                out.push(Action::SaveView);
            }
            let saved = s.map_or(&[][..], |s| &s.views[..]);
            if saved.is_empty() {
                ui.weak("No saved views");
            }
            for (i, v) in saved.iter().enumerate() {
                if ui.button(&v.name).on_hover_text("Bring back this camera and its layers").clicked() {
                    out.push(Action::RecallView(i));
                }
            }
            if !saved.is_empty() {
                ui.separator();
                ui.menu_button("Delete view", |ui| {
                    for (i, v) in saved.iter().enumerate() {
                        if ui.button(&v.name).clicked() {
                            out.push(Action::DeleteView(i));
                        }
                    }
                });
            }
        });
    });
    ui.separator();
    let shown = s.map(|s| s.collision);
    ui.add_enabled_ui(open, |ui| {
        ui.menu_button("Collision", |ui| {
            for c in super::collision::Show::ALL {
                if ui.add(egui::Button::selectable(shown == Some(c), c.label())).clicked() {
                    out.push(Action::Collision(c));
                }
            }
            ui.separator();
            let colour = s.map(|s| s.collision_colour);
            for c in super::collision::Colour::ALL {
                if ui.add(egui::Button::selectable(colour == Some(c), c.label())).clicked() {
                    out.push(Action::CollisionColour(c));
                }
            }
            let look = s.map(|s| s.collision_look).unwrap_or_default();
            ui.menu_button("Kinds", |ui| {
                for c in super::collision::Class::ALL {
                    if ui.add(egui::Button::selectable(look.shows(c), c.label())).clicked() {
                        out.push(Action::CollisionKind(c));
                    }
                }
                ui.separator();
                if ui.button("Show every kind").clicked() {
                    out.push(Action::CollisionEveryKind);
                }
            });
            ui.menu_button("Opacity", |ui| {
                for c in super::collision::OPACITIES {
                    if ui.add(egui::Button::selectable(look.opacity == c, super::collision::opacity_label(c))).clicked() {
                        out.push(Action::CollisionOpacity(c));
                    }
                }
            });
            if ui.add(egui::Button::selectable(look.wireframe, "Wireframe")).clicked() {
                out.push(Action::CollisionWireframe);
            }
            if ui.add(egui::Button::selectable(look.full_height, "Full-height walls")).on_hover_text("Walls the game lets reach from -2048 m to 2048 m drawn that tall, not only beside the floors").clicked() {
                out.push(Action::CollisionFullHeight);
            }
            if s.is_some_and(|s| s.focus == Focus::World) {
                ui.separator();
                let on = s.is_some_and(|s| s.race_barriers);
                if ui.add(egui::Button::selectable(on, "Show race barriers")).on_hover_text("The barriers a race switches on: in the game they are on only during that race. Races shows the selected race's.").clicked() {
                    out.push(Action::RaceBarriers);
                }
            }
        });
    });
    ui.separator();
    let sel = match s {
        None => Some("Open a map mod first"),
        Some(s) if s.selection.is_empty() => Some("Select a prop first"),
        Some(_) => None,
    };
    if item_why(ui, "Hide selected", "H", sel) {
        out.push(Action::HideSelected);
    }
    if item_why(ui, "Isolate selected", "Shift+H", sel) {
        out.push(Action::IsolateSelected);
    }
    let any = s.is_some_and(|s| s.filter.hidden.len() + s.filter.isolated.as_ref().map_or(0, |i| i.len().max(1)) > 0);
    if item_why(ui, "Show all", "Alt+H", if open { (!any).then_some("Nothing is hidden") } else { none }) {
        out.push(Action::ShowAll);
    }
    let only = s.is_some_and(|s| s.filter.mod_only);
    if ui
        .add_enabled(open, egui::Button::selectable(only, "This mod's changes only"))
        .on_hover_text("Draw only the props this mod moved, deleted or added (this view only)")
        .clicked()
    {
        out.push(Action::ModOnly);
    }
    ui.separator();
    if ui.add_enabled(open, egui::Button::selectable(app.layout.hint, "Hint line")).clicked() {
        out.push(Action::Hint(!app.layout.hint));
    }
}

/// The Race menu's items (Studio's Test menu).
pub fn race_items(app: &App, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let s = app.state.as_ref();
    command(ui, s, "New race", "", Command::NewRace);
    command(ui, s, "Test race", "F5", Command::TestRace);
    command(ui, s, "Drive here", "Shift+F5", Command::DriveHere);
    command(ui, s, "Restart race", "Ctrl+F5", Command::RestartTest);
    command(ui, s, "Drive to record", "", Command::RecordDrive);
    command(ui, s, "Test without selected props", "", Command::TestWithout);
    command(ui, s, "What the game loaded", "", Command::WhatLoaded);
    ui.separator();
    let why = s.is_none().then_some("Open a map mod first");
    if item_why(ui, "Test car and clock…", "", why) {
        out.push(Action::TestFocus);
    }
}

/// The Help menu's items.
pub fn help_items(_app: &App, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    if ui.button("Controls").clicked() {
        out.push(Action::Controls);
    }
    if ui.button("How to test your race").clicked() {
        out.push(Action::HowToTest);
    }
}

/// How much the toolbar spells out. The first tier that fits the window's
/// width is used: 0 every label with its key and the snap steps; 1 no key
/// hints or snap steps; 2 also no "Race" over the race tools, "Drive" for
/// Drive here and tighter spacing. The buttons and their order are the
/// same in every tier.
pub const TIERS: usize = 3;

/// The gap between the left group and the right one (Simulate, Drive
/// here, Test race).
const GAP: f32 = 16.0;

/// What the last frame's toolbar laid out: the tier, the toolbar's
/// rectangle and every item's (tests and `--shot` check it:
/// [`ToolbarFit::problems`]).
#[derive(Clone, Debug, PartialEq)]
pub struct ToolbarFit {
    pub tier: usize,
    pub bar: egui::Rect,
    /// Every item in order, left to right: (name, rectangle).
    pub items: Vec<(String, egui::Rect)>,
}

impl Default for ToolbarFit {
    fn default() -> ToolbarFit {
        ToolbarFit { tier: 0, bar: egui::Rect::NOTHING, items: Vec::new() }
    }
}

impl ToolbarFit {
    /// The items cut off at the toolbar's edge or on top of another one.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let bar = self.bar.expand(0.5);
        for (k, (name, r)) in self.items.iter().enumerate() {
            if !bar.contains_rect(*r) {
                out.push(format!("{name} cut off ({:.0}..{:.0} of 0..{:.0})", r.left(), r.right(), self.bar.right()));
            }
            for (other, o) in &self.items[k + 1..] {
                let i = r.intersect(*o);
                if i.width() > 0.5 && i.height() > 0.5 {
                    out.push(format!("{name} over {other}"));
                }
            }
        }
        out
    }

    /// The names in order.
    pub fn names(&self) -> Vec<&str> {
        self.items.iter().map(|(n, _)| n.as_str()).collect()
    }
}

fn fit_id() -> egui::Id {
    egui::Id::unique("editor.toolbar.fit")
}

/// The last toolbar's layout.
pub fn toolbar_fit(ctx: &egui::Context) -> Option<ToolbarFit> {
    ctx.data(|d| d.get_temp::<ToolbarFit>(fit_id()))
}

fn sim_width_id() -> egui::Id {
    egui::Id::unique("editor.toolbar.sim-width")
}

/// A button's words: the name in the label face, the key after it small
/// and dim.
fn label_job(name: &str, key: Option<&str>, on: bool, enabled: bool) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let ink = if !enabled {
        theme::dim()
    } else if on {
        theme::lime()
    } else {
        theme::ink()
    };
    job.append(name, 0.0, egui::TextFormat::simple(theme::font(theme::LABEL, 15.0), ink));
    if let Some(k) = key {
        job.append(k, 5.0, egui::TextFormat::simple(egui::FontId::proportional(12.0), theme::dim()));
    }
    job.halign = egui::Align::LEFT;
    job
}

fn text_width(ui: &egui::Ui, job: egui::text::LayoutJob) -> f32 {
    egui::WidgetText::from(job)
        .into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Button)
        .size()
        .x
}

/// The left group's parts in order.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Part {
    Tool(Tool),
    Snap,
    Sep,
    /// The race tools' slot: its start ("Race" in tier 0 and 1) and end.
    SlotStart,
    SlotEnd,
    Undo,
    Redo,
    Save,
}

const LEFT: [Part; 15] = [
    Part::Tool(Tool::Select),
    Part::Tool(Tool::Move),
    Part::Tool(Tool::Rotate),
    Part::Snap,
    Part::Sep,
    Part::SlotStart,
    Part::Tool(Tool::Checkpoint),
    Part::Tool(Tool::Start),
    Part::Tool(Tool::Finish),
    Part::Tool(Tool::Grid),
    Part::SlotEnd,
    Part::Sep,
    Part::Undo,
    Part::Redo,
    Part::Save,
];

/// The snap button's words.
fn snap_job(p: &Props, r: &Races, tool: Tool, tier: usize) -> egui::text::LayoutJob {
    let (on, steps) = if tool.armed() {
        (r.snap, "to road".to_string())
    } else {
        (p.snap, format!("{} m · {}°", trim_num(p.snap_metres), trim_num(p.snap_degrees)))
    };
    label_job("Snap", (tier == 0).then_some(steps.as_str()), on, true)
}

fn trim_num(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn part_job(
    part: Part,
    tier: usize,
    s: &EditorState,
    p: &Props,
    r: &Races,
    tool: Tool,
) -> Option<egui::text::LayoutJob> {
    let key = |t: Tool| (tier == 0).then(|| t.key().name());
    // World: no race tools (the race tools' slot is not drawn).
    if s.focus == Focus::World
        && matches!(
            part,
            Part::SlotStart | Part::SlotEnd | Part::Tool(Tool::Checkpoint | Tool::Start | Tool::Finish | Tool::Grid)
        )
    {
        return None;
    }
    Some(match part {
        Part::Tool(t) => label_job(t.name(), key(t), tool == t, t.unavailable(s).is_none()),
        Part::Snap => snap_job(p, r, tool, tier),
        Part::Undo => label_job(words::UNDO, None, false, true),
        Part::Redo => label_job(words::REDO, None, false, true),
        Part::Save => label_job(words::SAVE, None, false, true),
        Part::SlotStart if tier < 2 => {
            let mut j = egui::text::LayoutJob::default();
            j.append("Race", 0.0, egui::TextFormat::simple(egui::FontId::proportional(12.0), theme::dim()));
            j
        }
        _ => return None,
    })
}

/// (pad, item spacing) of a tier.
fn spacing(tier: usize) -> (f32, f32) {
    if tier >= 2 { (6.0, 5.0) } else { (10.0, 8.0) }
}

/// The right group's words: Drive here, Test race.
fn right_words(tier: usize) -> (&'static str, &'static str) {
    match tier {
        0 => ("Drive here", "Test race  F5"),
        1 => ("Drive here", "Test race"),
        _ => ("Drive", "Test race"),
    }
}

/// The width the toolbar needs at `tier`.
fn needed(ui: &egui::Ui, tier: usize, s: &EditorState, p: &Props, r: &Races, tool: Tool) -> f32 {
    let (pad, gap) = spacing(tier);
    let mut w = 0.0;
    let mut n = 0;
    for part in LEFT {
        n += 1;
        w += match part {
            Part::Sep => 6.0,
            Part::SlotEnd => 4.0,
            _ => match part_job(part, tier, s, p, r, tool) {
                Some(j) => text_width(ui, j) + if part == Part::SlotStart { 4.0 } else { 2.0 * pad },
                None => 4.0,
            },
        };
    }
    let sim = ui
        .ctx()
        .data(|d| d.get_temp::<f32>(sim_width_id()))
        .unwrap_or_else(|| text_width(ui, label_job("Simulate", None, false, true)) + 2.0 * pad);
    let (drive, test) = right_words(tier);
    if s.focus == Focus::World {
        return w + gap * (n - 1) as f32 + GAP + text_width(ui, label_job(drive, None, false, true)) + 4.0 * pad + 8.0;
    }
    let right = sim
        + text_width(ui, label_job(drive, None, false, true))
        + 2.0 * pad
        + text_width(ui, label_job(test, None, false, true))
        + 2.0 * pad
        + 2.0 * gap;
    w + gap * (n - 1) as f32 + GAP + right + 8.0
}

/// The toolbar: Select, Move, Rotate, Snap; the race tools in their slot;
/// Undo, Redo, Save; Simulate, Drive here and Test race at the right. The
/// same buttons in the same order in every state: a button that cannot
/// act is disabled with its reason.
pub fn toolbar(app: &mut App, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let Some(s) = app.state.as_mut() else { return };
    let panels = &mut app.panels;
    let bar = ui.max_rect();
    let tool = current_tool(s, &panels.props.borrow(), &panels.races.borrow());
    let tier = {
        let (p, r) = (panels.props.borrow(), panels.races.borrow());
        (0..TIERS).find(|&t| needed(ui, t, s, &p, &r, tool) <= bar.width()).unwrap_or(TIERS - 1)
    };
    let (pad, gap) = spacing(tier);
    let mut fit = ToolbarFit { tier, bar, items: Vec::new() };
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().button_padding.x = pad;
        ui.spacing_mut().item_spacing.x = gap;
        let mut slot = egui::Rect::NOTHING;
        for part in LEFT {
            let r = {
                let (p, r) = (panels.props.borrow(), panels.races.borrow());
                part_job(part, tier, s, &p, &r, tool)
            };
            match part {
                Part::Sep => {
                    ui.separator();
                }
                Part::SlotStart => {
                    slot = egui::Rect::from_min_size(ui.cursor().min, egui::Vec2::ZERO);
                    match r {
                        Some(j) => {
                            ui.add_space(4.0 - gap);
                            let resp = ui.label(j);
                            fit.items.push(("Race".into(), resp.rect));
                        }
                        None => ui.add_space(4.0 - gap),
                    }
                }
                Part::SlotEnd => {
                    ui.add_space(4.0 - gap);
                    slot.max = egui::pos2(ui.cursor().min.x, slot.min.y);
                }
                Part::Tool(t) => {
                    let Some(j) = r else { continue };
                    let why = t.unavailable(s);
                    let resp = ui.add_enabled(why.is_none(), egui::Button::selectable(tool == t, j));
                    let resp = match why {
                        Some(why) => resp.on_disabled_hover_text(why),
                        None => resp.on_hover_text(format!("{} ({})", t.what(), t.key().name())),
                    };
                    if resp.clicked() {
                        out.push(Action::Tool(t));
                    }
                    fit.items.push((t.name().into(), resp.rect));
                }
                Part::Snap => {
                    let Some(j) = r else { continue };
                    let (mut p, mut rc) = (panels.props.borrow_mut(), panels.races.borrow_mut());
                    let on = if tool.armed() { rc.snap } else { p.snap };
                    let tip = if tool.armed() {
                        "Snap race points to the road (hold Alt to place freely). Right-click: the steps"
                    } else {
                        "Snap moves and turns to steps (hold Alt to bypass). Right-click: the steps"
                    };
                    let resp = ui.add(egui::Button::selectable(on, j)).on_hover_text(tip);
                    if resp.clicked() {
                        if tool.armed() {
                            rc.snap = !rc.snap;
                        } else {
                            p.snap = !p.snap;
                        }
                    }
                    resp.context_menu(|ui| {
                        ui.checkbox(&mut p.snap, "Snap props");
                        ui.horizontal(|ui| {
                            ui.label("Step");
                            ui.add(
                                egui::DragValue::new(&mut p.snap_metres).range(0.01..=10.0).speed(0.01).suffix(" m"),
                            );
                            ui.add(egui::DragValue::new(&mut p.snap_degrees).range(1.0..=90.0).speed(0.5).suffix("°"));
                        });
                        ui.checkbox(&mut rc.snap, "Snap race points to the road");
                    });
                    fit.items.push(("Snap".into(), resp.rect));
                }
                Part::Undo | Part::Redo | Part::Save => {
                    let (on, tip, a, name) = match part {
                        Part::Undo => (s.session.undo_label().is_some(), "Ctrl+Z", Action::Undo, words::UNDO),
                        Part::Redo => (s.session.redo_label().is_some(), "Ctrl+Y", Action::Redo, words::REDO),
                        _ => (s.session.dirty, "Ctrl+S", Action::Save, words::SAVE),
                    };
                    let Some(j) = r else { continue };
                    let resp = ui.add_enabled(on, egui::Button::new(j)).on_hover_text(tip);
                    let resp = if on {
                        resp
                    } else {
                        resp.on_disabled_hover_text(format!("Nothing to {}", name.to_lowercase()))
                    };
                    if resp.clicked() {
                        out.push(a);
                    }
                    fit.items.push((name.into(), resp.rect));
                }
            }
        }
        // The race tools' slot: a frame around them.
        if slot.width() > 0.0 && s.focus != Focus::World {
            let r = egui::Rect::from_x_y_ranges(slot.x_range(), ui.max_rect().y_range()).shrink2(egui::vec2(0.0, 3.0));
            ui.painter().rect_stroke(
                r,
                4.0,
                egui::Stroke::new(1.0, theme::dim().gamma_multiply(0.5)),
                egui::StrokeKind::Inside,
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (drive, test) = right_words(tier);
            let world = s.focus == Focus::World;
            let errors = s.session.issues.iter().any(|i| i.level == Level::Error);
            if world {
                let r =
                    ui.button(label_job(drive, None, false, true)).on_hover_text(super::modes::DRIVE_HERE.tooltip());
                if r.hovered() || r.has_focus() {
                    super::testplay::mark_drive_hover(ui.ctx());
                }
                if r.clicked() {
                    post(ui.ctx(), Command::DriveHere);
                }
                fit.items.push(("Drive here".into(), r.rect));
                return;
            }
            let r = ui.add_enabled(!errors, theme::main_action(test));
            let r = if errors {
                r.on_disabled_hover_text("Fix the errors in Validation first")
            } else {
                r.on_hover_text(super::modes::TEST_RACE.tooltip())
            };
            if r.clicked() {
                post(ui.ctx(), Command::TestRace);
            }
            let test_rect = r.rect;
            let r = ui.button(label_job(drive, None, false, true)).on_hover_text(super::modes::DRIVE_HERE.tooltip());
            if r.hovered() || r.has_focus() {
                super::testplay::mark_drive_hover(ui.ctx());
            }
            if r.clicked() {
                post(ui.ctx(), Command::DriveHere);
            }
            let drive_rect = r.rect;
            let mut sims = Vec::new();
            for t in panels.run_tools.iter_mut().rev() {
                let r = ui.scope(|ui| t.ui(s, ui)).response.rect;
                sims.push(r);
            }
            if let Some(w) = sims.iter().map(|r| r.width()).reduce(|a, b| a + b) {
                ui.ctx().data_mut(|d| d.insert_temp(sim_width_id(), w));
            }
            for r in sims.into_iter().rev() {
                fit.items.push(("Simulate".into(), r));
            }
            fit.items.push(("Drive here".into(), drive_rect));
            fit.items.push(("Test race".into(), test_rect));
        });
    });
    ui.ctx().data_mut(|d| d.insert_temp(fit_id(), fit));
}

/// What the hint line says for `tool`.
pub fn tool_hint(tool: Tool, r: &Races) -> &'static str {
    match tool {
        Tool::Select => "Click a prop or a race point to select it · Ctrl+click: add to the selection",
        Tool::Move => "Drag an arrow to move the selection · Shift+drag: copy · Alt: no snapping",
        Tool::Rotate => {
            "Drag the ring to turn the selection · Alt: turn freely · right-click a prop: Rotate 90°, Face camera"
        }
        _ => r.hint(),
    }
}

/// The hint line's words: the tool in force, what a click does, how to
/// get back to Select (a race tool armed) or clear the selection, how to
/// fly.
pub fn hint_job(s: &EditorState, p: &Props, r: &Races) -> egui::text::LayoutJob {
    hint_job_in(s, p, r, false)
}

/// [`hint_job`] for the view shown: over the top view (`plan`) the end names the overview's own mouse controls
/// instead of how to fly.
pub fn hint_job_in(s: &EditorState, p: &Props, r: &Races, plan: bool) -> egui::text::LayoutJob {
    let tool = current_tool(s, p, r);
    let small = egui::FontId::proportional(13.0);
    let mut job = egui::text::LayoutJob::default();
    // A collision drawing tool names itself and what its clicks do; Esc steps back one level at a time.
    let drawing = s.draw.as_ref();
    let name = match drawing {
        Some(super::collision::Draw::Wall { .. }) => "Draw wall".to_string(),
        Some(super::collision::Draw::Ground { .. }) => "Draw ground".to_string(),
        None => format!("{} tool", tool.name()),
    };
    job.append(&name, 0.0, egui::TextFormat::simple(theme::font(theme::LABEL, 14.0), theme::lime()));
    let chip = hint_chip;
    // The way back first: it must show on the narrowest window.
    if let Some(d) = drawing {
        chip(&mut job, "Esc", 14.0);
        job.append(&format!(" {}", super::collision::esc_words(d)), 0.0, egui::TextFormat::simple(small.clone(), theme::ink()));
    } else if tool.armed() {
        chip(&mut job, "Esc", 14.0);
        job.append(" back to Select", 0.0, egui::TextFormat::simple(small.clone(), theme::ink()));
    }
    let words = match (drawing, tool, s.focus) {
        (Some(d), _, _) => super::collision::draw_words(d),
        (None, Tool::Select, _) if s.wall.is_some() => "A wall is selected · Del removes it · click another wall to pick it",
        (None, Tool::Select, Focus::World) => "Click a prop to select it · Ctrl+click: add to the selection",
        (None, Tool::Select, Focus::Races) => "Click a race point to select it",
        _ => tool_hint(tool, r),
    };
    job.append(words, 18.0, egui::TextFormat::simple(small.clone(), theme::ink()));
    // Turning a prop: the key and the menu, where the tool is not Rotate.
    if matches!(tool, Tool::Select | Tool::Move) && s.mode == Mode::Props && !s.selection.is_empty() {
        chip(&mut job, "E", 18.0);
        job.append(
            " rotate · right-click: Rotate 90°, Face camera",
            0.0,
            egui::TextFormat::simple(small.clone(), theme::ink()),
        );
    }
    if drawing.is_none() && !tool.armed() && (!s.selection.is_empty() || s.point.is_some() || s.wall.is_some()) {
        chip(&mut job, "Esc", 18.0);
        job.append(" clear the selection", 0.0, egui::TextFormat::simple(small.clone(), theme::ink()));
    }
    // The snap mode in force, in the same place for every tool that places or moves.
    if matches!(tool, Tool::Move | Tool::Rotate) || tool.armed() {
        let snap = if tool.armed() {
            if r.snap { crate::studio::words::SNAP_ROAD.to_string() } else { crate::studio::words::SNAP_OFF.to_string() }
        } else if p.snap {
            crate::studio::words::snap_steps(&trim_num(p.snap_metres), &trim_num(p.snap_degrees))
        } else {
            crate::studio::words::SNAP_OFF.to_string()
        };
        job.append(&snap, 18.0, egui::TextFormat::simple(small.clone(), theme::dim()));
    }
    if plan {
        chip(&mut job, "RMB", 18.0);
        job.append(" drag pan · wheel zoom · ", 0.0, egui::TextFormat::simple(small.clone(), theme::ink()));
        chip(&mut job, "Home", 0.0);
        job.append(" fit · double-click: fly there", 0.0, egui::TextFormat::simple(small, theme::ink()));
        return job;
    }
    chip(&mut job, "RMB", 18.0);
    job.append(" + ", 0.0, egui::TextFormat::simple(small.clone(), theme::dim()));
    chip(&mut job, "WASD", 0.0);
    job.append(" fly", 0.0, egui::TextFormat::simple(small, theme::ink()));
    job
}

/// A key chip in a hint-line job (`Esc`, `RMB`).
fn hint_chip(job: &mut egui::text::LayoutJob, key: &str, lead: f32) {
    let mut f = egui::TextFormat::simple(egui::FontId::proportional(12.0), theme::ink());
    f.background = theme::rgb(theme::tokens().page);
    job.append(&format!(" {key} "), lead, f);
}

/// The game-window strip's words, set like the hint line: "Game window
/// open", what runs, `Esc` in the game ends it (`modes`).
pub fn game_job(what: &str) -> egui::text::LayoutJob {
    let small = egui::FontId::proportional(13.0);
    let mut job = egui::text::LayoutJob::default();
    let lead = egui::TextFormat::simple(theme::font(theme::LABEL, 14.0), theme::lime());
    job.append(super::modes::STRIP_LEAD, 0.0, lead);
    job.append(what, 18.0, egui::TextFormat::simple(small.clone(), theme::ink()));
    hint_chip(&mut job, "Esc", 18.0);
    job.append(&format!(" {}", super::modes::STRIP_END), 0.0, egui::TextFormat::simple(small, theme::ink()));
    job
}

/// The hint line under the view ([`hint_job`]), with "hide" at its
/// right (View > Hint line brings it back).
pub fn hint_line(s: &EditorState, p: &Props, r: &Races, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let job = hint_job_in(s, p, r, super::viewport::top_view(ui.ctx()).is_some());
    ui.horizontal_centered(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("hide").on_hover_text("Hide the hint line (View > Hint line)").clicked() {
                out.push(Action::Hint(false));
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(job).truncate());
            });
        });
    });
}

/// The status bar: save state, issues, the mod and region, the camera,
/// the frame time (not in a `--shot`: the picture must not change between
/// runs), and the last message in the room left.
pub fn status(app: &App, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        let Some(s) = app.state.as_ref() else {
            ui.weak("No map mod open");
            return;
        };
        let (text, color) = if s.session.dirty { ("Unsaved changes", theme::warn()) } else { ("Saved", theme::lime()) };
        ui.colored_label(color, egui::RichText::new(text).font(theme::font(theme::SEMI, 13.0)));
        if !app.save_note.is_empty() {
            ui.colored_label(theme::error(), format!("· {}", app.save_note));
        }
        ui.separator();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !app.fixed_clock {
                ui.weak(egui::RichText::new(format!("UI {:.1} ms", app.frame_ms)).size(12.0));
                ui.separator();
            }
            let wide = ui.available_width() > 700.0;
            if wide {
                let c = s.camera;
                let place = s.place_name().map(|n| format!("{n}  ")).unwrap_or_default();
                ui.weak(
                    egui::RichText::new(format!(
                        "{place}{:.0}, {:.0}, {:.0}  heading {:.0}°  FOV {:.0}°",
                        c.position.x,
                        c.position.y,
                        c.position.z,
                        c.yaw.to_degrees().rem_euclid(360.0),
                        c.fov_y.to_degrees()
                    ))
                    .size(12.0),
                );
                ui.separator();
            }
            let e = s.session.issues.iter().filter(|i| i.level == Level::Error).count();
            let w = s.session.issues.iter().filter(|i| i.level == Level::Warning).count();
            ui.colored_label(
                if w > 0 { theme::warn() } else { theme::dim() },
                egui::RichText::new(format!("{w} {}", if w == 1 { "warning" } else { "warnings" })).size(12.0),
            );
            ui.colored_label(
                if e > 0 { theme::error() } else { theme::dim() },
                egui::RichText::new(format!("{e} {}", if e == 1 { "error" } else { "errors" })).size(12.0),
            );
            ui.separator();
            if let Some(g) = super::testplay::link_words(ui.ctx()) {
                let on = g == super::testplay::GAME_CONNECTED;
                ui.colored_label(if on { theme::lime() } else { theme::warn() }, egui::RichText::new(g).size(12.0));
                ui.separator();
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(egui::RichText::new(&s.status).size(13.0)).truncate());
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hint_names_the_overview_controls_over_the_top_view() {
        let s = crate::editor::props::tests::state();
        let (p, r) = (Props::default(), Races::default());
        let fly = hint_job_in(&s, &p, &r, false).text;
        let plan = hint_job_in(&s, &p, &r, true).text;
        assert!(fly.contains("fly") && !fly.contains("pan"), "{fly}");
        assert!(plan.contains("pan") && plan.contains("zoom") && plan.contains("double-click"), "{plan}");
    }

    #[test]
    fn commands_are_taken_once_and_cleared() {
        let ctx = egui::Context::default();
        post(&ctx, Command::Delete);
        post(&ctx, Command::TestRace);
        assert!(take(&ctx, Command::Delete));
        assert!(!take(&ctx, Command::Delete));
        assert_eq!(clear_commands(&ctx), vec![Command::TestRace]);
        assert!(!take(&ctx, Command::TestRace));
        assert!(clear_commands(&ctx).is_empty());
    }

    /// A world with one cone to select.
    fn world() -> crate::world::World {
        let mut w = crate::world::World::flat(500.0);
        w.solids = vec![crate::formats::solid::Solid {
            name: "XC_CONE_01".into(),
            hash: crate::formats::hash::bin_hash("XC_CONE_01"),
            bbox_min: [-0.5, -0.5, 0.0],
            bbox_max: [0.5, 0.5, 1.0],
            textures: Vec::new(),
            vertices: Vec::new(),
            normals: Vec::new(),
            light_materials: Vec::new(),
            indices: Vec::new(),
            materials: Vec::new(),
            mesh_flags: 0,
            markers: Vec::new(),
            light_markers: Vec::new(),
        }];
        w.placements = vec![crate::world::Placement {
            solid: 0,
            low_solid: None,
            mid_solid: None,
            transform: glam::Mat4::from_translation(glam::Vec3::new(10.0, 0.0, 0.0)),
            section: 618,
            flags: 0,
            cull_radius: 5.0,
        }];
        w
    }

    /// No silent no-ops: with a prop or a race point selected and in both
    /// views, every command the menu offers (not disabled with a reason) is taken by a part, and
    /// every command is offered somewhere.
    #[test]
    fn every_command_offered_is_taken() {
        use crate::editor::state::RacePoint;
        use crate::tools::map::Spot;
        use crate::tools::map::edit::RacePart;

        let dir = crate::testkit::Scratch::new("editor-commands");
        // Test race and Drive here save the mod first: a maps "folder" that
        // is a file stops them there, so no game is started.
        let not_a_folder = dir.path().join("not-a-folder");
        std::fs::write(&not_a_folder, "").unwrap();
        let mut app = App::new(
            dir.path().join("game"),
            dir.path().join("maps"),
            Box::new(|_: &std::path::Path, _: &str| Ok(world())),
        );
        app.new_mod("commands", "Commands", "L4RA").unwrap();
        let mut race = crate::editor::validate::tests::sprint(&[(20.0, 0.0), (200.0, 0.0)]);
        race.start = Some(Spot { position: glam::Vec3::ZERO, heading: 0.0 });
        let raw = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0))),
            ..Default::default()
        };
        let _ = app.frame(raw());
        let mut offered = std::collections::HashSet::new();
        for mode in [Mode::Props, Mode::Races] {
            for view in ["3D", "top"] {
                for c in Command::ALL {
                    let s = app.state.as_mut().unwrap();
                    s.session.map.races = vec![race.clone()];
                    (s.mode, s.race) = (mode, Some(0));
                    let _ = view;
                    s.selection = vec![0];
                    use crate::tools::map::ops::CollisionKey as Target;
                    s.wall = (c == Command::RemoveWall).then(|| crate::editor::collision::Picked {
                        key: Target::Drawn("wall-1".into()),
                        what: "Drawn wall wall-1".into(),
                        outline: vec![glam::Vec3::ZERO, glam::Vec3::X],
                        z: (0.0, 2.0),
                        surface: None,
                        owner: String::new(),
                    });
                    s.point = Some(RacePoint { part: RacePart::Checkpoint, index: 0 });
                    s.maps = match c {
                        Command::TestRace | Command::DriveHere | Command::RecordDrive | Command::TestWithout => not_a_folder.clone(),
                        _ => dir.path().join("maps"),
                    };
                    if c.unavailable(s).is_some() {
                        continue;
                    }
                    offered.insert(c);
                    post(&app.ctx, c);
                    let _ = app.frame(raw());
                    assert!(app.untaken.is_empty(), "{c:?} not taken in {mode:?} / {view:?}");
                    let s = app.state.as_ref().unwrap();
                    if matches!(c, Command::TestRace | Command::DriveHere | Command::RecordDrive | Command::TestWithout) {
                        assert!(
                            s.status.starts_with("The run copy was not written")
                                || s.status.starts_with("Cannot test")
                                || s.status.starts_with("Nothing to leave out"),
                            "{c:?}: {}",
                            s.status
                        );
                    }
                }
            }
        }
        let never: Vec<_> = Command::ALL.iter().filter(|c| !offered.contains(c)).collect();
        assert!(never.is_empty(), "never offered: {never:?}");
        // Disabled ones say why.
        let s = app.state.as_mut().unwrap();
        s.mode = Mode::Props;
        s.selection.clear();
        assert_eq!(Command::Duplicate.unavailable(s), Some("Select a prop first"));
        s.mode = Mode::Races;
        assert_eq!(Command::DropToGround.unavailable(s), Some("Select a prop first"));
    }

    fn key(k: Key) -> egui::Event {
        egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
    }

    fn frame_with(app: &mut App, events: Vec<egui::Event>) {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 960.0))),
            events,
            ..Default::default()
        };
        let _ = app.frame(raw);
    }

    fn tool(app: &App) -> Tool {
        current_tool(app.state.as_ref().unwrap(), &app.panels.props.borrow(), &app.panels.races.borrow())
    }

    /// L steps the collision layer Off, Invisible barriers, All collision, Off (hidden by default).
    #[test]
    fn l_steps_the_collision_layer() {
        use super::super::collision::Show;
        let dir = crate::testkit::Scratch::new("editor-collision-key");
        let mut app = App::new(
            dir.path().join("game"),
            dir.path().join("maps"),
            Box::new(|_: &std::path::Path, _: &str| Ok(world())),
        );
        app.new_mod("coll", "Coll", "L4RA").unwrap();
        frame_with(&mut app, Vec::new());
        assert_eq!(app.state.as_ref().unwrap().collision, Show::Off);
        for want in [Show::Invisible, Show::All, Show::Off] {
            frame_with(&mut app, vec![key(Key::L)]);
            assert_eq!(app.state.as_ref().unwrap().collision, want);
        }
    }

    /// The tools' keys pick tools; a race tool needs a race and stays armed
    /// until Esc; Tab is not the editor's (keyboard focus); a prop picked
    /// disarms the race tool.
    #[test]
    fn tool_keys_esc_and_no_tab() {
        let dir = crate::testkit::Scratch::new("editor-tools");
        let mut app = App::new(
            dir.path().join("game"),
            dir.path().join("maps"),
            Box::new(|_: &std::path::Path, _: &str| Ok(world())),
        );
        app.new_mod("tools", "Tools", "L4RA").unwrap();
        frame_with(&mut app, Vec::new());
        assert_eq!(tool(&app), Tool::Select, "nothing armed at the start");
        frame_with(&mut app, vec![key(Key::C)]);
        assert_eq!(tool(&app), Tool::Select, "no race: the race tools stay off");
        assert_eq!(
            Tool::Checkpoint.unavailable(app.state.as_ref().unwrap()),
            Some("Make a race first: Race > New race")
        );
        let s = app.state.as_mut().unwrap();
        s.session.map.races = vec![crate::editor::validate::tests::sprint(&[(20.0, 0.0), (200.0, 0.0)])];
        s.race = Some(0);
        for (k, t) in [(Key::W, Tool::Move), (Key::E, Tool::Rotate), (Key::Q, Tool::Select), (Key::C, Tool::Checkpoint)]
        {
            frame_with(&mut app, vec![key(k)]);
            assert_eq!(tool(&app), t, "{k:?}");
        }
        assert_eq!(app.state.as_ref().unwrap().mode, Mode::Races);
        let s = app.state.as_ref().unwrap();
        let hint = hint_job(s, &app.panels.props.borrow(), &app.panels.races.borrow()).text;
        assert!(
            hint.starts_with("Checkpoint tool") && hint.contains("Esc") && hint.contains("back to Select"),
            "{hint}"
        );
        for (k, t) in [(Key::S, Tool::Start), (Key::X, Tool::Finish), (Key::G, Tool::Grid)] {
            frame_with(&mut app, vec![key(k)]);
            assert_eq!(tool(&app), t, "{k:?}");
        }
        frame_with(&mut app, vec![key(Key::Escape)]);
        assert_eq!(tool(&app), Tool::Select, "Esc: back to Select");
        let s = app.state.as_ref().unwrap();
        let hint = hint_job(s, &app.panels.props.borrow(), &app.panels.races.borrow()).text;
        assert!(hint.starts_with("Select tool") && !hint.contains("back to Select"), "{hint}");
        // Armed again, then a prop selected (the tree, the view): disarmed.
        frame_with(&mut app, vec![key(Key::C)]);
        assert_eq!(tool(&app), Tool::Checkpoint);
        let s = app.state.as_mut().unwrap();
        (s.selection, s.mode) = (vec![0], Mode::Props);
        frame_with(&mut app, Vec::new());
        assert_eq!(tool(&app), Tool::Select);
        // Tab moves the keyboard focus only (named, so the key scan below
        // finds no Tab read).
        let s = app.state.as_mut().unwrap();
        s.point = None;
        s.mode = Mode::Races;
        frame_with(&mut app, vec![key(Key::C)]);
        frame_with(&mut app, vec![key(Key::from_name("Tab").unwrap())]);
        assert_eq!(tool(&app), Tool::Checkpoint, "Tab changes no tool");
        assert_eq!(app.state.as_ref().unwrap().mode, Mode::Races, "nor what is selected");
    }

    /// Every `Key::…` the editor reads is in the key table (or is a tool's
    /// key), so Help > Controls lists it; Tab is read nowhere.
    #[test]
    fn controls_list_every_key_read() {
        let mut table: std::collections::HashSet<String> =
            KEYS.iter().flat_map(|b| b.reads.iter().map(|k| format!("{k:?}"))).collect();
        table.extend(Tool::ALL.iter().map(|t| format!("{:?}", t.key())));
        let mut files = vec![std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src/editor"))];
        let mut read = std::collections::BTreeMap::<String, String>::new();
        while let Some(p) = files.pop() {
            if p.is_dir() {
                files.extend(std::fs::read_dir(&p).unwrap().flatten().map(|e| e.path()));
                continue;
            }
            if p.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap();
            for (at, _) in text.match_indices("Key::") {
                let name: String =
                    text[at + 5..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                if name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                    read.entry(name).or_insert_with(|| p.display().to_string());
                }
            }
        }
        assert!(read.len() > 20, "the scan found the keys: {read:?}");
        assert!(!read.contains_key("Tab"), "Tab is read in {:?}", read.get("Tab"));
        let missing: Vec<_> = read.iter().filter(|(k, _)| !table.contains(*k)).collect();
        assert!(missing.is_empty(), "keys read but not in the key table: {missing:?}");
        // The dialog lists every row and every tool.
        let rows: Vec<(String, &str)> = controls().into_iter().flat_map(|(_, r)| r).collect();
        for t in Tool::ALL {
            assert!(rows.iter().any(|(k, w)| k == t.key().name() && *w == t.what()), "{t:?}");
        }
        for b in KEYS {
            assert!(rows.iter().any(|(k, w)| k == b.keys && *w == b.what), "{}", b.keys);
        }
    }
}

//! Keyboard and pad navigation (docs/modding/studio.md, "Keyboard and pad"): the seven areas and their order, what
//! each key and pad button does, the key table that Help ▸ Controls and the legend row are made from, and when
//! the legend row shows.
//!
//! The pad's buttons do the same in Studio as in the launcher and the game's menus; the keyboard uses Studio's
//! key set (ob-look `KeySet::Studio`), since single letters are the tools' keys and typing here.

use ob_look::legends::{Btn, InputKind};

use super::layout::{Dock, Layout};
use super::workspace::Workspace;

/// The areas, always in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Area {
    Rail,
    Address,
    Toolbar,
    Left,
    /// The view, or the page (Home, Mod, Settings).
    View,
    Right,
    Bottom,
}

impl Area {
    pub const ALL: [Area; 7] =
        [Area::Rail, Area::Address, Area::Toolbar, Area::Left, Area::View, Area::Right, Area::Bottom];

    pub fn name(self) -> &'static str {
        match self {
            Area::Rail => "Rail",
            Area::Address => "Address",
            Area::Toolbar => "Toolbar",
            Area::Left => "Left panel",
            Area::View => "View",
            Area::Right => "Right dock",
            Area::Bottom => "Bottom dock",
        }
    }
}

/// The areas a layout shows, in order: a folded panel is skipped; a drawer counts as its dock.
pub fn shown(l: &Layout) -> Vec<Area> {
    Area::ALL
        .into_iter()
        .filter(|a| match a {
            Area::Rail | Area::Address | Area::View => true,
            Area::Toolbar => l.toolbar.is_some(),
            Area::Left => matches!(l.left_dock, Dock::Open(_)),
            Area::Right => matches!(l.right_dock, Dock::Open(_) | Dock::Drawer { .. }),
            Area::Bottom => matches!(l.bottom_dock, Dock::Open(_)),
        })
        .collect()
}

/// What the player asked for, by key or by pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Enter / A.
    Select,
    /// Esc / B: one level out.
    Back,
    /// Shift+F10, the Menu key / X: the focused thing's menu.
    Menu,
    /// Ctrl+K / Y: Find.
    Find,
    /// Ctrl+PgUp / Ctrl+PgDn / LB RB.
    PrevWorkspace,
    NextWorkspace,
    /// Ctrl+number.
    Workspace(Workspace),
    /// F6 / Shift+F6 / RT LT.
    NextArea,
    PrevArea,
    /// Tab, Shift+Tab, arrows / D-pad.
    Move,
    /// ← → on a value / D-pad ◀ ▶.
    Change,
    /// F5 / Menu button: test.
    Test,
    /// Alt+← / View button; Alt+→.
    LastPlace,
    NextPlace,
    /// Ctrl+L: the address as text.
    EditAddress,
    /// Ctrl+Shift+C: copy the address.
    CopyAddress,
    Undo,
    Redo,
    /// Ctrl+N: the New mod dialog.
    NewMod,
    /// F1: the tutorial's step (starts the tutorial when it is not running).
    Tutorial,
}

/// One line of the key table.
#[derive(Clone, Copy, Debug)]
pub struct KeyRow {
    pub command: Command,
    /// The keys, as written on key caps (several ways joined with " / ").
    pub keys: &'static str,
    /// The pad role (None: keyboard only).
    pub pad: Option<Btn>,
    /// What it does, in the legend's word and Help's words.
    pub word: &'static str,
    pub help: &'static str,
}

/// The key table: Help ▸ Controls and the legend row are made from it.
pub const KEYS: [KeyRow; 21] = [
    KeyRow {
        command: Command::Select,
        keys: "Enter",
        pad: Some(Btn::Confirm),
        word: "Select",
        help: "Take the focused item",
    },
    KeyRow {
        command: Command::Back,
        keys: "Esc",
        pad: Some(Btn::Back),
        word: "Back",
        help: "One level out: menu, field, row, area, then the page's back link",
    },
    KeyRow {
        command: Command::Menu,
        keys: "Shift+F10 / Menu key",
        pad: Some(Btn::X),
        word: "Menu",
        help: "The focused thing's menu: undo, redo, test here, reset (on Changes), copy address",
    },
    KeyRow {
        command: Command::Find,
        keys: "Ctrl+K",
        pad: Some(Btn::Y),
        word: "Find",
        help: "Find names, commands, places and pasted addresses",
    },
    KeyRow {
        command: Command::NextWorkspace,
        keys: "Ctrl+PgUp / Ctrl+PgDn",
        pad: Some(Btn::Pages),
        word: "Workspace",
        help: "The workspace above or below on the rail",
    },
    KeyRow {
        command: Command::Workspace(Workspace::Home),
        keys: "Ctrl+1 … Ctrl+0, Ctrl+,",
        pad: None,
        word: "Workspace",
        help: "A workspace by its number on the rail; Ctrl+, opens Settings",
    },
    KeyRow {
        command: Command::NextArea,
        keys: "F6 / Shift+F6",
        pad: Some(Btn::Area),
        word: "Area",
        help: "The next or previous area: rail, address, toolbar, left panel, view, right dock, bottom dock",
    },
    KeyRow {
        command: Command::Move,
        keys: "Tab / arrows",
        pad: Some(Btn::Move),
        word: "Move",
        help: "Between the controls of an area, in reading order",
    },
    KeyRow {
        command: Command::Change,
        keys: "← →",
        pad: Some(Btn::Change),
        word: "Change",
        help: "Change a value by its nudge step (Shift: ten steps)",
    },
    KeyRow {
        command: Command::Test,
        keys: "F5",
        pad: Some(Btn::Start),
        word: "Test",
        help: "Test what the Test button names",
    },
    KeyRow {
        command: Command::LastPlace,
        keys: "Alt+←",
        pad: Some(Btn::LastPlace),
        word: "Last place",
        help: "Back to the last place (forward: Alt+→)",
    },
    KeyRow { command: Command::NextPlace, keys: "Alt+→", pad: None, word: "Next place", help: "Forward again" },
    KeyRow {
        command: Command::EditAddress,
        keys: "Ctrl+L",
        pad: None,
        word: "Address",
        help: "Show the address to edit or paste; Enter goes there",
    },
    KeyRow {
        command: Command::CopyAddress,
        keys: "Ctrl+Shift+C",
        pad: None,
        word: "Copy address",
        help: "Copy the focused item's address",
    },
    KeyRow {
        command: Command::Workspace(Workspace::Changes),
        keys: "Ctrl+D",
        pad: None,
        word: "Changes",
        help: "What the open mod changes, value by value, and which mod wins",
    },
    KeyRow {
        command: Command::Workspace(Workspace::Data),
        keys: "Ctrl+G",
        pad: None,
        word: "Data",
        help: "Your props and races as a table: sort, search, edit a cell",
    },
    KeyRow {
        command: Command::NewMod,
        keys: "Ctrl+N",
        pad: None,
        word: "New mod",
        help: "Make a new map mod (Home: Empty map mod)",
    },
    KeyRow {
        command: Command::Tutorial,
        keys: "F1",
        pad: None,
        word: "Tutorial",
        help: "Do the tutorial's step (Home's card holds it for the pad); starts the tutorial",
    },
    KeyRow {
        command: Command::Undo,
        keys: "Ctrl+Z",
        pad: Some(Btn::DeckGrips),
        word: "Undo",
        help: "Take back the last step (Deck: L4)",
    },
    KeyRow {
        command: Command::Redo,
        keys: "Ctrl+Y",
        pad: Some(Btn::DeckGrips),
        word: "Redo",
        help: "Make it again (Deck: R4)",
    },
    KeyRow {
        command: Command::Move,
        keys: "RMB + WASD",
        pad: Some(Btn::Sticks),
        word: "Fly",
        help: "Move and look in the 3D view while it has focus",
    },
];

/// A pad's buttons (the names of an Xbox-style pad; a PlayStation-style pad and the Deck have the same places).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pad {
    A,
    B,
    X,
    Y,
    LB,
    RB,
    LT,
    RT,
    Up,
    Down,
    Left,
    Right,
    Start,
    View,
    L4,
    R4,
}

/// What a pad button does (single presses only: a chord is never the only way).
pub fn pad_command(b: Pad) -> Command {
    match b {
        Pad::A => Command::Select,
        Pad::B => Command::Back,
        Pad::X => Command::Menu,
        Pad::Y => Command::Find,
        Pad::LB => Command::PrevWorkspace,
        Pad::RB => Command::NextWorkspace,
        Pad::LT => Command::PrevArea,
        Pad::RT => Command::NextArea,
        Pad::Up | Pad::Down => Command::Move,
        Pad::Left | Pad::Right => Command::Change,
        Pad::Start => Command::Test,
        Pad::View => Command::LastPlace,
        Pad::L4 => Command::Undo,
        Pad::R4 => Command::Redo,
    }
}

/// A key press with its modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub key: egui::Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

/// What a key press does in Studio's shell (None: it is the focused control's).
pub fn key_command(k: Key) -> Option<Command> {
    use egui::Key as K;
    Some(match (k.key, k.ctrl, k.shift, k.alt) {
        (K::F6, false, false, false) => Command::NextArea,
        (K::F6, false, true, false) => Command::PrevArea,
        (K::F10, false, true, false) | (K::Menu, false, false, false) => Command::Menu,
        (K::F5, false, false, false) => Command::Test,
        (K::F1, false, false, false) => Command::Tutorial,
        (K::K, true, false, false) => Command::Find,
        (K::L, true, false, false) => Command::EditAddress,
        (K::C, true, true, false) => Command::CopyAddress,
        (K::PageUp, true, false, false) => Command::PrevWorkspace,
        (K::PageDown, true, false, false) => Command::NextWorkspace,
        (K::ArrowLeft, false, false, true) => Command::LastPlace,
        (K::ArrowRight, false, false, true) => Command::NextPlace,
        (K::Z, true, false, false) => Command::Undo,
        (K::N, true, false, false) => Command::NewMod,
        (K::D, true, false, false) => Command::Workspace(Workspace::Changes),
        (K::G, true, false, false) => Command::Workspace(Workspace::Data),
        (K::Y, true, false, false) => Command::Redo,
        (K::Escape, false, false, false) => Command::Back,
        (K::Enter, false, false, false) => Command::Select,
        (K::Tab, false, _, false) => Command::Move,
        (K::ArrowUp | K::ArrowDown, false, _, false) => Command::Move,
        (K::ArrowLeft | K::ArrowRight, false, _, false) => Command::Change,
        (key, true, false, false) => {
            let name = match key {
                K::Num0 => "0",
                K::Num1 => "1",
                K::Num2 => "2",
                K::Num3 => "3",
                K::Num4 => "4",
                K::Num5 => "5",
                K::Comma => ",",
                _ => return None,
            };
            Command::Workspace(Workspace::for_ctrl_key(name)?)
        }
        _ => return None,
    })
}

/// How deep the focus is inside an area (Esc / B goes one level out).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// The area as a whole.
    Area,
    /// A row or control in it.
    Row,
    /// A field being edited.
    Field,
    /// A menu over it.
    Menu,
}

/// The focus and the input state of one window.
#[derive(Clone, Debug)]
pub struct Nav {
    pub workspace: Workspace,
    pub area: Area,
    pub level: Level,
    /// The right dock's drawer is open (opens when the right dock gets focus while it is a drawer).
    pub drawer_open: bool,
    /// The Find list is open.
    pub find_open: bool,
    /// The address bar shows the address as text (Ctrl+L).
    pub address_text: bool,
    /// The device used last; the legend row shows while it is not the mouse.
    pub input: InputKind,
    /// The page's back link was taken (Esc at the area level on a page with one); the caller acts on it.
    pub crumb_back: bool,
}

impl Nav {
    pub fn new(workspace: Workspace) -> Nav {
        Nav {
            workspace,
            area: Area::View,
            level: Level::Area,
            drawer_open: false,
            find_open: false,
            address_text: false,
            input: InputKind::Mouse,
            crumb_back: false,
        }
    }

    /// Whether the legend row shows: hidden while the mouse is used.
    pub fn legend(&self) -> bool {
        self.input != InputKind::Mouse
    }

    /// The mouse moved or clicked.
    pub fn mouse(&mut self) {
        self.input = InputKind::Mouse;
    }

    fn step_area(&mut self, l: &Layout, step: i32) {
        let areas = shown(l);
        let i = areas.iter().position(|a| *a == self.area).map_or(0, |i| i as i32);
        let n = areas.len() as i32;
        self.area = areas[(i + step).rem_euclid(n) as usize];
        self.level = Level::Area;
        // A drawer opens when its dock gets focus and closes when focus leaves it.
        self.drawer_open = self.area == Area::Right && matches!(l.right_dock, Dock::Drawer { .. });
    }

    /// Run `c`, from a key (`pad`: None) or a pad of that kind, on the window laid out as `l`. Returns the
    /// workspace to go to when it changes.
    pub fn run(&mut self, c: Command, pad: Option<InputKind>, l: &Layout) -> Option<Workspace> {
        match pad {
            Some(kind) => self.input = kind,
            // Keys that move focus bring the legend; others (typing) leave the device as it was.
            None if !matches!(c, Command::Undo | Command::Redo | Command::CopyAddress) => {
                self.input = InputKind::Keyboard;
            }
            None => {}
        }
        self.crumb_back = false;
        match c {
            Command::NextArea => self.step_area(l, 1),
            Command::PrevArea => self.step_area(l, -1),
            Command::Back => self.back(),
            Command::Select => {
                if self.area == Area::Address && self.level == Level::Field {
                    self.address_text = false;
                    self.level = Level::Area;
                } else {
                    self.level = match self.level {
                        Level::Area => Level::Row,
                        _ => Level::Field,
                    };
                }
            }
            Command::Move | Command::Change => self.level = self.level.max(Level::Row),
            Command::Menu => self.level = Level::Menu,
            Command::Find => self.find_open = true,
            Command::EditAddress => {
                self.area = Area::Address;
                self.level = Level::Field;
                self.address_text = true;
                self.drawer_open = false;
            }
            Command::PrevWorkspace => return Some(self.go(self.workspace.step(-1))),
            Command::NextWorkspace => return Some(self.go(self.workspace.step(1))),
            Command::Workspace(w) => return Some(self.go(w)),
            _ => {}
        }
        None
    }

    fn go(&mut self, w: Workspace) -> Workspace {
        self.workspace = w;
        self.area = Area::View;
        self.level = Level::Area;
        self.drawer_open = false;
        self.find_open = false;
        self.address_text = false;
        w
    }

    /// Esc / B: one level out, never past the area into another workspace.
    fn back(&mut self) {
        if self.find_open {
            self.find_open = false;
            return;
        }
        if self.address_text {
            self.address_text = false;
            self.level = Level::Area;
            return;
        }
        match self.level {
            Level::Menu => self.level = Level::Row,
            Level::Field => self.level = Level::Row,
            Level::Row => self.level = Level::Area,
            Level::Area if self.drawer_open => {
                self.drawer_open = false;
                self.area = Area::View;
            }
            Level::Area => self.crumb_back = true,
        }
    }

    /// The legend row's roles on the focused area (five or six), with their words.
    pub fn legend_roles(&self) -> Vec<(Btn, &'static str)> {
        let mut v = vec![(Btn::Confirm, "Select"), (Btn::Back, "Back")];
        if self.input.is_pad() {
            v.push((Btn::X, "Menu"));
        }
        v.push((Btn::Y, "Find"));
        v.push((Btn::Pages, "Workspace"));
        v.push((Btn::Area, "Area"));
        if self.input.is_pad() && self.workspace.kind() == super::workspace::Kind::Tool {
            v.push((Btn::Start, "Test"));
        }
        if self.input.is_pad() {
            v.push((Btn::LastPlace, "Last place"));
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::super::layout::{Want, layout};
    use super::super::workspace::Kind;
    use super::*;

    fn world(w: f32, h: f32) -> Layout {
        layout(w, h, &Want::new(Kind::Tool))
    }

    #[test]
    fn f6_walks_the_areas_in_order_and_wraps() {
        let l = world(1536.0, 864.0);
        assert_eq!(shown(&l), Area::ALL);
        let mut n = Nav::new(Workspace::World);
        n.area = Area::Rail;
        let mut seen = vec![n.area];
        for _ in 0..7 {
            n.run(Command::NextArea, None, &l);
            seen.push(n.area);
        }
        assert_eq!(&seen[..7], Area::ALL);
        assert_eq!(seen[7], Area::Rail, "wraps");
        n.run(Command::PrevArea, None, &l);
        assert_eq!(n.area, Area::Bottom);
    }

    #[test]
    fn rt_and_lt_do_what_f6_does() {
        let l = world(1536.0, 864.0);
        let (mut k, mut p) = (Nav::new(Workspace::World), Nav::new(Workspace::World));
        for (key, pad) in [(Command::NextArea, Pad::RT), (Command::NextArea, Pad::RT), (Command::PrevArea, Pad::LT)] {
            k.run(key, None, &l);
            p.run(pad_command(pad), Some(InputKind::Xbox), &l);
            assert_eq!(k.area, p.area);
        }
        // Every pad button is one press; each has a key that does the same.
        for b in [Pad::A, Pad::B, Pad::X, Pad::Y, Pad::LB, Pad::RB, Pad::LT, Pad::RT, Pad::Start, Pad::View] {
            let c = pad_command(b);
            assert!(KEYS.iter().any(|r| r.command == c
                || (matches!(c, Command::PrevWorkspace) && r.command == Command::NextWorkspace)
                || (matches!(c, Command::PrevArea) && r.command == Command::NextArea)));
        }
    }

    #[test]
    fn folded_panels_are_skipped_and_a_drawer_opens_on_focus() {
        // 1024 × 768: the tree folded, the Inspector a drawer.
        let l = world(1024.0, 768.0);
        let areas = shown(&l);
        assert!(!areas.contains(&Area::Left));
        assert!(areas.contains(&Area::Right));
        let mut n = Nav::new(Workspace::World);
        n.run(Command::NextArea, None, &l);
        assert_eq!(n.area, Area::Right);
        assert!(n.drawer_open);
        // Esc closes the drawer and gives the view focus back.
        n.run(Command::Back, None, &l);
        assert!(!n.drawer_open);
        assert_eq!(n.area, Area::View);
        // Leaving the dock by F6 closes it too.
        n.run(Command::NextArea, None, &l);
        n.run(Command::NextArea, None, &l);
        assert!(!n.drawer_open);
        // A page has no toolbar, no panels.
        let page = layout(1536.0, 864.0, &Want::new(Kind::Page));
        assert_eq!(shown(&page), [Area::Rail, Area::Address, Area::View]);
    }

    #[test]
    fn esc_goes_one_level_out_and_never_leaves_the_workspace() {
        let l = world(1536.0, 864.0);
        let mut n = Nav::new(Workspace::World);
        n.area = Area::Right;
        n.run(Command::Select, None, &l);
        n.run(Command::Select, None, &l);
        assert_eq!(n.level, Level::Field);
        n.run(Command::Menu, None, &l);
        assert_eq!(n.level, Level::Menu);
        for want in [Level::Row, Level::Area, Level::Area] {
            n.run(Command::Back, None, &l);
            assert_eq!(n.level, want);
            assert_eq!(n.workspace, Workspace::World);
            assert_eq!(n.area, Area::Right);
        }
        assert!(n.crumb_back, "at the area level Esc is the page's back link");
        // Find closes first.
        n.run(Command::Find, None, &l);
        n.run(Command::Back, None, &l);
        assert!(!n.find_open);
    }

    #[test]
    fn workspaces_by_keys_and_bumpers() {
        let l = world(1536.0, 864.0);
        let mut n = Nav::new(Workspace::Home);
        assert_eq!(n.run(pad_command(Pad::RB), Some(InputKind::Deck), &l), Some(Workspace::World));
        let k = |key, ctrl| key_command(Key { key, ctrl, shift: false, alt: false });
        assert_eq!(k(egui::Key::PageDown, true), Some(Command::NextWorkspace));
        assert_eq!(k(egui::Key::Num3, true), Some(Command::Workspace(Workspace::Races)));
        assert_eq!(k(egui::Key::Num4, true), Some(Command::Workspace(Workspace::Browser)));
        assert_eq!(k(egui::Key::Num0, true), Some(Command::Workspace(Workspace::Mod)));
        assert_eq!(k(egui::Key::Num5, true), Some(Command::Workspace(Workspace::Runs)));
        assert_eq!(k(egui::Key::Num9, true), None, "no workspace there yet");
        // Letters are the tools' keys and typing: Studio takes none without Ctrl.
        assert_eq!(k(egui::Key::Q, false), None);
        assert_eq!(k(egui::Key::E, false), None);
    }

    #[test]
    fn ctrl_l_shows_the_address_and_enter_or_esc_returns() {
        let l = world(1536.0, 864.0);
        let mut n = Nav::new(Workspace::World);
        n.run(Command::EditAddress, None, &l);
        assert!(n.address_text);
        assert_eq!(n.area, Area::Address);
        n.run(Command::Back, None, &l);
        assert!(!n.address_text);
        n.run(Command::EditAddress, None, &l);
        n.run(Command::Select, None, &l);
        assert!(!n.address_text);
    }

    #[test]
    fn the_legend_hides_for_the_mouse_and_shows_on_a_focus_key_or_pad() {
        let l = world(1536.0, 864.0);
        let mut n = Nav::new(Workspace::World);
        assert!(!n.legend());
        n.run(Command::NextArea, None, &l);
        assert!(n.legend());
        assert_eq!(n.input, InputKind::Keyboard);
        n.mouse();
        assert!(!n.legend());
        n.run(Command::Select, Some(InputKind::PlayStation), &l);
        assert!(n.legend());
        let roles = n.legend_roles();
        assert!((5..=8).contains(&roles.len()), "{roles:?}");
        assert!(roles.iter().any(|r| r.0 == Btn::Area));
    }
}

//! Studio's window (docs/modding/studio.md): the menu bar with the address bar and Find, the rail, the docks, the
//! legend row and the status bar around the current workspace, laid out by [`super::layout`] and driven by
//! [`super::nav`]. Draws through egui into any target, so `--shot` renders it off-screen.

use std::path::PathBuf;

use egui::{Align2, Color32, CornerRadius, Pos2, Rect, RichText, Stroke, StrokeKind, UiBuilder, pos2, vec2};
use ob_look::lamp::Mark;
use ob_look::legends::{GlyphSet, InputKind, KeySet, glyphs_in};

use crate::editor::menu;
use crate::editor::theme::{self, SEMI, TITLE, font, rgb, tokens};

use super::address::{Address, CRUMB, Places, STUDIO, Target};
use super::changes::{self, Ask};
use super::draw::{self, Card, Source};
use super::focus::{self, Spot, Spots};
use super::guide::{self, Guide};
use super::home::{self, ModSummary, Saved};
use super::inside::{FindItem, PLACES_SHOWN};
use super::layout::{self, Dock, Layout, Memory, Want};
use super::memory::Memories;
use super::mod_page::{self, Act};
use super::nav::{self, Area, Command, KEYS, Level, Nav, Pad};
use super::new_mod;
use super::words;
use super::workspace::{Kind, Section, Workspace};
use ob_studio::create::Template;

/// The menus, left to right (Test and Mod fold into Go and File on narrow windows).
const MENUS: [&str; 7] = ["File", "Edit", "View", "Go", "Test", "Mod", "Help"];
/// Home's "what each place does" rows, in rail order.
const PLACES: [Workspace; 4] = [Workspace::World, Workspace::Races, Workspace::Changes, Workspace::Mod];
/// Home's "start something new" rows: Empty map mod, the templates, the sample (in that order; as many as fit).
const NEW_ROWS: [Template; 4] = [Template::Empty, Template::Sprint, Template::Circuit, Template::Sample];
/// The sample race pack's folder in your mods.
pub const SAMPLE_FOLDER: &str = "sample-race-pack";
/// Between two menus.
const MENU_GAP: f32 = 16.0;

pub use super::focus::RightTab;

/// The bottom dock's shared tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BottomTab {
    Problems,
    Log,
    Jobs,
    /// The map editor's tab shown (World and Races: Prop browser, Validation, Simulate).
    Editor,
}

/// One Studio window.
pub struct Studio {
    pub ctx: egui::Context,
    /// The mods' folder.
    pub maps: PathBuf,
    pub mods: Vec<ModSummary>,
    /// The open mod (an index into `mods`).
    pub open: Option<usize>,
    pub nav: Nav,
    pub places: Places,
    pub memories: Memories,
    pub right_tab: RightTab,
    pub bottom_tab: BottomTab,
    /// The address bar's text while it shows the address (Ctrl+L).
    pub address_edit: String,
    pub find_text: String,
    /// A verdict on an address that could not be followed.
    pub verdict: Option<String>,
    /// Help ▸ Controls is open.
    pub controls: bool,
    /// The size of the last frame's layout (logical points) and the layout.
    pub last: Option<Layout>,
    /// A fixed clock (`--shot`): busy lamps stand still.
    pub fixed_clock: bool,
    /// Set by File ▸ Quit.
    pub quit: bool,
    /// The map editor, hosted as the World and Races workspaces ([`super::world`]).
    pub editor: Option<super::world::Hosted>,
    /// egui's texture changes not yet painted (as the editor keeps them): a frame that is not painted (a test
    /// without a GPU) loses none, and none is dropped unapplied.
    pub(crate) textures: crate::editor::app::PendingTextures,
    /// The Changes page.
    pub changes: changes::Page,
    /// The Mod page.
    pub mod_page: mod_page::Page,
    /// The New mod dialog, while it is open.
    pub new_mod: Option<new_mod::Dialog>,
    /// Home's focused control (keyboard and pad): an index into [`Studio::home_targets`].
    pub home_focus: usize,
    /// How many of Home's mod rows the last frame had room for.
    home_rows: usize,
    /// How many of Home's "what each place does" rows the last frame had room for.
    home_places: usize,
    /// How many of Home's "start something new" rows (Empty map mod, the templates, the sample) it had room for.
    home_new: usize,
    /// A tutorial, while it runs.
    pub guide: Option<Guide>,
    /// Where the tutorial's card was drawn in the last frame (the layout check counts it as a part).
    pub guide_rect: Option<Rect>,
    /// The shell's own controls and the focus inside each area ([`focus`]).
    pub spots: Spots,
    /// The Find list's focused entry (↑ ↓, the D-pad).
    pub find_focus: usize,
    /// The places list under ← (a right-click on it) is open.
    pub places_menu: bool,
}

/// What one of Home's controls does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HomeAct {
    GoToWorld,
    MakeARace,
    OpenAMod,
    NewMod,
    /// Open one of your mods (an index into `mods`).
    Mod(usize),
    /// Go to a workspace ("what each place does").
    Place(Workspace),
    /// Start a tutorial (START HERE).
    StartTutorial(guide::Tutorial),
    /// A new mod from a template (the New mod dialog with it), or the sample race pack (opened).
    Template(Template),
    /// The tutorial card's main action (the step's button) and its second one.
    GuideMain,
    GuideSecond,
}

impl Studio {
    /// A window on the mods in `maps`, with `folder` open when given.
    pub fn new(maps: PathBuf, folder: Option<&str>) -> Studio {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mods = home::all(&maps);
        let open = folder.and_then(|f| mods.iter().position(|m| m.folder == f));
        let memories = open.map(|i| Memories::load(&mods[i].dir)).unwrap_or_default();
        let mut s = Studio {
            ctx,
            maps: maps.clone(),
            mods,
            open,
            nav: Nav::new(Workspace::Home),
            places: Places::default(),
            memories,
            right_tab: RightTab::Inspector,
            bottom_tab: BottomTab::Problems,
            address_edit: String::new(),
            find_text: String::new(),
            verdict: None,
            controls: false,
            last: None,
            fixed_clock: false,
            quit: false,
            editor: None,
            textures: Default::default(),
            changes: changes::Page::new(maps.clone()),
            mod_page: mod_page::Page::new(maps.clone()),
            new_mod: None,
            home_focus: 0,
            home_rows: 6,
            home_places: 4,
            guide: None,
            guide_rect: None,
            home_new: NEW_ROWS.len(),
            spots: Spots::default(),
            find_focus: 0,
            places_menu: false,
        };
        let here = s.address();
        s.places.go(here);
        s.check_open();
        s
    }

    /// The open mod's checks as the Mod page makes them (the status bar's counts, the Problems tab), read when a
    /// mod opens and again when World or Races is left.
    fn check_open(&mut self) {
        let f = self.open_mod().map(|m| m.folder.clone());
        if f.is_some() {
            self.mod_page.load(f.as_deref());
        }
    }

    pub fn open_mod(&self) -> Option<&ModSummary> {
        self.open.map(|i| &self.mods[i])
    }

    /// The pack id addresses of this window start with.
    fn pack(&self) -> String {
        self.open_mod().map_or_else(|| STUDIO.to_string(), |m| m.folder.clone())
    }

    /// Where the window is, as an address.
    pub fn address(&self) -> Address {
        if let Some(a) = self.editor_address() {
            return a;
        }
        let pack =
            if self.nav.workspace == Workspace::Home || self.open.is_none() { STUDIO.to_string() } else { self.pack() };
        let mut a = Address::view(&pack, self.nav.workspace.view());
        if self.nav.workspace == Workspace::Changes {
            a.target = Target::View { view: "changes".into(), query: self.changes.mode.query() };
        }
        a
    }

    /// Where the window is, as names ("Harbour edits › World").
    pub fn crumbs(&self) -> Vec<String> {
        let mut v = vec![];
        if let Some(m) = self.open_mod() {
            v.push(m.name.clone());
        } else {
            v.push(words::STUDIO.to_string());
        }
        v.push(self.nav.workspace.name().to_string());
        v.extend(self.editor_crumbs());
        if self.nav.workspace == Workspace::Changes {
            v.push(self.changes.mode.name().to_string());
        }
        v
    }

    /// Go to an address (typed, pasted, from Find or from back/forward). A screen of the open mod or of Studio
    /// is followed; anything else gets a verdict instead of a silent jump.
    pub fn go(&mut self, a: &Address) -> Result<(), String> {
        let ok_pack = a.pack == STUDIO || Some(a.pack.as_str()) == self.open_mod().map(|m| m.folder.as_str());
        if !ok_pack {
            if let Some(i) = self.mods.iter().position(|m| m.folder == a.pack) {
                self.switch_mod(i);
            } else {
                return Err(format!("No mod {} in your mods folder", a.pack));
            }
        }
        match &a.target {
            Target::View { view, query } => {
                let w = Workspace::from_view(&format!("@{view}"))
                    .ok_or_else(|| format!("Studio has no screen @{view} yet"))?;
                if w == Workspace::Changes {
                    if self.open.is_none() || a.pack == STUDIO {
                        return Err("Changes shows a mod: open one first".into());
                    }
                    let m = changes::Mode::from_query(query).ok_or_else(|| format!("Changes has no view {a}"))?;
                    self.changes.mode = m;
                }
                self.workspace(w);
                Ok(())
            }
            Target::Item { kind, key, .. } if kind == ob_studio::changes::looks::TEXTURE => {
                // A texture has no view of its own yet: its row on Changes.
                if self.open.is_none() || a.pack == STUDIO {
                    return Err("Changes shows a mod: open one first".into());
                }
                self.workspace(Workspace::Changes);
                if !self.changes.focus_item(kind, &key.join("/")) {
                    return Err(format!("{} does not paint {}", a.pack, key.join("/")));
                }
                Ok(())
            }
            Target::Item { kind, .. } => {
                // The items' own places come with their workspaces (World, Races); until then show the workspace.
                let w = match kind.as_str() {
                    "placement" | "prop" => Workspace::World,
                    "race" => Workspace::Races,
                    _ => return Err(format!("Studio cannot show a {kind} yet")),
                };
                self.workspace(w);
                self.editor_go(a)
            }
        }
    }

    fn switch_mod(&mut self, i: usize) {
        self.open = Some(i);
        self.memories = Memories::load(&self.mods[i].dir);
        if self.nav.workspace == Workspace::Changes {
            self.changes.ensure(Some(&self.mods[i].folder.clone()));
        }
        if self.nav.workspace == Workspace::Mod {
            self.mod_page.load(Some(&self.mods[i].folder.clone()));
        } else {
            self.check_open();
        }
    }

    pub(super) fn workspace(&mut self, w: Workspace) {
        if w == Workspace::Changes {
            // Read again on every visit: World may have changed the mod since.
            match self.open_mod().map(|m| m.folder.clone()) {
                Some(f) => self.changes.load(&f),
                None => self.changes.ensure(None),
            }
        }
        if w == Workspace::Mod || (self.nav.workspace.kind() == Kind::Tool && w.kind() == Kind::Page) {
            let f = self.open_mod().map(|m| m.folder.clone());
            self.mod_page.load(f.as_deref());
        }
        if self.nav.workspace != w {
            self.nav.workspace = w;
            self.nav.area = Area::View;
            self.nav.level = Level::Area;
            self.nav.drawer_open = false;
        }
        let here = self.address();
        self.places.go(here);
    }

    pub(super) fn layout_now(&self) -> Layout {
        let size = self.last.as_ref().map_or(vec2(1536.0, 864.0), |l| l.size);
        layout::layout(size.x, size.y, &self.want())
    }

    pub fn want(&self) -> Want {
        Want {
            legend: self.nav.legend(),
            memory: self.memories.get(self.nav.workspace),
            drawer_open: self.nav.drawer_open,
            ..Want::new(self.nav.workspace.kind())
        }
    }

    /// Run a command from a key (`pad` None) or a pad.
    pub fn command(&mut self, c: Command, pad: Option<InputKind>) {
        let l = self.layout_now();
        let before = self.nav.workspace;
        match c {
            Command::LastPlace => {
                self.nav.run(c, pad, &l);
                if let Some(a) = self.places.back().cloned() {
                    let _ = self.go_quiet(&a);
                }
                return;
            }
            Command::NextPlace => {
                self.nav.run(c, pad, &l);
                if let Some(a) = self.places.forward().cloned() {
                    let _ = self.go_quiet(&a);
                }
                return;
            }
            Command::EditAddress => self.address_edit = self.address().to_string(),
            Command::Back if self.verdict.is_some() => {
                self.verdict = None;
                return;
            }
            Command::CopyAddress => self.ctx.copy_text(self.address().to_string()),
            Command::NewMod => {
                self.nav.run(c, pad, &l);
                self.open_new_mod();
                return;
            }
            Command::Tutorial => {
                self.nav.run(c, pad, &l);
                if self.guide.is_some() {
                    self.guide_main()
                } else {
                    self.start_tutorial(guide::Tutorial::FirstMod)
                }
                return;
            }
            // In World and Races: the map editor's undo, redo and Test race.
            Command::Undo | Command::Redo | Command::Test if self.editing() => {
                self.nav.run(c, pad, &l);
                if let Some(app) = self.editor_app() {
                    match c {
                        Command::Undo => app.post_action(crate::editor::menu::Action::Undo),
                        Command::Redo => app.post_action(crate::editor::menu::Action::Redo),
                        _ => crate::editor::menu::post(&self.ctx, crate::editor::menu::Command::TestRace),
                    }
                }
                return;
            }
            _ => {}
        }
        if let Some(w) = self.nav.run(c, pad, &l)
            && w != before
        {
            self.nav.workspace = before;
            self.workspace(w);
        }
        if self.nav.crumb_back && self.nav.workspace != Workspace::Home && self.nav.workspace.kind() == Kind::Page {
            // A page's back link: the place before.
            self.nav.crumb_back = false;
            self.command(Command::LastPlace, pad);
        }
    }

    /// Follow `a` without making a new place (back and forward).
    pub(super) fn go_quiet(&mut self, a: &Address) -> Result<(), String> {
        if let Target::View { view, query } = &a.target
            && let Some(w) = Workspace::from_view(&format!("@{view}"))
        {
            if w == Workspace::Changes
                && let Some(m) = changes::Mode::from_query(query)
            {
                self.changes.set_mode(m);
            }
            if w == Workspace::Mod {
                let f = self.open_mod().map(|m| m.folder.clone());
                self.mod_page.load(f.as_deref());
            }
            self.nav.workspace = w;
            self.nav.area = Area::View;
            self.nav.level = Level::Area;
            return Ok(());
        }
        Err("not a screen".into())
    }

    /// A pad button.
    pub fn pad(&mut self, b: Pad, kind: InputKind) {
        if self.new_mod.is_some() {
            self.nav.input = kind;
            match b {
                Pad::Up => self.new_mod_key(egui::Key::ArrowUp, false),
                Pad::Down => self.new_mod_key(egui::Key::ArrowDown, false),
                Pad::A => self.new_mod_key(egui::Key::Enter, false),
                Pad::B => self.new_mod_key(egui::Key::Escape, false),
                _ => {}
            }
            return;
        }
        if self.places_menu && b == Pad::B {
            self.places_menu = false;
            return;
        }
        if self.nav.find_open {
            // The Find list: the D-pad moves, A runs the entry, B closes.
            self.nav.input = kind;
            match b {
                Pad::Up => return self.find_step(-1),
                Pad::Down => return self.find_step(1),
                Pad::A => return self.find_enter(),
                _ => {}
            }
        }
        if self.on_spots() {
            let key = match b {
                Pad::Up => Some(egui::Key::ArrowUp),
                Pad::Down => Some(egui::Key::ArrowDown),
                Pad::Left => Some(egui::Key::ArrowLeft),
                Pad::Right => Some(egui::Key::ArrowRight),
                Pad::A => Some(egui::Key::Enter),
                _ => None,
            };
            if let Some(k) = key {
                self.nav.input = kind;
                self.spot_key(k, false);
                return;
            }
        }
        if self.on_home_rows() {
            let done = match b {
                Pad::Up => Some(self.home_step(-1)),
                Pad::Down => Some(self.home_step(1)),
                Pad::A => Some(self.home_activate()),
                _ => None,
            };
            if done.is_some() {
                self.nav.input = kind;
                self.nav.level = Level::Row;
                return;
            }
        }
        if self.on_changes_rows() {
            let done = match b {
                Pad::Up => Some(self.changes.step_focus(-1, false)),
                Pad::Down => Some(self.changes.step_focus(1, false)),
                Pad::A => Some(self.changes.toggle_pick()),
                _ => None,
            };
            if done.is_some() {
                self.nav.input = kind;
                self.nav.level = Level::Row;
                return;
            }
        }
        if self.on_mod_rows() {
            let act = match b {
                Pad::Up => Some(self.mod_page.step_focus(-1)).map(|_| None),
                Pad::Down => Some(self.mod_page.step_focus(1)).map(|_| None),
                Pad::A => Some(self.mod_page.activate()),
                Pad::B if self.mod_page.back() => Some(None),
                _ => None,
            };
            if let Some(act) = act {
                self.nav.input = kind;
                self.nav.level = Level::Row;
                if let Some(a) = act {
                    self.mod_act(a);
                }
                return;
            }
        }
        self.command(nav::pad_command(b), Some(kind));
    }

    // --- a new mod -------------------------------------------------------------------------------------------

    /// Open the New mod dialog (Home's Empty map mod, File ▸ New mod…, Ctrl+N).
    pub fn open_new_mod(&mut self) {
        self.nav.find_open = false;
        self.nav.address_text = false;
        self.verdict = None;
        if self.new_mod.is_none() {
            self.new_mod = Some(new_mod::Dialog::new());
        }
    }

    /// Home's templates: the New mod dialog, the mod to start from `t`; the sample: opened (made the first time).
    pub fn open_template(&mut self, t: Template) {
        if t == Template::Sample {
            return self.open_sample();
        }
        self.open_new_mod();
        if let Some(d) = self.new_mod.as_mut() {
            d.template = t;
        }
    }

    /// The sample race pack: your copy of it opened on Home (made from the template the first time; your changes
    /// to it stay yours).
    pub fn open_sample(&mut self) {
        if !self.mods.iter().any(|m| m.folder == SAMPLE_FOLDER) {
            if let Err(e) = self.create_mod_from(SAMPLE_FOLDER, words::SAMPLE_RACE_PACK, Template::Sample) {
                log::warn!("studio: the sample: {e}");
                return;
            }
        }
        self.mods = home::all(&self.maps);
        if let Some(i) = self.mods.iter().position(|m| m.folder == SAMPLE_FOLDER) {
            self.switch_mod(i);
        }
        self.workspace(Workspace::Home);
        self.home_focus = 0;
    }

    /// A key in the New mod dialog: Tab / arrows move, Enter takes, Esc cancels.
    fn new_mod_key(&mut self, key: egui::Key, shift: bool) {
        let maps = self.maps.clone();
        let Some(d) = self.new_mod.as_mut() else { return };
        let ask = match key {
            egui::Key::Tab => {
                d.step(if shift { -1 } else { 1 });
                None
            }
            egui::Key::ArrowDown => {
                d.step(1);
                None
            }
            egui::Key::ArrowUp => {
                d.step(-1);
                None
            }
            egui::Key::Enter => d.activate(&maps),
            egui::Key::Escape => Some(new_mod::Ask::Cancel),
            _ => None,
        };
        if let Some(a) = ask {
            self.new_mod_ask(a);
        }
    }

    fn new_mod_ask(&mut self, a: new_mod::Ask) {
        match a {
            new_mod::Ask::Cancel => self.new_mod = None,
            new_mod::Ask::Create { folder, name, template } => match self.create_mod_from(&folder, &name, template) {
                Ok(()) => self.new_mod = None,
                Err(e) => {
                    if let Some(d) = self.new_mod.as_mut() {
                        d.failed = Some(e.to_string());
                    }
                }
            },
        }
    }

    /// Make the mod `name` in `folder` (the way every program makes one) and open it on Home, which says "Your new
    /// mod is ready".
    pub fn create_mod(&mut self, folder: &str, name: &str) -> Result<(), ob_studio::create::Refused> {
        self.create_mod_from(folder, name, Template::Empty)
    }

    /// [`Studio::create_mod`], the mod starting from `template`.
    pub fn create_mod_from(
        &mut self,
        folder: &str,
        name: &str,
        template: Template,
    ) -> Result<(), ob_studio::create::Refused> {
        ob_studio::create::create_from(&self.maps, folder, name, crate::editor::app::DEFAULT_REGION, template)?;
        self.mods = home::all(&self.maps);
        if let Some(i) = self.mods.iter().position(|m| m.folder == folder) {
            self.switch_mod(i);
        }
        if let Some(g) = self.guide.as_mut().filter(|g| g.step == guide::Step::MakeMod) {
            g.made = Some(folder.to_string());
        }
        self.workspace(Workspace::Home);
        self.home_focus = 0;
        Ok(())
    }

    // --- Home's controls ------------------------------------------------------------------------------------

    /// Your mods in Home's order: the open one first.
    fn home_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.mods.len()).collect();
        order.sort_by_key(|i| Some(*i) != self.open);
        order
    }

    /// Home's controls in their order: the card's actions, your mods, then Empty map mod.
    pub fn home_targets(&self) -> Vec<HomeAct> {
        let mut v = match self.open_mod() {
            _ if self.guide.is_some() => vec![HomeAct::GuideMain, HomeAct::GuideSecond],
            Some(_) => vec![HomeAct::GoToWorld, HomeAct::MakeARace],
            None => vec![HomeAct::OpenAMod, HomeAct::NewMod],
        };
        v.extend(PLACES.into_iter().take(self.home_places).map(HomeAct::Place));
        if self.guide.is_none() {
            v.extend(guide::Tutorial::ALL.map(HomeAct::StartTutorial));
        }
        v.extend(self.home_order().into_iter().take(self.home_rows.min(6)).map(HomeAct::Mod));
        v.push(HomeAct::NewMod);
        v.extend(NEW_ROWS[1..].iter().take(self.home_new.saturating_sub(1)).map(|t| HomeAct::Template(*t)));
        v
    }

    fn home_step(&mut self, d: i32) {
        let n = self.home_targets().len() as i32;
        self.home_focus = (self.home_focus as i32 + d).clamp(0, n - 1) as usize;
    }

    fn home_activate(&mut self) {
        if let Some(a) = self.home_targets().get(self.home_focus).copied() {
            self.home_act(a);
        }
    }

    fn home_act(&mut self, a: HomeAct) {
        match a {
            HomeAct::GoToWorld => self.workspace(Workspace::World),
            HomeAct::MakeARace => self.workspace(Workspace::Races),
            HomeAct::OpenAMod => self.nav.find_open = true,
            HomeAct::NewMod => self.open_new_mod(),
            HomeAct::Mod(i) => {
                self.switch_mod(i);
                self.home_focus = 0;
            }
            HomeAct::Place(w) => self.workspace(w),
            HomeAct::StartTutorial(t) => self.start_tutorial(t),
            HomeAct::Template(t) => self.open_template(t),
            HomeAct::GuideMain => self.guide_main(),
            HomeAct::GuideSecond => self.guide_second(),
        }
    }

    /// Home's controls have the keys (↑ ↓, Enter / A): its page has focus and nothing is typed or open.
    fn on_home_rows(&self) -> bool {
        self.nav.workspace == Workspace::Home
            && self.nav.area == Area::View
            && self.nav.level != Level::Menu
            && !self.nav.find_open
            && !self.nav.address_text
            && self.verdict.is_none()
            && self.new_mod.is_none()
    }

    /// The Mod page's rows have the keys (↑ ↓, Enter / A), as the Changes page's do.
    fn on_mod_rows(&self) -> bool {
        self.nav.workspace == Workspace::Mod
            && self.nav.area == Area::View
            && self.nav.level != Level::Menu
            && !self.nav.find_open
            && !self.nav.address_text
            && self.verdict.is_none()
    }

    /// Do what the Mod page asks: follow an address, keep a detail as one step, export.
    pub fn mod_act(&mut self, a: Act) {
        let Some(folder) = self.open_mod().map(|m| m.folder.clone()) else { return };
        match a {
            Act::Go(a) => match Address::parse(&a) {
                Ok(a) => self.verdict = self.go(&a).err(),
                Err(e) => self.verdict = Some(e.to_string()),
            },
            Act::Detail(line, text) => {
                // The map editor has the mod open (World, Races): the step goes through it, so it is the one writer.
                let hosted =
                    self.editor.as_mut().and_then(|h| h.app.state.as_mut()).filter(|s| s.session.folder == folder);
                let r = match hosted {
                    Some(st) => {
                        let changed = mod_page::apply(&mut st.session.map, line, &text);
                        if changed {
                            crate::editor::commit(st, words::MOD_DETAILS);
                        }
                        let saved = self.editor.as_mut().map_or(Ok(()), |h| h.app.save());
                        saved.map(|()| changed)
                    }
                    None => mod_page::set_detail(&self.maps, &folder, line, &text),
                };
                match r {
                    Ok(true) => {
                        self.mod_page.note = Some((ob_look::lamp::Mark::Ok, format!("{} changed", line.label())))
                    }
                    Ok(false) => {}
                    Err(e) => self.mod_page.note = Some((ob_look::lamp::Mark::Error, format!("Not changed: {e:#}"))),
                }
                self.mod_page.load(Some(&folder));
                self.refresh_mod(&folder);
            }
            Act::Export { derived_ok } => {
                let hosted =
                    self.editor.as_mut().filter(|h| h.app.state.as_ref().is_some_and(|s| s.session.folder == folder));
                if let Some(h) = hosted
                    && let Err(e) = h.app.save()
                {
                    self.mod_page.note = Some((ob_look::lamp::Mark::Error, format!("Could not save first: {e:#}")));
                    return;
                }
                self.mod_page.load(Some(&folder));
                self.mod_page.export(derived_ok);
            }
        }
    }

    /// Read the mod's summary again (its name on Home and in the status bar).
    fn refresh_mod(&mut self, folder: &str) {
        let mods = home::all(&self.maps);
        if let Some(i) = mods.iter().position(|m| m.folder == folder) {
            self.mods = mods;
            self.open = Some(i);
        }
    }

    /// The Changes page's rows have the keys (↑ ↓, Enter / A): its view has focus and nothing is typed or open.
    fn on_changes_rows(&self) -> bool {
        self.nav.workspace == Workspace::Changes
            && self.nav.area == Area::View
            && self.nav.level != Level::Menu
            && !self.nav.find_open
            && !self.nav.address_text
            && self.verdict.is_none()
    }

    /// The keys of this frame's input that are the shell's: taken out of the input, run.
    fn take_keys(&mut self, raw: &mut egui::RawInput) {
        if self.new_mod.is_some() {
            // The dialog has the keys: Tab, the arrows up and down, Enter and Esc are its own; the rest types.
            let mut keys = vec![];
            raw.events.retain(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. }
                    if !modifiers.command
                        && !modifiers.alt
                        && matches!(
                            key,
                            egui::Key::Tab
                                | egui::Key::ArrowUp
                                | egui::Key::ArrowDown
                                | egui::Key::Enter
                                | egui::Key::Escape
                        ) =>
                {
                    keys.push((*key, modifiers.shift));
                    false
                }
                // Shortcuts never reach the window under the dialog (the map editor's own keys among them).
                egui::Event::Key { modifiers, key, .. }
                    if modifiers.command
                        && !matches!(
                            key,
                            egui::Key::A | egui::Key::C | egui::Key::V | egui::Key::X | egui::Key::Z | egui::Key::Y
                        ) =>
                {
                    false
                }
                egui::Event::PointerButton { pressed: true, .. } => {
                    self.nav.mouse();
                    true
                }
                _ => true,
            });
            for (key, shift) in keys {
                self.nav.input = InputKind::Keyboard;
                self.new_mod_key(key, shift);
            }
            return;
        }
        let mut cmds = vec![];
        let typing = self.nav.address_text || self.nav.find_open || self.mod_page.editing.is_some();
        // In World and Races the map editor has its own keys: its undo, redo and F5 (its shortcuts read them),
        // and while its view, tree or docks have focus the keys that act inside an area.
        let editing = self.editing() && !typing;
        let inside = editing && matches!(self.nav.area, Area::View | Area::Left | Area::Bottom);
        let rows =
            self.on_changes_rows() || (self.on_mod_rows() && self.mod_page.editing.is_none()) || self.on_home_rows();
        let spots = self.on_spots();
        let find = self.nav.find_open;
        let places_menu = self.places_menu;
        let mut row_keys = vec![];
        let mut spot_keys = vec![];
        let mut find_keys = vec![];
        raw.events.retain(|e| match e {
            egui::Event::Key { key, pressed: true, .. } if places_menu && *key == egui::Key::Escape => {
                find_keys.push(*key);
                false
            }
            egui::Event::Key { key, pressed: true, modifiers, .. }
                if find
                    && !modifiers.command
                    && !modifiers.alt
                    && matches!(key, egui::Key::ArrowUp | egui::Key::ArrowDown) =>
            {
                find_keys.push(*key);
                false
            }
            egui::Event::Key { key, pressed: true, modifiers, .. }
                if rows
                    && !modifiers.command
                    && !modifiers.alt
                    && matches!(key, egui::Key::ArrowUp | egui::Key::ArrowDown | egui::Key::Enter | egui::Key::Tab) =>
            {
                row_keys.push((*key, modifiers.shift));
                false
            }
            egui::Event::Key { key, pressed: true, modifiers, .. }
                if spots
                    && !modifiers.command
                    && !modifiers.alt
                    && matches!(
                        key,
                        egui::Key::ArrowUp
                            | egui::Key::ArrowDown
                            | egui::Key::ArrowLeft
                            | egui::Key::ArrowRight
                            | egui::Key::Enter
                            | egui::Key::Tab
                    ) =>
            {
                spot_keys.push((*key, modifiers.shift));
                false
            }
            egui::Event::Key { key, pressed: true, modifiers, .. } => {
                let k = nav::Key { key: *key, ctrl: modifiers.command, shift: modifiers.shift, alt: modifiers.alt };
                match nav::key_command(k) {
                    Some(Command::Undo | Command::Redo | Command::Test) if editing => true,
                    Some(Command::Back | Command::Select | Command::Move | Command::Change) if inside => true,
                    // While typing, only the keys that leave the field are the shell's.
                    Some(c)
                        if typing
                            && !matches!(
                                c,
                                Command::Back
                                    | Command::Select
                                    | Command::NextArea
                                    | Command::PrevArea
                                    | Command::NewMod
                            ) =>
                    {
                        true
                    }
                    Some(c) => {
                        cmds.push(c);
                        false
                    }
                    None => true,
                }
            }
            egui::Event::PointerButton { pressed: true, .. } | egui::Event::MouseWheel { .. } => {
                self.nav.mouse();
                true
            }
            _ => true,
        });
        for key in find_keys {
            self.nav.input = InputKind::Keyboard;
            match key {
                egui::Key::ArrowUp => self.find_step(-1),
                egui::Key::ArrowDown => self.find_step(1),
                _ => self.places_menu = false,
            }
        }
        for (key, shift) in spot_keys {
            self.nav.input = InputKind::Keyboard;
            self.spot_key(key, shift);
        }
        for (key, shift) in row_keys {
            self.nav.input = InputKind::Keyboard;
            self.nav.level = Level::Row;
            if key == egui::Key::Tab {
                let d = if shift { -1 } else { 1 };
                if !self.page_step(d) {
                    self.tab_out(d);
                }
                continue;
            }
            if self.nav.workspace == Workspace::Home {
                match key {
                    egui::Key::ArrowUp => self.home_step(-1),
                    egui::Key::ArrowDown => self.home_step(1),
                    _ => self.home_activate(),
                }
                continue;
            }
            if self.nav.workspace == Workspace::Mod {
                match key {
                    egui::Key::ArrowUp => self.mod_page.step_focus(-1),
                    egui::Key::ArrowDown => self.mod_page.step_focus(1),
                    _ => {
                        if let Some(a) = self.mod_page.activate() {
                            self.mod_act(a);
                        }
                    }
                }
                continue;
            }
            match key {
                egui::Key::ArrowUp => self.changes.step_focus(-1, shift),
                egui::Key::ArrowDown => self.changes.step_focus(1, shift),
                _ => self.changes.toggle_pick(),
            }
        }
        for c in cmds {
            if matches!(c, Command::Undo | Command::Redo) && self.nav.workspace == Workspace::Changes {
                let session = self.editor.as_mut().and_then(|h| h.app.state.as_mut()).map(|s| &mut s.session);
                self.changes.undo_with(c == Command::Redo, session);
                continue;
            }
            if self.nav.workspace == Workspace::Mod {
                // The text field: Enter keeps it (one step), Esc leaves it; the warning: Enter exports, Esc not.
                if c == Command::Select && (self.mod_page.editing.is_some() || self.mod_page.confirm) {
                    if let Some(a) = self.mod_page.activate() {
                        self.mod_act(a);
                    }
                    continue;
                }
                if c == Command::Back && self.mod_page.back() {
                    continue;
                }
                if matches!(c, Command::Undo | Command::Redo) && self.mod_page.editing.is_none() {
                    if let Some(f) = self.open_mod().map(|m| m.folder.clone()) {
                        if let Err(e) = changes::undo(&self.maps, &f, c == Command::Redo) {
                            self.mod_page.note = Some((ob_look::lamp::Mark::Error, format!("{e:#}")));
                        }
                        self.mod_page.load(Some(&f));
                        self.refresh_mod(&f);
                    }
                    continue;
                }
            }
            if c == Command::Select && self.nav.address_text {
                self.nav.input = InputKind::Keyboard;
                self.nav.address_text = false;
                self.nav.level = Level::Area;
                match Address::parse(&self.address_edit) {
                    Ok(a) => self.verdict = self.go(&a).err(),
                    Err(e) => self.verdict = Some(e.to_string()),
                }
                continue;
            }
            if c == Command::Select && self.nav.find_open {
                self.find_enter();
                continue;
            }
            self.command(c, None);
        }
    }

    /// One frame. Its texture changes are kept for the next paint (the output's `textures_delta` is left empty).
    pub fn frame(&mut self, mut raw: egui::RawInput) -> egui::FullOutput {
        self.sync_editor();
        self.guide_frame();
        self.take_mouse_places(&mut raw);
        self.take_keys(&mut raw);
        let ctx = self.ctx.clone();
        self.spots.begin();
        let mut full = ctx.run_ui(raw, |ui| self.ui(ui));
        self.spots.end();
        self.textures.0.append(std::mem::take(&mut full.textures_delta));
        full
    }

    /// Register one of the shell's controls drawn at `r` ([`focus`]); true when it has the focus and keys or a pad
    /// are in use (the selection fill drawn under it when `fill`; the ring comes after everything).
    fn spot(&mut self, p: &egui::Painter, a: Area, s: Spot, r: Rect, fill: bool) -> bool {
        let f = self.spots.add(a, s, r) && self.nav.area == a && self.nav.legend();
        if f && fill {
            focus::fill(p, r);
        }
        f
    }

    fn time(&self) -> f32 {
        if self.fixed_clock { 0.15 } else { self.ctx.input(|i| i.time as f32) }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let screen = ui.max_rect();
        // Rows and hit targets: 24 points, 32 while a pad is used.
        let row = if self.nav.input.is_pad() { 32.0 } else { 24.0 };
        ui.style_mut().spacing.interact_size.y = row;
        let mut want = self.want();
        // The menus' and the mod name's widths as drawn.
        let mw = |s: &str, f: egui::FontId| ui.painter().layout_no_wrap(s.to_string(), f, Color32::WHITE).size().x;
        // Each menu: its word in the button face with the buttons' padding, and the gap between menus.
        let pad = MENU_GAP + 4.0;
        want.menus_w = MENUS.iter().map(|m| mw(m, egui::TextStyle::Button.resolve(ui.style())) + pad).sum::<f32>();
        want.mod_name_w = mw(&self.open_mod().map_or(String::new(), |m| m.name.clone()), font(SEMI, 15.0)) + 26.0;
        let l = layout::layout(screen.width(), screen.height(), &want);
        ui.painter().rect_filled(screen, 0.0, draw::page_fill());
        if l.too_small {
            ui.painter().text(
                screen.center(),
                Align2::CENTER_CENTER,
                words::TOO_SMALL,
                font("body", 15.0),
                rgb(tokens().ink),
            );
            self.last = Some(l);
            return;
        }
        let editing = self.editing();
        if editing && let Some(app) = self.editor_app() {
            app.begin_frame(ui.ctx());
        }
        self.menu_bar(ui, &l);
        self.rail(ui, &l);
        match self.nav.workspace.kind() {
            Kind::Page => self.page(ui, &l),
            Kind::Tool => self.tool(ui, &l),
        }
        self.guide_rect = None;
        if self.nav.workspace != Workspace::Home && self.new_mod.is_none() {
            self.guide_card(ui, &l);
        }
        if let Some(r) = l.legend {
            self.legend(ui, r);
        }
        self.status(ui, &l);
        self.focus_line(ui, &l);
        if let Some(v) = self.verdict.clone() {
            self.verdict_box(ui, &l, &v);
        }
        if self.nav.find_open {
            self.find_list(ui, &l);
        }
        if self.places_menu {
            self.places_list(ui, &l);
        }
        if self.nav.level == Level::Menu {
            self.focus_menu(ui, &l);
        }
        if self.controls {
            self.controls_window(ui);
        }
        if self.new_mod.is_some() {
            let maps = self.maps.clone();
            let focused = self.nav.legend();
            let ask = self.new_mod.as_mut().and_then(|d| d.draw(ui, screen, &maps, focused));
            if let Some(a) = ask {
                self.new_mod_ask(a);
            }
        }
        if editing && let Some(app) = self.editor_app() {
            app.end_frame(ui.ctx());
            // The editor's own Quit (Ctrl+Q) closes the window.
            if app.quit {
                self.quit = true;
            }
        }
        self.last = Some(l);
    }

    // --- menu bar -------------------------------------------------------------------------------------------

    fn menu_bar(&mut self, ui: &mut egui::Ui, l: &Layout) {
        let t = tokens();
        let p = ui.painter().clone();
        p.rect_filled(l.menu.bar, 0.0, draw::page_fill());
        p.line_segment([l.menu.bar.left_bottom(), l.menu.bar.right_bottom()], Stroke::new(1.0, draw::rule()));
        ui.scope_builder(UiBuilder::new().max_rect(l.menu.menus), |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = MENU_GAP;
                ui.style_mut().visuals.button_frame = false;
                let short = l.menu.short_menus;
                ui.menu_button("File", |ui| {
                    if ui.add(egui::Button::new(words::NEW_MOD).shortcut_text("Ctrl+N")).clicked() {
                        self.open_new_mod();
                    }
                    ui.separator();
                    if short {
                        ui.label(RichText::new("Mod").color(rgb(t.dim)));
                        self.mod_menu(ui);
                        ui.separator();
                    }
                    if let Some(app) = self.editor_app() {
                        app.menu_items(ui, menu::mod_file_items);
                        ui.separator();
                    }
                    if ui.button(words::QUIT).clicked() && self.close_requested() {
                        self.quit = true;
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if let Some(app) = self.editor_app() {
                        app.menu_items(ui, menu::edit_items);
                        ui.separator();
                    } else {
                        let _ = ui.add_enabled(false, egui::Button::new(ob_look::words::UNDO).shortcut_text("Ctrl+Z"));
                        let _ = ui.add_enabled(false, egui::Button::new(ob_look::words::REDO).shortcut_text("Ctrl+Y"));
                    }
                    if ui.add(egui::Button::new(words::COPY_ADDRESS).shortcut_text("Ctrl+Shift+C")).clicked() {
                        self.command(Command::CopyAddress, None);
                    }
                });
                ui.menu_button("View", |ui| {
                    if let Some(app) = self.editor_app() {
                        app.menu_items(ui, menu::view_items);
                        ui.separator();
                    }
                    if ui.button(words::RESET_LAYOUT).clicked() {
                        self.reset_layout();
                    }
                });
                ui.menu_button("Go", |ui| {
                    for w in Workspace::ALL {
                        if ui.add(egui::Button::new(w.name()).shortcut_text(format!("Ctrl+{}", w.ctrl_key()))).clicked()
                        {
                            self.workspace(w);
                        }
                    }
                    ui.separator();
                    if ui.add(egui::Button::new("Find…").shortcut_text("Ctrl+K")).clicked() {
                        self.command(Command::Find, None);
                    }
                    if ui.add(egui::Button::new("Address").shortcut_text("Ctrl+L")).clicked() {
                        self.command(Command::EditAddress, None);
                    }
                    if short {
                        ui.separator();
                        if let Some(app) = self.editor_app() {
                            app.menu_items(ui, menu::race_items);
                        } else {
                            let _ = ui.add_enabled(false, egui::Button::new("Test").shortcut_text("F5"));
                        }
                    }
                });
                if !short {
                    ui.menu_button("Test", |ui| {
                        if let Some(app) = self.editor_app() {
                            app.menu_items(ui, menu::race_items);
                        } else {
                            let _ = ui.add_enabled(false, egui::Button::new("Test").shortcut_text("F5"));
                        }
                    });
                    ui.menu_button("Mod", |ui| self.mod_menu(ui));
                }
                ui.menu_button("Help", |ui| {
                    if ui.add(egui::Button::new(words::TUTORIAL).shortcut_text("F1")).clicked() {
                        self.start_tutorial(guide::Tutorial::FirstMod);
                    }
                    if ui.button(words::RACE_TUTORIAL_ROW).clicked() {
                        self.start_tutorial(guide::Tutorial::FirstRace);
                    }
                    if ui.button("Controls").clicked() {
                        self.controls = true;
                    }
                    if let Some(app) = self.editor_app() {
                        ui.separator();
                        ui.label(RichText::new("Map editor").color(rgb(t.dim)));
                        app.menu_items(ui, menu::help_items);
                    }
                });
            });
        });
        // ← →
        let bf = l.menu.back_fwd;
        let half = Rect::from_min_size(bf.min + vec2(0.0, 4.0), vec2(bf.width() / 2.0 - 2.0, bf.height() - 8.0));
        for (i, (txt, can, c, spot)) in [
            ("←", self.places.can_back(), Command::LastPlace, Spot::Back),
            ("→", self.places.can_forward(), Command::NextPlace, Spot::Forward),
        ]
        .into_iter()
        .enumerate()
        {
            let r = half.translate(vec2(i as f32 * (half.width() + 4.0), 0.0));
            let resp = ui.interact(r, egui::Id::unique(("backfwd", i)), egui::Sense::click());
            if !self.spot(&p, Area::Address, spot, r, true) {
                p.rect_filled(r, 4.0, theme::over(t.button_fill, t.page));
            }
            let col = if can { rgb(t.ink) } else { rgb(t.disabled) };
            p.text(r.center(), Align2::CENTER_CENTER, txt, font(SEMI, 15.0), col);
            if resp.clicked() && can {
                self.command(c, None);
            }
            // A right-click on ← lists the places you were.
            if i == 0 && resp.secondary_clicked() && can {
                self.nav.mouse();
                self.places_menu = true;
            }
        }
        // The address: names, or the address itself after Ctrl+L.
        let a = l.menu.address.shrink2(vec2(0.0, 4.0));
        self.spot(&p, Area::Address, Spot::Address, a, false);
        p.rect_filled(a, 4.0, theme::over(t.field, t.page));
        p.rect_stroke(a, 4.0, Stroke::new(1.0, theme::over(t.field_edge, t.page)), StrokeKind::Inside);
        if self.nav.address_text {
            let te =
                egui::TextEdit::singleline(&mut self.address_edit).frame(egui::Frame::NONE).font(font("body", 14.0));
            let resp = ui.put(a.shrink2(vec2(8.0, 2.0)), te);
            resp.request_focus();
        } else {
            let mut x = a.left() + 10.0;
            let crumbs = self.crumbs();
            let n = crumbs.len();
            let mut clip = p.clone();
            clip.set_clip_rect(a.shrink(2.0));
            for (i, c) in crumbs.iter().enumerate() {
                let (f, col) =
                    if i + 1 == n { (font(SEMI, 15.0), rgb(t.ink)) } else { (font("body", 15.0), rgb(t.dim)) };
                let r = clip.text(pos2(x, a.center().y), Align2::LEFT_CENTER, c, f, col);
                x = r.right();
                if i + 1 < n {
                    let r = clip.text(
                        pos2(x + 6.0, a.center().y),
                        Align2::LEFT_CENTER,
                        CRUMB.trim(),
                        font("body", 15.0),
                        rgb(t.dim),
                    );
                    x = r.right() + 6.0;
                }
            }
            if a.width() > 240.0 {
                p.text(
                    pos2(a.right() - 10.0, a.center().y),
                    Align2::RIGHT_CENTER,
                    "Ctrl+L",
                    font(SEMI, 11.0),
                    rgb(t.dim),
                );
            }
            if ui.interact(a, egui::Id::unique("address"), egui::Sense::click()).clicked() {
                self.command(Command::EditAddress, None);
            }
        }
        // Find.
        let f = l.menu.find.shrink2(vec2(0.0, 4.0));
        self.spot(&p, Area::Address, Spot::Find, f, false);
        p.rect_filled(f, 4.0, theme::over(t.field, t.page));
        p.rect_stroke(f, 4.0, Stroke::new(1.0, theme::over(t.field_edge, t.page)), StrokeKind::Inside);
        let cap_w = draw::glyph_width(&p, ob_look::legends::Glyph::Key("Ctrl+K"));
        let show_cap = f.width() > cap_w + 70.0;
        if show_cap {
            draw::key_cap_right(&p, "Ctrl+K", f.right() - 4.0, f.center().y);
        }
        let text_r = Rect::from_min_max(
            f.min + vec2(8.0, 2.0),
            pos2(f.right() - if show_cap { cap_w + 8.0 } else { 6.0 }, f.bottom() - 2.0),
        );
        if self.nav.find_open {
            let te =
                egui::TextEdit::singleline(&mut self.find_text).frame(egui::Frame::NONE).hint_text(words::FIND_HINT);
            ui.put(text_r, te).request_focus();
        } else {
            let mut clip = p.clone();
            clip.set_clip_rect(text_r);
            clip.text(
                pos2(text_r.left(), f.center().y),
                Align2::LEFT_CENTER,
                words::FIND_HINT,
                font("body", 15.0),
                rgb(t.placeholder),
            );
            if ui.interact(f, egui::Id::unique("find"), egui::Sense::click()).clicked() {
                self.command(Command::Find, None);
            }
        }
        // The open mod's name with its lamp.
        let m = l.menu.mod_name;
        if let Some(md) = self.open_mod() {
            let mark = match md.saved {
                Saved::Error(_) => Mark::Error,
                Saved::Behind => Mark::Warn,
                Saved::Saving => Mark::Info,
                Saved::Saved => Mark::Ok,
            };
            draw::lamp(&p, pos2(m.left() + 8.0, m.center().y), 9.0, mark, 0.0);
            p.text(pos2(m.left() + 20.0, m.center().y), Align2::LEFT_CENTER, &md.name, font(SEMI, 15.0), rgb(t.ink));
        }
    }

    /// Why an address was not followed: a message under the address bar (the launcher's message box: its left
    /// edge in the mark's colour). Esc, B or a click closes it.
    fn verdict_box(&mut self, ui: &mut egui::Ui, l: &Layout, text: &str) {
        let t = tokens();
        let p = ui.painter().clone();
        let a = l.menu.address;
        let galley = p.layout(text.to_string(), font("body", 15.0), rgb(t.ink), a.width().max(320.0) - 40.0);
        let r =
            Rect::from_min_size(pos2(a.left(), a.bottom() + 4.0), vec2(a.width().max(320.0), galley.size().y + 20.0));
        p.rect_filled(r, 6.0, rgb(t.dialog));
        p.rect_filled(
            Rect::from_min_size(r.min, vec2(4.0, r.height())),
            CornerRadius { nw: 6, sw: 6, ne: 0, se: 0 },
            rgb(t.warn),
        );
        draw::lamp(&p, pos2(r.left() + 18.0, r.top() + 10.0 + 9.0), 10.0, Mark::Warn, 0.0);
        p.galley(pos2(r.left() + 32.0, r.top() + 10.0), galley, rgb(t.ink));
        if ui.interact(r, egui::Id::unique("verdict"), egui::Sense::click()).clicked() {
            self.verdict = None;
        }
    }

    fn mod_menu(&mut self, ui: &mut egui::Ui) {
        let mut pick = None;
        for (i, m) in self.mods.iter().enumerate() {
            if ui.add(egui::Button::selectable(Some(i) == self.open, &m.name)).clicked() {
                pick = Some(i);
            }
        }
        if self.mods.is_empty() {
            ui.label(RichText::new(words::NO_MODS_YET).color(rgb(tokens().dim)));
        }
        if let Some(i) = pick {
            self.switch_mod(i);
        }
    }

    fn reset_layout(&mut self) {
        self.memories.set(self.nav.workspace, Memory::default());
        if let Some(m) = self.open_mod()
            && let Err(e) = self.memories.save(&m.dir)
        {
            log::warn!("studio: {e:#}");
        }
    }

    // --- rail -----------------------------------------------------------------------------------------------

    fn rail(&mut self, ui: &mut egui::Ui, l: &Layout) {
        let t = tokens();
        let p = ui.painter().clone();
        let r = l.rail;
        p.rect_filled(r, 0.0, draw::page_fill());
        p.line_segment(
            [r.right_top() + vec2(-1.0, 6.0), r.right_bottom() + vec2(-1.0, 0.0)],
            Stroke::new(2.0, rgb(t.rail)),
        );
        let names = l.rail_names;
        let item_h = if names { 82.0 } else { 42.0 };
        let mut top = r.top() + 8.0;
        let mut bottom = r.bottom() - 8.0;
        let mut slots = vec![];
        let mut last_section = Section::Top;
        for w in Workspace::ALL.into_iter().filter(|w| matches!(w.section(), Section::Top | Section::Make)) {
            if names && w.section() != last_section {
                p.text(pos2(r.center().x, top + 8.0), Align2::CENTER_CENTER, "MAKE", font("body", 11.0), rgb(t.dim));
                top += 20.0;
                last_section = w.section();
            }
            slots.push((w, Rect::from_min_size(pos2(r.left() + 6.0, top), vec2(r.width() - 14.0, item_h))));
            top += item_h + 4.0;
        }
        for w in Workspace::ALL.into_iter().rev().filter(|w| matches!(w.section(), Section::Share | Section::Bottom)) {
            bottom -= item_h;
            slots.push((w, Rect::from_min_size(pos2(r.left() + 6.0, bottom), vec2(r.width() - 14.0, item_h))));
            if names && w.section() == Section::Share {
                p.text(
                    pos2(r.center().x, bottom - 12.0),
                    Align2::CENTER_CENTER,
                    "SHARE",
                    font("body", 11.0),
                    rgb(t.dim),
                );
                bottom -= 20.0;
            }
            bottom -= 4.0;
        }
        // Reading order: top to bottom.
        slots.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
        for (w, slot) in slots {
            let current = w == self.nav.workspace.on_rail();
            let resp = ui.interact(slot, egui::Id::unique(("rail", w.name())), egui::Sense::click());
            let focused = self.spot(&p, Area::Rail, Spot::Rail(w), slot, !current);
            if current {
                p.rect_filled(slot, 6.0, theme::over(t.slab, t.page));
                // The launcher's page mark: a ringed dot on the rail line.
                let c = pos2(r.right() - 1.0, slot.center().y - if names { 18.0 } else { 0.0 });
                p.circle_filled(c, 7.0, draw::page_fill());
                p.circle_stroke(c, 6.0, Stroke::new(1.5, rgb(t.dot)));
                p.circle_filled(c, 3.0, rgb(t.dot));
            } else if resp.hovered() && !focused {
                p.rect_filled(slot, 6.0, theme::over(t.hover, t.page));
            }
            let col = if current { rgb(t.lime) } else { rgb(t.ink) };
            let icon_c = if names { pos2(slot.center().x, slot.top() + 22.0) } else { slot.center() };
            draw::rail_icon(&p, w, icon_c, col);
            if names {
                p.text(
                    pos2(slot.center().x, slot.top() + 48.0),
                    Align2::CENTER_CENTER,
                    w.name(),
                    draw::label_font(14.0),
                    col,
                );
                p.text(
                    pos2(slot.center().x, slot.top() + 67.0),
                    Align2::CENTER_CENTER,
                    format!("Ctrl+{}", w.ctrl_key()),
                    font("body", 11.0),
                    rgb(t.dim),
                );
            } else {
                resp.clone().on_hover_text(format!("{} · Ctrl+{}", w.name(), w.ctrl_key()));
            }
            if resp.clicked() {
                self.nav.mouse();
                self.workspace(w);
            }
        }
    }

    // --- pages ----------------------------------------------------------------------------------------------

    fn page(&mut self, ui: &mut egui::Ui, l: &Layout) {
        let t = tokens();
        let p = ui.painter().clone();
        let slab = l.view;
        p.rect_filled(slab, 10.0, theme::over(t.slab, t.page));
        p.rect_stroke(slab, 10.0, Stroke::new(1.0, theme::over(t.slab_edge, t.slab.over(t.page))), StrokeKind::Inside);
        let inner = slab.shrink2(vec2(36.0, 28.0));
        match self.nav.workspace {
            Workspace::Home => self.home(ui, inner),
            Workspace::Changes => {
                let inner = slab.shrink2(vec2(20.0, 14.0));
                let focused = self.nav.area == Area::View && self.nav.legend();
                // The game's textures for the Looks rows' thumbnails: from the region World has loaded.
                if let Some(s) = self.editor.as_ref().and_then(|h| h.app.state.as_ref()) {
                    self.changes.feed_originals(ui.ctx(), &s.session.world);
                }
                let (ask, _) = self.changes.draw(ui, inner, focused, self.time());
                match ask {
                    Some(Ask::Go(a)) if a == "back" => self.command(Command::LastPlace, None),
                    Some(Ask::Go(a)) => match Address::parse(&a) {
                        Ok(a) => self.verdict = self.go(&a).err(),
                        Err(e) => self.verdict = Some(e.to_string()),
                    },
                    Some(Ask::Moved) => {
                        let here = self.address();
                        self.places.go(here);
                    }
                    None => {}
                }
            }
            Workspace::Mod => {
                let focused = self.nav.area == Area::View && self.nav.legend();
                if let Some(a) = self.mod_page.draw(ui, inner, focused, self.time()) {
                    self.mod_act(a);
                }
            }
            w => {
                p.text(inner.left_top(), Align2::LEFT_TOP, w.name(), font(TITLE, 34.0), rgb(t.lime));
                p.text(
                    inner.left_top() + vec2(0.0, 58.0),
                    Align2::LEFT_TOP,
                    words::SETTINGS_PAGE,
                    font("body", 16.0),
                    rgb(t.dim),
                );
            }
        }
    }

    fn home(&mut self, ui: &mut egui::Ui, inner: Rect) {
        let t = tokens();
        let p = ui.painter().clone();
        let title = self.open_mod().map_or(words::STUDIO.to_string(), |m| m.name.clone());
        p.text(inner.left_top(), Align2::LEFT_TOP, &title, font(TITLE, 36.0), rgb(t.lime));
        let top = inner.top() + 64.0;
        let wide = inner.width() >= 760.0;
        let left_w = if wide { inner.width() * 0.58 } else { inner.width() };
        let left = Rect::from_min_size(pos2(inner.left(), top), vec2(left_w, inner.bottom() - top));
        let right = Rect::from_min_max(pos2(inner.left() + left_w + 28.0, top), inner.max);

        // The verdict card (the launcher's Game check): lamp, title, its words, the actions; as high as its words.
        // While the tutorial runs, it is the tutorial's card.
        let guide = self.guide.clone();
        let (head, line, go) = match self.open_mod() {
            None => (words::NO_MOD_TITLE.to_string(), words::NO_MOD_LINE.to_string(), None),
            Some(m) if m.read_only_by.is_some() => (
                words::READ_ONLY_TITLE.to_string(),
                format!(
                    "{} has it open. You can look; changes wait until it closes the mod.",
                    m.read_only_by.clone().unwrap_or_default()
                ),
                Some(Workspace::World),
            ),
            Some(m) if m.changes() == 0 => {
                (words::READY_TITLE.to_string(), words::READY_LINE.to_string(), Some(Workspace::World))
            }
            Some(m) => (
                home::plural(m.changes(), "change", "changes"),
                words::CHANGED_LINE.to_string(),
                Some(Workspace::World),
            ),
        };
        let kicker = guide.as_ref().map(Guide::kicker);
        let c = match &guide {
            Some(g) => Card {
                step: g.number(),
                kicker: kicker.as_deref(),
                head: g.title(),
                line: g.line(),
                main: g.step.act().1,
                second: guide::second_word(g),
            },
            None => Card {
                step: None,
                kicker: None,
                head: &head,
                line: &line,
                main: if go.is_some() { words::GO_TO_WORLD } else { words::OPEN_A_MOD },
                second: if go.is_some() { words::MAKE_A_RACE } else { words::NEW_MOD },
            },
        };
        // Home's controls (keyboard and pad focus): the ring on the focused one while a key or a pad is used.
        let targets = self.home_targets();
        let ring = self.nav.legend() && self.nav.area == Area::View && self.new_mod.is_none();
        let focus = targets.get(self.home_focus).copied();
        let mut target = 0usize;
        let mut acted = None;
        let take = |r: Rect, clicked: bool, target: &mut usize, acted: &mut Option<HomeAct>| {
            let i = *target;
            *target += 1;
            if ring && i == self.home_focus && focus.is_some() {
                focus::ring(&p, r.expand(5.0));
            }
            if clicked {
                *acted = targets.get(i).copied();
            }
        };
        let (card, a, b) = draw::card(ui, &p, left.min, left.width(), &c, theme::over(t.panel, t.slab.over(t.page)));
        take(a.rect, a.clicked(), &mut target, &mut acted);
        take(b.rect, b.clicked(), &mut target, &mut acted);

        // The summary rows.
        let mut y = card.bottom() + 10.0;
        let rows: Vec<(Mark, &str, String)> = match self.open_mod() {
            Some(m) => vec![
                (Mark::Ok, "Changes", m.changes_text()),
                (
                    if m.in_game { Mark::Ok } else { Mark::Info },
                    "In your game",
                    if m.in_game { "On".into() } else { words::OFF_IN_GAME.into() },
                ),
                match &m.saved {
                    Saved::Saved => (Mark::Ok, "Saved", "Automatically".into()),
                    Saved::Behind => (Mark::Warn, "Saved", words::SAVING_BEHIND.into()),
                    Saved::Saving => (Mark::Info, "Saved", words::SAVING.into()),
                    Saved::Error(e) => (Mark::Error, "Saved", e.clone()),
                },
                (
                    if m.read_only_by.is_some() { Mark::Warn } else { Mark::Ok },
                    "Open for",
                    match &m.read_only_by {
                        Some(o) => format!("Looking only · {o} has it"),
                        None => words::ONLY_THIS_WINDOW.into(),
                    },
                ),
            ],
            None => vec![(Mark::Info, "Your mods", home::plural(self.mods.len(), "mod", "mods"))],
        };
        // The busy line (the launcher's Game check): what runs now.
        let mut rows = rows;
        if let Some(job) = self.jobs().first() {
            rows.push((Mark::Busy, "Jobs", job.line()));
        }
        let label_w = (left.width() * 0.35).min(200.0);
        for (mark, name, value) in rows {
            let row = Rect::from_min_size(pos2(left.left(), y), vec2(left.width(), 38.0));
            draw::lamp(&p, pos2(row.left() + 14.0, row.center().y), 11.0, mark, self.time());
            p.text(pos2(row.left() + 32.0, row.center().y), Align2::LEFT_CENTER, name, font("body", 16.0), rgb(t.dim));
            let mut clip = p.clone();
            clip.set_clip_rect(Rect::from_min_max(pos2(row.left() + label_w, row.top()), row.max));
            clip.text(
                pos2(row.left() + label_w, row.center().y),
                Align2::LEFT_CENTER,
                value,
                font("body", 16.0),
                rgb(t.ink),
            );
            p.line_segment([row.left_bottom(), row.right_bottom()], Stroke::new(1.0, draw::rule()));
            y += 38.0;
        }

        // What each place does: a row per workspace a mod is made in, as many as fit (left column).
        y += 24.0;
        let mut places = 0;
        if y + 22.0 + 46.0 <= inner.bottom() {
            p.text(pos2(left.left(), y), Align2::LEFT_TOP, words::PLACES, font("body", 12.0), rgb(t.dim));
            y += 22.0;
            for w in PLACES {
                if y + 46.0 > inner.bottom() {
                    break;
                }
                places += 1;
                let row = Rect::from_min_size(pos2(left.left(), y), vec2(left.width(), 44.0));
                let resp = ui.interact(row, egui::Id::unique(("home.place", w.name())), egui::Sense::click());
                if resp.hovered() {
                    p.rect_filled(row, 6.0, theme::over(t.hover, t.slab.over(t.page)));
                }
                draw::rail_icon(&p, w, pos2(row.left() + 16.0, row.center().y), rgb(t.lime));
                p.text(
                    pos2(row.left() + 40.0, row.top() + 13.0),
                    Align2::LEFT_CENTER,
                    w.name(),
                    draw::label_font(16.0),
                    rgb(t.ink),
                );
                let key = format!("Ctrl+{}", w.ctrl_key());
                p.text(
                    pos2(row.right() - 8.0, row.top() + 13.0),
                    Align2::RIGHT_CENTER,
                    &key,
                    font("body", 12.0),
                    rgb(t.dim),
                );
                let mut clip = p.clone();
                clip.set_clip_rect(row);
                clip.text(
                    pos2(row.left() + 40.0, row.top() + 32.0),
                    Align2::LEFT_CENTER,
                    w.what(),
                    font("body", 13.0),
                    rgb(t.dim),
                );
                take(row, resp.clicked(), &mut target, &mut acted);
                y += 46.0;
            }
        }
        self.home_places = places;

        // Your mods and something new.
        let (col, mut y) = if wide { (right, right.top() + 6.0) } else { (left, y + 24.0) };
        if guide.is_none() {
            p.text(pos2(col.left(), y), Align2::LEFT_TOP, words::START_HERE, font("body", 12.0), rgb(t.dim));
            y += 22.0;
            for (n, tut) in guide::Tutorial::ALL.into_iter().enumerate() {
                let row = Rect::from_min_size(pos2(col.left(), y), vec2(col.width(), 56.0));
                let resp = ui.interact(row, egui::Id::unique(("home.tutorial", n)), egui::Sense::click());
                p.rect_filled(
                    row,
                    6.0,
                    theme::over(if resp.hovered() { t.hover } else { t.panel }, t.slab.over(t.page)),
                );
                let c = pos2(row.left() + 20.0, row.center().y);
                p.circle_filled(c, 10.0, rgb(t.lime));
                p.text(c, Align2::CENTER_CENTER, (n + 1).to_string(), font(SEMI, 13.0), rgb(t.on_play));
                let (head, line) = tut.row();
                p.text(
                    pos2(row.left() + 40.0, row.top() + 19.0),
                    Align2::LEFT_CENTER,
                    head,
                    draw::label_font(18.0),
                    rgb(t.ink),
                );
                let mut clip = p.clone();
                clip.set_clip_rect(row);
                clip.text(
                    pos2(row.left() + 40.0, row.top() + 40.0),
                    Align2::LEFT_CENTER,
                    line,
                    font("body", 13.0),
                    rgb(t.dim),
                );
                take(row, resp.clicked(), &mut target, &mut acted);
                y += 60.0;
            }
            y += 14.0;
        }
        p.text(pos2(col.left(), y), Align2::LEFT_TOP, "YOUR MODS", font("body", 12.0), rgb(t.dim));
        y += 22.0;
        let mut rows_drawn = 0;
        // Room for the new rows under your mods, one mod row still left: all four as tall as your mods' rows, else
        // all four a line lower (a short window), else as many as fit (Empty map mod always).
        let mods_h = if self.mods.is_empty() { 30.0 } else { 60.0 };
        let room = inner.bottom() - y - mods_h - 36.0 - 22.0;
        let n = NEW_ROWS.len();
        let step = if room >= 60.0 * n as f32 { 60.0 } else { 46.0 };
        let new_rows = ((room / step).floor() as usize).clamp(1, n);
        let new_h = 36.0 + step * new_rows as f32 + 22.0 * (new_rows == n) as u8 as f32;
        for (i, m) in self.home_order().into_iter().map(|i| (i, &self.mods[i])).take(6) {
            // As many as the slab has room for (the rest are in the Mod menu and Find).
            if y + 56.0 > inner.bottom() - new_h {
                break;
            }
            rows_drawn += 1;
            let row = Rect::from_min_size(pos2(col.left(), y), vec2(col.width(), 56.0));
            let open = Some(i) == self.open;
            if open {
                p.rect_filled(row, 6.0, theme::over(t.panel, t.slab.over(t.page)));
                p.rect_stroke(
                    row,
                    6.0,
                    Stroke::new(1.0, theme::over(t.tag_edge, t.slab.over(t.page))),
                    StrokeKind::Inside,
                );
            }
            let src = if open { Source::Yours } else { Source::AnotherMod };
            draw::source_mark(&p, pos2(row.left() + 20.0, row.center().y), 10.0, src);
            p.text(
                pos2(row.left() + 40.0, row.top() + 19.0),
                Align2::LEFT_CENTER,
                &m.name,
                draw::label_font(18.0),
                rgb(t.ink),
            );
            let sub = if open { format!("open · {}", m.line()) } else { m.line() };
            p.text(pos2(row.left() + 40.0, row.top() + 40.0), Align2::LEFT_CENTER, sub, font("body", 13.0), rgb(t.dim));
            let clicked = ui.interact(row, egui::Id::unique(("mod", i)), egui::Sense::click()).clicked();
            take(row, clicked, &mut target, &mut acted);
            y += 60.0;
        }
        if self.mods.is_empty() {
            p.text(pos2(col.left(), y), Align2::LEFT_TOP, "None yet", font("body", 15.0), rgb(t.dim));
            y += 30.0;
        }
        y += 14.0;
        p.text(pos2(col.left(), y), Align2::LEFT_TOP, words::START_NEW, font("body", 12.0), rgb(t.dim));
        y += 22.0;
        // Empty map mod and the templates: rows as your mods are, which open the New mod dialog; the sample under
        // its own heading.
        for (k, tpl) in NEW_ROWS.into_iter().take(new_rows).enumerate() {
            if tpl == Template::Sample {
                y += 8.0;
                p.text(pos2(col.left(), y), Align2::LEFT_TOP, words::SAMPLE, font("body", 12.0), rgb(t.dim));
                y += 14.0;
            }
            let row = Rect::from_min_size(pos2(col.left(), y), vec2(col.width(), step - 4.0));
            let resp = ui.interact(row, egui::Id::unique(("home.new", k)), egui::Sense::click());
            if resp.hovered() {
                p.rect_filled(row, 6.0, theme::over(t.hover, t.slab.over(t.page)));
            }
            let c = pos2(row.left() + 20.0, row.center().y);
            match tpl {
                Template::Empty => {
                    p.circle_stroke(c, 9.0, Stroke::new(1.5, rgb(t.lime)));
                    p.line_segment([c - vec2(4.5, 0.0), c + vec2(4.5, 0.0)], Stroke::new(1.5, rgb(t.lime)));
                    p.line_segment([c - vec2(0.0, 4.5), c + vec2(0.0, 4.5)], Stroke::new(1.5, rgb(t.lime)));
                }
                // A race's mark: the Races place's own icon.
                _ => draw::rail_icon(&p, Workspace::Races, c, rgb(t.lime)),
            }
            let (head, line) = new_mod::template_words(tpl);
            // A short window: the same row a line lower (the place rows' sizes).
            let (head_y, line_y, head_size, line_size) =
                if step < 60.0 { (13.0, 31.0, 16.0, 12.5) } else { (19.0, 40.0, 18.0, 13.0) };
            p.text(
                pos2(row.left() + 40.0, row.top() + head_y),
                Align2::LEFT_CENTER,
                head,
                draw::label_font(head_size),
                rgb(t.ink),
            );
            let mut clip = p.clone();
            clip.set_clip_rect(row);
            clip.text(
                pos2(row.left() + 40.0, row.top() + line_y),
                Align2::LEFT_CENTER,
                line,
                font("body", line_size),
                rgb(t.dim),
            );
            take(row, resp.clicked(), &mut target, &mut acted);
            y += step;
        }
        self.home_new = new_rows;
        let _ = target;
        self.home_rows = rows_drawn;
        if self.home_focus >= self.home_targets().len() {
            self.home_focus = 0;
        }
        if let Some(a) = acted {
            self.nav.mouse();
            self.home_act(a);
        }
    }

    // --- tool workspaces ------------------------------------------------------------------------------------

    fn tool(&mut self, ui: &mut egui::Ui, l: &Layout) {
        let t = tokens();
        let p = ui.painter().clone();
        let base = t.slab.over(t.page);
        let editing = self.editing();
        if let Some(tb) = l.toolbar {
            p.rect_filled(tb, 0.0, draw::panel_fill());
            p.line_segment([tb.left_bottom(), tb.right_bottom()], Stroke::new(1.0, draw::rule()));
        }
        if let Some(tb) = l.toolbar.filter(|_| editing) {
            self.editor_toolbar(ui, tb);
        } else if let Some(tb) = l.toolbar {
            let pill_r = Rect::from_min_size(pos2(tb.right() - 160.0, tb.top() + 6.0), vec2(150.0, 28.0));
            self.spot(&p, Area::Toolbar, Spot::Test, pill_r.expand(3.0), false);
            ui.put(pill_r, theme::main_action("Test  F5"));
            p.text(
                pos2(tb.left() + 12.0, tb.center().y),
                Align2::LEFT_CENTER,
                words::TOOLS_COME,
                draw::label_font(15.0),
                rgb(t.dim),
            );
        }
        // The view.
        if editing {
            self.editor_view(ui, l);
        } else {
            p.rect_filled(l.view, 0.0, draw::page_fill());
            p.text(l.view.center() - vec2(0.0, 14.0), Align2::CENTER_CENTER, "3D view", font(TITLE, 22.0), rgb(t.ink));
            p.text(
                l.view.center() + vec2(0.0, 14.0),
                Align2::CENTER_CENTER,
                words::VIEW_PLACEHOLDER,
                font("body", 15.0),
                rgb(t.dim),
            );
        }
        // Left panel.
        if let Some(r) = l.left {
            p.rect_filled(r, 0.0, draw::panel_fill());
            p.line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0, draw::rule()));
            match l.left_dock {
                Dock::Open(_) => {
                    tabs(&p, r, &["Mod"], 0, self.nav.area == Area::Left);
                    let body = r.shrink2(vec2(12.0, 0.0)).with_min_y(r.top() + 44.0);
                    if editing {
                        self.editor_tree(ui, body);
                    } else {
                        self.mod_tree(&p, body);
                    }
                }
                _ => strip(&p, r, "Mod"),
            }
            if l.left_dock == Dock::Folded
                && ui.interact(r, egui::Id::unique("left-strip"), egui::Sense::click()).clicked()
            {
                self.set_memory(|m| m.left_folded = Some(false));
            }
        }
        // Bottom dock.
        if let Some(r) = l.bottom {
            p.rect_filled(r, 0.0, draw::panel_fill());
            p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, draw::rule()));
            // Problems · Log · Jobs, then the map editor's own tabs (World, Races).
            let extra = self.editor_tabs();
            let mut names = vec!["Problems", "Log", "Jobs"];
            names.extend(extra.iter().map(|t| t.1));
            let shown_tab = if self.bottom_tab == BottomTab::Editor { self.editor_tab() } else { None };
            let sel = match self.bottom_tab {
                BottomTab::Problems => 0,
                BottomTab::Log => 1,
                BottomTab::Jobs => 2,
                BottomTab::Editor => 3 + extra.iter().position(|t| Some(t.0) == shown_tab).unwrap_or(0),
            };
            let rects = tabs(&p, r, &names, sel, self.nav.area == Area::Bottom);
            if !editing {
                for (i, tr) in rects.iter().enumerate() {
                    self.spot(&p, Area::Bottom, Spot::BottomTab(i), *tr, false);
                }
            }
            if matches!(l.bottom_dock, Dock::Open(_)) {
                let body = r.with_min_y(r.top() + 42.0).shrink2(vec2(12.0, 0.0));
                use super::world::EditorBody;
                match self.bottom_tab {
                    BottomTab::Editor if editing => self.editor_bottom(ui, body, EditorBody::Tab),
                    BottomTab::Log if editing => self.editor_bottom(ui, body, EditorBody::Log),
                    BottomTab::Problems if editing => self.editor_bottom(ui, body, EditorBody::Problems),
                    tab => {
                        // The shell's own lists: what the checks found, the jobs that run.
                        let mut lines: Vec<(Mark, String)> = match tab {
                            BottomTab::Problems => self.problem_lines(),
                            BottomTab::Jobs => self.jobs().iter().map(|j| (Mark::Busy, j.line())).collect(),
                            _ => vec![],
                        };
                        if lines.is_empty() {
                            let line = match tab {
                                BottomTab::Problems => words::NO_PROBLEMS,
                                BottomTab::Jobs => words::NO_JOBS,
                                _ => words::NOTHING_LOGGED,
                            };
                            lines.push((Mark::Ok, line.to_string()));
                        }
                        let time = self.time();
                        let mut clip = p.clone();
                        clip.set_clip_rect(body);
                        for (i, (mark, line)) in lines.into_iter().enumerate() {
                            let row = Rect::from_min_size(
                                pos2(body.left() - 6.0, body.top() + 2.0 + i as f32 * 30.0),
                                vec2(body.width() + 12.0, 28.0),
                            );
                            if row.bottom() > body.bottom() {
                                break;
                            }
                            if !editing {
                                self.spot(&clip, Area::Bottom, Spot::BottomRow(i), row, true);
                            }
                            draw::lamp(&clip, pos2(row.left() + 14.0, row.center().y), 10.0, mark, time);
                            clip.text(
                                pos2(row.left() + 30.0, row.center().y),
                                Align2::LEFT_CENTER,
                                line,
                                font("body", 15.0),
                                rgb(t.ink),
                            );
                        }
                    }
                }
            }
            for (i, tr) in rects.into_iter().enumerate() {
                if ui.interact(tr, egui::Id::unique(("btab", i)), egui::Sense::click()).clicked() {
                    self.pick_bottom_tab(i);
                }
            }
        }
        // Right dock, or its strip and drawer.
        if let Some(r) = l.right {
            p.rect_filled(r, 0.0, draw::panel_fill());
            p.line_segment([r.left_top(), r.left_bottom()], Stroke::new(1.0, draw::rule()));
            match l.right_dock {
                Dock::Open(_) => self.right_dock(ui, &p, r),
                _ => strip(&p, r, "Inspector"),
            }
            if !matches!(l.right_dock, Dock::Open(_))
                && ui.interact(r, egui::Id::unique("right-strip"), egui::Sense::click()).clicked()
            {
                if matches!(l.right_dock, Dock::Drawer { .. }) {
                    self.nav.drawer_open = !self.nav.drawer_open;
                } else {
                    self.set_memory(|m| m.right_folded = Some(false));
                }
            }
        }
        if let Some(d) = l.drawer {
            p.rect_filled(d.translate(vec2(-6.0, 0.0)), 0.0, Color32::from_black_alpha(90));
            p.rect_filled(d, 0.0, rgb(t.dialog));
            p.line_segment([d.left_top(), d.left_bottom()], Stroke::new(1.0, theme::over(t.dialog_edge, t.dialog)));
            self.right_dock(ui, &p, d);
        }
        let _ = base;
        // The docks' inner edges: drag to size them (kept per workspace, as the folds are).
        let side_max = l.size.x * layout::SIDE_MAX_FRAC;
        if let (Some(r), Dock::Open(_)) = (l.left, l.left_dock) {
            let edge = Rect::from_min_max(pos2(r.right() - 3.0, r.top()), pos2(r.right() + 3.0, r.bottom()));
            self.resize(ui, "left", edge, egui::CursorIcon::ResizeHorizontal, |m, at| {
                m.left_w = Some((at.x - r.left()).clamp(layout::LEFT_MIN, side_max.max(layout::LEFT_MIN)));
            });
        }
        if let (Some(r), Dock::Open(_)) = (l.right, l.right_dock) {
            let edge = Rect::from_min_max(pos2(r.left() - 3.0, r.top()), pos2(r.left() + 3.0, r.bottom()));
            self.resize(ui, "right", edge, egui::CursorIcon::ResizeHorizontal, |m, at| {
                m.right_w = Some((r.right() - at.x).clamp(layout::RIGHT_MIN, side_max.max(layout::RIGHT_MIN)));
            });
        }
        if let (Some(r), Dock::Open(_)) = (l.bottom, l.bottom_dock) {
            let edge = Rect::from_min_max(pos2(r.left(), r.top() - 3.0), pos2(r.right(), r.top() + 3.0));
            self.resize(ui, "bottom", edge, egui::CursorIcon::ResizeVertical, |m, at| {
                m.bottom_h = Some((r.bottom() - at.y).clamp(layout::BOTTOM_DRAG_MIN, layout::BOTTOM_MAX));
            });
        }
    }

    /// A dock's edge to drag: while dragged, `set` sizes the dock in the workspace's memory from the pointer; the
    /// size is kept (written) when the drag ends.
    fn resize(
        &mut self,
        ui: &mut egui::Ui,
        id: &'static str,
        edge: Rect,
        cursor: egui::CursorIcon,
        set: impl Fn(&mut Memory, Pos2),
    ) {
        let resp = ui.interact(edge, egui::Id::unique(("resize", id)), egui::Sense::drag());
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(cursor);
            let mid = edge.center();
            let line = if edge.width() < edge.height() {
                [pos2(mid.x, edge.top()), pos2(mid.x, edge.bottom())]
            } else {
                [pos2(edge.left(), mid.y), pos2(edge.right(), mid.y)]
            };
            ui.painter().line_segment(line, Stroke::new(2.0, rgb(tokens().lime)));
        }
        if resp.dragged()
            && let Some(at) = resp.interact_pointer_pos()
        {
            self.nav.mouse();
            let mut m = self.memories.get(self.nav.workspace);
            set(&mut m, at);
            self.memories.set(self.nav.workspace, m);
        }
        if resp.drag_stopped() {
            self.set_memory(|_| {});
        }
    }

    fn set_memory(&mut self, f: impl FnOnce(&mut Memory)) {
        let mut m = self.memories.get(self.nav.workspace);
        f(&mut m);
        self.memories.set(self.nav.workspace, m);
        if let Some(md) = self.open_mod()
            && let Err(e) = self.memories.save(&md.dir)
        {
            log::warn!("studio: {e:#}");
        }
    }

    fn mod_tree(&mut self, p: &egui::Painter, r: Rect) {
        let t = tokens();
        let Some(m) = self.open_mod() else {
            p.text(r.left_top() + vec2(0.0, 8.0), Align2::LEFT_TOP, "No mod open", font("body", 15.0), rgb(t.dim));
            return;
        };
        let rows = [
            (format!("Props · {}", m.moved + m.added + m.hidden), 0.0),
            (format!("Moved · {}", m.moved), 18.0),
            (format!("Added · {}", m.added), 18.0),
            (format!("Hidden · {}", m.hidden), 18.0),
            (format!("Races · {}", m.races), 0.0),
        ];
        let mut y = r.top() + 4.0;
        for (i, (text, indent)) in rows.into_iter().enumerate() {
            let row = Rect::from_min_size(pos2(r.left() - 6.0, y), vec2(r.width() + 12.0, 28.0));
            self.spot(p, Area::Left, Spot::Tree(i), row, true);
            p.text(
                pos2(r.left() + indent, row.center().y),
                Align2::LEFT_CENTER,
                text,
                draw::label_font(15.0),
                rgb(t.ink),
            );
            y += 30.0;
        }
    }

    fn right_dock(&mut self, ui: &mut egui::Ui, p: &egui::Painter, r: Rect) {
        let t = tokens();
        let sel = match self.right_tab {
            RightTab::Inspector => 0,
            RightTab::History => 1,
            RightTab::Help => 2,
        };
        let rects = tabs(p, r, &["Inspector", "History", "Help"], sel, self.nav.area == Area::Right);
        for (tab, tr) in [RightTab::Inspector, RightTab::History, RightTab::Help].into_iter().zip(rects) {
            self.spot(p, Area::Right, Spot::RightTab(tab), tr, false);
            let i = tab as usize;
            if ui.interact(tr, egui::Id::unique(("rtab", i)), egui::Sense::click()).clicked() {
                self.right_tab = tab;
            }
        }
        let body = r.with_min_y(r.top() + 48.0).shrink2(vec2(14.0, 0.0));
        let mut clip = p.clone();
        clip.set_clip_rect(body);
        match self.right_tab {
            RightTab::Inspector if self.editing() => self.editor_inspector(ui, body),
            RightTab::Inspector => {
                clip.text(body.left_top(), Align2::LEFT_TOP, words::NOTHING_SELECTED, font(TITLE, 20.0), rgb(t.lime));
                clip.text(
                    body.left_top() + vec2(0.0, 32.0),
                    Align2::LEFT_TOP,
                    words::INSPECTOR_EMPTY,
                    font("body", 14.0),
                    rgb(t.dim),
                );
                let mut y = body.top() + 74.0;
                for (src, word) in
                    [(Source::Original, "original"), (Source::Yours, "yours"), (Source::AnotherMod, "another mod wins")]
                {
                    draw::source_mark(&clip, pos2(body.left() + 6.0, y + 8.0), 10.0, src);
                    clip.text(
                        pos2(body.left() + 20.0, y + 8.0),
                        Align2::LEFT_CENTER,
                        word,
                        font("body", 14.0),
                        rgb(t.ink),
                    );
                    y += 24.0;
                }
            }
            RightTab::History => {
                let line = self
                    .open_mod()
                    .and_then(|m| m.last_step.clone())
                    .map_or(words::NO_STEPS.to_string(), |s| format!("Last step: {s}"));
                let row = Rect::from_min_size(body.left_top() - vec2(6.0, 0.0), vec2(body.width() + 12.0, 28.0));
                self.spot(&clip, Area::Right, Spot::RightRow(0), row, true);
                clip.text(pos2(body.left(), row.center().y), Align2::LEFT_CENTER, line, font("body", 15.0), rgb(t.ink));
            }
            RightTab::Help => {
                let mut y = body.top();
                for (i, k) in KEYS.iter().take(10).enumerate() {
                    let row = Rect::from_min_size(pos2(body.left() - 6.0, y), vec2(body.width() + 12.0, 24.0));
                    if row.bottom() > body.bottom() {
                        break;
                    }
                    self.spot(&clip, Area::Right, Spot::RightRow(i), row, true);
                    let cy = row.center().y;
                    clip.text(pos2(body.left(), cy), Align2::LEFT_CENTER, k.keys, font(SEMI, 13.0), rgb(t.ink));
                    clip.text(
                        pos2(body.left() + 150.0, cy),
                        Align2::LEFT_CENTER,
                        k.word,
                        draw::label_font(13.0),
                        rgb(t.dim),
                    );
                    y += 24.0;
                }
            }
        }
    }

    // --- legend row, status bar, focus ----------------------------------------------------------------------

    fn legend(&self, ui: &mut egui::Ui, r: Rect) {
        let t = tokens();
        let p = ui.painter().clone();
        p.rect_filled(r, 0.0, draw::page_fill());
        p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, draw::rule()));
        let set = GlyphSet::for_input(self.nav.input);
        let mut x = r.right() - 12.0;
        let cy = r.center().y;
        let mut left_edge = r.right();
        for (btn, word) in self.nav.legend_roles().into_iter().rev() {
            let gs = glyphs_in(set, KeySet::Studio, btn);
            if gs.is_empty() {
                continue;
            }
            let lw = p.layout_no_wrap(word.to_string(), draw::label_font(17.0), Color32::WHITE).size().x;
            let gw: f32 = gs.iter().map(|g| draw::glyph_width(&p, *g)).sum::<f32>() + 4.0 * (gs.len() as f32 - 1.0);
            let item = gw + 8.0 + lw;
            if x - item < r.left() + 12.0 {
                break;
            }
            x -= lw;
            p.text(pos2(x, cy), Align2::LEFT_CENTER, word, draw::label_font(17.0), rgb(t.ink));
            x -= 8.0 + gw;
            let mut gx = x;
            for g in gs {
                gx += draw::glyph(&p, *g, gx, cy) + 4.0;
            }
            x -= 22.0;
            left_edge = x;
        }
        if r.right() >= layout::DRAWER_FROM_W {
            let note = if self.nav.input.is_pad() { words::LEGEND_NOTE_PAD } else { words::LEGEND_NOTE_KEYS };
            let mut clip = p.clone();
            clip.set_clip_rect(Rect::from_min_max(r.min, pos2(left_edge, r.bottom())));
            clip.text(pos2(r.left() + 16.0, cy), Align2::LEFT_CENTER, note, font("body", 14.0), rgb(t.dim));
        }
    }

    fn status(&mut self, ui: &mut egui::Ui, l: &Layout) {
        let t = tokens();
        let p = ui.painter().clone();
        let r = l.status;
        p.rect_filled(r, 0.0, draw::page_fill());
        p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, draw::rule()));
        let cy = r.center().y;
        let f = font("body", 13.0);
        let editor =
            self.editor.as_ref().filter(|_| self.editing()).and_then(|h| h.app.state.as_ref().map(|s| (&h.app, s)));
        // In World and Races: the open editor's own save state.
        let saved = match editor {
            Some((app, _)) if !app.save_note.is_empty() => Some(Saved::Error(app.save_note.clone())),
            // Saved only once the files have the steps: until the next quiet point they are in the journal (kept
            // through a crash, offered back on the next start) and the bar says so.
            Some((_, s)) => Some(match s.session.history.as_ref() {
                Some(h) if h.pending().is_some() => Saved::Behind,
                Some(h) if h.unsaved() => Saved::Saving,
                _ => Saved::Saved,
            }),
            None => self.open_mod().map(|m| m.saved.clone()),
        };
        let (mark, text) = match saved.as_ref() {
            None => (Mark::Info, words::NO_MOD_STATUS.to_string()),
            Some(Saved::Saved) => (Mark::Ok, words::SAVED.to_string()),
            Some(Saved::Saving) => (Mark::Info, words::SAVING.to_string()),
            Some(Saved::Behind) => (Mark::Warn, words::SAVING_BEHIND.to_string()),
            Some(Saved::Error(e)) => (Mark::Error, e.clone()),
        };
        draw::lamp(&p, pos2(r.left() + 14.0, cy), 10.0, mark, 0.0);
        let col = if mark == Mark::Error { rgb(t.err) } else { rgb(t.ink) };
        let mut x = p.text(pos2(r.left() + 26.0, cy), Align2::LEFT_CENTER, text, f.clone(), col).right();
        if let Some(m) = self.open_mod() {
            // While editing, the editor's own word on who else has the mod open.
            let others = match editor {
                Some((app, _)) => app.read_only().map(|_| "another program".to_string()),
                None => m.read_only_by.clone(),
            };
            if let Some(o) = &others {
                x = p
                    .text(
                        pos2(x + 18.0, cy),
                        Align2::LEFT_CENTER,
                        format!("Looking only: {o} has it"),
                        f.clone(),
                        rgb(t.warn),
                    )
                    .right();
            } else if let Some(label) = editor.and_then(|(_, s)| s.session.undo_label()) {
                x = p
                    .text(pos2(x + 18.0, cy), Align2::LEFT_CENTER, format!("Undo: {label}"), f.clone(), rgb(t.ink))
                    .right();
            } else if let Some(s) = &m.last_step {
                x = p
                    .text(pos2(x + 18.0, cy), Align2::LEFT_CENTER, format!("Undo: {s}"), f.clone(), rgb(t.ink))
                    .right();
            }
        }
        // Right: problems, then whether the mod is in the game; the optional ones drop on narrow windows.
        let mut right = r.right() - 12.0;
        if let Some(m) = self.open_mod() {
            let (a, b) =
                if m.in_game { (words::IN_GAME_ON, "") } else { (words::IN_GAME_OFF, words::TURN_ON_IN_LAUNCHER) };
            if !b.is_empty() {
                right = p.text(pos2(right, cy), Align2::RIGHT_CENTER, b, f.clone(), rgb(t.lime)).left();
            }
            right = p.text(pos2(right, cy), Align2::RIGHT_CENTER, a, f.clone(), rgb(t.ink)).left();
        }
        // The busy job line (the oldest job; the Jobs tab lists them all), then what the checks found. Both are
        // optional: they drop when the bar has no room (narrow windows first).
        let wide = l.size.x >= layout::DRAWER_FROM_W;
        let time = self.time();
        if let Some(job) = self.jobs().first() {
            let line = job.line();
            let w = p.layout_no_wrap(line.clone(), f.clone(), Color32::WHITE).size().x + 18.0;
            if right - 24.0 - w > x + 24.0 {
                let left = p.text(pos2(right - 24.0, cy), Align2::RIGHT_CENTER, line, f.clone(), rgb(t.ink)).left();
                draw::lamp(&p, pos2(left - 10.0, cy), 10.0, Mark::Busy, time);
                right = left - 18.0;
            }
        }
        let mut clicked = false;
        if wide && let Some(c) = self.counts() {
            let (a, b) = c.words();
            let bw =
                b.as_ref().map_or(0.0, |b| p.layout_no_wrap(format!(" · {b}"), f.clone(), Color32::WHITE).size().x);
            let aw = p.layout_no_wrap(a.clone(), f.clone(), Color32::WHITE).size().x;
            let end = right - 24.0;
            if end - aw - bw > x + 24.0 {
                if let Some(b) = b {
                    let left = p.text(pos2(end, cy), Align2::RIGHT_CENTER, b, f.clone(), rgb(t.warn)).left();
                    p.text(pos2(left, cy), Align2::RIGHT_CENTER, " · ", f.clone(), rgb(t.ink));
                }
                let col = if c.problems > 0 { rgb(t.err) } else { rgb(t.ink) };
                p.text(pos2(end - bw, cy), Align2::RIGHT_CENTER, a, f.clone(), col);
                // A click shows them: the Problems tab while the map editor has the mod, else the Mod page's list.
                let hit = Rect::from_min_max(pos2(end - bw - aw - 4.0, r.top()), pos2(end + 4.0, r.bottom()));
                clicked = ui.interact(hit, egui::Id::unique("status-problems"), egui::Sense::click()).clicked();
            }
        }
        if clicked {
            self.nav.mouse();
            if self.editing() {
                self.set_memory(|m| m.bottom_folded = Some(false));
                self.bottom_tab = BottomTab::Problems;
            } else {
                self.workspace(Workspace::Mod);
            }
        }
    }

    /// The focused area: a 2 px lime line along its top (only while keys or a pad are used).
    fn focus_line(&self, ui: &mut egui::Ui, l: &Layout) {
        if !self.nav.legend() {
            return;
        }
        let r = match self.nav.area {
            Area::Rail => Some(l.rail),
            Area::Address => Some(l.menu.address.shrink2(vec2(0.0, 4.0))),
            Area::Toolbar => l.toolbar,
            Area::Left => l.left,
            Area::View => Some(l.view),
            Area::Right => l.drawer.or(l.right),
            Area::Bottom => l.bottom,
        };
        if let Some(r) = r {
            ui.painter().line_segment(
                [r.left_top() + vec2(0.0, 1.0), r.right_top() + vec2(0.0, 1.0)],
                Stroke::new(2.0, rgb(tokens().lime)),
            );
        }
        // The focused control inside the area: its ring, over everything the area drew.
        if self.spot_area()
            && let Some(r) = self.spots.focused_rect(self.nav.area)
        {
            focus::ring(ui.painter(), r);
        }
    }

    /// Find's list under its field: Go to, Commands (with their keys), Places you were. A click, or Enter / A on
    /// the focused entry, takes it.
    fn find_list(&mut self, ui: &mut egui::Ui, l: &Layout) {
        let t = tokens();
        let f = l.menu.find;
        let w = 420.0f32.min(l.size.x - 24.0);
        let at = pos2((f.right() - w).max(12.0), f.bottom() + 2.0);
        let items = self.find_items();
        self.find_focus = self.find_focus.min(items.len().saturating_sub(1));
        let ring = self.nav.legend();
        let names: Vec<String> = items
            .iter()
            .map(|i| match i {
                FindItem::Go(ws) => ws.name().to_string(),
                FindItem::Run(c) => KEYS.iter().find(|k| k.command == *c).map_or("", |k| k.word).to_string(),
                FindItem::Place(n) => self.places.back_list().nth(n - 1).map_or(String::new(), |a| self.place_name(a)),
            })
            .collect();
        let keys: Vec<String> = items
            .iter()
            .map(|i| match i {
                FindItem::Go(ws) => format!("Ctrl+{}", ws.ctrl_key()),
                FindItem::Run(c) => KEYS.iter().find(|k| k.command == *c).map_or("", |k| k.keys).to_string(),
                FindItem::Place(_) => String::new(),
            })
            .collect();
        let mut run = None;
        egui::Area::new(egui::Id::unique("find-list"))
            .fixed_pos(at)
            .fade_in(false)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(w - 16.0);
                    let mut head = None;
                    for (k, item) in items.iter().enumerate() {
                        let h = match item {
                            FindItem::Go(_) => words::FIND_GO_TO,
                            FindItem::Run(_) => words::FIND_COMMANDS,
                            FindItem::Place(_) => words::PLACES_YOU_WERE,
                        };
                        if head != Some(h) {
                            head = Some(h);
                            ui.label(RichText::new(h).font(font("body", 11.0)).color(rgb(t.dim)));
                        }
                        let focused = k == self.find_focus;
                        let b = egui::Button::new(&names[k]).shortcut_text(&keys[k]).frame(false);
                        let resp = ui.add_sized(vec2(ui.available_width(), ui.spacing().interact_size.y), b);
                        if focused && ring {
                            focus::ring(ui.painter(), resp.rect);
                        }
                        if resp.clicked() {
                            run = Some(*item);
                        }
                    }
                });
            });
        if let Some(i) = run {
            self.nav.mouse();
            self.find_run(i);
        }
    }

    /// The places you were, under ← (a right-click on it): newest first; a click goes back to one.
    fn places_list(&mut self, ui: &mut egui::Ui, l: &Layout) {
        let names: Vec<String> = self.places.back_list().take(PLACES_SHOWN).map(|a| self.place_name(a)).collect();
        let at = l.menu.back_fwd.left_bottom() + vec2(0.0, 2.0);
        let mut pick = None;
        let mut away = false;
        egui::Area::new(egui::Id::unique("places-list"))
            .fixed_pos(at)
            .fade_in(false)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(260.0);
                    ui.label(RichText::new(words::PLACES_YOU_WERE).font(font("body", 11.0)).color(rgb(tokens().dim)));
                    for (n, name) in names.iter().enumerate() {
                        if ui.add(egui::Button::new(name).frame(false)).clicked() {
                            pick = Some(n + 1);
                        }
                    }
                });
                away = ui.input(|i| i.pointer.any_pressed()) && !ui.ui_contains_pointer();
            });
        if let Some(n) = pick {
            self.places_menu = false;
            self.go_back(n);
        } else if away || names.is_empty() {
            self.places_menu = false;
        }
    }

    /// The focused thing's menu (Shift+F10, the Menu key, the pad's X), beside the focused area.
    fn focus_menu(&mut self, ui: &mut egui::Ui, l: &Layout) {
        let t = tokens();
        let at = match self.nav.area {
            Area::Rail => l.rail.right_top() + vec2(8.0, 40.0),
            Area::Address => l.menu.address.left_bottom() + vec2(0.0, 4.0),
            Area::Right => l.drawer.or(l.right).map_or(l.view.center(), |r| r.left_top() + vec2(20.0, 120.0)),
            Area::Left => l.left.map_or(l.view.center(), |r| r.left_top() + vec2(20.0, 120.0)),
            Area::Bottom => l.bottom.map_or(l.view.center(), |r| r.left_top() + vec2(40.0, 10.0)),
            Area::Toolbar | Area::View => l.view.left_top() + vec2(40.0, 40.0),
        };
        let mut pick = None;
        let mut pick_changes = None;
        egui::Area::new(egui::Id::unique("focus-menu"))
            .fixed_pos(at)
            .fade_in(false)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(240.0);
                    let _ = ui.add_enabled(false, egui::Button::new(ob_look::words::UNDO).shortcut_text("Ctrl+Z"));
                    let _ = ui.add_enabled(false, egui::Button::new(ob_look::words::REDO).shortcut_text("Ctrl+Y"));
                    ui.separator();
                    let _ = ui.add_enabled(false, egui::Button::new(words::TEST_HERE).shortcut_text("Shift+F5"));
                    if self.nav.workspace == Workspace::Changes && self.nav.area == Area::View {
                        ui.separator();
                        let can = !self.changes.reset_picks().is_empty();
                        if ui.add_enabled(can, egui::Button::new(words::RESET_TO_ORIGINAL)).clicked() {
                            pick_changes = Some(true);
                        }
                        if ui.add(egui::Button::new(words::SHOW_IN_WORLD)).clicked() {
                            pick_changes = Some(false);
                        }
                    }
                    if ui.add(egui::Button::new(words::COPY_ADDRESS).shortcut_text("Ctrl+Shift+C")).clicked() {
                        pick = Some(Command::CopyAddress);
                    }
                    ui.label(RichText::new(self.address().to_string()).font(font("body", 12.0)).color(rgb(t.dim)));
                });
            });
        if let Some(c) = pick {
            self.command(c, None);
            self.nav.level = Level::Row;
        }
        match pick_changes {
            Some(true) => {
                let session = self.editor.as_mut().and_then(|h| h.app.state.as_mut()).map(|s| &mut s.session);
                self.changes.reset_with(session);
                self.nav.level = Level::Row;
            }
            Some(false) => {
                self.nav.level = Level::Row;
                if let Some(a) = self.changes.focused_address().and_then(|a| Address::parse(&a).ok()) {
                    self.verdict = self.go(&a).err();
                }
            }
            None => {}
        }
    }

    fn controls_window(&mut self, ui: &mut egui::Ui) {
        let mut open = true;
        egui::Window::new("Controls").open(&mut open).collapsible(false).resizable(false).show(ui.ctx(), |ui| {
            egui::Grid::new("controls").striped(true).show(ui, |ui| {
                for k in KEYS {
                    ui.label(RichText::new(k.keys).font(font(SEMI, 13.0)));
                    ui.label(RichText::new(k.word).font(draw::label_font(14.0)));
                    ui.label(k.help);
                    ui.end_row();
                }
            });
        });
        self.controls = open;
    }
}

/// A dock's tab strip at the top of `r`: the current tab filled (lime while the dock has focus).
/// Returns the tabs' rectangles (the ones that fit), to click. A tab is 88 wide, wider for a longer name.
fn tabs(p: &egui::Painter, r: Rect, names: &[&str], sel: usize, focused: bool) -> Vec<Rect> {
    let t = tokens();
    let mut out = vec![];
    let mut x = r.left() + 6.0;
    for (i, n) in names.iter().enumerate() {
        let w = (p.layout_no_wrap(n.to_string(), draw::label_font(16.0), Color32::WHITE).size().x + 24.0).max(88.0);
        let tr = Rect::from_min_size(pos2(x, r.top() + 4.0), vec2(w, 28.0));
        x += w + 4.0;
        if tr.right() > r.right() {
            break;
        }
        out.push(tr);
        if i == sel {
            let fill = if focused { rgb(t.lime) } else { theme::over(t.pressed, t.slab.over(t.page)) };
            p.rect_filled(tr, 4.0, fill);
        }
        let col = if i == sel && focused {
            rgb(t.on_play)
        } else if i == sel {
            rgb(t.lime)
        } else {
            rgb(t.ink)
        };
        p.text(tr.center(), Align2::CENTER_CENTER, *n, draw::label_font(16.0), col);
    }
    out
}

/// A folded panel's strip with its name written upwards.
fn strip(p: &egui::Painter, r: Rect, name: &str) {
    let t = tokens();
    let g = p.layout_no_wrap(name.to_string(), draw::label_font(14.0), rgb(t.ink));
    let pos = Pos2::new(r.center().x - g.size().y / 2.0, r.top() + 16.0 + g.size().x);
    p.add(egui::epaint::TextShape::new(pos, g, rgb(t.ink)).with_angle(-std::f32::consts::FRAC_PI_2));
}

//! The window's keys and pad inside an area (docs/modding/studio.md, "Keyboard and pad"): the shell's own controls
//! ([`super::focus`]), Tab on past an area's ends, the Find list's entries, the places behind ←, the mouse's back
//! and forward buttons.

use ob_look::legends::InputKind;

use super::address::{Address, CRUMB, STUDIO, Target};
use super::focus::Spot;
use super::nav::{Area, Command, KEYS, Level};
use super::shell::{BottomTab, Studio};
use super::workspace::{Kind, Workspace};

/// One entry of the Find list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindItem {
    /// Go to a workspace.
    Go(Workspace),
    /// Open one of your mods (an index into the window's mods).
    Mod(usize),
    /// Run a command (the key table's).
    Run(Command),
    /// A place you were: so many steps back.
    Place(usize),
    /// A neighbourhood of the open mod's region (an index into the game's named places): the editor goes there.
    Hood(usize),
}

/// The commands Find lists and runs (the ones that do something by themselves; moving focus is not one).
pub fn runs_from_find(c: Command) -> bool {
    matches!(
        c,
        Command::Test
            | Command::LastPlace
            | Command::NextPlace
            | Command::EditAddress
            | Command::CopyAddress
            | Command::NewMod
            | Command::Workspace(Workspace::Changes)
            | Command::Workspace(Workspace::Data)
            | Command::Tutorial
            | Command::Undo
            | Command::Redo
    )
}

/// How many places Find and the list under ← show.
pub const PLACES_SHOWN: usize = 8;

impl Studio {
    /// The pad kind of the last input (None: keys or the mouse), for commands run on a control's behalf.
    fn input_pad(&self) -> Option<InputKind> {
        self.nav.input.is_pad().then_some(self.nav.input)
    }

    /// The shell's own controls have the keys: the focused area is not a page's rows or the map editor's own part,
    /// and nothing is typed or open over it.
    pub(super) fn on_spots(&self) -> bool {
        self.spot_area() && self.verdict.is_none()
    }

    /// The focused area's controls are the shell's (its ring shows), whatever message is up.
    pub(super) fn spot_area(&self) -> bool {
        !self.nav.find_open
            && !self.nav.address_text
            && !self.places_menu
            && self.new_mod.is_none()
            && self.nav.level != Level::Menu
            && !(self.nav.area == Area::View && self.nav.workspace.kind() == Kind::Page)
            && !(self.editing() && matches!(self.nav.area, Area::View | Area::Left | Area::Bottom))
    }

    /// A key (or the D-pad's press as that key) on the shell's controls: ↑ ↓ move, Tab moves and goes on past the
    /// ends, ← → change a tab, Enter takes the control.
    pub(super) fn spot_key(&mut self, key: egui::Key, shift: bool) {
        use egui::Key as K;
        let a = self.nav.area;
        self.nav.level = Level::Row;
        match key {
            K::ArrowUp => {
                self.spots.step(a, -1);
            }
            K::ArrowDown => {
                self.spots.step(a, 1);
            }
            K::Tab => {
                let d = if shift { -1 } else { 1 };
                if !self.spots.step(a, d) {
                    self.tab_out(d);
                }
            }
            K::ArrowLeft | K::ArrowRight => {
                let d = if key == K::ArrowLeft { -1 } else { 1 };
                if self.spots.focused(a).is_some_and(Spot::is_tab) && self.spots.step(a, d) {
                    match self.spots.focused(a) {
                        Some(s) if s.is_tab() => self.spot_act(s),
                        _ => {
                            self.spots.step(a, -d);
                        }
                    }
                }
            }
            K::Enter => {
                if let Some(s) = self.spots.focused(a) {
                    self.spot_act(s);
                }
            }
            _ => {}
        }
    }

    /// Take a control: what a click on it does.
    pub fn spot_act(&mut self, s: Spot) {
        let pad = self.input_pad();
        match s {
            Spot::Rail(w) => self.workspace(w),
            Spot::Back => self.command(Command::LastPlace, pad),
            Spot::Forward => self.command(Command::NextPlace, pad),
            Spot::Address => self.command(Command::EditAddress, pad),
            Spot::Find => self.command(Command::Find, pad),
            Spot::Test => self.command(Command::Test, pad),
            Spot::RightTab(t) => self.right_tab = t,
            Spot::BottomTab(i) => self.pick_bottom_tab(i),
            Spot::Tree(_) | Spot::RightRow(_) | Spot::BottomRow(_) => {}
        }
    }

    /// The bottom dock's `i`-th tab: Problems, Log, Jobs, then the map editor's.
    pub(super) fn pick_bottom_tab(&mut self, i: usize) {
        match i {
            0 => self.bottom_tab = BottomTab::Problems,
            1 => self.bottom_tab = BottomTab::Log,
            2 => self.bottom_tab = BottomTab::Jobs,
            n => {
                if let Some(t) = self.editor_tabs().get(n - 3) {
                    self.bottom_tab = BottomTab::Editor;
                    self.editor_pick_tab(t.0);
                }
            }
        }
    }

    /// Tab past an area's last control (`d` = 1) or Shift+Tab before its first (-1): the next or previous area,
    /// on its first or last control.
    pub(super) fn tab_out(&mut self, d: i32) {
        let l = self.layout_now();
        let c = if d > 0 { Command::NextArea } else { Command::PrevArea };
        // An area with nothing to focus (the view's placeholder) is passed by.
        for _ in 0..Area::ALL.len() {
            self.nav.run(c, self.input_pad(), &l);
            let a = self.nav.area;
            let own = (a == Area::View && self.nav.workspace.kind() == Kind::Page)
                || (self.editing() && matches!(a, Area::View | Area::Left | Area::Bottom));
            if own || self.spots.count(a) > 0 || (a == Area::Right && self.nav.drawer_open) {
                break;
            }
        }
        let a = self.nav.area;
        self.nav.level = Level::Row;
        let end = d < 0;
        match self.nav.workspace {
            Workspace::Home if a == Area::View => {
                self.home_focus = if end { self.home_targets().len().saturating_sub(1) } else { 0 }
            }
            Workspace::Mod if a == Area::View => self.mod_page.step_focus(if end { 1000 } else { -1000 }),
            Workspace::Browser if a == Area::View => {
                self.browser.focus = if end { self.browser.lines().saturating_sub(1) } else { 0 }
            }
            Workspace::Changes if a == Area::View => {
                self.changes.focus = if end { self.changes.lines().len().saturating_sub(1) } else { 0 }
            }
            Workspace::Data if a == Area::View => {
                let n = self.data.grid.as_ref().map_or(0, |g| g.counts().1);
                self.data.row = if end { n.checked_sub(1) } else { Some(0).filter(|_| n > 0) }
            }
            _ => self.spots.set(a, if end { usize::MAX } else { 0 }),
        }
    }

    /// One row on in a page's rows (Home, Mod, Changes); false past its ends.
    pub(super) fn page_step(&mut self, d: i32) -> bool {
        match self.nav.workspace {
            Workspace::Home => {
                let n = self.home_targets().len() as i32;
                let j = self.home_focus as i32 + d;
                if j < 0 || j >= n {
                    return false;
                }
                self.home_focus = j as usize;
                true
            }
            Workspace::Mod => {
                let before = self.mod_page.focus;
                self.mod_page.step_focus(d);
                self.mod_page.focus != before
            }
            Workspace::Browser => self.browser.step_focus(d),
            Workspace::Changes => {
                let before = self.changes.focus;
                self.changes.step_focus(d, false);
                self.changes.focus != before
            }
            Workspace::Data => self.data.step_row(d),
            _ => false,
        }
    }

    // --- Find -----------------------------------------------------------------------------------------------

    /// The Find list's entries for the typed words: workspaces, your mods, commands, places you were.
    pub fn find_items(&self) -> Vec<FindItem> {
        let q = self.find_text.trim().to_ascii_lowercase();
        let hit = |s: &str| q.is_empty() || s.to_ascii_lowercase().contains(&q);
        let mut v: Vec<FindItem> = Workspace::ALL.into_iter().filter(|w| hit(w.name())).map(FindItem::Go).collect();
        v.extend(
            self.mods.iter().enumerate().filter(|(_, m)| hit(&m.name) || hit(&m.folder)).map(|(i, _)| FindItem::Mod(i)),
        );
        let mut seen = vec![];
        for k in KEYS.iter().filter(|k| runs_from_find(k.command) && hit(k.word)) {
            let can = match k.command {
                Command::LastPlace => self.places.can_back(),
                Command::NextPlace => self.places.can_forward(),
                Command::Test => self.editing(),
                _ => true,
            };
            if can && !seen.contains(&k.command) {
                seen.push(k.command);
                v.push(FindItem::Run(k.command));
            }
        }
        let places: Vec<String> = self.places.back_list().map(|a| self.place_name(a)).collect();
        for (i, name) in places.into_iter().enumerate().take(PLACES_SHOWN) {
            if hit(&name) {
                v.push(FindItem::Place(i + 1));
            }
        }
        for (i, p) in self.hoods().iter().enumerate() {
            if hit(&p.name) {
                v.push(FindItem::Hood(i));
            }
        }
        v
    }

    /// Find open on its mods (Home's Open a mod): the first mod focused, or the first entry when there is none.
    pub fn find_mods(&mut self) {
        self.nav.find_open = true;
        self.find_text.clear();
        self.find_focus = self.find_items().iter().position(|i| matches!(i, FindItem::Mod(_))).unwrap_or(0);
    }

    /// Open the `i`-th mod from Find (as a mod row on Home does).
    fn open_from_find(&mut self, i: usize) {
        if i < self.mods.len() {
            self.switch_mod(i);
            self.home_focus = 0;
        }
    }

    /// ↑ ↓ (the D-pad) in the Find list.
    pub(super) fn find_step(&mut self, d: i32) {
        let n = self.find_items().len() as i32;
        self.find_focus = (self.find_focus as i32 + d).clamp(0, (n - 1).max(0)) as usize;
    }

    /// Enter / A in Find: a typed or pasted address goes there; otherwise the focused entry runs.
    pub(super) fn find_enter(&mut self) {
        let text = self.find_text.trim().to_string();
        let item = self.find_items().get(self.find_focus).copied();
        self.nav.find_open = false;
        self.find_text.clear();
        self.find_focus = 0;
        if text.starts_with("ob://") {
            match Address::parse(&text) {
                Ok(a) => self.verdict = self.go(&a).err(),
                Err(e) => self.verdict = Some(e.to_string()),
            }
            return;
        }
        if let Some(i) = item {
            self.find_run(i);
        }
    }

    /// Run one of Find's entries.
    pub fn find_run(&mut self, i: FindItem) {
        self.nav.find_open = false;
        self.find_text.clear();
        self.find_focus = 0;
        match i {
            FindItem::Go(w) => self.workspace(w),
            FindItem::Mod(i) => self.open_from_find(i),
            FindItem::Run(c) => self.command(c, self.input_pad()),
            FindItem::Place(n) => self.go_back(n),
            FindItem::Hood(i) => self.go_hood(i),
        }
    }

    // --- places ---------------------------------------------------------------------------------------------

    /// `n` places back (the list under ←, Find's places).
    pub fn go_back(&mut self, n: usize) {
        let mut to = None;
        for _ in 0..n {
            match self.places.back() {
                Some(a) => to = Some(a.clone()),
                None => break,
            }
        }
        if let Some(a) = to {
            let _ = self.go_quiet(&a);
        }
    }

    /// A place as names ("Harbour edits › World"), as the address bar shows where you are.
    pub fn place_name(&self, a: &Address) -> String {
        let pack = if a.pack == STUDIO {
            super::words::STUDIO.to_string()
        } else {
            self.mods.iter().find(|m| m.folder == a.pack).map_or_else(|| a.pack.clone(), |m| m.name.clone())
        };
        let what = match &a.target {
            Target::View { view, .. } => {
                Workspace::from_view(&format!("@{view}")).map_or_else(|| view.clone(), |w| w.name().to_string())
            }
            Target::Item { key, .. } => key.last().cloned().unwrap_or_default(),
        };
        format!("{pack}{CRUMB}{what}")
    }

    /// The mouse's back and forward buttons (4 and 5): taken out of the input, run.
    pub(super) fn take_mouse_places(&mut self, raw: &mut egui::RawInput) {
        let mut run = vec![];
        raw.events.retain(|e| match e {
            egui::Event::PointerButton {
                button: b @ (egui::PointerButton::Extra1 | egui::PointerButton::Extra2),
                ..
            } => {
                if let egui::Event::PointerButton { pressed: true, .. } = e {
                    run.push(if *b == egui::PointerButton::Extra1 { Command::LastPlace } else { Command::NextPlace });
                }
                false
            }
            _ => true,
        });
        for c in run {
            if self.new_mod.is_none() && !self.nav.address_text {
                self.nav.mouse();
                self.command(c, None);
                self.nav.mouse();
            }
        }
    }
}

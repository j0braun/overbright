use crate::frontend::fe_ids;
use super::*;

/// MC_Main (factory entry 0x7F9144 → 0x4F3320, vtable 0x797DFC, message
/// handler 0x4FD570 over the base 0x4FB8F0) in Start New Career's
/// operation: 0x4A3100 mode 0x21961 (low nibble 1 pushes the screen,
/// operation 0x60 create a profile, 0x100 hand the exit message to the
/// caller, 0x1000 go on with a named profile though the save is declined,
/// 0x20000 a new career, 0x800 not traced).
///
/// On the package's start (0x35F8620B → 0x4F3570) operation 0x60 sends
/// itself 0x5A051729 (0x4F33A0): with a profile current (0x83A9E0 +0x20)
/// state 1, Yes/No "Creating a new profile will …" (0xD36620FE, tag
/// 0x5000000; Yes starts the new career 0x532840 and goes to the name,
/// No leaves with 0x8867412D), else state 2, Yes/No "Would you like to
/// create a new profile?" (0x02B641C0, tag 0x1000000, flag 0x200000; Yes
/// goes to the name, No leaves). Each Yes/No (0x4A3890) shows the button
/// pairs 0 / 1 of the tables 0x78DFBC / 0x78DFD4, hides the others and
/// puts the focus on No (0xB8A7C6CD); Yes is the object 0xB8A7C6CC
/// (0x4F3780). Back does nothing in this operation (0x4FB8F0).
///
/// The name (state 3, 0x5564A0): the message and buttons go, the header
/// 0x426C7B4D says "Create Profile" (0xF54CC524), the group PC_NAME_ENTRY
/// runs SHOW and the editor (0x555F90 → 0x4FF3B0) edits the field
/// 0x5B9D88B9 from "NAME" (0x818F0385; empty when the game or system
/// language is Korean, 0x7F70D0 / 0x7FCA00 = 8), at most 7 characters of
/// kind 6: letters and digits only (0x4FF310). The text is the edit
/// buffer itself (0x4FF3B0 copies it to 0x83A8B0): typed characters are
/// appended to it, the first as any other, so the player erases "NAME"
/// with Backspace. 0x551280 counts every key it gets (+0x208, 0x83A9B8,
/// zeroed by 0x4FF3B0) and ignores a 0x0D that is the very first one
/// (**verified**, 0x5512B9): an Enter straight away leaves "NAME" as it is. Its key handler (0x551280):
/// Backspace drops the last, Enter with text sends 0xDA5B8712 (0x54C800),
/// Escape puts the text back and sends 0xC9D30688 (0x54C8B0). Either hides
/// the group (HIDE) and sets the header to "Save/Load" (0x5446B0).
/// 0xDA5B8712 names the profile (0x502F40) and asks "Are you sure you
/// want to save^the profile named %s?" (0x2A9BA6F9, state 5, 0x4A3C10;
/// the state setter 0x4E8D30 turns state 5 into 6, saving unasked, when
/// 0x844AD9 — the Options' Save row at Autosave, 0x49B170 — is set and
/// the operation lacks 0x200); Yes
/// saves (state 6, 0x4D9AB0) and leaves with 0x461A18EE; No sends
/// 0x8867412D, which with 0x1000 and a named profile leaves with
/// 0x461A18EE too (0x4FD5EE). 0xC9D30688 goes back to state 2 when the
/// flag 0x200000 is set, else leaves (0x4FBA4B).
pub struct McMain {
    pub step: McStep,
    /// Whether a profile was current when the screen opened (state 1).
    pub replacing: bool,
    pub name: String,
    /// 0 Yes, 1 No.
    pub selected: usize,
    pub opened: f32,
    /// 0x844AD9: the Save option at Autosave (state 5 becomes 6).
    pub autosave: bool,
    /// The editor's key count (0x83A9B8): every character 0x551280 gets,
    /// Enter, Escape and Backspace included.
    pub keys: u32,
    /// Opened by Start New Career (mode 0x21961: the career starts after);
    /// else by the Profile Manager's Create (0x861) or the boot.
    pub career: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McStep {
    /// State 1 or 2: the Yes/No before the name.
    Ask,
    /// State 3: the name editor.
    Name,
    /// State 5: "save the profile named %s?".
    Confirm,
}

/// What MC_Main leaves with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McOutcome {
    /// 0x8867412D: back to the caller.
    Cancel,
    /// 0x461A18EE: the profile is named; `save` when the player said Yes.
    Done { name: String, save: bool },
}

/// MC_Main's button groups (Yes, No, …), for the mouse.
pub const MC_MAIN_GROUPS: [u32; 6] = mc_main::GROUPS;

mod mc_main {
    pub const HEADER: u32 = crate::frontend::fe_ids::HEADER;
    pub const MESSAGE: u32 = crate::frontend::fe_ids::DIALOG_MESSAGE;
    pub const NAME_ENTRY: u32 = crate::frontend::fe_ids::NAME_ENTRY;
    pub const FIELD: u32 = crate::frontend::fe_ids::FIELD;
    /// The text entry's white bar (INIT chains off the screen).
    pub const CURSOR_BAR: u32 = crate::frontend::fe_ids::CURSOR_BAR;
    /// The button labels 0x78DFBC and their groups 0x78DFD4.
    pub const LABELS: [u32; 6] = [crate::frontend::fe_ids::BUTTON_TEXT_0, crate::frontend::fe_ids::BUTTON_TEXT_1, crate::frontend::fe_ids::BUTTON, crate::frontend::fe_ids::LABELS_3, crate::frontend::fe_ids::LABELS_4, crate::frontend::fe_ids::LABELS_5];
    pub const GROUPS: [u32; 6] = [crate::frontend::fe_ids::MEMCARD_GROUPS_0, crate::frontend::fe_ids::MEMCARD_GROUPS_1, crate::frontend::fe_ids::BUTTON_GROUP, crate::frontend::fe_ids::GROUPS_3, crate::frontend::fe_ids::GROUPS_4, crate::frontend::fe_ids::GROUPS_5];
    /// The editor's limit (0x5564A0 → 0x4FF3B0 +0x20C).
    pub const MAX_NAME: usize = 7;
}

/// MC_Background.fng (0x4C4F50 → 0x537990): the boot's memory-card base
/// package (PC_load.tga over the whole screen), drawn under MC_Main, the
/// profile list and the messages while the boot's profile step runs.
pub fn mc_background_quads(assets: &Assets, now: f32) -> Vec<Quad> {
    let Some(package) = assets.package("MC_Background") else { return Vec::new() };
    layout(assets, package, &ScreenState::default(), now)
}

impl McMain {
    pub fn new(replacing: bool, now: f32) -> McMain {
        McMain {
            step: McStep::Ask,
            replacing,
            name: String::new(),
            selected: 1,
            opened: now,
            autosave: false,
            keys: 0,
            career: true,
        }
    }

    fn to_name(&mut self, now: f32) {
        self.step = McStep::Name;
        // 0x5564EE: "NAME" (0x818F0385) in English (0x7F70D0 = 0).
        self.name = "NAME".into();
        self.keys = 0;
        self.opened = now;
    }

    pub fn key(&mut self, key: Key, now: f32) -> Option<McOutcome> {
        match self.step {
            McStep::Ask | McStep::Confirm => match key {
                Key::Left | Key::Up => self.selected = 0,
                Key::Right | Key::Down => self.selected = 1,
                Key::Accept if self.step == McStep::Ask => {
                    if self.selected == 1 {
                        return Some(McOutcome::Cancel);
                    }
                    self.to_name(now);
                }
                Key::Accept => return Some(McOutcome::Done { name: self.name.clone(), save: self.selected == 0 }),
                _ => {}
            },
            // 0x5512A9: Enter (0x0D) as the editor's first key does nothing.
            McStep::Name if key == Key::Accept && self.keys == 0 => self.keys = 1,
            McStep::Name => match key {
                // Autosave on: state 6 at once, saving (0x4E8DDD).
                Key::Accept if !self.name.is_empty() && self.autosave => {
                    return Some(McOutcome::Done { name: self.name.clone(), save: true });
                }
                Key::Accept if !self.name.is_empty() => {
                    self.step = McStep::Confirm;
                    self.selected = 1;
                    self.opened = now;
                }
                // Escape: state 2 again on the "create" path, else out.
                Key::Back => {
                    self.keys += 1;
                    if self.replacing {
                        return Some(McOutcome::Cancel);
                    }
                    self.step = McStep::Ask;
                    self.selected = 1;
                    self.opened = now;
                }
                _ => {}
            },
        }
        None
    }

    /// Controller support extra (Controller Layout Modern): the name edited with the
    /// d-pad / stick — up / down step the last character through A–Z then
    /// 0–9, right adds an "A", left drops the last character (as
    /// Backspace). `dir` 0 up, 1 down, 2 left, 3 right.
    pub fn pad_edit(&mut self, dir: usize) {
        if self.step != McStep::Name {
            return;
        }
        const SET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        match dir {
            0 | 1 => {
                let Some(last) = self.name.pop() else {
                    self.char('A');
                    return;
                };
                let at = SET.iter().position(|&c| c == last.to_ascii_uppercase() as u8).unwrap_or(0);
                let next = if dir == 0 { (at + 1) % SET.len() } else { (at + SET.len() - 1) % SET.len() };
                self.name.push(SET[next] as char);
                self.keys += 1;
            }
            2 => self.char('\u{8}'),
            _ => self.char('A'),
        }
    }

    /// A typed character in the editor (0x551280): Backspace (8) drops
    /// the last; letters and digits go on up to the limit — after the
    /// starting "NAME" too, which no key clears.
    pub fn char(&mut self, c: char) {
        // Enter and Escape come through `key` (they count there).
        if self.step != McStep::Name || c == '\r' || c == '\u{1b}' {
            return;
        }
        self.keys += 1;
        if c == '\u{8}' {
            self.name.pop();
        } else if c.is_ascii_alphanumeric() && self.name.len() < mc_main::MAX_NAME {
            self.name.push(c);
        }
    }

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("MC_Main") else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Select");
        let label = |h: u32, fallback: &str| assets.strings.get(h).unwrap_or(fallback).to_string();
        for (&l, &g) in mc_main::LABELS.iter().zip(&mc_main::GROUPS) {
            state.hidden.insert(l);
            state.hidden.insert(g);
        }
        match self.step {
            McStep::Name => {
                state.text.insert(mc_main::HEADER, label(fe_ids::TXT_CREATE_PROFILE, "Create Profile"));
                state.hidden.insert(mc_main::MESSAGE);
                state.run(mc_main::NAME_ENTRY, "SHOW", 0.0);
                // The white 1×18 bar 0x557FE993: its INIT chains to
                // 0x16A259, (-10, -300), off the screen; nothing brings it
                // back (Wine: no bar before the name).
                state.chained.insert(mc_main::CURSOR_BAR);
                // 0x540AC0 on the tick 0xC98356BA: the counter +8 steps
                // `c + 1`, back to 0 once it reaches 0x5A (0x540B30), so it
                // runs 0..89; the field shows the buffer plus "|"
                // (0x79BFC4) while c < 0x2D (45 of the 90 values), else " "
                // (0x7909A0). One tick a frame; the original's 120 fps cap
                // gives the 0.75 s blink seen under Wine (0.4 s on) — the
                // prov: inferred frontend.screens.quads-7
                // tick rate is **inferred** from that cap (open for the FE
                // clock unit).
                let caret = if caret_on(((now - self.opened).max(0.0) * 0.12) as u32) { "|" } else { " " };
                state.text.insert(mc_main::FIELD, format!("{}{caret}", self.name));
            }
            McStep::Ask | McStep::Confirm => {
                state.text.insert(mc_main::HEADER, label(fe_ids::OBJ_B19F8B41, "Save/Load"));
                // INIT chains to HIDE; 0x5446B0 runs it again on leaving
                // the editor.
                state.run(mc_main::NAME_ENTRY, "HIDE", 0.0);
                let text = match (self.step, self.replacing) {
                    (McStep::Confirm, _) => label(0x2A9B_A6F9, "")
                        .replace("%s", &self.name),
                    (_, true) => label(0xD366_20FE, ""),
                    (_, false) => label(0x02B6_41C0, "Would you like to create a new profile?"),
                };
                state.text.insert(mc_main::MESSAGE, text.replace('^', "\n"));
                state.shown.insert(mc_main::MESSAGE);
                // The package's event handler starts the message's fade in
                // (its response to 0xBDFD2DF9: script 0x5B0D9106).
                state.scripts.insert(mc_main::MESSAGE, (fe_ids::OBJ_5B0D9106, 0.0));
                for (i, text) in [label(0xF770, "Yes"), label(0x061C, "No")].into_iter().enumerate() {
                    let (l, g) = (mc_main::LABELS[i], mc_main::GROUPS[i]);
                    state.hidden.remove(&l);
                    state.hidden.remove(&g);
                    state.shown.insert(l);
                    state.shown.insert(g);
                    state.text.insert(l, text);
                    // 0x537C10 "Highlight": the script names hash
                    // case-blind (its HIGHLIGHT 0x249DB7B7).
                    let script = if i == self.selected { "HIGHLIGHT" } else { "UNHIGHLIGHT" };
                    state.run(l, script, 0.0);
                    state.run(g, script, 0.0);
                }
            }
        }
        layout(assets, package, &state, now - self.opened)
    }
}

/// The memory-card operation a profile list serves (UI_ProfileManager's
/// Load 0x4C5820, mode 0xC11, and Delete 0x4C5850, mode 0x831: MC_Main's
/// operations 0x10 and 0x30 list the profiles, 0x4E8CE0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McOp {
    Load,
    Delete,
}

/// MC_List.fng: the profiles on disk under the operation's name ("Load",
/// "Delete"; the header 0x426C7B4D and the title row 0x48D4FCAE), one
/// name a row (option_name_1…8, 0xB2980BA8…), the first selected. As the
/// original showed it under Wine (2026-10-29).
pub struct McList {
    pub op: McOp,
    pub names: Vec<String>,
    pub selected: usize,
    /// The list's window top (0x542CB0 list +0x20 / +0x28): a sticky
    /// window, moved by 0x542E00 only when the selection leaves it.
    pub top: usize,
    pub opened: f32,
}

/// The name field's caret after `ticks` FE ticks (0x540AC0: the counter
/// +8 goes 1, 2, … 89, 0, …; "|" while it is under 45).
pub(crate) fn caret_on(ticks: u32) -> bool {
    (ticks + 1) % 90 < 45
}

pub mod mc_list {
    pub const HEADER: u32 = crate::frontend::fe_ids::HEADER;
    pub const TITLE_ROW: u32 = crate::frontend::fe_ids::TITLE_ROW;
    /// The title row's left / right arrows (not shown by the original).
    pub const ARROWS: [u32; 2] = [crate::frontend::fe_ids::ARROWS_0, crate::frontend::fe_ids::ARROWS_1];
    /// Rows top down (the package's y −130 … 150): option_name_1…8,
    /// option_data_1…8 and their backings.
    pub const NAME: u32 = crate::frontend::fe_ids::NAME;
    pub const DATA: u32 = crate::frontend::fe_ids::DATA;
    pub const BACKING: u32 = crate::frontend::fe_ids::MC_LIST_BACKING;
    pub const ROWS: usize = 8;
}

impl McList {
    pub fn new(op: McOp, names: Vec<String>, now: f32) -> McList {
        McList { op, names, selected: 0, top: 0, opened: now }
    }

    pub fn title(&self, assets: &Assets) -> String {
        match self.op {
            McOp::Load => assets.strings.get(fe_ids::LOAD).unwrap_or("Load").to_string(),
            McOp::Delete => assets.strings.get(fe_ids::MEMCARD_DELETE).unwrap_or("Delete").to_string(),
        }
    }

    /// Up / Down move; Accept picks the name, Back leaves (None both).
    pub fn key(&mut self, key: Key) -> Option<Option<String>> {
        let n = self.names.len();
        match key {
            Key::Up if self.selected > 0 => self.selected -= 1,
            Key::Down if self.selected + 1 < n => self.selected += 1,
            Key::Accept => return self.names.get(self.selected).cloned().map(Some),
            Key::Back => return Some(None),
            _ => {}
        }
        // MC_List_Ctor 0x4D6D50 builds a 0x542CB0 list with +0x9E = 0, so
        // its steps are 0x542E00's: no wrap, and a sticky window whose top
        // (+0x20 / +0x28) moves one row only when the selection leaves it
        // (Down: index ≥ rows + top; Up: the selection above the top).
        if self.selected < self.top {
            self.top = self.selected;
        } else if self.selected >= self.top + mc_list::ROWS {
            self.top = self.selected + 1 - mc_list::ROWS;
        }
        None
    }

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("MC_List") else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Select");
        let title = self.title(assets);
        state.text.insert(mc_list::HEADER, title.clone());
        state.text.insert(mc_list::TITLE_ROW, title);
        for a in mc_list::ARROWS {
            state.hidden.insert(a);
        }
        let top = self.top;
        // MC_List_Ctor 0x4D6D50: its "ScrollBar" (0x542CB0) hides the
        // handle and both arrows; with the names fitting the eight rows
        // they stay hidden (Wine: one profile, the backing alone). The bar
        // follows the window's top, not the selection: 0x5384A0 passes
        // 0x538090(+0x18 rows, +0x1C count, +0x20 the top's index from 1)
        // (the (1, count, current) branch needs +0xA0, which the list ctor
        // only ever clears, 0x542D71).
        scroll_bar_at(&mut state, package, "ScrollBar", false, mc_list::ROWS, self.names.len(), top + 1);
        // Its backing (ScrollBar_Backing 0x688A1551, INIT chained to
        // 0x16A259) answers the screen's 0xBDFD2DF9 with 0x5B0D9106, the
        // fade in to alpha 100 (Wine: the bar stays with one profile).
        state.scripts.insert(fe_ids::REPLAY_CONTROL_BACKING, (fe_ids::OBJ_5B0D9106, 0.0));
        for r in 0..mc_list::ROWS {
            let (name, data) = (mc_list::NAME + r as u32, mc_list::DATA + r as u32);
            state.hidden.insert(data);
            match self.names.get(top + r) {
                Some(n) => {
                    state.text.insert(name, n.clone());
                    let script = if top + r == self.selected { "HIGHLIGHT" } else { "UNHIGHLIGHT" };
                    state.run(name, script, 0.0);
                    state.run(mc_list::BACKING + r as u32, script, 0.0);
                }
                None => {
                    state.hidden.insert(name);
                    state.hidden.insert(mc_list::BACKING + r as u32);
                }
            }
        }
        layout(assets, package, &state, now - self.opened)
    }
}

/// A memory-card message on MC_Main.fng: the "Save/Load" header, the text,
/// and Yes / No (No focused, as every one the original asked) or one
/// Continue. The texts are the library's (memcard/LOCALE_*.loc, see
/// `profiles::text`) or the game's language table's.
pub struct McMessage {
    pub text: String,
    pub buttons: Vec<String>,
    pub selected: usize,
    pub opened: f32,
}

impl McMessage {
    pub fn ask(text: &str, yes: &str, no: &str, now: f32) -> McMessage {
        McMessage { text: text.into(), buttons: vec![yes.into(), no.into()], selected: 1, opened: now }
    }

    pub fn tell(text: &str, ok: &str, now: f32) -> McMessage {
        McMessage { text: text.into(), buttons: vec![ok.into()], selected: 0, opened: now }
    }

    /// Left / Right move; Accept gives the button (Back is ignored, as on
    /// MC_Main's questions).
    pub fn key(&mut self, key: Key) -> Option<usize> {
        match key {
            Key::Left | Key::Up => self.selected = 0,
            Key::Right | Key::Down => self.selected = (self.buttons.len().max(1) - 1).min(1),
            Key::Accept => return Some(self.selected),
            _ => {}
        }
        None
    }

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("MC_Main") else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Select");
        for (&l, &g) in mc_main::LABELS.iter().zip(&mc_main::GROUPS) {
            state.hidden.insert(l);
            state.hidden.insert(g);
        }
        state.text.insert(mc_main::HEADER, assets.strings.get(fe_ids::OBJ_B19F8B41).unwrap_or("Save/Load").to_string());
        state.run(mc_main::NAME_ENTRY, "HIDE", 0.0);
        state.text.insert(mc_main::MESSAGE, self.text.replace('^', "\n"));
        state.shown.insert(mc_main::MESSAGE);
        state.scripts.insert(mc_main::MESSAGE, (fe_ids::OBJ_5B0D9106, 0.0));
        // One button takes the middle slot (0xB8A7C6CE), two the left and
        // right ones.
        let slots: &[usize] = if self.buttons.len() == 1 { &[2] } else { &[0, 1] };
        for (i, (&k, text)) in slots.iter().zip(&self.buttons).enumerate() {
            let (l, g) = (mc_main::LABELS[k], mc_main::GROUPS[k]);
            state.hidden.remove(&l);
            state.hidden.remove(&g);
            state.shown.insert(l);
            state.shown.insert(g);
            state.text.insert(l, text.clone());
            let script = if i == self.selected { "HIGHLIGHT" } else { "UNHIGHLIGHT" };
            state.run(l, script, 0.0);
            state.run(g, script, 0.0);
        }
        layout(assets, package, &state, now - self.opened)
    }
}

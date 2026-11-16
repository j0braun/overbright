use crate::frontend::fe_ids;
use crate::formats::hash::fe_hash;
use crate::frontend::sms::*;
use crate::frontend::{Assets, Quad, ScreenState, layout};

/// UI_SMS_Mailbox's objects (constructor 0x4E1BF0).
pub mod mailbox_objects {
    /// MESSAGE_TEXT_1..10 (the rows' subjects), HIGH_LIGHT_1..10 (their
    /// bars, "Highlight" / "UnHighlight") and ENGAGE_PHONE_ICON_1..10
    /// (their envelopes, 0x4B2040).
    pub const ROWS: usize = 10;
    /// The mailbox's title: the scroller's label string (**inferred**:
    /// the string the package authors "Unread Messages", where the
    /// scroller's item text goes).
    pub const TITLE: u32 = crate::frontend::fe_ids::MAILBOX_OBJECTS_TITLE;
    /// The list's arrows (+0x1B0 UP_ARROW, +0x1B4 DOWN_ARROW; 0x4B2190).
    pub const UP: u32 = crate::frontend::fe_ids::UP;
    pub const DOWN: u32 = crate::frontend::fe_ids::DOWN;
    /// "Delete Message" and "Read Message" (0x4CC900: "Show" with rows,
    /// "Hide" without).
    pub const DELETE: u32 = crate::frontend::fe_ids::MAILBOX_OBJECTS_DELETE;
    pub const READ: u32 = crate::frontend::fe_ids::READ;
}

/// What a key does on the SMS screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmsKey {
    None,
    /// Read entry n (Accept: state 2, then 0x497810 opens
    /// UI_EngageMessageDialog on the row's entry).
    Open(usize),
    /// Ask to delete entry n (BUTTON1, 0xC519BFC0: "Are you sure you want
    /// to delete this message?", 0xF0A6E6B5; Yes 0x01F23C75 → 0x4CCB10).
    Delete(usize),
    /// Back to the game (0x911AB364: state 1, then 0x4977C0).
    Leave,
    /// The viewer's Back (0x911AB364 in 0x497B00): the mailbox.
    Mailbox,
}

/// UI_SMS_Mailbox (0x4E1BF0, handler 0x4CCC10, refresh 0x4CC900).
#[derive(Clone, Debug)]
pub struct Mailbox {
    /// Index into [`MAILBOXES`] (the scroller's item, 0x7F4434 the
    /// category it opens on).
    pub mailbox: usize,
    /// The rows: inbox entry indices, newest first.
    pub rows: Vec<usize>,
    pub selected: usize,
    /// The first row shown.
    pub top: usize,
    pub opened: f32,
    /// When the selection last moved (ms), for its "Highlight".
    pub moved: f32,
}

impl Mailbox {
    /// Open on `category`, the row of entry `focus` selected if listed
    /// (0x4CC900 selects 0x836334's entry), else the first.
    pub fn new(inbox: &Inbox, messages: &[Message], category: u8, focus: Option<usize>, now: f32) -> Mailbox {
        let mailbox = MAILBOXES.iter().position(|m| m.0 == category).unwrap_or(0);
        let mut m = Mailbox { mailbox, rows: Vec::new(), selected: 0, top: 0, opened: now, moved: now };
        m.refresh(inbox, messages, focus);
        m
    }

    pub fn category(&self) -> u8 {
        MAILBOXES[self.mailbox].0
    }

    /// 0x4CC900: the rows anew, the focus entry's selected.
    pub fn refresh(&mut self, inbox: &Inbox, messages: &[Message], focus: Option<usize>) {
        self.rows = inbox.mailbox(messages, self.category());
        self.selected = focus.and_then(|f| self.rows.iter().position(|&r| r == f)).unwrap_or(0);
        self.top = self.selected.saturating_sub(mailbox_objects::ROWS - 1);
    }

    /// The handler's keys (0x4CCC10): Left / Right the mailboxes, stopping
    /// at the ends (0x4B2110), and the list anew; Up / Down the rows
    /// (0x4CCB40, stopping at the ends); Accept, BUTTON1 and Back as
    /// [`SmsKey`].
    pub fn key(&mut self, key: crate::frontend::screens::Key, inbox: &Inbox, messages: &[Message], now: f32) -> SmsKey {
        use crate::frontend::screens::Key;
        match key {
            // PAD_LTRIGGER / PAD_RTRIGGER (0x5073EF13 / 0xD9FEEC59) step
            // the mailboxes as Left / Right do (0x4CCC10).
            Key::Left | Key::Right | Key::LTrigger | Key::RTrigger => {
                let to = if matches!(key, Key::Left | Key::LTrigger) {
                    self.mailbox.checked_sub(1)
                } else {
                    Some(self.mailbox + 1).filter(|&m| m < MAILBOXES.len())
                };
                if let Some(to) = to {
                    self.mailbox = to;
                    self.refresh(inbox, messages, None);
                    self.moved = now;
                }
            }
            Key::Up if self.selected > 0 => {
                self.selected -= 1;
                self.top = self.top.min(self.selected);
                self.moved = now;
            }
            Key::Down if self.selected + 1 < self.rows.len() => {
                self.selected += 1;
                if self.selected >= self.top + mailbox_objects::ROWS {
                    self.top = self.selected + 1 - mailbox_objects::ROWS;
                }
                self.moved = now;
            }
            Key::Accept => return self.rows.get(self.selected).map_or(SmsKey::None, |&e| SmsKey::Open(e)),
            Key::Button1 => return self.rows.get(self.selected).map_or(SmsKey::None, |&e| SmsKey::Delete(e)),
            Key::Back => return SmsKey::Leave,
            _ => {}
        }
        SmsKey::None
    }

    pub fn quads(&self, assets: &Assets, inbox: &Inbox, messages: &[Message], now: f32) -> Vec<Quad> {
        use mailbox_objects::*;
        let Some(package) = assets.package("UI_SMS_Mailbox") else { return Vec::new() };
        let mut state = ScreenState::default();
        crate::frontend::screens::footer(&mut state, package, "Select");
        state.text.insert(TITLE, assets.strings.get(MAILBOXES[self.mailbox].1).unwrap_or("").to_string());
        for i in 0..ROWS {
            let (text, icon, bar) = (
                fe_hash(&format!("MESSAGE_TEXT_{}", i + 1)),
                fe_hash(&format!("ENGAGE_PHONE_ICON_{}", i + 1)),
                fe_hash(&format!("HIGH_LIGHT_{}", i + 1)),
            );
            let entry = self.rows.get(self.top + i).and_then(|&r| inbox.entries.get(r));
            match entry.and_then(|e| find(messages, e.message).map(|m| (e, m))) {
                Some((e, m)) => {
                    state.text.insert(text, subject_line(assets, m, Some(e)));
                    // 0x4B2040: the kind's colour, the open envelope once
                    // read, else the closed one.
                    let (_, c) = crate::frontend::sms::icon(m.icon);
                    state.color.insert(icon, c.map(|v| v as f32 / 255.0));
                    state.image.insert(icon, if e.read { ENVELOPE_OPEN } else { ENVELOPE });
                    state.shown.insert(icon);
                }
                None => {
                    // A slot past the list's items is hidden whole, its
                    // HIGH_LIGHT backing too (the list refresh 0x538510 →
                    // vtable +4; the original under Wine shows no backing
                    // under the empty rows, 2026-10-29).
                    state.hidden.insert(text);
                    state.hidden.insert(icon);
                    state.hidden.insert(bar);
                }
            }
            if self.top + i == self.selected && !self.rows.is_empty() {
                state.run(bar, "HIGHLIGHT", self.moved - self.opened);
            } else {
                state.run(bar, "UNHIGHLIGHT", 0.0);
            }
        }
        // 0x4B2190: no rows, no arrows; the first row selected, the down
        // arrow only; the last, the up arrow only.
        let n = self.rows.len();
        let (up, down) = match n {
            0 => (false, false),
            _ if self.selected == 0 => (false, true),
            _ if self.selected + 1 == n => (true, false),
            _ => (true, true),
        };
        for (o, on) in [(UP, up), (DOWN, down)] {
            if on {
                state.shown.insert(o)
            } else {
                state.hidden.insert(o)
            };
        }
        for o in [DELETE, READ] {
            if n > 0 {
                state.run(o, "SHOW", 0.0);
            } else {
                state.run(o, "HIDE", 0.0);
            }
        }
        crate::frontend::screens::scroll_bar_named(&mut state, package, "SideScrollBar", ROWS, n, self.top);
        // The mailbox scroller's horizontal bar ("scroll_area" /
        // "ScrollBar", 0x542CB0 with byte 0 clear): one mailbox of the five
        // shown (0x538090 → 0x52E330; the scroller's arguments, 1, the
        // prov: inferred frontend.sms.quads
        // count and the position from 1, are **inferred** from the other
        // scrollers' calls, 0x538993 / 0x5444D6).
        crate::frontend::screens::scroll_bar_at(
            &mut state,
            package,
            "ScrollBar",
            true,
            1,
            MAILBOXES.len(),
            self.mailbox + 1,
        );
        // 0x4CC900: 0x54049A3D shows unless on the Inbox, 0xD506A970 unless
        // on Unlocks (scripts "Show" / "Hide"; neither is in the package).
        layout(assets, package, &state, now - self.opened)
    }
}

/// UI_EngageMessageDialog (SMSMessageViewer, constructor 0x497850,
/// handler 0x497B00): the message read (0x514DC0), "Subject: %s"
/// (0x2C167533), "From: %s" with the sender (0xFECED617) and the body in
/// Body_Text_1..10 (a text list, 0x50DBB0, broken at the row width like
/// the sponsor terms, 0x544700), scrolled with Up / Down.
#[derive(Clone, Debug)]
pub struct Viewer {
    pub entry: usize,
    /// Opened on its own (0x83632C = 1: one unread message when the SMS
    /// key was pressed) or from the mailbox (2).
    pub alone: bool,
    /// The mailbox it came from, to go back to.
    pub category: u8,
    pub lines: Vec<String>,
    pub top: usize,
    pub opened: f32,
}

const SUBJECT: u32 = fe_ids::SUBJECT;
const FROM: u32 = fe_ids::FROM;
const BODY_ROWS: usize = 10;

impl Viewer {
    pub fn new(
        assets: &Assets,
        inbox: &Inbox,
        messages: &[Message],
        entry: usize,
        alone: bool,
        category: u8,
        now: f32,
    ) -> Viewer {
        let body = inbox
            .entries
            .get(entry)
            .and_then(|e| find(messages, e.message).map(|m| body(assets, m, Some(e))))
            .unwrap_or_default();
        let package = assets.package("UI_EngageMessageDialog");
        let row = package.and_then(|p| p.object(fe_hash("Body_Text_1")).map(|o| (p, o)));
        let font = row.and_then(|(p, o)| p.resources.get(o.resource as usize)).and_then(|r| assets.font_for(&r.name));
        let width = row.map_or(0.0, |(_, o)| o.max_width as f32 - 16.0);
        let text = match font {
            Some(f) if width > 0.0 => crate::frontend::wrap(f, &body, width),
            _ => body.replace('^', "\n"),
        };
        Viewer { entry, alone, category, lines: text.split('\n').map(str::to_string).collect(), top: 0, opened: now }
    }

    pub fn key(&mut self, key: crate::frontend::screens::Key) -> SmsKey {
        use crate::frontend::screens::Key;
        match key {
            Key::Up => self.top = self.top.saturating_sub(1),
            Key::Down => self.top = (self.top + 1).min(self.lines.len().saturating_sub(BODY_ROWS)),
            // 0x406415E3: state 2, then the tick closes both (0x4977C0).
            Key::Accept => return SmsKey::Leave,
            Key::Back => return SmsKey::Mailbox,
            _ => {}
        }
        SmsKey::None
    }

    pub fn quads(&self, assets: &Assets, inbox: &Inbox, messages: &[Message], now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("UI_EngageMessageDialog") else { return Vec::new() };
        let mut state = ScreenState::default();
        crate::frontend::screens::footer(&mut state, package, "Select");
        if let Some((e, m)) = inbox.entries.get(self.entry).and_then(|e| find(messages, e.message).map(|m| (e, m))) {
            state.text.insert(SUBJECT, subject_line(assets, m, Some(e)));
            let sender = assets.strings.get(m.sender).unwrap_or("").to_string();
            state.text.insert(FROM, printf(assets, FROM_FORMAT, "From: %s", &sender));
        }
        for i in 0..BODY_ROWS {
            let row = fe_hash(&format!("Body_Text_{}", i + 1));
            match self.lines.get(self.top + i) {
                Some(l) => {
                    state.text.insert(row, l.clone());
                }
                None => {
                    state.hidden.insert(row);
                }
            }
        }
        crate::frontend::screens::scroll_bar_named(
            &mut state,
            package,
            "SideScrollBar",
            BODY_ROWS,
            self.lines.len(),
            self.top,
        );
        layout(assets, package, &state, now - self.opened)
    }
}

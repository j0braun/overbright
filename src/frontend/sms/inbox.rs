use super::table::*;
use crate::frontend::Assets;

/// An inbox entry (0x14 bytes at 0x860A60 + 0x14 n).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// +0x00: the message, by its name's hash (the game keeps a pointer
    /// and saves the hash).
    pub message: u32,
    /// +0x04: read (0x514DC0).
    pub read: bool,
    /// +0x05: an urgent message's mailbox has opened for it (0x514C70).
    pub shown: bool,
    /// +0x06: deleted (0x5014C0); kept, but never listed.
    pub deleted: bool,
    /// +0x07: filed in its mailbox (0x514D80 on leaving the mailbox
    /// screens: every read entry); until then only the Inbox lists it.
    pub filed: bool,
    /// +0x08 / +0x10 / +0x0C: a body, subject and argument label set by
    /// the magazine triggers (0x529680); 0 = the message's own labels.
    pub body: u32,
    pub subject: u32,
    pub extra: u32,
}

/// The inbox holds 256 entries (0x529600 adds while +0x1400 < 0x100).
pub const CAPACITY: usize = 256;

/// The icon texture and colour of a message kind (0x4974A0): 1–7 their
/// own, any other the envelope 0x4B9B8F5A. The game copies four words into
/// an FEng colour (0x505560), which is B, G, R, A; returned here as RGBA.
pub fn icon(kind: u8) -> (u32, [u8; 4]) {
    let (texture, [b, g, r, a]) = match kind {
        1 => (0x88C9_990A, [0x2D, 200, 200, 0xFF]),
        2 => (0x8B2F_36EE, [0x2B, 0x80, 0xD5, 0xFF]),
        3 => (0x4702_22FD, [0x8A, 0xF4, 0xCA, 0xFF]),
        4 => (0xA9DF_E648, [0x30, 0x30, 0xCF, 0xFF]),
        5 => (0x5D9B_6555, [0xFF, 0x80, 0x80, 0xFF]),
        6 => (0x756B_5C1B, [200, 0x82, 0x32, 0xFF]),
        7 => (0x5547_6A96, [0xB5, 0x4A, 0x80, 0xFF]),
        _ => (ENVELOPE, [0x0E, 0x9C, 0x0E, 0xFF]),
    };
    (texture, [r, g, b, a])
}

/// The closed envelope (an unread row, the HUD's resting mail) and the
/// open one (a read row; 0x4B2040).
pub const ENVELOPE: u32 = 0x4B9B_8F5A;
pub const ENVELOPE_OPEN: u32 = 0xEE28_AC57;

/// The mailboxes (0x7F4438: 1..5) and their titles (0x4E1BF0).
pub const MAILBOXES: [(u8, u32); 5] =
    [(1, 0xC06E_138A), (2, 0xF8CB_19A3), (3, 0x06F8_E5BF), (4, 0xE244_2059), (5, 0xEA33_E5B6)];

/// A message by its name's hash (0x501310).
pub fn find(messages: &[Message], hash: u32) -> Option<&Message> {
    messages.iter().find(|m| m.hash == hash)
}

/// The inbox's rules over a list of entries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inbox {
    pub entries: Vec<Entry>,
}

impl Inbox {
    pub fn contains(&self, message: u32) -> bool {
        self.entries.iter().any(|e| e.message == message)
    }

    /// 0x529600: a message not yet in the inbox goes in unread (+0x1404
    /// and the new flag +0x140C set), while there is room.
    pub fn add(&mut self, message: u32) -> bool {
        if self.contains(message) || self.entries.len() >= CAPACITY {
            return false;
        }
        self.entries.push(Entry { message, ..Default::default() });
        true
    }

    /// 0x529680: [`Self::add`] with the entry's own body (+0x08), subject
    /// (+0x10) and argument (+0x0C) labels.
    pub fn add_custom(&mut self, message: u32, body: u32, subject: u32, extra: u32) -> bool {
        if !self.add(message) {
            return false;
        }
        let e = self.entries.last_mut().unwrap();
        (e.body, e.subject, e.extra) = (body, subject, extra);
        true
    }

    /// The unread count (+0x1404, 0x514D30(1)): entries neither read nor
    /// deleted (add counts up; reading and deleting an unread one count
    /// down; the load recounts so, 0x514E80).
    pub fn unread(&self) -> usize {
        self.entries.iter().filter(|e| !e.read && !e.deleted).count()
    }

    /// 0x514CF0: the first entry neither read nor deleted.
    pub fn first_unread(&self) -> Option<usize> {
        self.entries.iter().position(|e| !e.read && !e.deleted)
    }

    /// The kind whose icon the HUD's notification shows (0x4ABAF0): the
    /// last entry neither read nor deleted, searching down from the newest
    /// but stopping above entry 0 (the loop tests the index before
    /// reading it, so entry 0 is never looked at); none found: kind 8, the
    /// envelope.
    pub fn notify_kind(&self, messages: &[Message]) -> u8 {
        (1..self.entries.len())
            .rev()
            .map(|i| self.entries[i])
            .find(|e| !e.read && !e.deleted)
            .and_then(|e| find(messages, e.message))
            .map_or(8, |m| m.icon)
    }

    /// 0x514DC0: reading counts an unread entry down.
    pub fn read(&mut self, i: usize) {
        if let Some(e) = self.entries.get_mut(i) {
            e.read = true;
        }
    }

    /// 0x5014C0: deleting marks it read and deleted.
    pub fn delete(&mut self, i: usize) {
        if let Some(e) = self.entries.get_mut(i) {
            e.deleted = true;
            e.read = true;
        }
    }

    /// 0x514D80 (leaving the mailbox screens, 0x4977C0, or Back from a
    /// message opened on its own): every read entry is filed.
    pub fn file(&mut self) {
        for e in self.entries.iter_mut().filter(|e| e.read) {
            e.filed = true;
        }
    }

    /// 0x529760's second half: an entry of an urgent message not yet
    /// shown (+0x05) asks for the mailbox.
    pub fn urgent_pending(&self, messages: &[Message]) -> bool {
        self.entries.iter().any(|e| !e.shown && find(messages, e.message).is_some_and(|m| m.urgent))
    }

    /// 0x514C70: every urgent message's entry is shown.
    pub fn mark_shown(&mut self, messages: &[Message]) {
        for e in self.entries.iter_mut() {
            if find(messages, e.message).is_some_and(|m| m.urgent) {
                e.shown = true;
            }
        }
    }

    /// The rows of mailbox `category` (0x4CC900), newest first: entries
    /// not deleted; one not filed only in the Inbox (1), a filed one in
    /// its message's mailbox (+0x03).
    pub fn mailbox(&self, messages: &[Message], category: u8) -> Vec<usize> {
        (0..self.entries.len())
            .rev()
            .filter(|&i| {
                let e = self.entries[i];
                !e.deleted
                    && if e.filed {
                        find(messages, e.message).is_some_and(|m| m.category == category)
                    } else {
                        category == 1
                    }
            })
            .collect()
    }

    /// The save's lines: `sms <name hash> <read> <shown> <deleted> <filed>
    /// <body> <subject> <extra>` (0x5014F0 writes the name's hash, the four
    /// flag bytes and the three labels).
    pub fn save_lines(&self) -> String {
        let mut out = String::new();
        for e in &self.entries {
            out += &format!(
                "sms {} {} {} {} {} {} {} {}\n",
                e.message, e.read as u8, e.shown as u8, e.deleted as u8, e.filed as u8, e.body, e.subject, e.extra
            );
        }
        out
    }

    /// One saved line's value (0x514E80 finds each message by its hash,
    /// 0x501310; the table is the game's own, so every hash is found).
    pub fn load_line(&mut self, value: &str) {
        let v: Vec<u32> = value.split(' ').filter_map(|s| s.parse().ok()).collect();
        if (v.len() == 5 || v.len() == 8) && !self.contains(v[0]) && self.entries.len() < CAPACITY {
            let at = |i: usize| v.get(i).copied().unwrap_or(0);
            self.entries.push(Entry {
                message: v[0],
                read: v[1] != 0,
                shown: v[2] != 0,
                deleted: v[3] != 0,
                filed: v[4] != 0,
                body: at(5),
                subject: at(6),
                extra: at(7),
            });
        }
    }
}

/// A message's subject and body labels (0x5149C0, 0x514B00): names
/// whose first 14 characters are "SMS_CAR_UNLOCK" (the compare length is
/// strlen("SMS_CAR_UNLOCK_") − 1) use SMS_CAR_UNLOCK_1's, those starting
/// "ENGAGE_TIPS_AUT" (likewise) ENGAGE_TIPS_AUTO's body; any other
/// `<name>_SUBJECT` and `<name>_BODY`.
pub fn labels(name: &str) -> (String, String) {
    let subject = if name.starts_with("SMS_CAR_UNLOCK") {
        "SMS_CAR_UNLOCK_1_SUBJECT".to_string()
    } else {
        format!("{name}_SUBJECT")
    };
    let body = if name.starts_with("SMS_CAR_UNLOCK") {
        "SMS_CAR_UNLOCK_1_BODY".to_string()
    } else if name.starts_with("ENGAGE_TIPS_AUT") {
        "ENGAGE_TIPS_AUTO_BODY".to_string()
    } else {
        format!("{name}_BODY")
    };
    (subject, body)
}

/// "Subject: %s" (0x4B5DE3E9) and "From: %s" (0x9F29D70D), 0x497760.
pub const SUBJECT_FORMAT: u32 = 0x4B5D_E3E9;
pub const FROM_FORMAT: u32 = 0x9F29_D70D;

/// The language string `hash` with `%s` filled (the game's printf).
pub(crate) fn printf(assets: &Assets, hash: u32, fallback: &str, arg: &str) -> String {
    assets.strings.get(hash).unwrap_or(fallback).replacen("%s", arg, 1)
}

/// "FENG: Default string error." (0xC01A6F63): what 0x4FF9D0 stands in
/// for a label the language table lacks (0x5149C0 / 0x514B00).
pub const DEFAULT_STRING: u32 = 0xC01A_6F63;

/// An entry's own label pair (0x5149C0 / 0x514B00 when +0x10 / +0x08 is
/// set): the format label with `%s` the argument label (+0x0C), each the
/// default-string error when missing (0x50C960 = snprintf).
fn custom(assets: &Assets, format: u32, extra: u32) -> String {
    let get = |h: u32| assets.strings.get(h).or_else(|| assets.strings.get(DEFAULT_STRING)).unwrap_or("").to_string();
    get(format).replacen("%s", &get(extra), 1)
}

/// An entry's subject (0x5149C0): its own (+0x10) or the message's label.
pub fn subject(assets: &Assets, m: &Message, e: Option<&Entry>) -> String {
    match e.filter(|e| e.subject != 0) {
        Some(e) => custom(assets, e.subject, e.extra),
        None => assets.strings.label(&labels(&m.name).0),
    }
}

/// An entry's body (0x514B00): its own (+0x08) or the message's label.
/// The viewer then widens it (0x512530: 0x43E000 byte to UTF-16, or with
/// the double-byte table 0x50D3C0 when 0x8383DC is set; the text itself
/// is unchanged).
pub fn body(assets: &Assets, m: &Message, e: Option<&Entry>) -> String {
    match e.filter(|e| e.body != 0) {
        Some(e) => custom(assets, e.body, e.extra),
        None => assets.strings.label(&labels(&m.name).1),
    }
}

/// A message's subject line as the rows and the viewer show it
/// (0x497760: "Subject: %s" of the subject).
pub fn subject_line(assets: &Assets, m: &Message, e: Option<&Entry>) -> String {
    printf(assets, SUBJECT_FORMAT, "Subject: %s", &subject(assets, m, e))
}

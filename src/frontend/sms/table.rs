use crate::formats::bundle::{read_u16, read_u32};
use crate::formats::hash::bin_hash;

/// One message (career chunk 0x34A17, 0x14 bytes).
#[derive(Clone, Debug, Default)]
pub struct Message {
    /// +0x00: the name (string pool); `<name>_SUBJECT` / `<name>_BODY` are
    /// its language labels (0x5149C0, 0x514B00).
    pub name: String,
    pub hash: u32,
    /// +0x02: what sends it (see [`Trigger`]).
    pub condition: u8,
    /// +0x03: the mailbox it is filed in once read (1 Inbox, 2 Game Tips,
    /// 3 Special Events, 4 Rachel, 5 Unlocks; 0x4CC900).
    pub category: u8,
    /// +0x04: opens the mailbox by itself in free roam (0x529760).
    pub urgent: bool,
    /// +0x05: its icon and colour (0x4974A0).
    pub icon: u8,
    /// +0x08, +0x0C: the condition's values (two hashes for 1 and 2;
    /// stage << 16 | count for 9, 10 and 12; seconds for 3).
    pub value: [u32; 2],
    /// +0x10: the sender's label ("From: %s", 0x501470 → 0x4FFA80).
    pub sender: u32,
}

/// The conditions the clone sends (+0x02).
pub mod condition {
    /// An event won whose hash is +0x08 or +0x0C (0x529930 → 0x501370).
    pub const EVENT_WON: u8 = 1;
    /// A shop entered whose hash (+0x38) is +0x08 or +0x0C (0x529870).
    pub const SHOP: u8 = 2;
    /// Free-roam driving time past +0x08 seconds (0x529760).
    pub const DRIVE_TIME: u8 = 3;
    /// An outrun won / lost (0x529BE0).
    pub const OUTRUN_WON: u8 = 5;
    pub const OUTRUN_LOST: u8 = 6;
    /// A photo-shoot marker engaged (0x532B20 → 0x529A50 with the
    /// marker's hash): a message whose +0x08 or +0x0C names a milestone
    /// spot (+0x2C) in state 2 and not done. No message of the game's
    /// table has it.
    pub const SHOOT: u8 = 7;
    /// Any shop entered (0x529870).
    pub const ANY_SHOP: u8 = 8;
    /// The stage record's sponsor / URL / normal wins (0x501370).
    pub const SPONSOR_WINS: u8 = 9;
    pub const URL_WINS: u8 = 10;
    pub const NORMAL_WINS: u8 = 12;
    /// Sent only by the magazine triggers (0x34A1B → 0x5338D0), which name
    /// the message themselves; no condition scan looks for 13.
    pub const TRIGGERED: u8 = 13;
    /// A DVD cover offered (0x5331D0 → 0x5299C0): +0x08 is the cover's
    /// visual rating (ftol of milestone +0x3C).
    pub const DVD_OFFERED: u8 = 14;
    /// An engage tip: the car in a kind-0x12 route trigger whose hash is
    /// +0x08 (0x5EB0F8 → 0x532D50 → 0x529C50); +0x0C goes to the bank.
    pub const TIP: u8 = 15;
}

/// The message table (0x34A17), names from the career string pool.
pub fn parse(body: &[u8], string: impl Fn(u32) -> String) -> Vec<Message> {
    body.chunks_exact(0x14)
        .map(|r| {
            let name = string(read_u16(r, 0) as u32);
            Message {
                hash: bin_hash(&name),
                name,
                condition: r[2],
                category: r[3],
                urgent: r[4] == 1,
                icon: r[5],
                value: [read_u32(r, 8), read_u32(r, 0xC)],
                sender: read_u32(r, 0x10),
            }
        })
        .collect()
}

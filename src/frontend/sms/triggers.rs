use crate::formats::bundle::{read_u16, read_u32};
use crate::formats::hash::bin_hash;

/// A world challenge (career chunk 0x34A1B, 0x18 bytes, 28 records;
/// 0x838424 / count 0x838420), whose message is also the magazine /
/// unique-part SMS: +0x00 its name (string pool; its bin hash keys the
/// career's flag table, +6 = 2 started, 1 done), +0x02 its start marker
/// (a route trigger's name, 0x512A10), +0x04 (i16) the stage, +0x06 the
/// condition (0 the stage's normal wins, 0x5004D0; 1 0x500520, the stage
/// record's URL wins +0x9C..; 2 its outrun wins, stage record +0xDC; 3
/// 0x5005A0, its sponsor wins +0x8C..), +0x07 the count (the message
/// arrives when it is equal, 0x533B50 / 0x533AA0; the challenge is on
/// offer from then on, 0x527980), +0x08 the message (0x501310), +0x0C
/// the goal (a shop's +0x38 or a milestone's +0x20, 0x4FFF80), +0x10 the
/// time limit in seconds, +0x14 the kind (1 visual, 2 performance, 3 / 4
/// a milestone's: 4 the magazine shows), +0x15..+0x17 the part slots (the
/// first is the visual part's).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MagTrigger {
    pub name: u32,
    pub marker: u32,
    pub stage: i16,
    pub condition: u8,
    pub count: u8,
    pub message: u32,
    pub milestone: u32,
    pub time: i32,
    pub kind: u8,
    pub part: u8,
    pub slots: [u8; 3],
}

/// The trigger table (0x34A1B); `string` reads the career's string pool.
pub fn parse_triggers(body: &[u8], string: impl Fn(u32) -> String) -> Vec<MagTrigger> {
    body.chunks_exact(0x18)
        .map(|r| MagTrigger {
            name: bin_hash(&string(read_u16(r, 0) as u32)),
            marker: bin_hash(&string(read_u16(r, 2) as u32)),
            stage: read_u16(r, 4) as i16,
            condition: r[6],
            count: r[7],
            message: read_u32(r, 8),
            milestone: read_u32(r, 0xC),
            time: read_u32(r, 0x10) as i32,
            kind: r[0x14],
            part: r[0x15],
            slots: [r[0x15], r[0x16], r[0x17]],
        })
        .collect()
}

/// The counters 0x5338D0 cycles its generic texts by: magazines
/// (0x864210) and unique parts (0x86420C).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TriggerCounters {
    pub magazine: i32,
    pub unique: i32,
}

/// 0x5338D0: the labels a fired trigger's message arrives with (body,
/// subject, argument; 0x529680). With its milestone found (0x4FFF80):
/// SMS_MAG_GENERIC_BODY_%d / SMS_MAG_GENERIC_SUBJ_%d by the magazine
/// counter % 2 + 1, the argument `<cover>_TITLE` (0x43DB80 continues the
/// cover's hash, milestone +0x28) and the counter up. Else a performance
/// kind (2): SMS_PERF_GENERIC_%d_BODY / _SUBJECT by the unique counter %
/// 3 + 1; a visual kind (1): SMS_VISUAL_GENERIC_%d_BODY / _SUBJECT by it % 5
/// + 1 with the slot's name (0x0B Hoods, 0x0C Spoilers, 0x0D Rims, 0x16
/// Wide Body Kits, 0x3E Vinyls; others none); both count up. Any other
/// kind: the message's own labels. The label hashes are bin hashes
/// (0x50B990 → 0x43DB50).
pub fn trigger_labels(t: &MagTrigger, cover: Option<u32>, c: &mut TriggerCounters) -> (u32, u32, u32) {
    if let Some(cover) = cover {
        let n = c.magazine % 2 + 1;
        let labels = (
            bin_hash(&format!("SMS_MAG_GENERIC_BODY_{n}")),
            bin_hash(&format!("SMS_MAG_GENERIC_SUBJ_{n}")),
            crate::formats::hash::bin_hash_from(cover, "_TITLE"),
        );
        c.magazine += 1;
        return labels;
    }
    let labels = match t.kind {
        2 => {
            let n = c.unique % 3 + 1;
            (bin_hash(&format!("SMS_PERF_GENERIC_{n}_BODY")), bin_hash(&format!("SMS_PERF_GENERIC_{n}_SUBJECT")), 0)
        }
        1 => {
            let n = c.unique % 5 + 1;
            let part = match t.part {
                0x0B => 0x028F_7092,
                0x0C => 0xBB03_4EA6,
                0x0D => 0x0294_D2A3,
                0x16 => 0x945E_9BC5,
                0x3E => 0x78DE_EA1C,
                _ => 0,
            };
            (
                bin_hash(&format!("SMS_VISUAL_GENERIC_{n}_BODY")),
                bin_hash(&format!("SMS_VISUAL_GENERIC_{n}_SUBJECT")),
                part,
            )
        }
        _ => return (0, 0, 0),
    };
    c.unique += 1;
    labels
}

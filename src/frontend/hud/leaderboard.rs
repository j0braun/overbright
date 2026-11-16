/// A leader board row (0x34 bytes from hud +0x20).
#[derive(Clone, Debug, Default)]
pub struct LeaderRow {
    pub name: String,
    pub player: bool,
    /// The gap to the player, ticks (0x5F82E0; negative ahead).
    pub gap: i32,
    /// Once the racer is done (0x5EE260), its status label: Finished,
    /// Knocked Out….
    pub done: Option<u32>,
    /// A drift race's row (0x4AAC50): the node's points, shown "%d" on
    /// every row, the player's too, in place of the gap.
    pub points: Option<i32>,
}

/// 0x4AAED0: the race places (0-based) the leader board's `rows` rows
/// show, the player at place `player` of `racers` (**verified**): the
/// leader first, then a window round the player — each pass one place
/// above (from the player's − 1 up, while more than one row is left) and
/// one below (while any is left) — the places above in race order, the
/// player's own (unless the leader), then those below. **Observed** under
/// Wine (raceflow, URL, 6 racers): 5th shows 1, 4, 5, 6; 6th 1, 4, 5, 6.
pub fn board_rows(player: usize, racers: usize, rows: usize) -> Vec<usize> {
    let mut left = rows.saturating_sub(1) as i64;
    let (mut above, mut below) = (player as i64 - 1, player as i64 + 1);
    let (mut up, mut down) = (Vec::new(), Vec::new());
    if left > 0 {
        for _ in 0..racers {
            if left < 1 {
                break;
            }
            if above > 0 && left > 1 {
                up.push(above as usize);
                left -= 1;
                above -= 1;
            }
            if below < racers as i64 && left > 0 {
                down.push(below as usize);
                left -= 1;
                below += 1;
            }
        }
    }
    let mut out = vec![0];
    out.extend(up.into_iter().rev());
    if player != 0 {
        out.push(player);
    }
    out.extend(down);
    // The budget (+0x170 − 1 for the places above and below) leaves out
    // the player's own row, so with the player neither first nor last it
    // lists five places for the four rows: 3rd of 6 gives 1, 2, 3, 4, 5,
    // 4th of 6 gives 1, 3, 4, 5, 6. The rows (0x4AA9F0 / 0x4AA890) show the
    // list's first +0x170 entries, and the element's set-up makes +0x170 4
    // (0x4F61AF), so the fifth is never drawn: 3rd shows 1, 2, 3, 4 and
    // 4th shows 1, 3, 4, 5 (**verified**).
    out.truncate(rows);
    out
}

/// The status labels (0x4AA9F0, by racer status +0x10).
pub const FINISHED: u32 = crate::frontend::fe_ids::FINISHED;

/// A done racer's label by racer status (0x4AA9F0, jump table 0x4AAC28):
/// 3 "Knocked Out", 8 "Blown Engine", 9 "Totaled", 11 "Timeup", 12
/// "Disconnected", anything else "Finished".
pub fn status_label(status: u16) -> u32 {
    match status {
        3 => 0x5D82_DBA2,
        8 => 0x01E6_6364,
        9 => 0xB7B7_5185,
        11 => 0x1CE1_29E0,
        12 => 0xA7DB_717F,
        _ => FINISHED,
    }
}

/// The race-over place lines by place (0x7F66B4, read by 0x4AB4B0):
/// 1 "Winner!", 2 "You finished second!" … 5 "You finished fifth!" (entry
/// 0 has no English string).
pub(super) const PLACE_LINES: [u32; 6] = [crate::frontend::fe_ids::PLACE_LINES_0, crate::frontend::fe_ids::PLACE_LINES_1, crate::frontend::fe_ids::PLACE_LINES_2, crate::frontend::fe_ids::PLACE_LINES_3, crate::frontend::fe_ids::PLACE_LINES_4, crate::frontend::fe_ids::PLACE_LINES_5];

/// The place's suffix label (0x4FFAA0): 10 its own, else the table at
/// 0x7F70D8 by place % 10 ("th", "st", "nd", "rd", "th"…).
pub(super) fn nth(place: u32) -> u32 {
    const TABLE: [u32; 10] = [
        crate::frontend::fe_ids::TABLE_0,
        crate::frontend::fe_ids::TABLE_1,
        crate::frontend::fe_ids::TABLE_2,
        crate::frontend::fe_ids::TABLE_3,
        crate::frontend::fe_ids::TABLE_4,
        crate::frontend::fe_ids::TABLE_5,
        crate::frontend::fe_ids::TABLE_6,
        crate::frontend::fe_ids::TABLE_7,
        crate::frontend::fe_ids::TABLE_8,
        crate::frontend::fe_ids::TABLE_9,
    ];
    if place == 10 { crate::frontend::fe_ids::TABLE_9 } else { TABLE[place as usize % 10] }
}

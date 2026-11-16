//! The Quick Race traffic level on a new save (docs/formats.md, "Traffic
//! level"). The start-up defaults set every type's level byte to 1 (the
//! profile constructor's default builder, 0x52A230), so a single-player
//! Quick Race of any type runs at level 1 and its traffic pool reads that
//! level's pair of the track's traffic bytes.

use nfsu2::ai::traffic::{QUICK_RACE_LEVEL, pool_size};

#[test]
fn quick_race_level_is_the_new_save_default() {
    assert_eq!(QUICK_RACE_LEVEL, 1);
}

#[test]
fn new_save_quick_race_pool_reads_level_one() {
    // The track record's traffic bytes (+0xEC): level 1 is entries 2 (forward)
    // and 3 (backward), so a Quick Race on that track gets 5 forward cars and
    // 7 backward ones.
    let track: [i8; 8] = [0, 0, 5, 7, 0, 0, 0, 0];
    assert_eq!(pool_size(&track, QUICK_RACE_LEVEL, false), 5);
    assert_eq!(pool_size(&track, QUICK_RACE_LEVEL, true), 7);
}

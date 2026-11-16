//! Native front-end rules ported from SPEED2.EXE (part 1; see
//! docs/native-fixes/N-FE-1.tsv). Each test states the native rule and
//! its address.

use crate::frontend::hud::time_text;
use crate::frontend::screens::{Key, McList, McOp};

/// MC_List (0x4D6D50 → 0x542CB0 with +0x9E = 0) steps through 0x542E00: a
/// sticky window whose top moves only when the selection leaves it, no
/// wrap. The scroll bar follows that top (0x5384A0 → 0x538090(rows,
/// count, top + 1)), so the window must persist between steps.
#[test]
fn mc_list_window_is_sticky() {
    let names: Vec<String> = (0..12).map(|i| format!("P{i}")).collect();
    let mut l = McList::new(McOp::Load, names, 0.0);
    for _ in 0..9 {
        l.key(Key::Down);
    }
    // Selection 9 of 8 rows: the window moved two rows (top 2).
    assert_eq!((l.selected, l.top), (9, 2));
    // Up inside the window leaves it where it is (the clone used to
    // recompute top = selected − 7 = 1).
    l.key(Key::Up);
    assert_eq!((l.selected, l.top), (8, 2));
    for _ in 0..7 {
        l.key(Key::Up);
    }
    assert_eq!((l.selected, l.top), (1, 1));
    // No wrap at either end.
    l.key(Key::Up);
    l.key(Key::Up);
    assert_eq!((l.selected, l.top), (0, 0));
}

/// FormatRaceTime 0x577250 (flags 0) on ticks: seconds t / 4000 and
/// hundredths (t % 4000) × 1000 / 4000 / 10, all truncating (0x5771C0).
/// The World Map record (0x496D70) used to round (4039 ticks → 1.01); the
/// Quick Race record lost a hundredth through f32 seconds (4040 → 1.00).
#[test]
fn race_records_format_ticks_truncating() {
    assert_eq!(time_text(4039), " 1.00");
    assert_eq!(time_text(4040), " 1.01");
    assert_eq!(time_text(4000 * 83 + 1999), " 1:23.49");
    // The Quick Race record arrives as ticks / 4000 in f32 and is taken
    // back to the nearest tick before formatting.
    let best = 4040_f32 / 4000.0;
    assert_eq!(time_text((best * 4000.0).round() as i32), " 1.01");
}

/// "%$3.1f" / "%$.1f" (0x43E950): the last digit rounds half up (0x43EF92,
/// `10 × rest ≥ 5`) and '$' groups the thousands; Rust's `{:.1}` rounds
/// the exact binary value half to even. PostRaceStats top speed
/// (0x4BFA30, format 0x7951F0) and the dyno peaks use it.
#[test]
fn one_decimal_rounds_half_up() {
    assert_eq!(crate::frontend::format::grouped_1(0.25), "0.3");
    assert_eq!(crate::frontend::format::grouped_1(1234.75), "1,234.8");
    assert_eq!(crate::frontend::grouped_fixed(2.25, 1), "2.3");
}

/// UI_Status_DVD: a stage record holds 4 covers (0x835C9C + stage × 0x28:
/// 4 × {f32, u8}, the count at +0x20; 0x497330 hides k = count..4).
#[test]
fn dvd_cover_slots_are_four() {
    assert_eq!(crate::frontend::career::stats::DVD_SLOTS, 4);
}

/// The name field's caret (0x540AC0): counter +8 runs 0..89 (back to 0 at
/// 0x5A), "|" while under 0x2D: 45 of every 90 ticks (the clone had 44).
#[test]
fn caret_shows_45_of_90_ticks() {
    let on = (0..90).filter(|&t| crate::frontend::screens::caret_on(t)).count();
    assert_eq!(on, 45);
    assert!(crate::frontend::screens::caret_on(0));
    assert!(crate::frontend::screens::caret_on(89));
    assert!(!crate::frontend::screens::caret_on(44));
}

/// UI_Paint's finish arrows (+0x150 / +0x154): hidden by the ctor
/// 0x56AEC0, shown by 0x55CC60 only for the entered slots 0x3F, 0x45,
/// 0x4B, 0x4C (byte table 0x55CE90), hidden again on leaving (0x55CF0E).
#[test]
fn paint_finish_arrows_only_on_finish_slots() {
    use crate::frontend::screens::finish_arrows_shown;
    for slot in [0x3F, 0x45, 0x4B, 0x4C] {
        assert!(finish_arrows_shown(Some(slot)));
        assert!(!finish_arrows_shown(None), "not on the strip");
    }
    for slot in [0x44, 0x46, 0x47, 0x49, 0x4A, 0x4D, 0x50] {
        assert!(!finish_arrows_shown(Some(slot)), "slot {slot:#x}");
    }
}

/// 0x50E960: a rim brand found in the career brand table (0x838400) shows
/// that entry's name string — 5Zigen (0x352D08D1) reads "5ZIGEN", not its
/// label 0x0CF90EF1 — else the label's language string.
#[test]
fn rim_brand_name_prefers_the_brand_table() {
    use crate::frontend::career::Brand;
    use crate::frontend::feng::strings::Strings;
    let strings = Strings { map: [(0x0CF9_0EF1, "5Zigen".to_string()), (0xD2ED_DFFB, "NFSU 2".to_string())].into() };
    let brands =
        vec![Brand { name: "5ZIGEN".into(), display: "5Zigen".into(), hash: 0x352D_08D1 }];
    assert_eq!(crate::frontend::screens::rim_brand_name(&brands, &strings, 0x352D_08D1), "5ZIGEN");
    let nfsu = crate::frontend::screens::RIM_BRANDS.iter().find(|r| r.2 == 0xD2ED_DFFB).unwrap().0;
    assert_eq!(crate::frontend::screens::rim_brand_name(&brands, &strings, nfsu), "NFSU 2");
}

/// The trick popup's points per multiplier (0x4AB7A3): `fdivr` then
/// `_ftol` 0x75BED4; a count of 0 gives ±inf / NaN, whose integer-
/// indefinite low dword is 0 (Rust's `as i32` saturates to 2147483647).
#[test]
fn popup_points_over_zero_count_print_zero() {
    use crate::frontend::hud::ftol;
    assert_eq!(ftol(500.0 / 0.0), 0);
    assert_eq!(ftol(0.0 / 0.0), 0);
    assert_eq!(ftol(-12.9), -12);
    assert_eq!(ftol(250.7), 250);
}

/// 0x50D3C0: bytes under 0x80 are never mapped; a map entry 0 keeps the
/// byte, ≥ 0x80 replaces it, 1..0x7F leads a pair whose second byte under
/// 0x80 gives '_' and otherwise indexes entry v·128 + d − 0x80.
#[test]
fn language_map_double_byte_pairs() {
    use crate::frontend::feng::strings::decode;
    let mut map = vec![0u8; 4 + 2 * 0x300];
    map[..4].copy_from_slice(&256u32.to_le_bytes());
    let mut set = |k: usize, v: u16| map[4 + 2 * k..6 + 2 * k].copy_from_slice(&v.to_le_bytes());
    set(0x41, 0x2122); // never used: 'A' < 0x80
    set(0x82, 0x2122); // ™
    set(0x90, 0x02); // a lead byte
    set((2 << 7) + 0xA1 - 0x80, 0x3042); // あ
    assert_eq!(decode(Some(&map), b"A\x82\x83\0"), "A™\u{83}");
    assert_eq!(decode(Some(&map), b"\x90\xA1x\0"), "あx");
    assert_eq!(decode(Some(&map), b"\x90Ax\0"), "_x");
    assert_eq!(decode(None, b"\x82\0"), "\u{82}");
}

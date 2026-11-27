//! The speech banks: `speech/nisaudio.*` (the cut scenes' audio) and
//! `speech/UG2_Speech.*` (in-game speech), read from `SDATA/sdat.viv`.
//!
//! Loading (0x482A00 for the NIS bank; the speech bank's names are the
//! table 0x7ECFA8, by language): `.evt`, `.idx` and `.csi` are read whole
//! (0x57B4B0, completion 0x45AF90: the index's entry counts are summed
//! into 0x82BA5C, the names hand over to the speech library 0x45A7C0 →
//! 0x734E70 / 0x736020); the `.big` is streamed.
//!
//! * **`.csi`** ("MOIR"): +0xA the switch count, +0x10 the bank's id
//!   (low word of every switch id), +0x2C per switch {name offset, high
//!   word, 0}: e.g. NIS_Select_End = 0x48881A4B (the exe's own table
//!   0x7ED01C holds the same name / id pairs).
//! * **`.idx`**: +0x54 the entry count, then per entry {flags (byte 3 =
//!   1), size, offset in the file, the entry's base in the `.big`}. An
//!   entry (read by 0x738A20): +0 u16 id, +4 the key count k (bit 7: an
//!   enable bit per record, 0x737140), +5 the record count, +6 the depth
//!   of its recently played ring (0x735D70), +7 the
//!   unit − 1 in 256 bytes, +8 u16 the end; from +0xC the records of
//!   k + 2 bytes: a big-endian u16 offset in units, then k keys. Record r
//!   starts at offset[r] · unit and runs to the next record's offset (the
//!   last to the end); 0x737E56 adds (+7 + 1) · +8 · variant · 256 for a
//!   variant other than 0xFFFF. After the records (aligned to 4): per key
//!   the union of the records' values (0x7371C0), the enable bits (with
//!   bit 7), then the ring (0x7359E0): its position byte and +6 record
//!   indices, 0xFF unused.
//! * **`.evt`** (0x7386A0, 0x7363A0, 0x738340; **verified**):
//!   +4 the switch table's offset, +8 / +9 the bank's keys, +0x10 the
//!   event count n, +0x18 per event a u16 offset in dwords. An event: +0
//!   u16 id, +2 / +4 queue lifetime and priority, +6 the alternative
//!   count, +7 the key count, +8 count limit / switch-mask count, +9 the
//!   request chance, +0xA flags, +0xB the request's value count, +0xC per
//!   alternative a u16 offset in dwords. An alternative: +0 weight, +1
//!   chance, +2 step count << 2 | channel mode, +3 state words, +8 per
//!   step its offset in dwords; a step: the entry descriptor (id, value
//!   index, mode), +4 the constraint count, +8 per constraint {mask,
//!   component (1-based request value; 0 / 0xFE any), state slot}. The
//!   switch table: n records of 0x20 bytes {name offset (in the string
//!   pool after the records), id, the tuple's storage}; switch i fires
//!   event i. (The old reader searched for the dword 0x46439, which is the
//!   shipped alternatives' header 0x39, 100, 1 step, 0 — not a marker.)
//! * **Selection** (`Library::request`, see `library`): the switch's
//!   event; its chance gate, the alternatives' weighted order and chance,
//!   the step's records that fit (every constraint j: key j a bit of the
//!   mask and of its request value), the recently played ring as a
//!   fallback, and the keyed pick.

mod bank;
mod library;
mod picker;
mod tuples;

pub use bank::*;
pub use library::*;
pub use picker::*;
pub use tuples::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::viv::Viv;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn outrun_uses_each_native_selector_and_decodes_its_own_entry() {
        let root = crate::testkit::game_root();
        let viv = Viv::mount_game(std::path::Path::new(&root)).unwrap();
        let bank = Bank::load(&viv, "UG2_Speech").unwrap();
        // Native mode1 descriptor uses tuple component4 as an integer
        // entry selector, while component3 still filters record keys.
        for (voice, selector, base) in
            [(1, 3, 0x23EC100), (2, 1, 0x16CE100), (4, 2, 0x1E45100), (8, 4, 0x2A19400), (0x200, 5, 0x3051F00)]
        {
            for situation in [1, 8, 16, 2, 4] {
                let tuple = lines::outrun(situation, voice);
                let (entry, records) =
                    bank.fit(&tuple).unwrap_or_else(|| panic!("missing native outrun tuple {tuple:?}"));
                assert!(!records.is_empty());
                let selected = bank.entry(entry).unwrap();
                assert_eq!(selected.base, base, "selector {selector} resolved wrong audio entry");
                let pcm = bank.decode(&viv, entry, records[0]).unwrap();
                assert!(pcm.samples.len() > pcm.rate as usize);
                assert!(pcm.samples.iter().all(|v| v.is_finite()));
                assert!(pcm.samples.iter().any(|v| v.abs() > 0.01));
            }
        }
        // Actual stream selection/decoder/mixer path: one win from each
        // of the five voices, plus Rachel's special first-outrun win.
        for (situation, voice) in [(8, 1), (8, 2), (8, 4), (8, 8), (8, 0x200), (0x40, 0x200)] {
            let mixer = crate::audio::mixer::Mixer::offline(22050);
            let mut streams = crate::audio::streams::Streams::new(std::path::Path::new(&root));
            assert!(streams.say(&mixer, &lines::outrun(situation, voice)));
            let mut mixed = Vec::new();
            mixer.pump(22050 * 3, &mut mixed);
            assert!(mixed.iter().all(|v| v.is_finite()));
            assert!(mixed.iter().any(|v| v.abs() > 0.001), "silent win {situation}/{voice}");
        }
    }

    #[test]
    fn record_ranges() {
        let mut e = vec![0x24, 0x40, 0xFF, 0xFF, 3, 2, 0, 1, 0x00, 0x07, 0, 0];
        e.extend_from_slice(&[0x00, 0x00, 10, 14, 0, 0x03, 0x80, 8, 14, 0]);
        let mut d = vec![0u8; 0x58];
        d[0x54] = 1;
        d.extend_from_slice(&0x0100_0000u32.to_le_bytes());
        d.extend_from_slice(&(e.len() as u32).to_le_bytes());
        d.extend_from_slice(&0x68u32.to_le_bytes());
        d.extend_from_slice(&0x40AD00u32.to_le_bytes());
        d.extend_from_slice(&e);
        let entries = parse_idx(&d);
        assert_eq!(entries[0].range(0), Some((0x40AD00, 0x70000)));
        assert_eq!(entries[0].range(1), Some((0x40AD00 + 0x70000, 0x70000)));
    }

    #[test]
    fn the_pick_avoids_recent_lines() {
        let mut p = Picker::new(10);
        let mut q = Picker::new(10);
        assert_eq!(p.draw(), q.draw());
        // Of 8 records the last 4 of the same entry are avoided.
        let picks: Vec<u16> = (0..40).map(|_| p.pick(8, 0x27)).collect();
        for w in picks.windows(5) {
            assert!(!w[..4].contains(&w[4]), "{picks:?}");
            assert!(w[4] < 8);
        }
        // One record: always 0.
        assert_eq!(p.pick(1, 0x23), 0);
    }

    /// SoundFX_Select's lines: THUNDERSTRM (4, 1) fits the nine [2, 0]
    /// records, (4, 2) none; BLOWNENGINE (8, 0x10) and CollisionStream
    /// (2, 8) their own.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn soundfx_lines() {
        let root = crate::testkit::game_root();
        let viv = Viv::mount_game(&root).expect("mounting the game's archives");
        let bank = Bank::load(&viv, "nisaudio").expect("the nisaudio bank");
        assert_eq!(bank.select(nis::EVENT_SOUNDFX, &[4, 1]), (14..=22).collect::<Vec<_>>());
        assert!(bank.select(nis::EVENT_SOUNDFX, &[4, 2]).is_empty());
        assert_eq!(bank.select(nis::EVENT_SOUNDFX, &[8, 0x10]), (9..=12).collect::<Vec<_>>());
        assert_eq!(bank.select(nis::EVENT_SOUNDFX, &[2, 8]), (3..=8).collect::<Vec<_>>());
        assert_eq!(bank.select(nis::EVENT_SOUNDFX, &[1, 4]), vec![13]);
    }

    fn banks() -> (Bank, Bank) {
        let root = crate::testkit::game_root();
        let viv = Viv::mount_game(std::path::Path::new(&root)).expect("mounting the game's archives");
        (Bank::load(&viv, "UG2_Speech").expect("UG2_Speech"), Bank::load(&viv, "nisaudio").expect("nisaudio"))
    }

    /// The `.evt` read around the event header (0x7386A0 / 0x7363A0 /
    /// 0x738340): each bank's switches in table order fire its events in
    /// file order, and every shipped event uses only what
    /// `Library::request` ports (no keys, no switch masks, no channel
    /// modes, one step, no state components; constraints one per key).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_shipped_banks_use_only_the_ported_features() {
        let (ug2, nis) = banks();
        let ids = |b: &Bank| b.events.iter().map(|e| e.id).collect::<Vec<_>>();
        assert_eq!(ids(&ug2), [0x11, 0x18, 0x19, 0x12, 0x13, 0x10, 0x17, 0x1B, 0x1C, 0x27]);
        assert_eq!(
            ug2.event_switches,
            [
                "Career", "Career_Magazine", "Career_Dvd", "Outrun", "Time", "FE_Screen", "FE_OptionScreen",
                "Intro_Area", "Intro_Events", "FE_Dynoshop"
            ]
        );
        assert_eq!(ids(&nis), [nis::EVENT_START, nis::EVENT_END, 0x4014, nis::EVENT_SOUNDFX]);
        assert_eq!(nis.event_switches, ["NIS_Select_Start", "NIS_Select_End", "NIS_Select_Crowd", "SoundFX_Select"]);
        for b in [&ug2, &nis] {
            for e in &b.events {
                assert!(e.ported(), "{:#x}", e.id);
                assert_eq!((e.chance, e.alternatives.len()), (100, 1), "{:#x}", e.id);
                let a = &e.alternatives[0];
                assert_eq!((a.weight, a.chance, a.weight()), (0x39, 100, 100));
                let s = &a.steps[0];
                for en in b.entries.iter().filter(|en| en.id == s.entry) {
                    assert_eq!(s.constraints.len(), en.keys, "{:#x}", e.id);
                    // 0x737B20 passes the count as a signed byte: the
                    // setters' one-bit values never fit more records than
                    // share one key tuple, under 128 everywhere.
                    let most = en.records.iter().map(|r| en.records.iter().filter(|q| q.1 == r.1).count()).max();
                    assert!(most.unwrap_or(0) < 128);
                }
            }
        }
        // The rings the files carry: FE_Screen's entry 0x2D keeps 1, the
        // outrun entries 0x28 keep 7, the others none.
        let depth = |id: u16| ug2.entries.iter().filter(|e| e.id == id).map(|e| e.history).collect::<Vec<_>>();
        assert_eq!((depth(0x2D), depth(0x28), depth(0x27)), (vec![1], vec![7; 5], vec![0]));
    }

    /// Each setter's tuple plays its own switch's event (0x738630), not
    /// the first event in the file whose records fit it.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn each_setter_plays_its_own_switch_event() {
        let (ug2, _) = banks();
        let cases: [(Vec<u32>, u16); 9] = [
            (lines::career(0x800, 0x20), 0x11),
            (lines::magazine(4, 1), 0x18),
            (lines::dvd(8, 1), 0x19),
            (lines::outrun(8, 1), 0x12),
            (lines::time(1), 0x13),
            (lines::fe_screen(0x20, 0x20), 0x10),
            (lines::fe_option(0x100, 0x10), 0x27),
            (lines::intro_area(4, 0x20), 0x1B),
            (lines::intro_events(0x40_0000, 8), 0x1C),
        ];
        for (tuple, id) in cases {
            let ev = ug2.switch_event(lines::switch_of(&tuple).unwrap()).unwrap();
            assert_eq!(ev.id, id, "{tuple:x?}");
        }
    }

    /// Before/after evidence (docs/native-fixes/N-AUD2.tsv): the
    /// candidates the library picks from for some lines, the first entry
    /// of each from the switch's event.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn candidate_sets_come_from_the_switch_event() {
        let (ug2, nis) = banks();
        // Career (1, 1, 0x20) → event 0x11, entry 0x27: records 11–13.
        assert_eq!(ug2.fit(&lines::career(1, 0x20)), Some((0x27, vec![11, 12, 13])));
        // FE_Screen (2, 1, 0x1000000, 0x20) → event 0x10, entry 0x2D
        // (ring depth 1: the last one played is only a fallback).
        assert_eq!(ug2.fit(&lines::fe_screen(0x100_0000, 0x20)), Some((0x2D, vec![10, 11, 12])));
        // Outrun (8, 8, 1, 3) → event 0x12, entry 0x28 selector 3 (ring
        // depth 7): fourteen records.
        assert_eq!(ug2.fit(&lines::outrun(8, 1)), Some((0x3_0028, (0..14).collect())));
        // THUNDERSTRM (4, 1): the nine [2, 0] records, picked at random.
        assert_eq!(nis.select(nis::EVENT_SOUNDFX, &[4, 1]), (14..=22).collect::<Vec<_>>());
    }

    #[test]
    fn end_tuples() {
        assert_eq!(nis::end_tuple("ShopEnter1", 0), Some([0x200, 0x4000, 1]));
        assert_eq!(nis::end_tuple("EndingNis05", 2), Some([4, 2, 1]));
    }
}

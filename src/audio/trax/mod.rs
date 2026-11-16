//! EA Trax: the interactive music (`pfdata/MusicSFx.mpf` + `.mus` in
//! `SDATA/sdat.viv`), the PathFinder library's data, and the song player
//! (0x47AF90 / 0x47B080 on the music object 0x82B974).
//!
//! * **`.mpf`** ("xDFP", version 4, little-endian; the slot set up by
//!   0x73A7C0, **verified**): +0xD the bank count, +0xF the event count
//!   (0x4F), +0x12 the node count (162); from +0x20 (slot +0x38) one u16
//!   per node, its offset in dwords. A node is 0x13 + ((dword +4 >> 12) &
//!   0x1F) · 4 bytes rounded down to 4; its type is dword +4 >> 24: type 4
//!   plays the stream whose 1-based id is the u16 at +0, type 1 has 0 /
//!   0xFFFF there (no stream); +0x12 the next node. After the last node
//!   (slot +0x40) one u16 per event, its offset in dwords; an event record
//!   (0x13 + (byte +0xE & 0x3F) · 16 bytes, rounded down to 4) holds at
//!   +0x1C the u16 node it starts at. After the last event (slot +0x48)
//!   a dword → (slot +0x4C) per bank {start, end} in dwords of its stream
//!   table (slot +0x50): per stream {offset in the `.mus` / 0x80,
//!   samples / 32}, 43 EALayer3 streams.
//! * **Song → stream** (**verified**): 0x47AF90(i) / 0x4580E0 pass i to
//!   0x7399E0, which takes event i (low byte) of the table at slot +0x40
//!   (0x739A85: +0x40[i] · 4 from the file) and starts it (0x739540 copies
//!   it, 0x7393D0 queues it). Event i starts at node 3i (type 1), whose
//!   next is node 3i + 2 (type 4): song i plays stream id − 1. Song 0,
//!   "Riders on the Storm (Fredwreck Remix)": id 39 → table entry 38
//!   (373.6 s). Checked under Wine (2026-10-29, the title screen's
//!   caption "Riders on the Storm" while the recorded output matched
//!   entry 38's decoded waveform, normalised correlation 0.98; entry 39,
//!   the clone's old pick, 0.2).
//! * **Player**: 0x47AF90(i) (the jukebox's pick) stops the current song
//!   (0x739030), starts event i (0x7399E0), sets the stream's volume
//!   (music object +0x1C, 0..127 → × 1/127, 0x723E9D) and shows the
//!   song's caption (Chyron_ShowSong 0x4AC9F0). 0x47B080 each frame: in
//!   the front end (audio +0x90 = 1) once a song has ended it picks the
//!   next (0x46B6F0, the playlist with the menu mask; 0x479220 turns the
//!   profile states 1 / 3 into the menu mask and 2 / 3 into the race
//!   mask), plays it (0x4580E0) and shows its caption; in game the same
//!   with the race mask, unless paused (0x4A9330). The volume each frame
//!   is the music object's +0xBC >> 8.

mod caption;
mod decode;
mod mpf;
mod playlist;

pub use caption::*;
pub use decode::*;
pub use mpf::*;
pub use playlist::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::viv::Viv;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn front_end_contexts() {
        use crate::formats::hash::bin_hash;
        assert_eq!(front_end_context(false, Some(("CARLOT01.BUN", 0))), 0x1C);
        assert_eq!(front_end_context(true, None), 0x1D);
        assert_eq!(front_end_context(true, Some(("CARLOT01", 0))), 0x17);
        assert_eq!(front_end_context(true, Some(("PERFSHOP01", bin_hash("CC_MEGALOW")))), 0x18);
        assert_eq!(bin_hash("CC_MEGALOW"), 0x7FC8_D0D0);
        assert_eq!(front_end_context(true, Some(("PERFSHOP01", 1))), 0x1A);
        assert_eq!(front_end_context(true, Some(("PARTSSHOP01", bin_hash("SH_BODYSHOP_01")))), 0x17);
        assert_eq!(front_end_context(true, Some(("PARTSSHOP01", 1))), 0x19);
        assert_eq!(front_end_context(true, Some(("PAINTSHOP01", 1))), 0x1B);
        assert_eq!(front_end_context(true, Some(("AUDIOSHOP01", 1))), 0x16);
        assert_eq!(front_end_context(true, Some(("CRIB01", 1))), 0x1D);
        // Each has a piece (0x7ECC10).
        for z in 0x16..=0x1D {
            assert!(score_event(z).is_some(), "{z:#x}");
        }
    }

    #[test]
    fn playlist_sides_and_modes() {
        use crate::opponents::GameRng;
        let mut rng = GameRng::default();
        // A fresh profile: the first six songs Menu, the rest Racing.
        let states: Vec<u8> = (0..27).map(|i| if i < 6 { 1 } else { 2 }).collect();
        let mut p = Playlist::in_table_order(&states, false);
        assert_eq!(p.next(false, &mut rng), Some(0));
        assert_eq!(p.next(true, &mut rng), Some(6));
        assert_eq!(p.next(true, &mut rng), Some(7));
        // The front end goes on from its own position.
        assert_eq!(p.next(false, &mut rng), Some(1));
        for i in 2..6 {
            assert_eq!(p.next(false, &mut rng), Some(i));
        }
        // Spent: the pool refills from the first position.
        assert_eq!(p.next(false, &mut rng), Some(0));
        // A playlist order other than the table's is followed.
        let mut q = Playlist::new(&[(5, 1), (2, 3), (9, 0)], false);
        assert_eq!(q.next(false, &mut rng), Some(5));
        assert_eq!(q.next(false, &mut rng), Some(2));
        assert_eq!(q.next(true, &mut rng), Some(2));
        assert_eq!(q.next(true, &mut rng), Some(2));
        assert_eq!(Playlist::new(&[(1, 0)], false).next(false, &mut rng), None);
        // Random: every front-end song once before any repeats; the
        // static seed 0x12345678 % 6 = 0 picks song 0 first.
        let mut r = Playlist::in_table_order(&states, true);
        let mut rng = GameRng::default();
        let mut seen: Vec<_> = (0..6).map(|_| r.next(false, &mut rng).unwrap()).collect();
        assert_eq!(seen[0], 0);
        seen.sort();
        assert_eq!(seen, vec![0, 1, 2, 3, 4, 5]);
        assert!(r.next(false, &mut rng).is_some_and(|i| i < 6));
    }

    /// The title screen's start (0x47AC50 state 1 with −2): song 0 first
    /// in either mode and from any generator state; then the pool refills
    /// from the mask (Random may draw song 0 again, Sequential starts over
    /// at the first position).
    #[test]
    fn the_title_starts_song_0() {
        use crate::opponents::GameRng;
        let states: Vec<u8> = (0..27).map(|i| if i < 6 { 1 } else { 2 }).collect();
        for seed in [0x1234_5678u32, 1, 0xA420_A8BC, 0xDEAD_BEEF, 0x99_2721_90] {
            let mut rng = GameRng(seed);
            let mut r = Playlist::in_table_order(&states, true);
            r.title_start();
            assert_eq!(r.next(false, &mut rng), Some(0), "seed {seed:#x}");
            // Rand_Int(1) was drawn: the generator moved.
            assert_ne!(rng.0, seed);
            let mut seen: Vec<_> = (0..6).map(|_| r.next(false, &mut rng).unwrap()).collect();
            seen.sort();
            assert_eq!(seen, vec![0, 1, 2, 3, 4, 5], "the refilled pool, seed {seed:#x}");
            // The race side is not touched.
            assert!(r.next(true, &mut rng).is_some_and(|i| i >= 6));
        }
        let mut rng = GameRng::default();
        let mut s = Playlist::in_table_order(&states, false);
        s.title_start();
        assert_eq!(s.next(false, &mut rng), Some(0));
        assert_eq!(s.next(false, &mut rng), Some(0), "Sequential starts over at position 0");
        assert_eq!(s.next(false, &mut rng), Some(1));
        // Bit 0 is song 0, whatever song the first position holds.
        let mut o = Playlist::new(&[(5, 3), (0, 2), (1, 1)], true);
        o.title_start();
        assert_eq!(o.next(false, &mut rng), Some(0));
        // A first position that is not a front-end song (Racing / Off)
        // leaves the pool full.
        for first in [0u8, 2] {
            let mut q = Playlist::new(&[(0, first), (1, 1), (2, 1)], false);
            q.title_start();
            assert_eq!(q.next(false, &mut rng), Some(1));
        }
    }

    /// The boot playlist from the player's game: the profile reset
    /// (0x515AA0) puts each song of the EA Trax table (0x7ECD80) at its
    /// own position, "FE" → Menu, "IG" → Racing, "AL" → All, else Off, in
    /// Random mode (settings +0x54 = 1, 0x5116E0); the title then plays
    /// song 0, Riders on the Storm, for any boot seed (0x57ECC0).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_title_plays_riders_on_the_storm_in_the_game() {
        use crate::opponents::GameRng;
        let t = crate::exe_tables::tables();
        let states: Vec<u8> = t
            .ea_trax_where
            .iter()
            .map(|w| match *w {
                "FE" => 1,
                "IG" => 2,
                "AL" => 3,
                _ => 0,
            })
            .collect();
        assert_eq!(states[0], 1, "song 0 is a front-end song");
        assert!(t.ea_trax[0].0.contains("Riders on the Storm"), "{:?}", t.ea_trax[0]);
        for ticks in [0u64, 1, 0x8284_E4, 123_456_789, 0xFFFF_FFFF] {
            let mut rng = GameRng::boot(ticks);
            let mut p = Playlist::in_table_order(&states, true);
            p.title_start();
            assert_eq!(p.next(false, &mut rng), Some(0), "boot ticks {ticks}");
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn score_events_and_streams() {
        assert_eq!(score_event(SCORE_FRONT_END), Some(41));
        assert_eq!(score_event(0), Some(27));
        assert_eq!(score_event(15), None);
        assert_eq!(score_event(40), None);
    }

    /// The game's score table (the part of `score_events_and_streams` that
    /// reads the music archive).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn score_events_and_streams_in_the_game() {
        let root = crate::testkit::game_root();
        let viv = Viv::mount_game(&root).expect("mounting the game's archives");
        let mpf = viv.read("pfdata/MusicSFx.mpf").and_then(|d| Mpf::parse(&d)).expect("pfdata/MusicSFx.mpf");
        // Event 27 → node 108 → node 110 (stream id 1); the front end's
        // 41 → node 99 → node 101 (id 0x29).
        assert_eq!(mpf.event_sample(27), Some(0));
        assert_eq!(mpf.event_sample(41), Some(40));
        let songs: Vec<_> = (0..27).filter_map(|i| mpf.song_sample(i)).collect();
        for (i, &s) in songs.iter().enumerate() {
            let (off, n) = mpf.samples[s];
            println!("song {i}: stream {s} at {off:#x}, {n} samples ({:.1} s at 32 kHz)", n as f32 / 32000.0);
        }
        for z in 0..31u8 {
            if let Some(e) = score_event(z) {
                let s = mpf.event_sample(e).unwrap();
                assert!(!songs.contains(&s), "score stream {s} is a song's");
            }
        }
    }

    /// The in-game score context (0x46B330: the "Ambience" map's field 0
    /// at the car, 0x5FA550 / 0x5F0B90): read under Wine in free roam on
    /// the stage-2 save — at its resume place (462.4, −46.2) the accessor's
    /// row holds 0 and +0xB4 = 0 (event 27); placed on the highway (352.6,
    /// −1445.0) 5 (event 55's piece). The clone's map gives the same.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_score_context_is_the_ambience_map_at_the_car() {
        let root = crate::testkit::game_root();
        let raw = std::fs::read(crate::game_fs::path(&root, "TRACKS/L4RA.BUN")).expect("TRACKS/L4RA.BUN");
        let data = if raw.starts_with(b"JDLZ") { crate::formats::jdlz::decompress(&raw).expect("JDLZ") } else { raw };
        let maps = crate::formats::parameter_map::parse_parameter_maps(&data).expect("parameter maps");
        let ambience = maps.get(&crate::formats::hash::bin_hash("Ambience")).expect("the Ambience map");
        let at = |x: f32, y: f32| ambience.sample(glam::Vec2::new(x, y))[0].to_bits() as i32;
        assert_eq!(at(462.4, -46.2), 0);
        assert_eq!(at(352.6, -1445.0), 5);
        assert_eq!(score_event(0), Some(27));
    }

    #[test]
    fn pace_waits_only_past_its_lead() {
        let p = Pace::LIVE;
        assert_eq!(p.wait(0.0, 0.0), 0.0);
        assert_eq!(p.wait(p.lead, 0.0), 0.0);
        // A second past its head start at the start: a quarter second's wait.
        assert!((p.wait(p.lead + p.speed, 0.0) - 1.0).abs() < 1e-12);
        assert_eq!(p.wait(p.lead + p.speed, 1.0), 0.0);
        // Ahead of the live mixer (1 s per s) from the start on.
        for t in [0.0, 0.5, 10.0, 300.0] {
            assert!(p.lead + t * p.speed > t);
        }
    }

    /// A paced decode gives the unpaced decode's samples, and paced live it
    /// has decoded only its lead (and the blocks the wall time allows) a
    /// moment after the start, not the whole song.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn paced_decode_gives_the_same_samples() {
        use crate::audio::mixer::{TrackBuf, TrackPcm};
        let root = crate::testkit::game_root();
        let viv = std::sync::Arc::new(Viv::mount_game(&root).expect("mounting the game's archives"));
        let mpf = std::sync::Arc::new(
            viv.read("pfdata/MusicSFx.mpf").and_then(|d| Mpf::parse(&d)).expect("pfdata/MusicSFx.mpf"),
        );
        let fresh = || -> TrackBuf {
            std::sync::Arc::new(std::sync::Mutex::new(TrackPcm { channels: 2, rate: 32000, samples: Vec::new(), done: false }))
        };
        // Each decoded through a second hold, as the mixer's track holds
        // the buffer (a decoder alone with its buffer stops).
        let whole = fresh();
        decode_song(&viv, &mpf, 1, &whole.clone()).expect("song 1");
        // Fast enough to finish here, slow enough to wait between blocks.
        let paced = fresh();
        decode_song_paced(&viv, &mpf, 1, &paced.clone(), Some(Pace { lead: 2.0, speed: 120.0 })).expect("song 1, paced");
        let (a, b) = (whole.lock().unwrap(), paced.lock().unwrap());
        assert!(a.samples.len() > 32000 * 2 * 60);
        assert_eq!((a.rate, a.channels), (b.rate, b.channels));
        assert!(a.samples.iter().map(|s| s.to_bits()).eq(b.samples.iter().map(|s| s.to_bits())));
        drop((a, b));
        // Live: a moment in, about its head start decoded; the decoder ends once
        // the song is dropped.
        let live = fresh();
        let (v, m, l) = (viv.clone(), mpf.clone(), live.clone());
        let job = std::thread::spawn(move || decode_song_paced(&v, &m, 1, &l, Some(Pace::LIVE)));
        let t = std::time::Instant::now();
        std::thread::sleep(std::time::Duration::from_millis(400));
        let secs = {
            let p = live.lock().unwrap();
            (p.samples.len() / p.channels.max(1)) as f64 / f64::from(p.rate.max(1))
        };
        let bound = Pace::LIVE.lead + t.elapsed().as_secs_f64() * Pace::LIVE.speed + 1.5;
        assert!(secs > 0.0 && secs <= bound, "{secs:.1} s decoded, at most {bound:.1} expected");
        drop(live);
        assert!(job.join().unwrap().is_none(), "a dropped song's decoder stops");
    }
}

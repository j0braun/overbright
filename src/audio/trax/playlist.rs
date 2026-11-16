/// g_InteractiveMusicEvents 0x7ECC10: per music context (the music
/// object's +0xB4) the score's {start, second} events; the first is the
/// one 0x47B260 starts (0x7399E0 / 0x4580E0 on its low 16 bits). −1: no
/// score. Context 0x1C is the front end's (0x46B330), 0x16–0x1D the
/// career shops'; in game the context is the "Ambience" parameter map's
/// field 0 at the player car (0x46B330 → 0x5FA550 / 0x5F0B90 on the map
/// object 0x82C710).
pub static SCORE_EVENTS: crate::exe_tables::Table<[(i16, i16); 31]> = crate::exe_tables::Table::new(|t| &t.score_events);

/// The front end's music context (0x46B330 outside a career shop).
pub const SCORE_FRONT_END: u8 = 0x1C;

/// The front end's music context (0x46B330 with audio +0x90 = 1): 0x1C;
/// in a career (0x850078 = 1) by the shop record (0x86344C while
/// 0x863444 is set): its scene name (+0x40, up to a '.') and hash
/// (+0x38). CARLOT01 → 0x17; PERFSHOP01 → 0x18 at CC_MEGALOW, else 0x1A;
/// PARTSSHOP01 → 0x17 at SH_BODYSHOP / SH_BODYSHOP_01, else 0x19;
/// PAINTSHOP01 → 0x1B; AUDIOSHOP01 → 0x16; CRIB01 or no record → 0x1D;
/// another name keeps 0x1C (the names hashed by 0x486160).
pub fn front_end_context(career: bool, shop: Option<(&str, u32)>) -> u8 {
    use crate::formats::hash::bin_hash;
    if !career {
        return SCORE_FRONT_END;
    }
    let Some((name, hash)) = shop else {
        return 0x1D;
    };
    let name = name.split('.').next().unwrap_or_default().to_ascii_uppercase();
    match name.as_str() {
        "CARLOT01" => 0x17,
        "CRIB01" => 0x1D,
        "PERFSHOP01" => {
            if hash == bin_hash("CC_MEGALOW") {
                0x18
            } else {
                0x1A
            }
        }
        "PARTSSHOP01" => {
            if hash == bin_hash("SH_BODYSHOP") || hash == bin_hash("SH_BODYSHOP_01") {
                0x17
            } else {
                0x19
            }
        }
        "PAINTSHOP01" => 0x1B,
        "AUDIOSHOP01" => 0x16,
        _ => SCORE_FRONT_END,
    }
}

/// The score event context `zone` starts, if any.
pub fn score_event(zone: u8) -> Option<usize> {
    SCORE_EVENTS.get(zone as usize).and_then(|e| usize::try_from(e.0).ok())
}

/// The EA Trax playlist state on the music object (0x82B884): per side
/// (front end / game) the mask of the songs it may play, the pool of
/// those not played yet and the playlist position of the last sequential
/// pick, plus the play mode. 0x479220 builds the masks from the profile's
/// playlist (0x83ACB8: 27 × (index, state); states 1 / 3 into the front
/// end's mask, 2 / 3 into the game's) and 0x46BAA0 stores them: masks
/// +0x84 / +0x8C, pools +0x88 / +0x90 (full), last positions +0xA8 /
/// +0xAC (−1), mode +0x9C (settings +0x54, 1 Random). It runs on the
/// profile's reset (0x515BCE) and load (0x5346AA), each jukebox state
/// change (0x53DEC0 → 0x53E09D), its play-mode toggles (0x524246,
/// 0x565E38), the jukebox's exit (0x56B1D0 on 0x911AB364 → 0x56B25A) and
/// 0x551071. The pools live for the session: going between the front end
/// and the game does not touch them, so each side goes on from where it
/// was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Playlist {
    /// The profile's playlist: per position (song index, state).
    pub order: Vec<(usize, u8)>,
    pub random: bool,
    /// Front end, game: (mask, pool, last sequential position).
    sides: [(u32, u32, i32); 2],
}

impl Playlist {
    /// 0x479220 → 0x46BAA0.
    pub fn new(order: &[(usize, u8)], random: bool) -> Playlist {
        let (mut fe, mut ig) = (0u32, 0u32);
        for &(i, s) in order {
            let bit = 1u32 << (i & 31);
            if s == 1 || s == 3 {
                fe |= bit;
            }
            if s == 2 || s == 3 {
                ig |= bit;
            }
        }
        Playlist { order: order.to_vec(), random, sides: [(fe, fe, -1), (ig, ig, -1)] }
    }

    /// The playlist in table order (what a fresh profile's reset
    /// 0x515AA0 writes: position j holds song j) with these states.
    pub fn in_table_order(states: &[u8], random: bool) -> Playlist {
        let order: Vec<_> = states.iter().enumerate().map(|(i, &s)| (i, s)).collect();
        Playlist::new(&order, random)
    }

    /// The title screen's music start (UG_LS_Splash's constructor 0x4A90F0
    /// → 0x480C30(−2) → 0x47AC50 state 1 with −2, at 0x47AEC4): when the
    /// playlist's first position is a front-end song (its state byte
    /// 0x83ACBC has bit 0: Menu or All), the front end's pool becomes 1 —
    /// song 0 alone (bit 0 is song index 0, whatever song the first
    /// position holds) — so the next front-end pick is song 0, "Riders on
    /// the Storm (Fredwreck Remix)", in either play mode (Random draws
    /// Rand_Int(1) = 0). The mask and the last position stay as they were;
    /// the pick empties the pool, so the pick after it refills from the
    /// mask.
    pub fn title_start(&mut self) {
        if self.order.first().is_some_and(|&(_, s)| s & 1 != 0) {
            self.sides[0].1 = 1;
        }
    }

    /// Whether the side's mask has a song (0x482180 tests the front end's
    /// +0x84 or the game's +0x8C before it asks 0x47B080 for one).
    pub fn has_songs(&self, game: bool) -> bool {
        self.sides[game as usize].0 != 0
    }

    /// 0x46B6F0: the next song of the front end's (`game` false; also
    /// the dyno's, g_RaceType 8) or the game's playlist, or none (−1:
    /// that side has no song). A spent pool refills with the mask first
    /// (and the last position goes back to −1). Sequential: the first
    /// playlist position after the last one whose song is still in the
    /// pool. Random: Rand_Int (0x43C1C0) over the count of songs left,
    /// the n-th of them by song index; the last position is left as it
    /// was. The pick leaves the pool.
    pub fn next(&mut self, game: bool, rng: &mut crate::opponents::GameRng) -> Option<usize> {
        let (mask, pool, last) = &mut self.sides[game as usize];
        if *mask == 0 {
            return None;
        }
        if *pool == 0 {
            *pool = *mask;
            *last = -1;
        }
        if !self.random {
            for (pos, &(i, _)) in self.order.iter().enumerate().take(27) {
                let bit = 1u32 << (i & 31);
                if pos as i32 > *last && *pool & bit != 0 {
                    *last = pos as i32;
                    *pool ^= bit;
                    return Some(i);
                }
            }
            return None;
        }
        // The count runs over bits 0..26 (0x46B7A0's unrolled loop).
        let count = (*pool & 0x7FF_FFFF).count_ones();
        let n = rng.below(count);
        let mut k = 0;
        for i in 0..27 {
            let bit = 1u32 << i;
            if *pool & bit != 0 {
                if k == n {
                    *pool ^= bit;
                    return Some(i);
                }
                k += 1;
            }
        }
        None
    }
}

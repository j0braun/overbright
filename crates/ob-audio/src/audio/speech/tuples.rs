/// The in-game speech bank's switch tuples the game sets (the setters
/// 0x46D150…0x46D580 on the speech object 0x82BA30 +0x2C, each through
/// 0x735880 on its block; names table 0x7ED0DC).
pub mod lines {
    /// 0x510ED0 → 0x46D150(1, a, b): Career = (1, a, b).
    pub fn career(a: u32, b: u32) -> Vec<u32> {
        vec![1, a, b]
    }
    /// 0x510F10 → 0x46D390(4, a): Time = (4, a).
    pub fn time(a: u32) -> Vec<u32> {
        vec![4, a]
    }
    /// 0x510F30 → 0x46D400(2, 1, a, b): FE_Screen = (2, 1, a, b).
    pub fn fe_screen(a: u32, b: u32) -> Vec<u32> {
        vec![2, 1, a, b]
    }
    /// 0x510F50 → 0x46D580(a, b): FE_Dynoshop = (2, 0x20, a, b) (0x46D580
    /// registers the name 0x7ED124, FE_Dynoshop; FE_OptionScreen, 0x7ED10C,
    /// is only registered by 0x4587A0 and never set).
    pub fn fe_option(a: u32, b: u32) -> Vec<u32> {
        vec![2, 0x20, a, b]
    }
    /// 0x510F70 → 0x46D500(a, +8 or +0xC by Rand_Int(2)): Intro_Events =
    /// (0x10, a, b).
    pub fn intro_events(a: u32, b: u32) -> Vec<u32> {
        vec![0x10, a, b]
    }
    /// 0x510FA0 → 0x46D480(a, b): Intro_Area = (0x20, a, b).
    pub fn intro_area(a: u32, b: u32) -> Vec<u32> {
        vec![0x20, a, b]
    }
    /// 0x510FC0 → 0x46D1D0(1, 0x200, a, b): Career_Magazine.
    pub fn magazine(a: u32, b: u32) -> Vec<u32> {
        vec![1, 0x200, a, b]
    }
    /// 0x510FE0 → 0x46D250(1, 0x400, a, b): Career_Dvd.
    pub fn dvd(a: u32, b: u32) -> Vec<u32> {
        vec![1, 0x400, a, b]
    }
    /// The switch whose setter built `tuple`: each setter above stores a
    /// fixed length with fixed leading values (its own constants), so the
    /// tuples of different setters never coincide. The library fires the
    /// switch's own event (0x7386A0 / 0x738630), so this is the event a
    /// line plays — not a search over the events (verified in the disassembly).
    pub fn switch_of(tuple: &[u32]) -> Option<&'static str> {
        Some(match *tuple {
            [1, _, _] => "Career",
            [1, 0x200, _, _] => "Career_Magazine",
            [1, 0x400, _, _] => "Career_Dvd",
            [8, _, _, _] => "Outrun",
            [4, _] => "Time",
            [2, 1, _, _] => "FE_Screen",
            [2, 0x20, _, _] => "FE_Dynoshop",
            [0x20, _, _] => "Intro_Area",
            [0x10, _, _] => "Intro_Events",
            _ => return None,
        })
    }

    /// 0x510EF0 (vtable 0x79AD00) → 0x46D2D0(8, a, b): Outrun = (8, a, b,
    /// m) with m = 3, 2, 4, 5 for b = 1, 4, 8, 0x200, else 1 (0x46D2D0's
    /// switch; verified, the disassembly).
    pub fn outrun(a: u32, b: u32) -> Vec<u32> {
        let m = match b {
            1 => 3,
            4 => 2,
            8 => 4,
            0x200 => 5,
            _ => 1,
        };
        vec![8, a, b, m]
    }
}

/// The NIS stream kinds of 0x484510 and the NIS_Select_End tuples it sets.
pub mod nis {
    /// The NIS bank's events (`nisaudio.evt`): NIS_Select_Start's (race
    /// starts, entry 0x4023) and NIS_Select_End's (endings, shops, the
    /// career start; entry 0x4024).
    pub const EVENT_START: u16 = 0x4000;
    pub const EVENT_END: u16 = 0x4011;
    /// SoundFX_Select's event (0x45A770; constraints (0x1F, 0), (0x3F,
    /// 1)): the first value's bit names the stream — 1 GB_NOS (second 4),
    /// 2 CollisionStream (8), 4 THUNDERSTRM (1 or 2), 8 BLOWNENGINE
    /// (0x10), as 0x484510 sets it — and the records carry those keys
    /// ([0, 2], [1, 3], [2, 0] × 9, [3, 4]; [4, 5] has no caller). No
    /// record has [2, 1], so THUNDERSTRM's variant 2 (rain ≥ 0.5) fits
    /// none and nothing plays (0x737230 returns 0, 0x737882 plays nothing:
    /// now the library's own path).
    pub const EVENT_SOUNDFX: u16 = 0x400E;
    /// CareerNis1 (0x484AF4): NIS_Select_End = (0x400, 0x4000, 1), stream
    /// kind 9 (0x46CA60), then Rachel's hints are told (0x5258C0).
    pub const CAREER: [u32; 3] = [0x400, 0x4000, 1];

    /// A race start scene's NIS_Select_Start tuple (NisAudio_Request
    /// 0x484510 kind 2 from the NIS load 0x435CE8, which passes the
    /// scene's hash and the pick's variant): the scene's bit (IntroNis`n`
    /// → table 0x7ED068[n − 1] = 1 << (n − 1); IntroNis30..32 read past
    /// it, none here), the camera track's bit, and 4 when the flow's +0x1C
    /// is 3, else 1 (`flow_3`). The track (0x48473E..0x4847C1): the
    /// variant, + 8 in game modes 8 / 0x200 (online, LAN) or with the
    /// split-screen flag (manager +0x84 = 1), else + 4 for a tournament
    /// whose +0x0D is 1 (0x863470) or when the player car's engine sound
    /// index (context +0x8C) is 0x15, 0xE or 9; the bit is 1 << (track −
    /// 1) for tracks 1..13, else 0x2000 (jump table 0x484B98). The
    /// tournament and split-screen cases are not modelled.
    pub fn race_start(n: u8, variant: u8, game_mode: u32, engine: usize, flow_3: bool) -> [u32; 3] {
        let bit = track_bit(track(variant, game_mode, engine));
        let scene = if (1..=29).contains(&n) { 1u32 << (n - 1) } else { 0 };
        [scene, bit, if flow_3 { 4 } else { 1 }]
    }

    /// 0x48473E..0x4847C1, for every scene NisAudio_Request kind 2 loads:
    /// the variant, + 8 in game modes 8 / 0x200, else + 4 for the engine
    /// sound indices 0x15, 0xE and 9 (the tournament and split-screen
    /// cases not modelled).
    pub fn track(variant: u8, game_mode: u32, engine: usize) -> u32 {
        let mut track = variant as u32;
        if game_mode == 8 || game_mode == 0x200 {
            track += 8;
        } else if matches!(engine, 0x15 | 0xE | 9) {
            track += 4;
        }
        track
    }

    /// NIS_Select_End for the shop / dyno / ending scenes (0x4848F0..):
    /// EndingNis0n → 1 << (n − 1) (n ≥ 3 → 4), the dynos 0x100, the shops
    /// 0x200; the second value 0x4000 entering, 0x8000 leaving, else the
    /// camera track's bit ([`track_bit`] of [`track`]); the third 1. Set
    /// by 0x45A720 at 0x484AC5, then stream kind 6 is queued (0x46CA60).
    pub fn end_tuple(scene: &str, track: u32) -> Option<[u32; 3]> {
        let first = match scene {
            "DynoEnter" | "DynoExit" => 0x100,
            "ShopEnter1" | "ShopExit1" => 0x200,
            s => {
                let n: u32 = s.strip_prefix("EndingNis0")?.parse().ok()?;
                if !(1..=8).contains(&n) {
                    return None;
                }
                let f = 1 << (n - 1);
                if f > 4 { 4 } else { f }
            }
        };
        let second = match scene {
            "DynoEnter" | "ShopEnter1" => 0x4000,
            "DynoExit" | "ShopExit1" => 0x8000,
            _ => track_bit(track),
        };
        Some([first, second, 1])
    }

    /// The camera track's bit (0x484510's jump table 0x484B98: 0 or over
    /// 13 → 0x2000, n → 1 << (n − 1)).
    pub fn track_bit(track: u32) -> u32 {
        if track == 0 || track > 13 { 0x2000 } else { 1 << (track - 1) }
    }
}

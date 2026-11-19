//! The settings model: per-player and global options.

/// Per-player settings (0x83AA64 + player × 0x28).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerSettings {
    /// +0x00 Gauges, +0x01 Race Position, +0x02 Lap Info, +0x03 Score,
    /// +0x05 Leaderboard: the custom-HUD elements.
    pub gauges: bool,
    pub position: bool,
    pub lap_info: bool,
    pub score: bool,
    pub leaderboard: bool,
    /// +0x04 Feedback (force feedback).
    pub feedback: bool,
    /// +0x06 Ask for Transmission.
    pub ask_transmission: bool,
    /// +0x10 Camera: 0 Bumper, 1 Hood, 2 Close, 3 Far, 4 Super Far.
    pub camera: u8,
    /// +0x14 Split Time: 0 Leader, 1 Lap Record, 2 Best Lap, 3 Last Lap,
    /// 4 Off.
    pub split: u8,
    /// +0x15 Transmission: manual.
    pub manual: bool,
    /// +0x16 Stability Control: 1 shows "Off".
    pub stability_off: bool,
    /// UI_Wheel_Options' sliders (0x4B8A10): +0x1C Wheel Range (0.1–1),
    /// +0x20 Road Feedback (2–20, the slider showing 1 − (v − 2) / 18),
    /// +0x24 Overall Force Strength (0–1).
    pub wheel_range: f32,
    pub road_feedback: f32,
    pub force: f32,
}

impl Default for PlayerSettings {
    /// 0x5116E0: the HUD elements, feedback and the transmission question
    /// on, camera Far, split time Leader, automatic, stability value 1.
    fn default() -> PlayerSettings {
        PlayerSettings {
            gauges: true,
            position: true,
            lap_info: true,
            score: true,
            leaderboard: true,
            feedback: true,
            ask_transmission: true,
            camera: 3,
            split: 0,
            manual: false,
            stability_off: true,
            // 0x511793…0x51179D: 1.0, 12.4, 1.0.
            wheel_range: 1.0,
            road_feedback: 12.4,
            force: 1.0,
        }
    }
}

/// The options the screens edit.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Volumes 0–1 (profile options +0x24…): master, speech and movies,
    /// menu music, game music, sound effects, engine, tires.
    pub master: f32,
    pub speech: f32,
    pub menu_music: f32,
    pub game_music: f32,
    pub effects: f32,
    pub engine: f32,
    pub tires: f32,
    /// +0x4C Audio Mode: stereo (0 mono).
    pub stereo: bool,
    /// The Resolution row's size. The game keeps an index (0x870D1C) into
    /// its six (`display::NATIVE_MODES`); the clone keeps the size, from
    /// the longer list `display::modes` (agreed deviation).
    pub resolution: (u32, u32),
    /// Clone option (agreed deviation; the game runs full screen unless
    /// told otherwise): full screen at the resolution (the monitor's own
    /// mode at that size with its highest refresh rate, else borderless
    /// at the desktop size), or a window of that size.
    pub fullscreen: bool,
    /// Clone option: `display::AspectMode` (0 Wide, the default; 1
    /// Stretched, native).
    pub aspect: u8,
    /// Clone option: Vsync off presents by Mailbox (true, the default) or
    /// Immediate (native, tears).
    pub vsync_off_mailbox: bool,
    /// Level Of Detail (0x87099C), 1–5.
    pub detail: u8,
    /// UI_PC_Customize_Options' values (0x870CB8…), by [`CUSTOM_ROWS`].
    pub custom: [u8; 22],
    /// Clone: the player's own `custom` value that an enhanced Graphics
    /// tier raised to World Detail 5's (`graphics_table::custom_part`),
    /// given back once no enhanced tier holds it (Restore Native, the
    /// Original preset); options.txt `enh.raised.N v`.
    pub custom_raised: [Option<u8>; 22],
    /// docs/notes/performance/overlay.md: the performance overlay's options
    /// (options.txt `overlay.*`).
    pub overlay: crate::overlay::Settings,
    /// +0x1C Jump Camera: 0 off, else on (the option writes 127, the
    /// defaults 127.5).
    pub jump_camera: bool,
    /// +0x14 Rearview Mirror.
    pub rearview: bool,
    /// +0x16 Speedometer Units: km/h (0 mph).
    pub metric: bool,
    /// Career Difficulty (profile +0x250): 0 Easy, 1 Medium, 2 Hard.
    pub difficulty: u8,
    /// +0x18 Explore Map Mode and +0x17 Race Map Mode: 0 Off, 1 Static,
    /// 2 Rotate.
    pub explore_map: u8,
    pub race_map: u8,
    /// Save (0x844AD8 / 0x844AD9): prompts and autosave.
    pub save_prompts: bool,
    pub autosave: bool,
    pub players: [PlayerSettings; 2],
    /// The EA Trax playlist (profile 0x83ACB8: per track an index and a
    /// state, 0 Off, 1 Menu, 2 Racing, 3 All), here in track table order.
    pub trax: [u8; 27],
    /// The playlist's order: per position the song index (the profile's
    /// 0x83ACB8 entry +0; a fresh profile's reset 0x515AA0 writes the
    /// table order, and no code but that reset and the profile load
    /// writes it).
    pub trax_order: [u8; 27],
    /// The EA Trax play mode (settings +0x54, 0x83AA58): Random, else
    /// Sequential (0x524130; a fresh profile's is Random, 0x5116E0).
    pub trax_random: bool,
    /// The Quick Race mode records (profile race record 0x83AAB4 +0x10,
    /// one per race type; see `quick_race`).
    pub qr: [super::quick_race::QrRecord; super::quick_race::RECORDS],
    /// The keyboard map (input map 0x874C40: per action a primary and a
    /// secondary key), by Controls row, as DirectInput key names.
    pub keys: [[String; 2]; 18],
    /// The Enhancements page (QoL, clone deviations; the defaults native),
    /// kept in options.txt.
    pub enh: crate::qol::Enhancements,
    /// The profile's controller maps (payload 0xD254, `pad::BLOCK_LEN`
    /// bytes each, by device GUID; the keyboard's block is not here): the
    /// records' blocks of the devices that were attached when it was last
    /// written (0x5B9430).
    pub pad_blocks: Vec<[u8; crate::pad::BLOCK_LEN]>,
}

impl Settings {
    /// The EA Trax playlist as the music object reads it (0x479220): per
    /// position (song index, state).
    pub fn trax_playlist(&self) -> Vec<(usize, u8)> {
        self.trax_order.iter().map(|&i| (i as usize, self.trax.get(i as usize).copied().unwrap_or(0))).collect()
    }
}

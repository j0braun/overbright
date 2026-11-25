//! The engine's tables in one struct, as their users read them
//! ([`super::Table`]): built once from the constants, the formulas and the
//! decoders' standard tables (docs/original/exe-tables.md lists every table with
//! its users).

use super::constants::{car_audio as au, driving as dr, frontend as fe, garage as ga};
use crate::exe_tables::records::{Rec80, Rec84, ShiftRec, SkidRecord};
use crate::exe_tables::records::{RigLight, Shot};

/// The VP6 decoder's tables (0x7C4618..0x7C6668, 0x816708).
#[derive(Debug)]
pub struct Vp6Tables {
    pub ac_dequant: [i32; 64],
    pub dc_dequant: [i16; 64],
    pub zigzag: [u8; 64],
    pub coeff_group: [i8; 64],
    pub default_reorder: [u8; 64],
    pub ref_of_type: [u8; 10],
    pub dccv_update: [u8; 22],
    pub reorder_update: [u8; 64],
    pub runv_update: [u8; 28],
    pub runv_default: [u8; 28],
    pub ract_update: [u8; 396],
    pub dccv_lc: [i32; 30],
    pub vector_update: [u8; 34],
    pub mb_stats_presets: [u8; 960],
    pub mb_stats_default: [u8; 60],
    pub cat_table: [u8; 176],
    pub candidates: [i32; 24],
    pub bilinear: [i32; 16],
    pub token_base: [i32; 12],
    pub coeff_group_huffman: [i8; 64],
}

/// The tables, built once on first use ([`super::tables`]).
#[derive(Debug)]
pub struct Tables {
    // ---- car physics and driving
    /// Stick response over input −1..1 (graph 0x7E9E60 → 0x7E9228).
    pub steer_response: [f32; 21],
    /// The same in drift and URL races (0x7E9E9C → 0x7E9330).
    pub steer_response_drift: [f32; 21],
    /// A wheel's response (0x4061C0: graph 0x7E9E88 → 0x7E92D8).
    pub steer_response_wheel: [f32; 21],
    /// Lateral force curves: row 0 in zero-filled data (0x86B34C), rows
    /// 1..6 at 0x7FC744 (graph 0x7FC7D4).
    pub lateral: [[f32; 6]; 7],
    /// The race tunes, rows 0x144 apart by layout: drift 0x7FC2E4, type 2
    /// 0x7FC33C, drag 0x7FC394.
    pub drift_tune: [[f32; 22]; 3],
    pub street_x_tune: [[f32; 22]; 3],
    pub drag_tune: [[f32; 22]; 3],
    /// The 0-60 / 0-100 mph time factors (0x7FC120 / 0x7FC1D8).
    pub zero_60_factor: [f32; 31],
    pub zero_100_factor: [f32; 31],
    /// The contact surface weight: surface table 0x800EE0 + 0x98·s, +0x34.
    pub surface_weight: [f32; 29],
    /// The chase camera's shake graph (0x7EC430).
    pub shake: [[f32; 4]; 5],
    // ---- drift scoring
    /// The multiplier levels' speeds (0x79DA78) and points (0x79DA90).
    pub drift_level_speed: [f32; 6],
    pub drift_level_points: [[i32; 4]; 4],
    /// The downhill virtual rivals (0x7FA5C0, 6 rows of 13 dwords).
    pub drift_virtual_scores: [(u16, [f32; 12]); 6],
    // ---- AI driving
    /// The AI steering lock and table rate by speed (0x7E9074, 0x7E909C).
    pub ai_lock: [f32; 10],
    pub ai_table_rate_speed: [f32; 10],
    /// The 16-bit arcsine (0x7EA628, 8-byte entries {u16 angle, f32 slope}).
    pub asin: [(u16, f32); 208],
    // ---- audio
    /// The collision level curve (0x7ECB78) and list 0's gains (0x7F34A0).
    pub collision_levels: [f32; 20],
    pub collision_gain: [u16; 289],
    /// The mix map's dB ↔ linear tables (0x7EDE88, 0x7EE7F0).
    pub db_to_lin: [i32; 602],
    pub lin_to_db: [i32; 512],
    /// Per sound index and level, the 12-byte entries of 0x7F2230: the
    /// engine entry (+0), the shift record (+4), the turbo record (+8).
    pub engine_levels: [[u8; 4]; 33],
    pub shifts: [[u8; 4]; 33],
    pub turbos: [[u8; 4]; 33],
    /// The shift sweeteners (0x7F3040, 12 bytes: the SWTN_CAR_<nn> bank's
    /// number, the CAR_SWTN and CAR_Sputter volumes).
    pub swtn: [(u8, i32, i32); 43],
    /// The engine entries' GIN names (the engine records' +0x0C less
    /// "GIN_" and ".gin").
    pub gins: [&'static str; 43],
    /// The shift records (0x7F2860, 0x60 bytes) and launch records
    /// (0x7F3248, 20 bytes); each engine record's launch record (+0x18).
    pub shift_recs: [ShiftRec; 16],
    pub launches: [(f32, i32, i32, i32, i32); 12],
    pub launch_index: [u8; 43],
    /// The skid tuning records (0x7F2F20, 0x48 bytes) and the skid class
    /// by sound index (0x458F00's switch).
    pub skid_records: [SkidRecord; 4],
    pub skid_class: [u8; 32],
    /// The car type's sound index (0x459240's switch).
    pub sound_index: [u8; 31],
    /// The engine and decel records (0x7F0DE0, 0x7F1AA8).
    pub engines: [Rec80; 43],
    pub decels: [Rec84; 37],
    /// The 15 scripted car-audio patterns (0x7F0070..0x7F08F8,
    /// {u16 rpm, u16 throttle}).
    pub patterns: [&'static [(u16, u8)]; 15],
    /// The music contexts' score events (0x7ECC10, dword pairs).
    pub score_events: [(i16, i16); 31],
    /// The trains (0x7ED798, 12-byte entries).
    pub trains: [(u32, u32); 24],
    /// The UTK decoder: reflection coefficients (0x819208), codebooks
    /// (0x819308), commands (0x819508, 12 bytes).
    pub utk_rc: [f32; 64],
    pub utk_codebooks: [[u8; 256]; 2],
    pub utk_commands: [(u8, u8, f32); 29],
    /// The layer III decoder: the Huffman trees (pointer list 0x7D0C30),
    /// big values' linbits (0x7D0CC0), the synthesis window (0x7D1600,
    /// each row twice).
    pub mp3_huff: [Option<Box<[i16]>>; 32],
    pub mp3_linbits: [u8; 16],
    pub mp3_window_rows: [[f32; 16]; 17],
    pub vp6: Vp6Tables,
    // ---- rendering
    /// The in-game and default car light setups (0x7F3F00, 0x7F3E10).
    pub in_game_rig: [RigLight; 3],
    pub default_rig: [RigLight; 3],
    // ---- garage cameras
    /// The part views (0x7F4608: two tables of 0x31 records of 0x3C bytes,
    /// cars then SUVs; the slot at +8, the view at +0xC): records 0..41.
    pub slot_shots: [[(u32, Shot); 42]; 2],
    /// The vinyl views (0x7F5D40, 7 records of 0x3C a class) and the SUV
    /// engine view (0x7F5D00).
    pub vinyl_shots: [[Shot; 7]; 2],
    pub suv_engine_shot: Shot,
    // ---- front end and career
    /// The world map's zoom records (0x7F42C8, 16 bytes).
    pub world_map_zooms: [(f32, f32, f32, u32); 6],
    /// Magazine / DVD covers and the DVD by rating (0x7FA3F0, 0x7FA490,
    /// 0x7FA4DC).
    pub magazine_lines: [(u32, u32); 20],
    pub dvd_lines: [(u32, u32); 10],
    pub rating_lines: [u32; 11],
    /// The part type names (pointers 0x80333C).
    pub part_type_names: [&'static str; 0x69],
    /// The slots' part types (0x802960, dwords), cost categories (set up
    /// by 0x501920 into 0x837D30) and vinyl slot categories by kind
    /// (0x501D70's switch 0x502020).
    pub slot_types: [u8; 0xAA],
    pub slot_categories: [u8; 0xAA],
    pub vinyl_categories: [u8; 28],
    /// The RANDOM opponents' car types (0x7F6DA4).
    pub opponent_cars: [u8; 26],
    /// The vinyl groups (0x546040's calls of 0x545930: kind, icon, label).
    pub vinyl_groups: [(u8, u32, u32); 28],
    /// Quick Race opponent names (pointers 0x7F6CD0).
    pub opponent_names: [&'static str; 53],
    /// EA Trax (0x7ECD80, 16 bytes: title, artist, album pointers).
    pub ea_trax: [(&'static str, &'static str, &'static str); 27],
    /// Where each EA Trax track plays (+0xC: "FE" or "IG").
    pub ea_trax_where: [&'static str; 27],
    /// The tuning parameters' names (0x50FE90's switch).
    pub tuning_names: [u32; 25],
    /// The graphics cards' scores (0x8005C0, 12 bytes: vendor, device,
    /// f32 score).
    pub card_scores: [(u32, u32, u8); 120],
    // ---- read by the fresh profile writer (career::fresh_payload)
    /// The Quick Race records' default cars by car type index (0x7FAA48,
    /// 0x7FAA4C).
    pub qr_default_cars: [u32; 2],
    /// The ranked rides' table as 0x77D3B0 fills it (0x8641A8, 0x12
    /// dwords: the ONLINE_CAREER_* name hashes, stock car indices, kept
    /// numbers).
    pub ranked_rides: [u32; 0x12],
}

impl Tables {
    /// Every table from the code ([`super::constants`], [`super::generated`],
    /// [`super::third_party`]).
    pub fn builtin() -> Tables {
        Tables {
            steer_response: dr::STEER_RESPONSE,
            steer_response_drift: dr::STEER_RESPONSE_DRIFT,
            steer_response_wheel: dr::STEER_RESPONSE_WHEEL,
            lateral: dr::LATERAL,
            drift_tune: dr::DRIFT_TUNE,
            street_x_tune: dr::STREET_X_TUNE,
            drag_tune: dr::DRAG_TUNE,
            zero_60_factor: dr::ZERO_60_FACTOR,
            zero_100_factor: dr::ZERO_100_FACTOR,
            surface_weight: dr::SURFACE_WEIGHT,
            shake: dr::SHAKE,
            drift_level_speed: dr::DRIFT_LEVEL_SPEED,
            drift_level_points: dr::DRIFT_LEVEL_POINTS,
            drift_virtual_scores: dr::DRIFT_VIRTUAL_SCORES,
            ai_lock: dr::AI_LOCK,
            ai_table_rate_speed: dr::AI_TABLE_RATE_SPEED,
            asin: *super::generated::asin_table(),
            collision_levels: au::COLLISION_LEVELS,
            collision_gain: au::COLLISION_GAIN,
            db_to_lin: *super::generated::db_to_lin(),
            lin_to_db: *super::generated::lin_to_db(),
            engine_levels: au::ENGINE_LEVELS,
            shifts: au::SHIFTS,
            turbos: au::TURBOS,
            swtn: au::SWTN,
            // The GIN bank names less "GIN_" and ".gin".
            gins: au::ENGINES.map(|e| e.gin.strip_prefix("GIN_").and_then(|g| g.strip_suffix(".gin")).unwrap_or(e.gin)),
            shift_recs: au::SHIFT_RECS,
            launches: au::LAUNCHES,
            launch_index: au::LAUNCH_INDEX,
            skid_records: au::SKID_RECORDS,
            skid_class: au::SKID_CLASS,
            sound_index: au::SOUND_INDEX,
            engines: au::ENGINES,
            decels: au::DECELS,
            patterns: [
                &au::PATTERN_1,
                &au::PATTERN_2,
                &au::PATTERN_3,
                &au::PATTERN_4,
                &au::PATTERN_5,
                &au::PATTERN_6,
                &au::PATTERN_7,
                &au::PATTERN_8,
                &au::PATTERN_9,
                &au::PATTERN_10,
                &au::PATTERN_11,
                &au::PATTERN_12,
                &au::PATTERN_13,
                &au::PATTERN_14,
                &au::PATTERN_15,
            ],
            score_events: au::SCORE_EVENTS,
            trains: au::TRAINS,
            utk_rc: super::third_party::vgmstream_utk::UTK_RC_TABLE,
            utk_codebooks: super::third_party::vgmstream_utk::UTK_CODEBOOKS,
            utk_commands: super::third_party::vgmstream_utk::UTK_COMMANDS,
            mp3_huff: super::third_party::mp3_huffman_trees(),
            mp3_linbits: super::third_party::mp3_linbits(),
            mp3_window_rows: super::third_party::mp3_window_rows(),
            vp6: super::third_party::vp6(),
            in_game_rig: ga::IN_GAME_RIG,
            default_rig: ga::DEFAULT_RIG,
            slot_shots: ga::SLOT_SHOTS,
            vinyl_shots: ga::VINYL_SHOTS,
            suv_engine_shot: ga::SUV_ENGINE_SHOT,
            world_map_zooms: fe::WORLD_MAP_ZOOMS,
            magazine_lines: fe::MAGAZINE_LINES,
            dvd_lines: fe::DVD_LINES,
            rating_lines: fe::RATING_LINES,
            part_type_names: fe::PART_TYPE_NAMES,
            slot_types: fe::SLOT_TYPES,
            slot_categories: fe::SLOT_CATEGORIES,
            vinyl_categories: fe::VINYL_CATEGORIES,
            opponent_cars: fe::OPPONENT_CARS,
            vinyl_groups: fe::VINYL_GROUPS,
            opponent_names: fe::OPPONENT_NAMES,
            ea_trax: fe::EA_TRAX,
            ea_trax_where: fe::EA_TRAX_WHERE,
            tuning_names: fe::TUNING_NAMES,
            card_scores: fe::CARD_SCORES,
            qr_default_cars: fe::QR_DEFAULT_CARS,
            ranked_rides: fe::RANKED_RIDES,
        }
    }
}

impl Tables {
    /// Each table's name and `Debug` text (what [`FIELD_PINS`] hashes).
    pub fn field_texts(&self) -> Vec<(&'static str, String)> {
        vec![
            ("steer_response", format!("{:?}", self.steer_response)),
            ("steer_response_drift", format!("{:?}", self.steer_response_drift)),
            ("steer_response_wheel", format!("{:?}", self.steer_response_wheel)),
            ("lateral", format!("{:?}", self.lateral)),
            ("drift_tune", format!("{:?}", self.drift_tune)),
            ("street_x_tune", format!("{:?}", self.street_x_tune)),
            ("drag_tune", format!("{:?}", self.drag_tune)),
            ("zero_60_factor", format!("{:?}", self.zero_60_factor)),
            ("zero_100_factor", format!("{:?}", self.zero_100_factor)),
            ("surface_weight", format!("{:?}", self.surface_weight)),
            ("shake", format!("{:?}", self.shake)),
            ("drift_level_speed", format!("{:?}", self.drift_level_speed)),
            ("drift_level_points", format!("{:?}", self.drift_level_points)),
            ("drift_virtual_scores", format!("{:?}", self.drift_virtual_scores)),
            ("ai_lock", format!("{:?}", self.ai_lock)),
            ("ai_table_rate_speed", format!("{:?}", self.ai_table_rate_speed)),
            ("asin", format!("{:?}", self.asin)),
            ("collision_levels", format!("{:?}", self.collision_levels)),
            ("collision_gain", format!("{:?}", self.collision_gain)),
            ("db_to_lin", format!("{:?}", self.db_to_lin)),
            ("lin_to_db", format!("{:?}", self.lin_to_db)),
            ("engine_levels", format!("{:?}", self.engine_levels)),
            ("shifts", format!("{:?}", self.shifts)),
            ("turbos", format!("{:?}", self.turbos)),
            ("swtn", format!("{:?}", self.swtn)),
            ("gins", format!("{:?}", self.gins)),
            ("shift_recs", format!("{:?}", self.shift_recs)),
            ("launches", format!("{:?}", self.launches)),
            ("launch_index", format!("{:?}", self.launch_index)),
            ("skid_records", format!("{:?}", self.skid_records)),
            ("skid_class", format!("{:?}", self.skid_class)),
            ("sound_index", format!("{:?}", self.sound_index)),
            ("engines", format!("{:?}", self.engines)),
            ("decels", format!("{:?}", self.decels)),
            ("patterns", format!("{:?}", self.patterns)),
            ("score_events", format!("{:?}", self.score_events)),
            ("trains", format!("{:?}", self.trains)),
            ("utk_rc", format!("{:?}", self.utk_rc)),
            ("utk_codebooks", format!("{:?}", self.utk_codebooks)),
            ("utk_commands", format!("{:?}", self.utk_commands)),
            ("mp3_huff", format!("{:?}", self.mp3_huff)),
            ("mp3_linbits", format!("{:?}", self.mp3_linbits)),
            ("mp3_window_rows", format!("{:?}", self.mp3_window_rows)),
            ("vp6", format!("{:?}", self.vp6)),
            ("in_game_rig", format!("{:?}", self.in_game_rig)),
            ("default_rig", format!("{:?}", self.default_rig)),
            ("slot_shots", format!("{:?}", self.slot_shots)),
            ("vinyl_shots", format!("{:?}", self.vinyl_shots)),
            ("suv_engine_shot", format!("{:?}", self.suv_engine_shot)),
            ("world_map_zooms", format!("{:?}", self.world_map_zooms)),
            ("magazine_lines", format!("{:?}", self.magazine_lines)),
            ("dvd_lines", format!("{:?}", self.dvd_lines)),
            ("rating_lines", format!("{:?}", self.rating_lines)),
            ("part_type_names", format!("{:?}", self.part_type_names)),
            ("slot_types", format!("{:?}", self.slot_types)),
            ("slot_categories", format!("{:?}", self.slot_categories)),
            ("vinyl_categories", format!("{:?}", self.vinyl_categories)),
            ("opponent_cars", format!("{:?}", self.opponent_cars)),
            ("vinyl_groups", format!("{:?}", self.vinyl_groups)),
            ("opponent_names", format!("{:?}", self.opponent_names)),
            ("ea_trax", format!("{:?}", self.ea_trax)),
            ("ea_trax_where", format!("{:?}", self.ea_trax_where)),
            ("tuning_names", format!("{:?}", self.tuning_names)),
            ("card_scores", format!("{:?}", self.card_scores)),
            ("qr_default_cars", format!("{:?}", self.qr_default_cars)),
            ("ranked_rides", format!("{:?}", self.ranked_rides)),
        ]
    }
}

/// The SHA-256 of each table's `Debug` text as the reference executable
/// gives it (European PC 1.2): a test checks the built-in tables against
/// these in any checkout, and a data test against the executable itself.
pub const FIELD_PINS: &[(&str, &str)] = &[
    ("steer_response", "97c9d4db9902554aea087dddb38927c577d3fcafcf8d75c6adc56c7642d267fd"),
    ("steer_response_drift", "7128c7f08971f38da4ebc05425c1cf5d996ca28208a674cc8e877eba817cf775"),
    ("steer_response_wheel", "fd52a1e66b72b9fe82c6e8e779db6890d9538c4622f3be9da852a97123c7e8a2"),
    ("lateral", "7991663ac51f4456b95eb62d1773f4cf61f80f13edae1a206ee85f8f5d52ae49"),
    ("drift_tune", "401f0f31a5cdbeecbd76212ef69956fbb6f6689f99c522b69a480ea9c425c6df"),
    ("street_x_tune", "7984df633f3cb5c20c8b45a2c0c55d1b0d88a85b5dc4d62458c223d8a1d45fad"),
    ("drag_tune", "9cd9f05d36b372dc01e3e831e10dd152604a057b1614bce08ae88f9373683075"),
    ("zero_60_factor", "d833f6a592bfb8c0bb150dc30fe380a7436287988f525a50726f105b3c0ab144"),
    ("zero_100_factor", "73f01dfd85960f77cdd0b9245a11b3d66c037f04a6a1d9e8e5679c77e1127959"),
    ("surface_weight", "91ed557543e740c60c16f2aefac0cd640f5ac5c6d06c0033b7fb7e3e5ea07179"),
    ("shake", "c345a386246dcceff899bbfa60539e53bf198d736026d7951734d4c83d64d10c"),
    ("drift_level_speed", "837d136655ec5d2ea0a008cbf52097578f54ec79e51906693a88b3ae9d16ae54"),
    ("drift_level_points", "74fa6692ddf3e140ac5a6a56616678f69e1892067cf8f4a74bb6d0de4d221c58"),
    ("drift_virtual_scores", "f89278c88a3d6f099a1bfabf10a24acdb0be9178a2e81299e80ff2451a9e7b0c"),
    ("ai_lock", "9028802df21e737e41ea7f42ff600b5a3653c3d1c355611071959e01e54dcbbb"),
    ("ai_table_rate_speed", "421219fef71828c9ffc462d2db2ce1326922bddca5ea94763142cf17ca86c3ee"),
    ("asin", "cb074f073140e07a3d11d762c5d9944ff03afb910a9f029b9e42b8171acf63aa"),
    ("collision_levels", "c9e4241381d60e8775b46304967f7fb65499b7789eccea91604df6a8e11ac956"),
    ("collision_gain", "826ab4d85747e1608702d68ee452c8a7da57424669473805d15fc5cd826851fb"),
    ("db_to_lin", "1687d241da9111cfdf54a74fbaa0c788a6a9e517d0a730300d318b7c9129c098"),
    ("lin_to_db", "6372738eb668a6ef985f2439c90d21df15d5243753c78e6f2d4aaeea2c3899c8"),
    ("engine_levels", "a016675968f97f5b493403b7df93c5a732c2a6899c3b5e482131f5b123509514"),
    ("shifts", "55246f364c7680edfc6379b6e2642d77b186948797d8e5254a4df01cbb4527dd"),
    ("turbos", "438bcae3eed895f3cdee23ffc9ffdf9a4fecbc6288670f7c1a18d02190ac0dac"),
    ("swtn", "427023369ba87d9c41308c55ff674e5a124e754ffaed91bac02bb1d1f1b41948"),
    ("gins", "3aa880fd813232949b0750d6b8547eed90f4b63ca79f720517289e96579bfa82"),
    ("shift_recs", "318553225da4f9ad099a74ba5a2cccba80275eff86646631d5c4613db71ec6be"),
    ("launches", "bcc7bd5ff3483bc3f6e2b82995f1377d5d04bd0d034970e9fbec85cdb57f32b9"),
    ("launch_index", "0ff776193525cd76e513c3054ba8e56347351a2489e8525ca54a492056849b6a"),
    ("skid_records", "097fc8100dbb72ad110979a639c4840a5ac10ca18f416011070cd97259226870"),
    ("skid_class", "270733b70c9be7b4521be2af0b18cf8a627d3c70d6cca483a72c36f78133e552"),
    ("sound_index", "faf9398e2f8fde6eafc921bc57de38d060e140ee9758b5aa473f62ae4ba86d43"),
    ("engines", "75a7ab2fe917ae6712b7bedd65ba00430677ae3ee708284f865fc49443dd5f7c"),
    ("decels", "64bed80613638fbfeaeeb7a0d0de2589ad7538221dc215cd868cc3f369fb0040"),
    ("patterns", "1a6ef5491f327ad50372d7925109d0e08fd758db62518590f005dea8006d9711"),
    ("score_events", "77092a0893be1d5f5dc7f0e77d2de9b904de579035db35ed64526769cadbca78"),
    ("trains", "0c4fc3abe255e5cf5272cf0193955eb07fd3457c8d17a3963a0ece5dbd79277a"),
    ("utk_rc", "4e5dcb5047da02d7dd593b829c045b56e9ce10ee5450c973379270e83ec9b7d0"),
    ("utk_codebooks", "1838ca39000c86ad35e785d4dc62d69649cadcb99ee100c46246d4c384772662"),
    ("utk_commands", "f1ab7742a1c507ec463a173d43eb25acd2a4165d7b44ae8a464df94222ca5e6e"),
    ("mp3_huff", "b39a7c5a253b1b5caefcc601fa723646733a14b970a0684a5dc2a2c32829412b"),
    ("mp3_linbits", "b7e81c641166b01f75da998ec47a11c5886979ecee7de7e49047f8c4b9f8520c"),
    ("mp3_window_rows", "208f70e46530512ed6cf409c2b56d3eb5cfbb6ad405611785218206a98dcb0cf"),
    ("vp6", "bba46113a64edcd76a2868a304e61407b4656b088cd5759dbc49b1abd33df836"),
    ("in_game_rig", "374b868341104910743c56cb18b8193b94256382a5aa229179eadea0a8c53e2d"),
    ("default_rig", "f6dae390943fdb39da0b1684c55f3f02d589bc36aadcfebe0f5646d882a19f80"),
    ("slot_shots", "638b8afafb5796a98d98faccd90da2243f0be7829284acd6f29d95ce636ae10e"),
    ("vinyl_shots", "6a2aa13b820f262a335cd6aabeef269ae54aa6b6e726e5a1ae15891f8137e260"),
    ("suv_engine_shot", "d1a4befd0479cd2f1d5d1ac2d40f05f36be6f742e4bc9869d270404218fc78c3"),
    ("world_map_zooms", "9daf14fc727953fe4649ccb7a43db7882e7c90ca61367ea873eacdd778a1c624"),
    ("magazine_lines", "79f55c8d8cd6ef1b232eb8f5bdb41af91ac11bf288206d269b5cab58ddefad92"),
    ("dvd_lines", "edde62f9ff0733849a36cf326598ceb4e3d5a7920aa107bc2e21d7da98afa72b"),
    ("rating_lines", "66daefe78d65c72894300bad8e015b67824036ea37374f7a2ed95cb89ba9b49d"),
    ("part_type_names", "003a85d02507726f664ca10a0818d2c59c4eba71c20c0c5930851f3cbf96b777"),
    ("slot_types", "5e4e90db4101788ab7e3260131d358060c0d6fb4264dafb0d999ccfd6c5c5184"),
    ("slot_categories", "ce74331b9579407b1e31d508146c1737ea8c91c94254b54290fd140c0fea81e8"),
    ("vinyl_categories", "36cf1219ce39477b12c9eb45ef1d13cc3d78d58bb18dda6aa2680ad8980a2063"),
    ("opponent_cars", "8755e296db5a66d29e85c43c672ac5971225977728f923ff87e13ed3bd424722"),
    ("vinyl_groups", "586c490810b912985b5af2656a4b439e48c92eebbcead5c29e930fc795479376"),
    ("opponent_names", "cadbe9328922ad1e910d1d97d7b1c8df9124cd583aac132ed2accc49c50c09e6"),
    ("ea_trax", "f50c95034a0d803977a372045c30a2a4f5d2681bd20b4372bf2406d9583109c7"),
    ("ea_trax_where", "b329477db993a2f8881121438cc6a3d485236c60fe2ffca8166299f4e9b90d09"),
    ("tuning_names", "b2207292939ee822cf26145efa5d3cf89e0aaf3f4595360f92895de625a33b86"),
    ("card_scores", "743fdc46be9929eeccb70a9397d09c9fc8f03eaf6bbc8ab6f4f92ba0e5d1423e"),
    ("qr_default_cars", "4bf4e88fcf270fedb7c9ab1126d20bf33ab7ab74816395f48391320127a49af2"),
    ("ranked_rides", "8040550a1a0f45e23596219fe4f1da5efbbf56492cdda571ac0e559fb5b90ea6"),
];

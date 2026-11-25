//! The engine's tables equal the original's: every table's `Debug` text
//! hashes to the value the reference executable gives (no game data
//! needed), and with the reference executable at hand (the maintainer's
//! private set, `testkit::reference_exe`) every table equals the one read
//! from it, entry for entry.

use super::*;

/// (table, SHA-256 of its `Debug` text as the reference executable gives
/// it), finer than [`FIELD_PINS`]: the VP6 tables and the patterns one by
/// one, the arcsine's first 192 entries, the generated sine and arctangent.
const PINS: &[(&str, &str)] = &[
    ("slot_shots", "638b8afafb5796a98d98faccd90da2243f0be7829284acd6f29d95ce636ae10e"),
    ("vinyl_shots", "6a2aa13b820f262a335cd6aabeef269ae54aa6b6e726e5a1ae15891f8137e260"),
    ("suv_engine_shot", "d1a4befd0479cd2f1d5d1ac2d40f05f36be6f742e4bc9869d270404218fc78c3"),
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
    ("asin192", "436bd81835b05d03b1def6ae22b150c21811f1eeb24be3e08a05bebaba6ffd66"),
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
    ("score_events", "77092a0893be1d5f5dc7f0e77d2de9b904de579035db35ed64526769cadbca78"),
    ("trains", "0c4fc3abe255e5cf5272cf0193955eb07fd3457c8d17a3963a0ece5dbd79277a"),
    ("utk_rc", "4e5dcb5047da02d7dd593b829c045b56e9ce10ee5450c973379270e83ec9b7d0"),
    ("utk_codebooks", "1838ca39000c86ad35e785d4dc62d69649cadcb99ee100c46246d4c384772662"),
    ("utk_commands", "f1ab7742a1c507ec463a173d43eb25acd2a4165d7b44ae8a464df94222ca5e6e"),
    ("mp3_huff", "b39a7c5a253b1b5caefcc601fa723646733a14b970a0684a5dc2a2c32829412b"),
    ("mp3_linbits", "b7e81c641166b01f75da998ec47a11c5886979ecee7de7e49047f8c4b9f8520c"),
    ("mp3_window_rows", "208f70e46530512ed6cf409c2b56d3eb5cfbb6ad405611785218206a98dcb0cf"),
    ("in_game_rig", "374b868341104910743c56cb18b8193b94256382a5aa229179eadea0a8c53e2d"),
    ("default_rig", "f6dae390943fdb39da0b1684c55f3f02d589bc36aadcfebe0f5646d882a19f80"),
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
    ("tuning_names", "b2207292939ee822cf26145efa5d3cf89e0aaf3f4595360f92895de625a33b86"),
    ("card_scores", "743fdc46be9929eeccb70a9397d09c9fc8f03eaf6bbc8ab6f4f92ba0e5d1423e"),
    ("vp6.ac_dequant", "da61872bb10b249557afb85d47eda80cb79ac4d6ed5499a0cfbb0c6acd195ac8"),
    ("vp6.dc_dequant", "40da2809bce40eeda40cd79c72bf01ff3e4c8034afe8cb260474b20aaaf316ef"),
    ("vp6.zigzag", "9f4a6f4fe84e188e7f440219ee7f5789d99db19c8585c38f9ba9a4c6010a2426"),
    ("vp6.coeff_group", "2d58ad33592848200c88fb9708a1ae220a0229cb2b64f775a1f15750fd02ad73"),
    ("vp6.default_reorder", "e684e1989ea044f40ae53d9450844f2e005ebf8535f920b228f57a1b91a69e6d"),
    ("vp6.ref_of_type", "ebf84ad19159fd731005cd8b853e7e8732f4d9b0c74c7055af1e5331a93185cf"),
    ("vp6.dccv_update", "729eed227db848bb79d2f27719ba460800f3f9711d5e39923f9ddb79b3d008df"),
    ("vp6.reorder_update", "5076b44d1ff1c5d1be8d703673d2af30323a9ac188ba524f77d185c139048896"),
    ("vp6.runv_update", "c83ea821a8c5478699aab3b9177855e933d9fe495effb72b10a17a24af99374a"),
    ("vp6.runv_default", "1dacecaf4fedfa7f2412511568418f217c30282f326854d1da8d08b07bc063b3"),
    ("vp6.ract_update", "1ae18d9081c959e2828cb8ae0eca7d59339daafafcdc229f284e4d3a12aa57fb"),
    ("vp6.dccv_lc", "ce2afa1c7c21f40b1bbf294b75269610258314ab1bd2085312f6b4d24df7b442"),
    ("vp6.vector_update", "3788d8667789b1a97e814ff95d2ad19f160184ad9f2ce25163cfff5ce8227ec6"),
    ("vp6.mb_stats_presets", "547173329b3b99d15cb24f463b9b988f60718ffce366e2cf01b4a248b8c86f05"),
    ("vp6.mb_stats_default", "abc0abca8a0e615b28beabce8fac97539c7776c51e2ba4b77f57c5a2e15a7d6a"),
    ("vp6.cat_table", "8cdfb40947db95adcebe39f2aedc4b464398bae77a3b267e09c7250d2dd8bb1b"),
    ("vp6.candidates", "71d4c70afab27e5187c5a362f495a9fb9552a2df12e500d9158918f9b012e729"),
    ("vp6.bilinear", "9122d1b91583e042adf229ef2592380b2edd779ac6eecc3d6c6e686b6be851ce"),
    ("vp6.token_base", "39e187ed25e518cbac48191eb9666790dce2530d95d6acd6c33f0c4b795a1688"),
    ("vp6.coeff_group_huffman", "2f106581031835c4624a02f292463078f97448ca43b54d863e82f4a5058282be"),
    ("patterns[0]", "066a9444c117cd9437fe7824223e53d19d54a086158c12415fa2fb2b54395795"),
    ("patterns[1]", "4b8fab3b0f52ce370fa263b4b202842a4998606e8cce1a12d949323e2aa36b2d"),
    ("patterns[2]", "d89c025e3923b4346d82fdb6ce2a8673cfced6218c5c6ff68d347ea28f841103"),
    ("patterns[3]", "5c982b7ff95f2b36fed0c3f0d1190228ee28cad0a62f84bd0bd8cae6b48212a4"),
    ("patterns[4]", "dd4737ec9ca0de6eac69d127cee99e6ae624ba304493f48d0ce36887a75ffa95"),
    ("patterns[5]", "ce38969312bad0fd0736188a0fbe93bd70a1a772def2877f4b3d7395f0e7f655"),
    ("patterns[6]", "e3532f780641e2fd497955319b808702897b043654b2654b771ffb178b5d8901"),
    ("patterns[7]", "03c12b265ce2e19916513ea4443625c3344099d646da95e4d87d226de305c2c4"),
    ("patterns[8]", "4bc7c84e040501d34481194259198cafb8b69c1dea4180ad4bdcd1b5dd2869d4"),
    ("patterns[9]", "2f57f3694560e252d400a0f81d8347a0b3f5e6a9ee3f85f5dcc1e5cd3bdb260c"),
    ("patterns[10]", "037af86422d459fed058a2be859fa0502923f48fbe071c5c9199f60d3d5e0f50"),
    ("patterns[11]", "f46cfb7805559b43f4de7dbc7390fdd62d024008ce9089a4af97438957872c78"),
    ("patterns[12]", "e8a776630617d307a2428050c570206da0994ae889cec14a3c6f122109765dae"),
    ("patterns[13]", "9c5bca9e430cff4bc2085728d1f5d696b1e0ea15f9dab4ca74372a7c8e51a989"),
    ("patterns[14]", "8b4fc6e194794f35fd85bf155e4537f7d2da4816767f9ec2cdbfd1568add495d"),
    ("atan_generated", "f933a3e18fe3310489d262d985e07a5de4633d6cc69d1fe5e9bdc2b997c7506d"),
    ("sine_generated", "20c311d00120e292b9d8063674b335141c4a58b91386396d1cf68e5bfc2c9fb6"),
];

fn debug_text(t: &Tables, name: &str) -> String {
    match name {
        "slot_shots" => format!("{:?}", t.slot_shots),
        "vinyl_shots" => format!("{:?}", t.vinyl_shots),
        "suv_engine_shot" => format!("{:?}", t.suv_engine_shot),
        "steer_response" => format!("{:?}", t.steer_response),
        "steer_response_drift" => format!("{:?}", t.steer_response_drift),
        "steer_response_wheel" => format!("{:?}", t.steer_response_wheel),
        "lateral" => format!("{:?}", t.lateral),
        "drift_tune" => format!("{:?}", t.drift_tune),
        "street_x_tune" => format!("{:?}", t.street_x_tune),
        "drag_tune" => format!("{:?}", t.drag_tune),
        "zero_60_factor" => format!("{:?}", t.zero_60_factor),
        "zero_100_factor" => format!("{:?}", t.zero_100_factor),
        "surface_weight" => format!("{:?}", t.surface_weight),
        "shake" => format!("{:?}", t.shake),
        "drift_level_speed" => format!("{:?}", t.drift_level_speed),
        "drift_level_points" => format!("{:?}", t.drift_level_points),
        "drift_virtual_scores" => format!("{:?}", t.drift_virtual_scores),
        "ai_lock" => format!("{:?}", t.ai_lock),
        "ai_table_rate_speed" => format!("{:?}", t.ai_table_rate_speed),
        "asin192" => format!("{:?}", &t.asin[..192]),
        "asin" => format!("{:?}", t.asin),
        "collision_levels" => format!("{:?}", t.collision_levels),
        "collision_gain" => format!("{:?}", t.collision_gain),
        "db_to_lin" => format!("{:?}", t.db_to_lin),
        "lin_to_db" => format!("{:?}", t.lin_to_db),
        "engine_levels" => format!("{:?}", t.engine_levels),
        "shifts" => format!("{:?}", t.shifts),
        "turbos" => format!("{:?}", t.turbos),
        "swtn" => format!("{:?}", t.swtn),
        "gins" => format!("{:?}", t.gins),
        "shift_recs" => format!("{:?}", t.shift_recs),
        "launches" => format!("{:?}", t.launches),
        "launch_index" => format!("{:?}", t.launch_index),
        "skid_records" => format!("{:?}", t.skid_records),
        "skid_class" => format!("{:?}", t.skid_class),
        "sound_index" => format!("{:?}", t.sound_index),
        "engines" => format!("{:?}", t.engines),
        "decels" => format!("{:?}", t.decels),
        "score_events" => format!("{:?}", t.score_events),
        "trains" => format!("{:?}", t.trains),
        "utk_rc" => format!("{:?}", t.utk_rc),
        "utk_codebooks" => format!("{:?}", t.utk_codebooks),
        "utk_commands" => format!("{:?}", t.utk_commands),
        "mp3_huff" => format!("{:?}", t.mp3_huff),
        "mp3_linbits" => format!("{:?}", t.mp3_linbits),
        "mp3_window_rows" => format!("{:?}", t.mp3_window_rows),
        "in_game_rig" => format!("{:?}", t.in_game_rig),
        "default_rig" => format!("{:?}", t.default_rig),
        "world_map_zooms" => format!("{:?}", t.world_map_zooms),
        "magazine_lines" => format!("{:?}", t.magazine_lines),
        "dvd_lines" => format!("{:?}", t.dvd_lines),
        "rating_lines" => format!("{:?}", t.rating_lines),
        "part_type_names" => format!("{:?}", t.part_type_names),
        "slot_types" => format!("{:?}", t.slot_types),
        "slot_categories" => format!("{:?}", t.slot_categories),
        "vinyl_categories" => format!("{:?}", t.vinyl_categories),
        "opponent_cars" => format!("{:?}", t.opponent_cars),
        "vinyl_groups" => format!("{:?}", t.vinyl_groups),
        "opponent_names" => format!("{:?}", t.opponent_names),
        "ea_trax" => format!("{:?}", t.ea_trax),
        "tuning_names" => format!("{:?}", t.tuning_names),
        "card_scores" => format!("{:?}", t.card_scores),
        "vp6.ac_dequant" => format!("{:?}", t.vp6.ac_dequant),
        "vp6.dc_dequant" => format!("{:?}", t.vp6.dc_dequant),
        "vp6.zigzag" => format!("{:?}", t.vp6.zigzag),
        "vp6.coeff_group" => format!("{:?}", t.vp6.coeff_group),
        "vp6.default_reorder" => format!("{:?}", t.vp6.default_reorder),
        "vp6.ref_of_type" => format!("{:?}", t.vp6.ref_of_type),
        "vp6.dccv_update" => format!("{:?}", t.vp6.dccv_update),
        "vp6.reorder_update" => format!("{:?}", t.vp6.reorder_update),
        "vp6.runv_update" => format!("{:?}", t.vp6.runv_update),
        "vp6.runv_default" => format!("{:?}", t.vp6.runv_default),
        "vp6.ract_update" => format!("{:?}", t.vp6.ract_update),
        "vp6.dccv_lc" => format!("{:?}", t.vp6.dccv_lc),
        "vp6.vector_update" => format!("{:?}", t.vp6.vector_update),
        "vp6.mb_stats_presets" => format!("{:?}", t.vp6.mb_stats_presets),
        "vp6.mb_stats_default" => format!("{:?}", t.vp6.mb_stats_default),
        "vp6.cat_table" => format!("{:?}", t.vp6.cat_table),
        "vp6.candidates" => format!("{:?}", t.vp6.candidates),
        "vp6.bilinear" => format!("{:?}", t.vp6.bilinear),
        "vp6.token_base" => format!("{:?}", t.vp6.token_base),
        "vp6.coeff_group_huffman" => format!("{:?}", t.vp6.coeff_group_huffman),
        "patterns[0]" => format!("{:?}", t.patterns[0]),
        "patterns[1]" => format!("{:?}", t.patterns[1]),
        "patterns[2]" => format!("{:?}", t.patterns[2]),
        "patterns[3]" => format!("{:?}", t.patterns[3]),
        "patterns[4]" => format!("{:?}", t.patterns[4]),
        "patterns[5]" => format!("{:?}", t.patterns[5]),
        "patterns[6]" => format!("{:?}", t.patterns[6]),
        "patterns[7]" => format!("{:?}", t.patterns[7]),
        "patterns[8]" => format!("{:?}", t.patterns[8]),
        "patterns[9]" => format!("{:?}", t.patterns[9]),
        "patterns[10]" => format!("{:?}", t.patterns[10]),
        "patterns[11]" => format!("{:?}", t.patterns[11]),
        "patterns[12]" => format!("{:?}", t.patterns[12]),
        "patterns[13]" => format!("{:?}", t.patterns[13]),
        "patterns[14]" => format!("{:?}", t.patterns[14]),
        "atan_generated" => format!("{:?}", generated::atan_table()),
        "sine_generated" => format!("{:?}", generated::quarter_sine()),
        _ => panic!("no table {name}"),
    }
}

#[test]
fn every_table_equals_its_pin() {
    let t = tables();
    let differ: Vec<&str> = PINS
        .iter()
        .filter(|(name, sha)| crate::fingerprint::sha256_hex(debug_text(t, name).as_bytes()) != *sha)
        .map(|p| p.0)
        .collect();
    assert!(differ.is_empty(), "tables unlike the original's: {differ:?}");
    let fields = t.field_texts();
    assert_eq!(FIELD_PINS.len(), fields.len());
    let differ: Vec<&str> = fields
        .iter()
        .filter(|(name, text)| {
            FIELD_PINS.iter().find(|p| p.0 == *name).is_none_or(|p| crate::fingerprint::sha256_hex(text.as_bytes()) != p.1)
        })
        .map(|(name, _)| *name)
        .collect();
    assert!(differ.is_empty(), "tables unlike the original's: {differ:?}");
}

/// The tables computed from their formulas ([`generated`]).
const FORMULA: [&str; 3] = ["asin", "db_to_lin", "lin_to_db"];
/// The decoders' tables from vgmstream and FFmpeg ([`third_party`]).
const DECODERS: [&str; 7] = ["utk_rc", "utk_codebooks", "utk_commands", "mp3_huff", "mp3_linbits", "mp3_window_rows", "vp6"];

/// The reference executable's tables ([`reader::read_tables`]), read once.
fn reference() -> &'static Tables {
    static T: std::sync::OnceLock<&'static Tables> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let exe: &'static reader::Exe = Box::leak(Box::new(reader::Exe::from_bytes(crate::testkit::reference_exe()).unwrap()));
        Box::leak(Box::new(reader::read_tables(exe).unwrap_or_else(|e| panic!("{e}"))))
    })
}

/// The built-in tables named by `pick` that differ from the reference
/// executable's.
fn differing(pick: &dyn Fn(&str) -> bool) -> Vec<&'static str> {
    let ours = tables().field_texts();
    let theirs = reference().field_texts();
    assert_eq!(ours.len(), theirs.len());
    ours.iter().zip(&theirs).filter(|((n, a), (_, b))| pick(n) && a != b).map(|((n, _), _)| *n).collect()
}

/// The formula tables equal the executable's entry for entry (the sine
/// and the arctangent too: 0x8181D8, 0x7EACB0).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn formula_tables_equal_the_executable() {
    assert_eq!(differing(&|n| FORMULA.contains(&n)), Vec::<&str>::new());
    let exe = reader::Exe::from_bytes(crate::testkit::reference_exe()).unwrap();
    assert_eq!(&exe.array::<u8, 256>(generated::ATAN_TABLE_VA).unwrap(), generated::atan_table());
    assert_eq!(&exe.array::<u16, 257>(generated::QUARTER_SINE_VA).unwrap(), generated::quarter_sine());
    let asin: [(u16, u32); 208] = exe.records(0x7E_A628, 8, |r| Ok((r.u16(0), r.u32(4)))).unwrap();
    assert_eq!(asin, generated::asin_table().map(|(a, s)| (a, s.to_bits())));
}

/// The decoders' tables from vgmstream and FFmpeg equal the executable's.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn decoder_tables_equal_the_executable() {
    assert_eq!(differing(&|n| DECODERS.contains(&n)), Vec::<&str>::new());
    // The window bit for bit (`Debug` shows -0.0 and 0.0 apart, but not
    // every float's bits).
    let ours = tables().mp3_window_rows.as_flattened().iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    let theirs = reference().mp3_window_rows.as_flattened().iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(ours, theirs);
}

/// Every tuning constant equals the executable's ([`constants`]).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn tuning_constants_equal_the_executable() {
    assert_eq!(differing(&|n| !FORMULA.contains(&n) && !DECODERS.contains(&n)), Vec::<&str>::new());
}

/// Native rule (docs/original/exe-tables.md, quarter sine): the event VM's sine 0x7254CF reads the
/// stored quarter sine 0x8181D8 as is, and the stored table is the exact
/// formula: entries 11, 13, 15, 19, 20, 21 = 4420, 5222, 6023, 7623, 8022,
/// 8421 (the clone had each one less).
#[test]
fn quarter_sine_is_the_exact_formula() {
    let t = generated::quarter_sine();
    let six: Vec<u16> = [11, 13, 15, 19, 20, 21].iter().map(|&i| t[i]).collect();
    assert_eq!(six, [4420, 5222, 6023, 7623, 8022, 8421]);
    assert_eq!((t[0], t[256]), (0, 65535));
}

/// The section reader: addresses map through the section table, data
/// past a section's raw size reads as zeros, ranges outside fail.
#[test]
fn sections_map_addresses() {
    // A minimal PE: one section at RVA 0x1000, 0x10 raw bytes at file
    // offset 0x200, 0x100 bytes in memory.
    let mut b = vec![0u8; 0x210];
    b[..2].copy_from_slice(b"MZ");
    b[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    b[0x40..0x44].copy_from_slice(b"PE\0\0");
    b[0x46..0x48].copy_from_slice(&1u16.to_le_bytes());
    b[0x54..0x56].copy_from_slice(&0xE0u16.to_le_bytes());
    b[0x58 + 28..0x58 + 32].copy_from_slice(&0x40_0000u32.to_le_bytes());
    let s = 0x58 + 0xE0;
    b[s + 8..s + 12].copy_from_slice(&0x100u32.to_le_bytes());
    b[s + 12..s + 16].copy_from_slice(&0x1000u32.to_le_bytes());
    b[s + 16..s + 20].copy_from_slice(&0x10u32.to_le_bytes());
    b[s + 20..s + 24].copy_from_slice(&0x200u32.to_le_bytes());
    for (k, v) in b[0x200..0x210].iter_mut().enumerate() {
        *v = k as u8 + 1;
    }
    let exe = reader::Exe::from_bytes(b).unwrap();
    assert_eq!(&*exe.bytes(0x40_1000, 4).unwrap(), &[1, 2, 3, 4]);
    assert_eq!(&*exe.bytes(0x40_100E, 4).unwrap(), &[15, 16, 0, 0]);
    assert_eq!(exe.read::<u32>(0x40_1080).unwrap(), 0);
    assert!(matches!(exe.bytes(0x40_10FE, 4), Err(reader::Error::Range { .. })));
    assert!(matches!(exe.bytes(0x40_0000, 4), Err(reader::Error::Range { .. })));
}

/// What building the tables costs (diagnostic: `cargo test --release
/// exe_tables_cost -- --ignored --nocapture`), best of 20.
#[test]
#[ignore = "diagnostic timing"]
fn exe_tables_cost() {
    let mut best = f64::MAX;
    for _ in 0..20 {
        let t = std::time::Instant::now();
        let tables = Tables::builtin();
        best = best.min(t.elapsed().as_secs_f64() * 1e3);
        drop(tables);
    }
    eprintln!("exe_tables: all tables {best:.3} ms");
}

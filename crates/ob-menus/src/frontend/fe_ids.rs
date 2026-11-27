//! Named hashes of the front end's objects, scripts, text labels and textures
//! (an audit finding): every `0x…` literal of `screens`, `hud`,
//! `options` and `widgets` that stands for a hash is named here once.
//!
//! The name comes from what the code or its comments said at the first use
//! (listed in each constant's doc): the module-level constant it was, the
//! name whose hash it is (`fe_ids::tests::constants_match_their_names`
//! recomputes those), the row of a table, or the trailing comment. A name
//! that only repeats the table and column (`PAINT_SLOTS_3`) or `OBJ_…`
//! states where the value sits, not what it is: the game's binary has the
//! real name. Values are exactly the literals they replaced.
#![allow(dead_code)]

// ---- hud/drag.rs
/// text at hud/drag.rs:73
pub(crate) const TXT_GOOD_SHIFT: u32 = 0x27D2_DD45;
/// text at hud/drag.rs:50
pub(crate) const TXT_OVER_REV: u32 = 0x27D7_5CD8;

// ---- hud/elements.rs
/// `BEST_LAP_TIME` in hud/elements.rs
pub(crate) const BEST_LAP_TIME: u32 = 0xA824_7197;
/// `COUNTDOWN` in hud/elements.rs
pub(crate) const COUNTDOWN: u32 = 0x2DA2_20AA;
/// `LAP_TIME` in hud/elements.rs
pub(crate) const ELEMENTS_LAP_TIME: u32 = 0xFC1B_498A;
/// `ENGAGE_COUNT` in hud/elements.rs
pub(crate) const ENGAGE_COUNT: u32 = 0x668C_9CD6;
/// `ENGAGE_DPAD` in hud/elements.rs
pub(crate) const ENGAGE_DPAD: u32 = 0x5091_0822;
/// seq at hud/elements.rs:276
pub(crate) const ENGAGE_HIGHLIGHTS_0: u32 = 0xA206_A0B4;
/// seq at hud/elements.rs:276
pub(crate) const ENGAGE_HIGHLIGHTS_1: u32 = 0x0A72_9B1B;
/// `ENGAGE_ICON` in hud/elements.rs
pub(crate) const ENGAGE_ICON: u32 = 0x3B1B_624A;
/// `ENGAGE_MAIL` in hud/elements.rs
pub(crate) const ENGAGE_MAIL: u32 = 0x4B9B_8F5A;
/// seq at hud/elements.rs:275
pub(crate) const HASHED_0: u32 = 0xD80C_97E1;
/// seq at hud/elements.rs:276
pub(crate) const HASHED_7: u32 = 0x99F6_65F4;
/// seq at hud/elements.rs:276
pub(crate) const HASHED_8: u32 = 0xB996_A84C;
/// `KMH` in hud/elements.rs
pub(crate) const KMH: u32 = 0x8569_A25F;
/// `MPH` in hud/elements.rs
pub(crate) const MPH: u32 = 0x8569_AB44;
/// seq at hud/elements.rs:287
pub(crate) const UNTRACED_0: u32 = 0xD292_009B;
/// seq at hud/elements.rs:287
pub(crate) const UNTRACED_1: u32 = 0xF7CF_C1FC;
/// `YOUR_TIME` in hud/elements.rs
pub(crate) const YOUR_TIME: u32 = 0xC287_8EBC;

// ---- hud/features.rs
/// role not stated where it is used (hud/features.rs:55); needs the binary
pub(crate) const OBJ_20000000: u32 = 0x2000_0000;

// ---- hud/leaderboard.rs
/// `FINISHED` in hud/leaderboard.rs
pub(crate) const FINISHED: u32 = 0xD429_3449;
/// seq at hud/leaderboard.rs:81
pub(crate) const PLACE_LINES_0: u32 = 0x3708_3607;
/// seq at hud/leaderboard.rs:81
pub(crate) const PLACE_LINES_1: u32 = 0xE815_BC6D;
/// seq at hud/leaderboard.rs:81
pub(crate) const PLACE_LINES_2: u32 = 0xC9A6_4EFB;
/// seq at hud/leaderboard.rs:81
pub(crate) const PLACE_LINES_3: u32 = 0x443F_AB5A;
/// seq at hud/leaderboard.rs:81
pub(crate) const PLACE_LINES_4: u32 = 0xAC12_2037;
/// seq at hud/leaderboard.rs:81
pub(crate) const PLACE_LINES_5: u32 = 0x4342_D430;
/// seq at hud/leaderboard.rs:87
pub(crate) const TABLE_0: u32 = 0x7447_C0E3;
/// seq at hud/leaderboard.rs:88
pub(crate) const TABLE_1: u32 = 0xBC4C_E12F;
/// seq at hud/leaderboard.rs:89
pub(crate) const TABLE_2: u32 = 0x63EB_6A03;
/// seq at hud/leaderboard.rs:90
pub(crate) const TABLE_3: u32 = 0xBD49_8562;
/// seq at hud/leaderboard.rs:91
pub(crate) const TABLE_4: u32 = 0x4657_3B3F;
/// seq at hud/leaderboard.rs:92
pub(crate) const TABLE_5: u32 = 0xBC4C_AE38;
/// seq at hud/leaderboard.rs:93
pub(crate) const TABLE_6: u32 = 0xBD38_3957;
/// seq at hud/leaderboard.rs:94
pub(crate) const TABLE_7: u32 = 0xE2AB_0284;
/// seq at hud/leaderboard.rs:95
pub(crate) const TABLE_8: u32 = 0x438D_A7C0;
/// seq at hud/leaderboard.rs:96
pub(crate) const TABLE_9: u32 = 0xBCDD_9448;

// ---- hud/minimap.rs
/// `GPS` in hud/minimap.rs
pub(crate) const GPS: u32 = 0x697D_2BCA;

// ---- hud/mod.rs
/// `DRIFT_LABEL` in hud/mod.rs
pub(crate) const DRIFT_LABEL: u32 = 0xA46A_AEA8;
/// role not stated where it is used (hud/mod.rs:884); needs the binary
pub(crate) const OBJ_8CA76279: u32 = 0x8CA7_6279;
/// role not stated where it is used (hud/mod.rs:840); needs the binary
pub(crate) const OBJ_A074449F: u32 = 0xA074_449F;
/// role not stated where it is used (hud/mod.rs:838); needs the binary
pub(crate) const OBJ_EAC720D3: u32 = 0xEAC7_20D3;
/// text at hud/mod.rs:1045
pub(crate) const TXT_HURRY: u32 = 0x862A_0519;
/// text at hud/mod.rs:833
pub(crate) const TXT_YOUR_SCORE: u32 = 0x1364_0DE9;

// ---- hud/prompts.rs
/// role not stated where it is used (hud/prompts.rs:191); needs the binary
pub(crate) const OBJ_6FFF0767: u32 = 0x6FFF_0767;
/// role not stated where it is used (hud/prompts.rs:20); needs the binary
pub(crate) const OBJ_EA570722: u32 = 0xEA57_0722;

// ---- options/category.rs
/// role not stated where it is used (options/category.rs:135); needs the binary
pub(crate) const OBJ_381FE1E9: u32 = 0x381F_E1E9;
/// role not stated where it is used (options/category.rs:136); needs the binary
pub(crate) const OBJ_4DEAB931: u32 = 0x4DEA_B931;

// ---- options/screen.rs
/// role not stated where it is used (options/screen.rs:260); needs the binary
pub(crate) const OBJ_C90DC4AB: u32 = 0xC90D_C4AB;
/// `fe_hash("PLAYER")`
pub(crate) const SCREEN_PLAYER: u32 = 0x7326_E44C;

// ---- screens/brand_select.rs
/// role not stated where it is used (screens/brand_select.rs:91); needs the binary
pub(crate) const OBJ_3FBB7A4E: u32 = 0x3FBB_7A4E;
/// role not stated where it is used (screens/brand_select.rs:157); needs the binary
pub(crate) const OBJ_9943D584: u32 = 0x9943_D584;

// ---- screens/car_lot.rs
/// `BADGE` in screens/car_lot.rs
pub(crate) const BADGE: u32 = 0xB05D_D708;
/// `BOTTOM_GROUP` in screens/car_lot.rs
pub(crate) const BOTTOM_GROUP: u32 = 0xF5A9_D520;
/// `fe_hash("Customize")`
pub(crate) const CAR_LOT_CUSTOMIZE: u32 = 0xA936_C3A2;
/// `fe_hash("FILTERTYPE")`
pub(crate) const FILTERTYPE: u32 = 0xD999_B6E7;
/// seq at screens/car_lot.rs:222
pub(crate) const INSTALL_1: u32 = 0x8978_2734;
/// `LOCKED_TEXT` in screens/car_lot.rs
pub(crate) const LOCKED_TEXT: u32 = 0xBA57_09C8;
/// `LOGO` in screens/car_lot.rs
pub(crate) const LOGO: u32 = 0x3E01_AD1D;
/// role not stated where it is used (screens/car_lot.rs:229); needs the binary
pub(crate) const OBJ_A9205BD8: u32 = 0xA920_5BD8;
/// role not stated where it is used (screens/car_lot.rs:111); needs the binary
pub(crate) const OBJ_E798A2F7: u32 = 0xE798_A2F7;
/// `SLOT_TITLE` in screens/car_lot.rs
pub(crate) const SLOT_TITLE: u32 = 0x6D22_1AA2;
/// text at screens/car_lot.rs:238
pub(crate) const TXT_CUSTOMIZE_NEW_CAR: u32 = 0xC805_4CE0;
/// text at screens/car_lot.rs:272
pub(crate) const TXT_SLOT: u32 = 0x9347_5966;

// ---- screens/controls.rs
/// table at screens/controls.rs:16 ("Back / Reverse")
pub(crate) const CONTROL_BACK_REVERSE: u32 = 0x82EB_CC98;
/// table at screens/controls.rs:21 ("Camera")
pub(crate) const CONTROL_CAMERA: u32 = 0x419C_A63A;
/// table at screens/controls.rs:26 ("Career Statistics")
pub(crate) const CONTROL_CAREER_STATISTICS: u32 = 0x735B_2E6C;
/// table at screens/controls.rs:23 ("Enter Race / Shop")
pub(crate) const CONTROL_ENTER_RACE_SHOP: u32 = 0x4A13_41B3;
/// table at screens/controls.rs:19 ("Handbrake")
pub(crate) const CONTROL_HANDBRAKE: u32 = 0xB3D1_EA35;
/// table at screens/controls.rs:29 ("Hydraulics Bounce")
pub(crate) const CONTROL_HYDRAULICS_BOUNCE: u32 = 0x6473_0AAC;
/// table at screens/controls.rs:28 ("Hydraulics Pressure")
pub(crate) const CONTROL_HYDRAULICS_PRESSURE: u32 = 0xA081_F0CC;
/// table at screens/controls.rs:22 ("Look Back")
pub(crate) const CONTROL_LOOK_BACK: u32 = 0x35B9_F571;
/// table at screens/controls.rs:20 ("Nitrous Boost")
pub(crate) const CONTROL_NITROUS_BOOST: u32 = 0xDF5A_EF26;
/// table at screens/controls.rs:30 ("Pause")
pub(crate) const CONTROL_PAUSE: u32 = 0x6F99_F62A;
/// table at screens/controls.rs:25 ("Read SMS")
pub(crate) const CONTROL_READ_SMS: u32 = 0x58BA_EBBA;
/// table at screens/controls.rs:27 ("Reset Car")
pub(crate) const CONTROL_RESET_CAR: u32 = 0x6FC0_4E6F;
/// table at screens/controls.rs:18 ("Shift Down")
pub(crate) const CONTROL_SHIFT_DOWN: u32 = 0xB7D3_3921;
/// table at screens/controls.rs:17 ("Shift Up")
pub(crate) const CONTROL_SHIFT_UP: u32 = 0x3EB3_18CE;
/// table at screens/controls.rs:13 ("Steering (Left)")
pub(crate) const CONTROL_STEERING_LEFT: u32 = 0x153E_7337;
/// table at screens/controls.rs:14 ("Steering (Right)")
pub(crate) const CONTROL_STEERING_RIGHT: u32 = 0xBD7B_A1AA;
/// table at screens/controls.rs:15 ("Throttle")
pub(crate) const CONTROL_THROTTLE: u32 = 0x4096_07E2;
/// table at screens/controls.rs:24 ("World Map")
pub(crate) const CONTROL_WORLD_MAP: u32 = 0x1E7E_3391;
/// `DATA` in screens/controls.rs
pub(crate) const DATA: u32 = 0x9B48_3B01;
/// `PRIMARY_MASTER` in screens/controls.rs
pub(crate) const DATA_MASTER: u32 = 0xC128_B184;
/// `fe_hash("Instructions_Label")`
pub(crate) const INSTRUCTIONS_LABEL: u32 = 0x4F79_F553;
/// `NAME` in screens/controls.rs
pub(crate) const NAME: u32 = 0xB298_0BA8;
/// seq at screens/controls.rs:99
pub(crate) const NAMES_0: u32 = 0x0599_80D9;
/// seq at screens/controls.rs:100
pub(crate) const NAMES_1: u32 = 0x0599_80DA;
/// seq at screens/controls.rs:110
pub(crate) const NAMES_11: u32 = 0xB298_0BA9;
/// seq at screens/controls.rs:111
pub(crate) const NAMES_12: u32 = 0xB298_0BAA;
/// seq at screens/controls.rs:112
pub(crate) const NAMES_13: u32 = 0xB298_0BAB;
/// seq at screens/controls.rs:113
pub(crate) const NAMES_14: u32 = 0xB298_0BAC;
/// seq at screens/controls.rs:114
pub(crate) const NAMES_15: u32 = 0xB298_0BAD;
/// seq at screens/controls.rs:115
pub(crate) const NAMES_16: u32 = 0xB298_0BAE;
/// seq at screens/controls.rs:116
pub(crate) const NAMES_17: u32 = 0xB298_0BAF;
/// seq at screens/controls.rs:101
pub(crate) const NAMES_2: u32 = 0x0599_80DB;
/// seq at screens/controls.rs:102
pub(crate) const NAMES_3: u32 = 0x0599_80DC;
/// seq at screens/controls.rs:103
pub(crate) const NAMES_4: u32 = 0x0599_80DD;
/// seq at screens/controls.rs:104
pub(crate) const NAMES_5: u32 = 0x0599_80DE;
/// seq at screens/controls.rs:105
pub(crate) const NAMES_6: u32 = 0x0599_80DF;
/// seq at screens/controls.rs:106
pub(crate) const NAMES_7: u32 = 0x0599_80E0;
/// seq at screens/controls.rs:107
pub(crate) const NAMES_8: u32 = 0x0599_80E1;
/// seq at screens/controls.rs:108
pub(crate) const NAMES_9: u32 = 0x0599_80F9;
/// role not stated where it is used (screens/controls.rs:240); needs the binary
pub(crate) const OBJ_4E11CF90: u32 = 0x4E11_CF90;
/// role not stated where it is used (screens/controls.rs:266); needs the binary
pub(crate) const OBJ_9345E4A7: u32 = 0x9345_E4A7;
/// role not stated where it is used (screens/controls.rs:266); needs the binary
pub(crate) const OBJ_D1CDA594: u32 = 0xD1CD_A594;
/// seq at screens/controls.rs:119
pub(crate) const PRIMARY_0: u32 = 0x044F_9B52;
/// seq at screens/controls.rs:120
pub(crate) const PRIMARY_1: u32 = 0x044F_9B53;
/// seq at screens/controls.rs:130
pub(crate) const PRIMARY_11: u32 = 0x9B48_3B02;
/// seq at screens/controls.rs:131
pub(crate) const PRIMARY_12: u32 = 0x9B48_3B03;
/// seq at screens/controls.rs:132
pub(crate) const PRIMARY_13: u32 = 0x9B48_3B04;
/// seq at screens/controls.rs:133
pub(crate) const PRIMARY_14: u32 = 0x9B48_3B05;
/// seq at screens/controls.rs:134
pub(crate) const PRIMARY_15: u32 = 0x9B48_3B06;
/// seq at screens/controls.rs:135
pub(crate) const PRIMARY_16: u32 = 0x9B48_3B07;
/// seq at screens/controls.rs:136
pub(crate) const PRIMARY_17: u32 = 0x9B48_3B08;
/// seq at screens/controls.rs:121
pub(crate) const PRIMARY_2: u32 = 0x044F_9B54;
/// seq at screens/controls.rs:122
pub(crate) const PRIMARY_3: u32 = 0x044F_9B55;
/// seq at screens/controls.rs:123
pub(crate) const PRIMARY_4: u32 = 0x044F_9B56;
/// seq at screens/controls.rs:124
pub(crate) const PRIMARY_5: u32 = 0x044F_9B57;
/// seq at screens/controls.rs:125
pub(crate) const PRIMARY_6: u32 = 0x044F_9B58;
/// seq at screens/controls.rs:126
pub(crate) const PRIMARY_7: u32 = 0x044F_9B59;
/// seq at screens/controls.rs:127
pub(crate) const PRIMARY_8: u32 = 0x044F_9B5A;
/// seq at screens/controls.rs:128
pub(crate) const PRIMARY_9: u32 = 0x044F_9B72;
/// seq at screens/controls.rs:139
pub(crate) const SECONDARY_0: u32 = 0x56A5_0E63;
/// seq at screens/controls.rs:140
pub(crate) const SECONDARY_1: u32 = 0x56A5_0E64;
/// seq at screens/controls.rs:149
pub(crate) const SECONDARY_10: u32 = 0x8E43_0FF2;
/// seq at screens/controls.rs:150
pub(crate) const SECONDARY_11: u32 = 0x8E43_0FF3;
/// seq at screens/controls.rs:151
pub(crate) const SECONDARY_12: u32 = 0x8E43_0FF4;
/// seq at screens/controls.rs:152
pub(crate) const SECONDARY_13: u32 = 0x8E43_0FF5;
/// seq at screens/controls.rs:153
pub(crate) const SECONDARY_14: u32 = 0x8E43_0FF6;
/// seq at screens/controls.rs:154
pub(crate) const SECONDARY_15: u32 = 0x8E43_0FF7;
/// seq at screens/controls.rs:155
pub(crate) const SECONDARY_16: u32 = 0x8E43_0FF8;
/// seq at screens/controls.rs:156
pub(crate) const SECONDARY_17: u32 = 0x8E43_0FF9;
/// seq at screens/controls.rs:141
pub(crate) const SECONDARY_2: u32 = 0x56A5_0E65;
/// seq at screens/controls.rs:142
pub(crate) const SECONDARY_3: u32 = 0x56A5_0E66;
/// seq at screens/controls.rs:143
pub(crate) const SECONDARY_4: u32 = 0x56A5_0E67;
/// seq at screens/controls.rs:144
pub(crate) const SECONDARY_5: u32 = 0x56A5_0E68;
/// seq at screens/controls.rs:145
pub(crate) const SECONDARY_6: u32 = 0x56A5_0E69;
/// seq at screens/controls.rs:146
pub(crate) const SECONDARY_7: u32 = 0x56A5_0E6A;
/// seq at screens/controls.rs:147
pub(crate) const SECONDARY_8: u32 = 0x56A5_0E6B;
/// seq at screens/controls.rs:148
pub(crate) const SECONDARY_9: u32 = 0x56A5_0E83;
/// `SECONDARY_MASTER` in screens/controls.rs
pub(crate) const SECONDARY_MASTER: u32 = 0xE63E_E236;
/// `NAME_MASTER` in screens/controls.rs
pub(crate) const TITLE_MASTER: u32 = 0xA753_C46C;
/// text at screens/controls.rs:253
pub(crate) const TXT_CONFIGURE_YOUR_SETTINGS: u32 = 0x8BAB_66EF;
/// text at screens/controls.rs:239
pub(crate) const TXT_CONTROLS: u32 = 0x04DC_6DB5;
/// `TITLE` in screens/controls.rs
pub(crate) const UI_MAIN_TITLE: u32 = 0x42AD_B44C;

// ---- screens/credits.rs
/// `AREA` in screens/credits.rs
pub(crate) const AREA: u32 = 0xE609_3486;
/// `PLAYER_NAME` in screens/credits.rs
pub(crate) const PLAYER_NAME: u32 = 0xEB40_6FEC;
/// `ROLL` in screens/credits.rs
pub(crate) const ROLL: u32 = 0x587E_5F53;
/// `CASH` in screens/credits.rs
pub(crate) const UI_MAIN_CASH: u32 = 0xEB3A_688A;

// ---- screens/dialog.rs
/// `BUTTON` in screens/dialog.rs
pub(crate) const BUTTON: u32 = 0xB8A7_C6CE;
/// seq at screens/dialog.rs:139
pub(crate) const BUTTON_TEXT_0: u32 = 0xB8A7_C6CC;
/// seq at screens/dialog.rs:140
pub(crate) const BUTTON_TEXT_1: u32 = 0xB8A7_C6CD;
/// `MESSAGE` in screens/dialog.rs
pub(crate) const DIALOG_MESSAGE: u32 = 0x1E26_40FA;
/// seq at screens/dialog.rs:139
pub(crate) const LEFT_1: u32 = 0xDA47_3D2F;
/// seq at screens/dialog.rs:138
pub(crate) const ONE_1: u32 = 0xDA47_3D31;
/// seq at screens/dialog.rs:140
pub(crate) const RIGHT_1: u32 = 0xDA47_3D30;
/// `SWATCH_PANEL` in screens/dialog.rs
pub(crate) const SWATCH_PANEL: u32 = 0xDB28_5767;

// ---- screens/dyno.rs
/// `GROUP` in screens/dyno.rs
pub(crate) const GROUP: u32 = 0x11BF_9934;

// ---- screens/engage.rs
/// `BUTTON_GROUP` in screens/engage.rs
pub(crate) const BUTTON_GROUP: u32 = 0xB100_0CC8;
/// `fe_hash("Continue")`
pub(crate) const CONTINUE: u32 = 0xDE02_CF84;
/// `CURRENT` in screens/engage.rs
pub(crate) const CURRENT: u32 = 0xC39A_1BAE;
/// `DIALOG_READY` in screens/engage.rs
pub(crate) const DIALOG_READY: u32 = 0x35F8_620B;
/// `DIALOG_ACCEPT` in screens/engage.rs
pub(crate) const ENGAGE_ACCEPT: u32 = 0xD72F_002A;
/// `DIALOG_CANCEL` in screens/engage.rs
pub(crate) const ENGAGE_CANCEL: u32 = 0x6385_7DE0;
/// `DISTANCE` in screens/engage.rs
pub(crate) const ENGAGE_DISTANCE: u32 = 0x080C_9DAA;
/// `MAP` in screens/engage.rs
pub(crate) const ENGAGE_MAP: u32 = 0x0F36_5871;
/// `MESSAGE` in screens/engage.rs
pub(crate) const ENGAGE_MESSAGE: u32 = 0xDE07_5B62;
/// `TYPE` in screens/engage.rs
pub(crate) const ENGAGE_TYPE: u32 = 0x15E1_351B;
/// `FILL` in screens/engage.rs
pub(crate) const FILL: u32 = 0x30B7_4609;
/// `FROM` in screens/engage.rs
pub(crate) const FROM: u32 = 0xFECE_D617;
/// `LAP_LABEL` in screens/engage.rs
pub(crate) const LAP_LABEL: u32 = 0x788C_05E8;
/// `LAP_NUMBER` in screens/engage.rs
pub(crate) const LAP_NUMBER: u32 = 0x57F0_E3A5;
/// `NUMBER_LAPS` in screens/engage.rs
pub(crate) const NUMBER_LAPS: u32 = 0xF268_5238;
/// role not stated where it is used (screens/engage.rs:110); needs the binary
pub(crate) const OBJ_0FDA6758: u32 = 0x0FDA_6758;
/// role not stated where it is used (screens/engage.rs:208); needs the binary
pub(crate) const OBJ_15D6B438: u32 = 0x15D6_B438;
/// role not stated where it is used (screens/engage.rs:108); needs the binary
pub(crate) const OBJ_268D063C: u32 = 0x268D_063C;
/// role not stated where it is used (screens/engage.rs:111); needs the binary
pub(crate) const OBJ_40C91BA7: u32 = 0x40C9_1BA7;
/// role not stated where it is used (screens/engage.rs:458); needs the binary
pub(crate) const OBJ_5382B2CA: u32 = 0x5382_B2CA;
/// role not stated where it is used (screens/engage.rs:112); needs the binary
pub(crate) const OBJ_59ECB2C2: u32 = 0x59EC_B2C2;
/// role not stated where it is used (screens/engage.rs:108); needs the binary
pub(crate) const OBJ_9B39C122: u32 = 0x9B39_C122;
/// role not stated where it is used (screens/engage.rs:107); needs the binary
pub(crate) const OBJ_B46377BC: u32 = 0xB463_77BC;
/// role not stated where it is used (screens/engage.rs:109); needs the binary
pub(crate) const OBJ_BA613729: u32 = 0xBA61_3729;
/// role not stated where it is used (screens/engage.rs:150); needs the binary
pub(crate) const OBJ_C01A6F63: u32 = 0xC01A_6F63;
/// seq at screens/engage.rs:299
pub(crate) const PAIR_GROUPS_0: u32 = 0x949F_4947;
/// seq at screens/engage.rs:299
pub(crate) const PAIR_GROUPS_1: u32 = 0x949F_4948;
/// `PAUSE` in screens/engage.rs
pub(crate) const PAUSE: u32 = 0xCBED_A229;
/// `PLAY` in screens/engage.rs
pub(crate) const PLAY: u32 = 0x8A0F_21A1;
/// `PURSE` in screens/engage.rs
pub(crate) const PURSE: u32 = 0xB54B_81A7;
/// `BACKING` in screens/engage.rs
pub(crate) const REPLAY_CONTROL_BACKING: u32 = 0x688A_1551;
/// `LAP_TIME` in screens/engage.rs
pub(crate) const REPLAY_CONTROL_LAP_TIME: u32 = 0x30D9_802E;
/// `fe_hash("Retry")`
pub(crate) const RETRY: u32 = 0x039D_F715;
/// `MESSAGE` in screens/engage.rs
pub(crate) const SHOP_ENGAGE_MESSAGE: u32 = 0x04A0_4E7C;
/// `TITLE` in screens/engage.rs
pub(crate) const SHOP_ENGAGE_TITLE: u32 = 0x02ED_BDE5;
/// `SUBJECT` in screens/engage.rs
pub(crate) const SUBJECT: u32 = 0x2C16_7533;
/// text at screens/engage.rs:139
pub(crate) const TXT_KM: u32 = 0xB5D6_40B1;
/// text at screens/engage.rs:139
pub(crate) const TXT_MI: u32 = 0xAC3A_4F4C;

// ---- screens/ingame_map.rs
/// `BOX_OFF` in screens/ingame_map.rs
pub(crate) const BOX_OFF: u32 = 0xB90F_50BB;
/// `BOX_ON` in screens/ingame_map.rs
pub(crate) const BOX_ON: u32 = 0x62BC_0DFC;
/// `fe_hash("Disabled")`
pub(crate) const DISABLED: u32 = 0x06B5_5037;
/// `MAP` in screens/ingame_map.rs
pub(crate) const IN_GAME_MAP_MAP: u32 = 0x90FE_80F3;
/// `PLAYER` in screens/ingame_map.rs
pub(crate) const IN_GAME_MAP_PLAYER: u32 = 0xDD9E_F5FF;
/// `OFF_COLOUR` in screens/ingame_map.rs
pub(crate) const OFF_COLOUR: u32 = 0xC118_04C2;
/// `SELECTION` in screens/ingame_map.rs
pub(crate) const SELECTION: u32 = 0xE284_8973;
/// seq at screens/ingame_map.rs:163
pub(crate) const WORLD_MAP_ZOOMS_0: u32 = 0xD009_05F2;
/// seq at screens/ingame_map.rs:164
pub(crate) const WORLD_MAP_ZOOMS_1: u32 = 0xC08A_F0B3;
/// seq at screens/ingame_map.rs:165
pub(crate) const WORLD_MAP_ZOOMS_2: u32 = 0xC08A_F0B4;
/// seq at screens/ingame_map.rs:166
pub(crate) const WORLD_MAP_ZOOMS_3: u32 = 0xC08A_F0B5;
/// seq at screens/ingame_map.rs:167
pub(crate) const WORLD_MAP_ZOOMS_4: u32 = 0xC08A_F0B6;
/// seq at screens/ingame_map.rs:168
pub(crate) const WORLD_MAP_ZOOMS_5: u32 = 0xC08A_F0B7;
/// `ZOOM_LABEL` in screens/ingame_map.rs
pub(crate) const ZOOM_LABEL: u32 = 0x42FA_AD5A;

// ---- screens/jukebox.rs
/// `ALBUM` in screens/jukebox.rs
pub(crate) const ALBUM: u32 = 0x77BD_189E;
/// `ARTIST` in screens/jukebox.rs
pub(crate) const ARTIST: u32 = 0x6664_6FC4;
/// `fe_hash("Grab_Arrow_1")`
pub(crate) const GRAB_ARROW_1: u32 = 0x07CD_00B5;
/// seq at screens/jukebox.rs:69
pub(crate) const JUKEBOX_GROUPS_0: u32 = 0xDE63_5F7C;
/// seq at screens/jukebox.rs:69
pub(crate) const JUKEBOX_GROUPS_1: u32 = 0x8065_767B;
/// `LEFT` in screens/jukebox.rs
pub(crate) const JUKEBOX_LEFT: u32 = 0x7BCD_6704;
/// `RIGHT` in screens/jukebox.rs
pub(crate) const JUKEBOX_RIGHT: u32 = 0x7C51_B6D7;
/// `TITLE` in screens/jukebox.rs
pub(crate) const JUKEBOX_TITLE: u32 = 0x2890_C8AF;
/// `NUMBER` in screens/jukebox.rs
pub(crate) const NUMBER: u32 = 0xE454_E9A5;
/// `LEFT` in screens/jukebox.rs
pub(crate) const POST_RACE_STATS_LEFT: u32 = 0x4449_69FD;
/// `RIGHT` in screens/jukebox.rs
pub(crate) const POST_RACE_STATS_RIGHT: u32 = 0x4449_69FE;
/// `STATE` in screens/jukebox.rs
pub(crate) const STATE: u32 = 0xF3EB_DC4E;
/// seq at screens/jukebox.rs:72
pub(crate) const STATES_0: u32 = 0x9CCE_9F86;
/// seq at screens/jukebox.rs:72
pub(crate) const STATES_1: u32 = 0x5278_C50B;
/// seq at screens/jukebox.rs:72
pub(crate) const STATES_2: u32 = 0x5C1B_351C;
/// seq at screens/jukebox.rs:72
pub(crate) const STATES_3: u32 = 0x9CCE_64C4;
/// text at screens/jukebox.rs:168
pub(crate) const TXT_RANDOM: u32 = 0xA9C9_C8F7;
/// text at screens/jukebox.rs:168
pub(crate) const TXT_SEQUENTIAL: u32 = 0x4CA3_6B89;

// ---- screens/magazine.rs
/// `BACK_BUTTON` in screens/magazine.rs
pub(crate) const BACK_BUTTON: u32 = 0x1C7F_CF8B;
/// `DVD_ICON` in screens/magazine.rs
pub(crate) const DVD_ICON: u32 = 0x597D_F2A7;
/// `IMAGE` in screens/magazine.rs
pub(crate) const IMAGE: u32 = 0xD1CC_9C37;
/// seq at screens/magazine.rs:117
pub(crate) const LABELS_3: u32 = 0xB8A7_C6CF;
/// `fe_hash("MagazineChick")`
pub(crate) const MAGAZINECHICK: u32 = 0x8222_7F6D;
/// `fe_hash("MagazineExtra")`
pub(crate) const MAGAZINEEXTRA: u32 = 0x824F_A72F;
/// `fe_hash("MagazineMast")`
pub(crate) const MAGAZINEMAST: u32 = 0xB663_8240;
/// `MAGAZINE_ICON` in screens/magazine.rs
pub(crate) const MAGAZINE_ICON: u32 = 0xFF29_3517;
/// `REWARD_BUTTON` in screens/magazine.rs
pub(crate) const REWARD_BUTTON: u32 = 0x69BA_0838;

// ---- screens/memcard.rs
/// seq at screens/memcard.rs:302
pub(crate) const ARROWS_0: u32 = 0x4449_6A03;
/// seq at screens/memcard.rs:302
pub(crate) const ARROWS_1: u32 = 0x4449_6A02;
/// `CURSOR_BAR` in screens/memcard.rs
pub(crate) const CURSOR_BAR: u32 = 0x557F_E993;
/// `FIELD` in screens/memcard.rs
pub(crate) const FIELD: u32 = 0x5B9D_88B9;
/// seq at screens/memcard.rs:93
pub(crate) const GROUPS_3: u32 = 0xB100_0CC9;
/// seq at screens/memcard.rs:93
pub(crate) const GROUPS_4: u32 = 0xB100_0CCA;
/// seq at screens/memcard.rs:93
pub(crate) const GROUPS_5: u32 = 0xB100_0CCB;
/// `HEADER` in screens/memcard.rs
pub(crate) const HEADER: u32 = 0x426C_7B4D;
/// seq at screens/memcard.rs:92
pub(crate) const LABELS_4: u32 = 0xB8A7_C6D0;
/// seq at screens/memcard.rs:92
pub(crate) const LABELS_5: u32 = 0xB8A7_C6D1;
/// `fe_hash("Load")`
pub(crate) const LOAD: u32 = 0x0018_ECFF;
/// `BACKING` in screens/memcard.rs
pub(crate) const MC_LIST_BACKING: u32 = 0x66E2_9936;
/// `fe_hash("Delete")`
pub(crate) const MEMCARD_DELETE: u32 = 0x56B0_0632;
/// seq at screens/memcard.rs:93
pub(crate) const MEMCARD_GROUPS_0: u32 = 0xB100_0CC6;
/// seq at screens/memcard.rs:93
pub(crate) const MEMCARD_GROUPS_1: u32 = 0xB100_0CC7;
/// `NAME_ENTRY` in screens/memcard.rs
pub(crate) const NAME_ENTRY: u32 = 0x6042_A783;
/// role not stated where it is used (screens/memcard.rs:258); needs the binary
pub(crate) const OBJ_5B0D9106: u32 = 0x5B0D_9106;
/// role not stated where it is used (screens/memcard.rs:241); needs the binary
pub(crate) const OBJ_B19F8B41: u32 = 0xB19F_8B41;
/// `TITLE_ROW` in screens/memcard.rs
pub(crate) const TITLE_ROW: u32 = 0x48D4_FCAE;
/// text at screens/memcard.rs:222
pub(crate) const TXT_CREATE_PROFILE: u32 = 0xF54C_C524;

// ---- screens/menus.rs
/// opt at screens/menus.rs:54 ("\"Car Select\"")
pub(crate) const CAR_SELECT_ICON: u32 = 0xA964_095B;
/// opt at screens/menus.rs:55 ("\"Change Parts\"")
pub(crate) const CHANGE_PARTS_ICON: u32 = 0x511F_9D75;
/// `fe_hash("Create")`
pub(crate) const CREATE: u32 = 0x5542_3473;
/// opt at screens/menus.rs:124 ("\"Change Body Parts\"")
pub(crate) const CUSTOMIZE_CATEGORY_0_ICON: u32 = 0xB802_F10E;
/// opt at screens/menus.rs:125 ("\"Change Performance Parts\"")
pub(crate) const CUSTOMIZE_CATEGORY_1_ICON: u32 = 0xC56D_3032;
/// opt at screens/menus.rs:126 ("\"Change Paint / Vinyls\"")
pub(crate) const CUSTOMIZE_CATEGORY_2_ICON: u32 = 0xDC99_AAFE;
/// opt at screens/menus.rs:127 ("\"Change Interiors / Car Specialties\"")
pub(crate) const CUSTOMIZE_CATEGORY_3_ICON: u32 = 0x3F40_E75F;
/// opt at screens/menus.rs:34 ("\"Resume Career\"")
pub(crate) const EXPLORE_AND_RESUME_CAREER_ICON: u32 = 0xA230_CD88;
/// opt at screens/menus.rs:158
pub(crate) const JUMP_TO_GARAGE_ICON: u32 = 0xB803_8AC0;
/// opt at screens/menus.rs:158
pub(crate) const JUMP_TO_GARAGE_LABEL: u32 = 0x4538_90E1;
/// opt at screens/menus.rs:42 ("\"Load Game\"")
pub(crate) const LOAD_CAREER_ICON: u32 = 0x72C8_7F3C;
/// `fe_hash("MAIN_ICON_CAREER")`
pub(crate) const MAIN_ICON_CAREER: u32 = 0x0370_4F3D;
/// `fe_hash("MAIN_ICON_CUSTOMIZE_CAR")`
pub(crate) const MAIN_ICON_CUSTOMIZE_CAR: u32 = 0xB0C4_6023;
/// `fe_hash("MAIN_ICON_GO_ONLINE")`
pub(crate) const MAIN_ICON_GO_ONLINE: u32 = 0x7718_49A5;
/// `fe_hash("MAIN_ICON_LAN")`
pub(crate) const MAIN_ICON_LAN: u32 = 0xD13A_2E66;
/// `fe_hash("MAIN_ICON_OPTIONS")`
pub(crate) const MAIN_ICON_OPTIONS: u32 = 0x3058_FE37;
/// comment at screens/menus.rs:21 ("MAIN_ICON_PROFILE_MANAG, \"Profile Manager\"")
pub(crate) const MAIN_ICON_PROFILE_MANAG: u32 = 0x6B30_3856;
/// `fe_hash("MAIN_ICON_QUICK_RACE")`
pub(crate) const MAIN_ICON_QUICK_RACE: u32 = 0x4E6F_BB02;
/// opt at screens/menus.rs:59 ("\"Back to Main Menu\"")
pub(crate) const MAIN_MENU_ICON: u32 = 0xFC39_853F;
/// opt at screens/menus.rs:207
pub(crate) const MENUS_RETURN_TO_EXPLORE_LABEL_3C0D: u32 = 0xDB4E_3C0D;
/// opt at screens/menus.rs:156
pub(crate) const MENUS_RETURN_TO_EXPLORE_LABEL_BCDC: u32 = 0xC3E5_BCDC;
/// opt at screens/menus.rs:36 ("\"Start New Career\" (resumes)")
pub(crate) const NEW_CAREER_AND_RESUME_CAREER_ICON: u32 = 0x3CB9_2E26;
/// opt at screens/menus.rs:204
pub(crate) const NEXT_RACE_AND_RESUME_RACE_ICON: u32 = 0x12BB_5EA2;
/// opt at screens/menus.rs:227
pub(crate) const NEXT_RACE_LABEL: u32 = 0x23F3_59B8;
/// opt at screens/menus.rs:163
pub(crate) const OPTIONS_ICON: u32 = 0x520D_E4E3;
/// opt at screens/menus.rs:163
pub(crate) const OPTIONS_LABEL: u32 = 0xB349_93C9;
/// `SLOT` in screens/menus.rs
pub(crate) const OPTION_MASTER: u32 = 0xB046_69E3;
/// opt at screens/menus.rs:94 ("\"Audio\"")
pub(crate) const PAUSE_SETTING_0_AND_SETTING_0_ICON: u32 = 0xF37A_F144;
/// opt at screens/menus.rs:95 ("\"Display\"")
pub(crate) const PAUSE_SETTING_1_AND_SETTING_1_ICON: u32 = 0x8A00_6328;
/// opt at screens/menus.rs:96 ("\"Gameplay\"")
pub(crate) const PAUSE_SETTING_2_AND_SETTING_2_ICON: u32 = 0x4DF9_8FB2;
/// opt at screens/menus.rs:97 ("\"Player\"")
pub(crate) const PAUSE_SETTING_3_AND_SETTING_3_ICON: u32 = 0xD708_EFEF;
/// opt at screens/menus.rs:138
pub(crate) const PROFILE_CREATE_ICON: u32 = 0x5602_6748;
/// opt at screens/menus.rs:141
pub(crate) const PROFILE_DELETE_ICON: u32 = 0xB992_9709;
/// opt at screens/menus.rs:139
pub(crate) const PROFILE_LOAD_ICON: u32 = 0x2287_E063;
/// opt at screens/menus.rs:140
pub(crate) const PROFILE_SAVE_ICON: u32 = 0x228B_7E32;
/// opt at screens/menus.rs:83
pub(crate) const QR_MODE_CIRCUIT_ICON: u32 = 0xE963_8D3E;
/// opt at screens/menus.rs:83
pub(crate) const QR_MODE_CIRCUIT_LABEL: u32 = 0x93FD_EE74;
/// opt at screens/menus.rs:85
pub(crate) const QR_MODE_DRAG_ICON: u32 = 0xAAAB_31E9;
/// opt at screens/menus.rs:85
pub(crate) const QR_MODE_DRAG_LABEL: u32 = 0x3510_2EDF;
/// opt at screens/menus.rs:86
pub(crate) const QR_MODE_DRIFT_ICON: u32 = 0x0011_9144;
/// opt at screens/menus.rs:86
pub(crate) const QR_MODE_DRIFT_LABEL: u32 = 0xD716_2CFA;
/// opt at screens/menus.rs:89
pub(crate) const QR_MODE_FREE_RUN_ICON: u32 = 0x713D_99C1;
/// opt at screens/menus.rs:89
pub(crate) const QR_MODE_FREE_RUN_LABEL: u32 = 0x6F24_21B7;
/// opt at screens/menus.rs:84
pub(crate) const QR_MODE_SPRINT_ICON: u32 = 0x2521_E5EB;
/// opt at screens/menus.rs:84
pub(crate) const QR_MODE_SPRINT_LABEL: u32 = 0xDCB9_F861;
/// opt at screens/menus.rs:87
pub(crate) const QR_MODE_STREET_X_ICON: u32 = 0x9170_3AEF;
/// opt at screens/menus.rs:87
pub(crate) const QR_MODE_STREET_X_LABEL: u32 = 0x30DA_61E1;
/// opt at screens/menus.rs:88
pub(crate) const QR_MODE_URL_ICON: u32 = 0x3F64_95C5;
/// opt at screens/menus.rs:88
pub(crate) const QR_MODE_URL_LABEL: u32 = 0xC786_C4E7;
/// opt at screens/menus.rs:171
pub(crate) const QUIT_TO_MAIN_MENU_ICON: u32 = 0x4C9E_34E6;
/// opt at screens/menus.rs:171
pub(crate) const QUIT_TO_MAIN_MENU_LABEL: u32 = 0xC7E1_420C;
/// opt at screens/menus.rs:235
pub(crate) const RACE_STATS_ICON: u32 = 0x70BB_9626;
/// opt at screens/menus.rs:235
pub(crate) const RACE_STATS_LABEL: u32 = 0xF82D_F37E;
/// opt at screens/menus.rs:169
pub(crate) const REPLAY_AND_UNAVAILABLE_ICON: u32 = 0x84D1_D864;
/// opt at screens/menus.rs:205
pub(crate) const RESTART_ICON: u32 = 0xB295_A6B6;
/// opt at screens/menus.rs:205
pub(crate) const RESTART_LABEL: u32 = 0x8083_3FC2;
/// opt at screens/menus.rs:204
pub(crate) const RESUME_RACE_LABEL: u32 = 0xF660_1088;
/// opt at screens/menus.rs:156
pub(crate) const RETURN_TO_EXPLORE_ICON: u32 = 0x62AA_0DF6;
/// opt at screens/menus.rs:161
pub(crate) const RETURN_TO_FREE_RUN_LABEL: u32 = 0xC44A_6515;
/// opt at screens/menus.rs:202
pub(crate) const RETURN_TO_WORLD_MAP_ICON: u32 = 0x3109_913C;
/// opt at screens/menus.rs:202
pub(crate) const RETURN_TO_WORLD_MAP_LABEL: u32 = 0x5E67_0A9E;
/// opt at screens/menus.rs:57 ("\"View Rewards\"")
pub(crate) const REWARDS_ICON: u32 = 0x49D2_E3DE;
/// `fe_hash("Save")`
pub(crate) const SAVE: u32 = 0x001C_8ACE;
/// opt at screens/menus.rs:58 ("\"Save\"")
pub(crate) const SAVE_ICON: u32 = 0x8ADA_6FD4;
/// opt at screens/menus.rs:98 ("\"Controls\"")
pub(crate) const SETTING_4_ICON: u32 = 0xA04A_7B26;
/// opt at screens/menus.rs:99 ("\"EA GAMES TRAX\"")
pub(crate) const SETTING_5_ICON: u32 = 0xC52C_CBF6;
/// opt at screens/menus.rs:100 ("\"Credits\"")
pub(crate) const SETTING_6_ICON: u32 = 0x5100_9E20;
/// opt at screens/menus.rs:56 ("\"Performance Tuning / Dyno\"")
pub(crate) const TUNING_ICON: u32 = 0x7A9B_397A;
/// text at screens/menus.rs:94 ("\"Audio\"")
pub(crate) const TXT_AUDIO: u32 = 0xE76C_D783;
/// text at screens/menus.rs:59 ("\"Back to Main Menu\"")
pub(crate) const TXT_BACK_TO_MAIN_MENU: u32 = 0x046E_7D84;
/// text at screens/menus.rs:16 ("MAIN_ICON_CAREER, \"Career\"")
pub(crate) const TXT_CAREER: u32 = 0x5E7E_1192;
/// text at screens/menus.rs:54 ("\"Car Select\"")
pub(crate) const TXT_CAR_SELECT: u32 = 0x4518_79E2;
/// text at screens/menus.rs:124 ("\"Change Body Parts\"")
pub(crate) const TXT_CHANGE_BODY_PARTS: u32 = 0xA92B_4AE8;
/// text at screens/menus.rs:127 ("\"Change Interiors / Car Specialties\"")
pub(crate) const TXT_CHANGE_INTERIORS_CAR_SPECIALTIES: u32 = 0xC10D_7979;
/// text at screens/menus.rs:126 ("\"Change Paint / Vinyls\"")
pub(crate) const TXT_CHANGE_PAINT_VINYLS: u32 = 0xC608_9CA7;
/// text at screens/menus.rs:55 ("\"Change Parts\"")
pub(crate) const TXT_CHANGE_PARTS: u32 = 0xABB2_4BBC;
/// text at screens/menus.rs:125 ("\"Change Performance Parts\"")
pub(crate) const TXT_CHANGE_PERFORMANCE_PARTS: u32 = 0xECA6_B0CC;
/// text at screens/menus.rs:100 ("\"Credits\"")
pub(crate) const TXT_CREDITS: u32 = 0x0905_101F;
/// text at screens/menus.rs:20 ("MAIN_ICON_CUSTOMIZE_CAR, \"Customize\"")
pub(crate) const TXT_CUSTOMIZE: u32 = 0xA530_39A3;
/// text at screens/menus.rs:95 ("\"Display\"")
pub(crate) const TXT_DISPLAY: u32 = 0x4204_D527;
/// text at screens/menus.rs:99 ("\"EA GAMES TRAX\"")
pub(crate) const TXT_EA_GAMES_TRAX: u32 = 0xDCFB_6B36;
/// text at screens/menus.rs:96 ("\"Gameplay\"")
pub(crate) const TXT_GAMEPLAY: u32 = 0xD0CF_6EE1;
/// text at screens/menus.rs:19 ("MAIN_ICON_LAN, \"LAN\"")
pub(crate) const TXT_LAN: u32 = 0x7294_434C;
/// text at screens/menus.rs:42 ("\"Load Game\"")
pub(crate) const TXT_LOAD_GAME: u32 = 0xAA3F_09D8;
/// text at screens/menus.rs:18 ("MAIN_ICON_GO_ONLINE, \"Online\"")
pub(crate) const TXT_ONLINE: u32 = 0x7B63_FF85;
/// text at screens/menus.rs:22 ("MAIN_ICON_OPTIONS, \"Options\"")
pub(crate) const TXT_OPTIONS: u32 = 0xED1F_0B2C;
/// text at screens/menus.rs:56 ("\"Performance Tuning / Dyno\"")
pub(crate) const TXT_PERFORMANCE_TUNING_DYNO: u32 = 0x8513_F2C0;
/// text at screens/menus.rs:97 ("\"Player\"")
pub(crate) const TXT_PLAYER: u32 = 0xF760_EABE;
/// text at screens/menus.rs:21 ("MAIN_ICON_PROFILE_MANAG, \"Profile Manager\"")
pub(crate) const TXT_PROFILE_MANAGER: u32 = 0xBCB1_8F38;
/// text at screens/menus.rs:17 ("MAIN_ICON_QUICK_RACE, \"Quick Race\"")
pub(crate) const TXT_QUICK_RACE: u32 = 0x2490_9FD7;
/// text at screens/menus.rs:169
pub(crate) const TXT_REPLAY: u32 = 0x7840_428A;
/// text at screens/menus.rs:34 ("\"Resume Career\"")
pub(crate) const TXT_RESUME_CAREER: u32 = 0xE072_DB21;
/// text at screens/menus.rs:49 ("\"Return to Explore Mode\"")
pub(crate) const TXT_RETURN_TO_EXPLORE_MODE: u32 = 0x194E_FA7A;
/// text at screens/menus.rs:58 ("\"Save\"")
pub(crate) const TXT_SAVE: u32 = 0x03EC_7395;
/// text at screens/menus.rs:36 ("\"Start New Career\" (resumes)")
pub(crate) const TXT_START_NEW_CAREER: u32 = 0x97B1_DCDA;
/// text at screens/menus.rs:57 ("\"View Rewards\"")
pub(crate) const TXT_VIEW_REWARDS: u32 = 0xCE62_C0BF;
/// text at screens/menus.rs:51 ("\"World Map\"")
pub(crate) const TXT_WORLD_MAP: u32 = 0x9C07_CA52;
/// `OPTION_TITLE` in screens/menus.rs
pub(crate) const UI_MAIN_OPTION_TITLE: u32 = 0x5E7B_09C9;
/// opt at screens/menus.rs:51 ("\"World Map\"")
pub(crate) const WORLD_MAP_ICON: u32 = 0x22F2_CEAB;

// ---- screens/mod.rs
/// seq at screens/mod.rs:386
pub(crate) const BOXES_0: u32 = 0xC2EA_8DE7;
/// seq at screens/mod.rs:386
pub(crate) const BOXES_1: u32 = 0xC2EA_8DE8;
/// seq at screens/mod.rs:386
pub(crate) const BOXES_2: u32 = 0xC2EA_8DE9;
/// seq at screens/mod.rs:386
pub(crate) const BOXES_3: u32 = 0xC2EA_8DEA;
/// seq at screens/mod.rs:388
pub(crate) const BUTTONS_0: u32 = 0x69BA_0835;
/// seq at screens/mod.rs:388
pub(crate) const BUTTONS_1: u32 = 0x69BA_0836;
/// seq at screens/mod.rs:388
pub(crate) const BUTTONS_2: u32 = 0x69BA_0837;
/// `FRAME` in screens/mod.rs
pub(crate) const FRAME: u32 = 0x2253_BD97;
/// seq at screens/mod.rs:396
pub(crate) const FRAMES_0: u32 = 0xF9F1_F5CD;
/// seq at screens/mod.rs:396
pub(crate) const FRAMES_1: u32 = 0xF9F1_F5CE;
/// seq at screens/mod.rs:396
pub(crate) const FRAMES_2: u32 = 0xF9F1_F5CF;
/// `INIT_PULSE` in screens/mod.rs
pub(crate) const INIT_PULSE: u32 = 0xA3E4_657B;
/// `MENU_GROUP` in screens/mod.rs
pub(crate) const MENU_GROUP: u32 = 0xF9F1_F5D0;
/// seq at screens/mod.rs:385
pub(crate) const OPTIONS_2: u32 = 0x95AD_094B;
/// seq at screens/mod.rs:385
pub(crate) const OPTIONS_3: u32 = 0x95AD_094C;
/// seq at screens/mod.rs:385
pub(crate) const SCREENS_OPTIONS_0: u32 = 0x95AD_0949;
/// seq at screens/mod.rs:385
pub(crate) const SCREENS_OPTIONS_1: u32 = 0x95AD_094A;
/// `SELECT` in screens/mod.rs
pub(crate) const SELECT: u32 = 0x79AD_657F;
/// `ACCEPT` in screens/mod.rs
pub(crate) const SHOWCASE_ACCEPT: u32 = 0x4F87_6B6F;
/// `BACK` in screens/mod.rs
pub(crate) const SHOWCASE_BACK: u32 = 0x0013_35F0;

// ---- screens/online.rs
/// `TITLE` in screens/online.rs
pub(crate) const EA_LOGIN_TITLE: u32 = 0x3C14_9013;
/// `ESRB` in screens/online.rs
pub(crate) const ESRB: u32 = 0x3564_57BE;
/// `fe_hash("Cancel")`
pub(crate) const ONLINE_CANCEL: u32 = 0x5413_89E5;
/// seq at screens/online.rs:83
pub(crate) const ONLINE_OPTIONS_0: u32 = 0xED21_B47F;
/// seq at screens/online.rs:83
pub(crate) const ONLINE_OPTIONS_1: u32 = 0x036E_79B9;
/// `fe_hash("TITLE")`
pub(crate) const ONLINE_TITLE: u32 = 0x03C4_58C1;

// ---- screens/paint.rs
/// table at screens/paint.rs:5 ("Base Paint")
pub(crate) const BASE_PAINT_ICON: u32 = 0xA3B7_6154;
/// table at screens/paint.rs:5 ("Base Paint")
pub(crate) const BASE_PAINT_LABEL: u32 = 0xEF5B_4A75;
/// table at screens/paint.rs:11 ("Brakes")
pub(crate) const BRAKES_ICON: u32 = 0x7EB5_5A26;
/// table at screens/paint.rs:11 ("Brakes")
pub(crate) const BRAKES_LABEL: u32 = 0xDACD_9FFF;
/// table at screens/paint.rs:12 ("Engine Accents")
pub(crate) const ENGINE_ACCENTS_ICON: u32 = 0x4E87_5A92;
/// table at screens/paint.rs:12 ("Engine Accents")
pub(crate) const ENGINE_ACCENTS_LABEL: u32 = 0x3B3B_FAF0;
/// `GLOSS` in screens/paint.rs
pub(crate) const GLOSS: u32 = 0x02DA_AB07;
/// `HIGHLIGHT` in screens/paint.rs
pub(crate) const HIGHLIGHT: u32 = 0x9BA6_D658;
/// `METAL` in screens/paint.rs
pub(crate) const METAL: u32 = 0x0343_7A52;
/// table at screens/paint.rs:8 ("Mirrors")
pub(crate) const MIRRORS_ICON: u32 = 0xDF5C_2BCA;
/// table at screens/paint.rs:8 ("Mirrors")
pub(crate) const MIRRORS_LABEL: u32 = 0x4D90_F415;
/// `fe_hash("Muffler")`
pub(crate) const MUFFLER: u32 = 0x0663_50F0;
/// table at screens/paint.rs:13 ("Muffler Tip")
pub(crate) const MUFFLER_TIP_ICON: u32 = 0xA634_3AEC;
/// table at screens/paint.rs:13 ("Muffler Tip")
pub(crate) const MUFFLER_TIP_LABEL: u32 = 0xBA1C_C49C;
/// role not stated where it is used (screens/paint.rs:201); needs the binary
pub(crate) const OBJ_23A43844: u32 = 0x23A4_3844;
/// UI_Paint's second finish-selector arrow (ctor 0x56AEC0: 0x56B016 → +0x154).
pub(crate) const PAINT_FINISH_ARROW_2: u32 = 0xABA9_3D3F;
/// UI_Paint's first finish-selector arrow (ctor 0x56AEC0: 0x56AFD1 → +0x150).
pub(crate) const PAINT_FINISH_ARROW_1: u32 = 0xCF6F_0439;
/// `OPTION_TEMPLATE` in screens/paint.rs
pub(crate) const OPTION_TEMPLATE: u32 = 0x4B4E_3888;
/// `CASH` in screens/paint.rs
pub(crate) const PAINT_CASH: u32 = 0x0013_C45E;
/// `fe_hash("PAINT_ICON_1")`
pub(crate) const PAINT_ICON_1: u32 = 0x43CE_0A53;
/// table at screens/paint.rs:10 ("Spinners")
pub(crate) const PAINT_SPINNERS_LABEL: u32 = 0xDD3B_EBF9;
/// `fe_hash("Paint_text")`
pub(crate) const PAINT_TEXT: u32 = 0xEF65_3AFF;
/// `PEARL` in screens/paint.rs
pub(crate) const PEARL: u32 = 0x0379_7533;
/// table at screens/paint.rs:9 ("Rims")
pub(crate) const RIMS_AND_SPINNERS_ICON: u32 = 0xD223_F84A;
/// table at screens/paint.rs:9 ("Rims")
pub(crate) const RIMS_LABEL: u32 = 0x0F03_0942;
/// table at screens/paint.rs:7 ("Roof Scoop")
pub(crate) const ROOF_SCOOP_ICON: u32 = 0xD224_1205;
/// table at screens/paint.rs:7 ("Roof Scoop")
pub(crate) const ROOF_SCOOP_LABEL: u32 = 0xEF64_4B90;
/// table at screens/paint.rs:6 ("Spoiler")
pub(crate) const SPOILER_ICON: u32 = 0xA8D7_E6ED;
/// table at screens/paint.rs:6 ("Spoiler")
pub(crate) const SPOILER_LABEL: u32 = 0xDDA5_B898;
/// text at screens/paint.rs:105
pub(crate) const TXT_CHOOSE_VINYL_COLORS: u32 = 0xC034_CE3D;

// ---- screens/parts_browser.rs
/// `fe_hash("Avus")`
pub(crate) const AVUS: u32 = 0x0013_053E;
/// table at screens/parts_browser.rs:18 ("Carbon Fiber")
pub(crate) const CARBON_FIBER: u32 = 0x2491_1E04;
/// `CATEGORY_ICON` in screens/parts_browser.rs
pub(crate) const CATEGORY_ICON: u32 = 0x8725_7585;
/// `CATEGORY_MASTER` in screens/parts_browser.rs
pub(crate) const CATEGORY_MASTER: u32 = 0xEF8B_B088;
/// `CATEGORY_ONLY` in screens/parts_browser.rs
pub(crate) const CATEGORY_ONLY: u32 = 0xDF42_2345;
/// `fe_hash("Donz")`
pub(crate) const DONZ: u32 = 0x0014_8BBA;
/// `fe_hash("Enkei")`
pub(crate) const ENKEI: u32 = 0x02B7_7FEB;
/// table at screens/parts_browser.rs:16 ("Exhaust Tips")
pub(crate) const EXHAUST_TIPS_ICON: u32 = 0xA434_A006;
/// table at screens/parts_browser.rs:16 ("Exhaust Tips")
pub(crate) const EXHAUST_TIPS_LABEL: u32 = 0x2ACE_D725;
/// `fe_hash("Foxx")`
pub(crate) const FOXX: u32 = 0x0015_A5C4;
/// table at screens/parts_browser.rs:7 ("Front Bumpers")
pub(crate) const FRONT_BUMPERS: u32 = 0x739C_4A1B;
/// `fe_hash("Gianelle")`
pub(crate) const GIANELLE: u32 = 0xA26A_D820;
/// `fe_hash("Giovanna")`
pub(crate) const GIOVANNA: u32 = 0xC3A1_AC32;
/// table at screens/parts_browser.rs:13 ("Headlights")
pub(crate) const HEADLIGHTS_ICON: u32 = 0xF69A_C384;
/// table at screens/parts_browser.rs:13 ("Headlights")
pub(crate) const HEADLIGHTS_LABEL: u32 = 0x3DC6_A111;
/// table at screens/parts_browser.rs:11 ("Hoods")
pub(crate) const HOODS: u32 = 0x028F_7092;
/// `fe_hash("INSTALL")`
pub(crate) const INSTALL: u32 = 0xC318_FF76;
/// `fe_hash("Kaizer")`
pub(crate) const KAIZER: u32 = 0x66BA_72E5;
/// `fe_hash("Konig")`
pub(crate) const KONIG: u32 = 0x0324_AC97;
/// `fe_hash("Lexani")`
pub(crate) const LEXANI: u32 = 0x695F_CE00;
/// `LOCK_MASTER` in screens/parts_browser.rs
pub(crate) const LOCK_MASTER: u32 = 0xA2F1_1B13;
/// `fe_hash("Lowenhart")`
pub(crate) const LOWENHART: u32 = 0x48E2_5793;
/// `MINI_MASTER` in screens/parts_browser.rs
pub(crate) const MINI_MASTER: u32 = 0x38E3_9AD7;
/// `fe_hash("Momo")`
pub(crate) const MOMO: u32 = 0x0019_7AF7;
/// `fe_hash("NFSU")`
pub(crate) const NFSU: u32 = 0x0019_E1DB;
/// role not stated where it is used (screens/parts_browser.rs:397); needs the binary
pub(crate) const OBJ_640F0F23: u32 = 0x640F_0F23;
/// `OPTION_TITLE` in screens/parts_browser.rs
pub(crate) const PARTS_BROWSER_OPTION_TITLE: u32 = 0x6C9E_933E;
/// `PARTS_ICON` in screens/parts_browser.rs
pub(crate) const PARTS_ICON: u32 = 0x0937_E8F1;
/// `PART_NAME` in screens/parts_browser.rs
pub(crate) const PART_NAME: u32 = 0xD28B_9316;
/// `PART_PRICE` in screens/parts_browser.rs
pub(crate) const PART_PRICE: u32 = 0x0380_B632;
/// `PART_TEXTURE` in screens/parts_browser.rs
pub(crate) const PART_TEXTURE: u32 = 0x73C4_EC2D;
/// table at screens/parts_browser.rs:8 ("Rear Bumpers")
pub(crate) const REAR_BUMPERS: u32 = 0x015C_767C;
/// table at screens/parts_browser.rs:17 ("Rims")
pub(crate) const RIMS: u32 = 0x0294_D2A3;
/// table at screens/parts_browser.rs:57 ("Avus")
pub(crate) const RIM_AVUS_COL1: u32 = 0x20ED_AD1E;
/// table at screens/parts_browser.rs:57 ("Avus")
pub(crate) const RIM_AVUS_COL2: u32 = 0xD2E7_035E;
/// table at screens/parts_browser.rs:58 ("BBS")
pub(crate) const RIM_BBS_COL0: u32 = 0x0000_9536;
/// table at screens/parts_browser.rs:58 ("BBS")
pub(crate) const RIM_BBS_COL1: u32 = 0xF224_AB29;
/// table at screens/parts_browser.rs:58 ("BBS")
pub(crate) const RIM_BBS_COL2: u32 = 0x6B3D_4F56;
/// table at screens/parts_browser.rs:75 ("Donz")
pub(crate) const RIM_DONZ_COL1: u32 = 0x20EF_339A;
/// table at screens/parts_browser.rs:75 ("Donz")
pub(crate) const RIM_DONZ_COL2: u32 = 0xD2E8_89DA;
/// table at screens/parts_browser.rs:59 ("Enkei")
pub(crate) const RIM_ENKEI_COL1: u32 = 0x0E38_DE9E;
/// table at screens/parts_browser.rs:59 ("Enkei")
pub(crate) const RIM_ENKEI_COL2: u32 = 0x300B_420B;
/// table at screens/parts_browser.rs:76 ("Foxx")
pub(crate) const RIM_FOXX_COL1: u32 = 0x20F0_4DA4;
/// table at screens/parts_browser.rs:76 ("Foxx")
pub(crate) const RIM_FOXX_COL2: u32 = 0xD2E9_A3E4;
/// table at screens/parts_browser.rs:60 ("Gianelle")
pub(crate) const RIM_GIANELLE_COL1: u32 = 0xBE39_9DF3;
/// table at screens/parts_browser.rs:60 ("Gianelle")
pub(crate) const RIM_GIANELLE_COL2: u32 = 0x9D50_E640;
/// table at screens/parts_browser.rs:78 ("Giovanna")
pub(crate) const RIM_GIOVANNA_COL1: u32 = 0x9F4D_4412;
/// table at screens/parts_browser.rs:78 ("Giovanna")
pub(crate) const RIM_GIOVANNA_COL2: u32 = 0xBE87_BA52;
/// table at screens/parts_browser.rs:79 ("Kaizer")
pub(crate) const RIM_KAIZER_COL1: u32 = 0x28DE_92C5;
/// table at screens/parts_browser.rs:79 ("Kaizer")
pub(crate) const RIM_KAIZER_COL2: u32 = 0x3E86_7905;
/// table at screens/parts_browser.rs:61 ("Konig")
pub(crate) const RIM_KONIG_COL1: u32 = 0x0EA6_0B4A;
/// table at screens/parts_browser.rs:61 ("Konig")
pub(crate) const RIM_KONIG_COL2: u32 = 0x3078_6EB7;
/// table at screens/parts_browser.rs:62 ("Lexani")
pub(crate) const RIM_LEXANI_COL1: u32 = 0xE50D_0313;
/// table at screens/parts_browser.rs:62 ("Lexani")
pub(crate) const RIM_LEXANI_COL2: u32 = 0x412B_D420;
/// table at screens/parts_browser.rs:63 ("Lowenhart")
pub(crate) const RIM_LOWENHART_COL1: u32 = 0x9A00_EB73;
/// table at screens/parts_browser.rs:63 ("Lowenhart")
pub(crate) const RIM_LOWENHART_COL2: u32 = 0xA08A_29B3;
/// table at screens/parts_browser.rs:64 ("Momo")
pub(crate) const RIM_MOMO_COL1: u32 = 0x36C0_4F4A;
/// table at screens/parts_browser.rs:64 ("Momo")
pub(crate) const RIM_MOMO_COL2: u32 = 0xD2ED_7917;
/// table at screens/parts_browser.rs:66 ("NFSU 2")
pub(crate) const RIM_NFSU_2_COL1: u32 = 0x3F85_C14D;
/// table at screens/parts_browser.rs:66 ("NFSU 2")
pub(crate) const RIM_NFSU_2_COL2: u32 = 0xD2ED_DFFB;
/// table at screens/parts_browser.rs:56 ("5Zigen")
pub(crate) const RIM_N_5ZIGEN_COL0: u32 = 0x352D_08D1;
/// table at screens/parts_browser.rs:56 ("5Zigen")
pub(crate) const RIM_N_5ZIGEN_COL1: u32 = 0xF751_28B1;
/// table at screens/parts_browser.rs:56 ("5Zigen")
pub(crate) const RIM_N_5ZIGEN_COL2: u32 = 0x0CF9_0EF1;
/// table at screens/parts_browser.rs:65 ("O.Z.")
pub(crate) const RIM_O_Z_COL0: u32 = 0x0000_0648;
/// table at screens/parts_browser.rs:65 ("O.Z.")
pub(crate) const RIM_O_Z_COL1: u32 = 0x6C2F_A9DB;
/// table at screens/parts_browser.rs:65 ("O.Z.")
pub(crate) const RIM_O_Z_COL2: u32 = 0xFB7D_FC68;
/// table at screens/parts_browser.rs:67 ("Racing Hart")
pub(crate) const RIM_RACING_HART_COL0: u32 = 0xDD54_4A02;
/// table at screens/parts_browser.rs:67 ("Racing Hart")
pub(crate) const RIM_RACING_HART_COL1: u32 = 0x27EB_D095;
/// table at screens/parts_browser.rs:67 ("Racing Hart")
pub(crate) const RIM_RACING_HART_COL2: u32 = 0x29F6_6022;
/// table at screens/parts_browser.rs:68 ("Rota")
pub(crate) const RIM_ROTA_COL1: u32 = 0x20F6_E195;
/// table at screens/parts_browser.rs:68 ("Rota")
pub(crate) const RIM_ROTA_COL2: u32 = 0xD2F0_37D5;
/// table at screens/parts_browser.rs:71 ("Stock")
pub(crate) const RIM_STOCK_COL0: u32 = 0x0F39_90B6;
/// table at screens/parts_browser.rs:71 ("Stock")
pub(crate) const RIM_STOCK_COL1: u32 = 0x310B_F423;
/// table at screens/parts_browser.rs:82 ("StreetSpin")
pub(crate) const RIM_STREETSPIN_COL0: u32 = 0x029E_75D1;
/// table at screens/parts_browser.rs:82 ("StreetSpin")
pub(crate) const RIM_STREETSPIN_COL1: u32 = 0x8B71_EBF0;
/// table at screens/parts_browser.rs:82 ("StreetSpin")
pub(crate) const RIM_STREETSPIN_COL2: u32 = 0x6322_F230;
/// table at screens/parts_browser.rs:69 ("Volk")
pub(crate) const RIM_VOLK_COL1: u32 = 0x20F9_121B;
/// table at screens/parts_browser.rs:69 ("Volk")
pub(crate) const RIM_VOLK_COL2: u32 = 0xD2F2_685B;
/// table at screens/parts_browser.rs:83 ("Weld")
pub(crate) const RIM_WELD_COL0: u32 = 0xBF36_08C0;
/// table at screens/parts_browser.rs:83 ("Weld")
pub(crate) const RIM_WELD_COL1: u32 = 0xF6CA_43BE;
/// table at screens/parts_browser.rs:83 ("Weld")
pub(crate) const RIM_WELD_COL2: u32 = 0xC49C_11FE;
/// table at screens/parts_browser.rs:70 ("Work")
pub(crate) const RIM_WORK_COL1: u32 = 0x20F9_9F42;
/// table at screens/parts_browser.rs:70 ("Work")
pub(crate) const RIM_WORK_COL2: u32 = 0xD2F2_F582;
/// table at screens/parts_browser.rs:12 ("Roof Scoops")
pub(crate) const ROOF_SCOOPS: u32 = 0x7916_5861;
/// `fe_hash("Rota")`
pub(crate) const ROTA: u32 = 0x001C_39B5;
/// `fe_hash("SideScrollBar_Arrow_1")`
pub(crate) const SIDESCROLLBAR_ARROW_1: u32 = 0x640F_0F22;
/// `fe_hash("SideScrollBar_Backing")`
pub(crate) const SIDESCROLLBAR_BACKING: u32 = 0x884F_BA76;
/// `fe_hash("SideScrollBar_Handle")`
pub(crate) const SIDESCROLLBAR_HANDLE: u32 = 0xF31E_8ED3;
/// table at screens/parts_browser.rs:15 ("Side Mirrors")
pub(crate) const SIDE_MIRRORS_ICON: u32 = 0xC83D_E19A;
/// table at screens/parts_browser.rs:15 ("Side Mirrors")
pub(crate) const SIDE_MIRRORS_LABEL: u32 = 0x0DD3_5467;
/// table at screens/parts_browser.rs:9 ("Side Skirts")
pub(crate) const SIDE_SKIRTS: u32 = 0x4E6C_2AF9;
/// table at screens/parts_browser.rs:10 ("Spoilers")
pub(crate) const SPOILERS: u32 = 0xBB03_4EA6;
/// `STOCK_RIMS` in screens/parts_browser.rs
pub(crate) const STOCK_RIMS: u32 = 0x03B8_3203;
/// table at screens/parts_browser.rs:14 ("Taillights")
pub(crate) const TAILLIGHTS_ICON: u32 = 0x2CBB_2D1C;
/// table at screens/parts_browser.rs:14 ("Taillights")
pub(crate) const TAILLIGHTS_LABEL: u32 = 0x6634_37A9;
/// `TOTAL_COST` in screens/parts_browser.rs
pub(crate) const TOTAL_COST: u32 = 0x9FF8_4A88;
/// `fe_hash("U2_mini_icon_backing")`
pub(crate) const U2_MINI_ICON_BACKING: u32 = 0xD82B_4A08;
/// `fe_hash("Volk")`
pub(crate) const VOLK: u32 = 0x001E_6A3B;
/// table at screens/parts_browser.rs:19 ("Wide Body Kits")
pub(crate) const WIDE_BODY_KITS: u32 = 0x945E_9BC5;
/// `fe_hash("Work")`
pub(crate) const WORK: u32 = 0x001E_F762;

// ---- screens/perf_parts.rs
/// `BHP` in screens/perf_parts.rs
pub(crate) const BHP: u32 = 0xCBBC_6512;
/// `FT_LBS` in screens/perf_parts.rs
pub(crate) const FT_LBS: u32 = 0xACB8_DFF3;
/// `INSTALLED` in screens/perf_parts.rs
pub(crate) const INSTALLED: u32 = 0xED56_BE1F;
/// `KW` in screens/perf_parts.rs
pub(crate) const KW: u32 = 0x5B81_D5BA;
/// `NM` in screens/perf_parts.rs
pub(crate) const NM: u32 = 0x5B81_D613;
/// `NONE` in screens/perf_parts.rs
pub(crate) const NONE: u32 = 0x001A_076F;
/// `NOT_INSTALLED` in screens/perf_parts.rs
pub(crate) const NOT_INSTALLED: u32 = 0x4287_206F;
/// `OWNED` in screens/perf_parts.rs
pub(crate) const OWNED: u32 = 0x0371_711C;
/// `POWER` in screens/perf_parts.rs
pub(crate) const POWER: u32 = 0x6BB3_2446;
/// `TORQUE` in screens/perf_parts.rs
pub(crate) const TORQUE: u32 = 0x1853_1358;

// ---- screens/post_race.rs
/// `fe_hash("HEADER_TEXT_3")`
pub(crate) const HEADER_TEXT_3: u32 = 0xA4EC_03BE;
/// `NO_LAP_TIME` in screens/post_race.rs
pub(crate) const NO_LAP_TIME: u32 = 0x61C9_B7A2;
/// role not stated where it is used (screens/post_race.rs:85); needs the binary
pub(crate) const OBJ_C03F9F19: u32 = 0xC03F_9F19;
/// `POSITION_MASTER` in screens/post_race.rs
pub(crate) const POSITION_MASTER: u32 = 0x154A_2AFF;
/// `TIME_MASTER` in screens/post_race.rs
pub(crate) const TIME_MASTER: u32 = 0xB8E8_CB19;

// ---- screens/qr_track.rs
/// role not stated where it is used (screens/qr_track.rs:192); needs the binary
pub(crate) const OBJ_DB0C5EFD: u32 = 0xDB0C_5EFD;
/// text at screens/qr_track.rs:193
pub(crate) const QR_TRACK_TXT_BEST_LAP_1ECB: u32 = 0x3B4A_1ECB;
/// text at screens/qr_track.rs:191
pub(crate) const QR_TRACK_TXT_BEST_LAP_9567: u32 = 0x3B7B_9567;

// ---- screens/shop_tables.rs
/// table at screens/shop_tables.rs:8 ("Custom Gauges")
pub(crate) const CUSTOM_GAUGES_ICON: u32 = 0xC591_C4B6;
/// table at screens/shop_tables.rs:8 ("Custom Gauges")
pub(crate) const CUSTOM_GAUGES_LABEL: u32 = 0x1EB1_04C8;
/// table at screens/shop_tables.rs:17 ("Doors")
pub(crate) const DOORS_ICON: u32 = 0xB279_8BB4;
/// table at screens/shop_tables.rs:17 ("Doors")
pub(crate) const DOORS_LABEL: u32 = 0x2789_1593;
/// table at screens/shop_tables.rs:48 ("Engine Neon")
pub(crate) const ENGINE_NEON_ICON: u32 = 0x05D4_1FAC;
/// table at screens/shop_tables.rs:48 ("Engine Neon")
pub(crate) const ENGINE_NEON_LABEL: u32 = 0x437E_4111;
/// table at screens/shop_tables.rs:13 ("Hydraulics")
pub(crate) const HYDRAULICS_ICON: u32 = 0x0C9E_0D00;
/// table at screens/shop_tables.rs:13 ("Hydraulics")
pub(crate) const HYDRAULICS_LABEL: u32 = 0xEAC7_4624;
/// table at screens/shop_tables.rs:11 ("Lights")
pub(crate) const LIGHTS_ICON: u32 = 0x599C_870F;
/// table at screens/shop_tables.rs:11 ("Lights")
pub(crate) const LIGHTS_LABEL: u32 = 0xEF66_79C9;
/// table at screens/shop_tables.rs:12 ("Nitrous Purge")
pub(crate) const NITROUS_PURGE_ICON: u32 = 0xBDED_289A;
/// table at screens/shop_tables.rs:12 ("Nitrous Purge")
pub(crate) const NITROUS_PURGE_LABEL: u32 = 0x2981_CD3E;
/// table at screens/shop_tables.rs:15 ("Spinners")
pub(crate) const SHOP_TABLES_SPINNERS_LABEL: u32 = 0x094A_635E;
/// seq at screens/shop_tables.rs:14 ("Trunk audio: opens its menu")
pub(crate) const SPECIALTIES_12: u32 = 0xE803_E1CD;
/// seq at screens/shop_tables.rs:14 ("Trunk audio: opens its menu")
pub(crate) const SPECIALTIES_13: u32 = 0x8B54_3F71;
/// seq at screens/shop_tables.rs:9 ("Neon: opens UI_CustomNeonMain")
pub(crate) const SPECIALTIES_2: u32 = 0x0292_9058;
/// seq at screens/shop_tables.rs:9 ("Neon: opens UI_CustomNeonMain")
pub(crate) const SPECIALTIES_3: u32 = 0xA420_BE7C;
/// table at screens/shop_tables.rs:15 ("Spinners")
pub(crate) const SPINNERS_ICON: u32 = 0x0DC9_C33A;
/// table at screens/shop_tables.rs:16 ("Split Hoods")
pub(crate) const SPLIT_HOODS_ICON: u32 = 0x5A30_9B10;
/// table at screens/shop_tables.rs:16 ("Split Hoods")
pub(crate) const SPLIT_HOODS_LABEL: u32 = 0xA41D_9EB6;
/// table at screens/shop_tables.rs:49 ("Trunk Neon")
pub(crate) const TRUNK_NEON_ICON: u32 = 0x759C_2BCA;
/// table at screens/shop_tables.rs:49 ("Trunk Neon")
pub(crate) const TRUNK_NEON_LABEL: u32 = 0x2225_348F;
/// table at screens/shop_tables.rs:47 ("Underglow Neon")
pub(crate) const UNDERGLOW_NEON_ICON: u32 = 0x745E_E753;
/// table at screens/shop_tables.rs:47 ("Underglow Neon")
pub(crate) const UNDERGLOW_NEON_LABEL: u32 = 0xD8EA_6D2F;
/// table at screens/shop_tables.rs:10 ("Window Tint")
pub(crate) const WINDOW_TINT_ICON: u32 = 0x3F23_165C;
/// table at screens/shop_tables.rs:10 ("Window Tint")
pub(crate) const WINDOW_TINT_LABEL: u32 = 0xA424_19AB;

// ---- screens/sms_mailbox.rs
/// `DOWN` in screens/sms_mailbox.rs
pub(crate) const DOWN: u32 = 0x20D7_3D01;
/// `DELETE` in screens/sms_mailbox.rs
pub(crate) const MAILBOX_OBJECTS_DELETE: u32 = 0x31CF_7EC2;
/// `TITLE` in screens/sms_mailbox.rs
pub(crate) const MAILBOX_OBJECTS_TITLE: u32 = 0x9808_22B3;
/// `READ` in screens/sms_mailbox.rs
pub(crate) const READ: u32 = 0x7D8C_9EE7;
/// `UP` in screens/sms_mailbox.rs
pub(crate) const UP: u32 = 0xEDF2_E4EE;

// ---- screens/splash.rs
/// `fe_hash("start_click")`
pub(crate) const START_CLICK: u32 = 0xC4DF_3FF2;

// ---- screens/status.rs
/// role not stated where it is used (screens/status.rs:246); needs the binary
pub(crate) const OBJ_02A9AFED: u32 = 0x02A9_AFED;
/// role not stated where it is used (screens/status.rs:216); needs the binary
pub(crate) const OBJ_317AD6F2: u32 = 0x317A_D6F2;
/// role not stated where it is used (screens/status.rs:216); needs the binary
pub(crate) const OBJ_317AD6F3: u32 = 0x317A_D6F3;
/// role not stated where it is used (screens/status.rs:223); needs the binary
pub(crate) const OBJ_317AD6F4: u32 = 0x317A_D6F4;
/// role not stated where it is used (screens/status.rs:224); needs the binary
pub(crate) const OBJ_317AD6F5: u32 = 0x317A_D6F5;
/// role not stated where it is used (screens/status.rs:225); needs the binary
pub(crate) const OBJ_317AD6F6: u32 = 0x317A_D6F6;
/// role not stated where it is used (screens/status.rs:257); needs the binary
pub(crate) const OBJ_42E73D71: u32 = 0x42E7_3D71;
/// role not stated where it is used (screens/status.rs:214); needs the binary
pub(crate) const OBJ_4D2F5C61: u32 = 0x4D2F_5C61;
/// role not stated where it is used (screens/status.rs:215); needs the binary
pub(crate) const OBJ_4D2F5C62: u32 = 0x4D2F_5C62;
/// role not stated where it is used (screens/status.rs:230); needs the binary
pub(crate) const OBJ_4D2F5C63: u32 = 0x4D2F_5C63;
/// `fe_hash("Slider_base")`
pub(crate) const SLIDER_BASE: u32 = 0xBDCD_0D7C;
/// `fe_hash("Slider_Beg")`
pub(crate) const SLIDER_BEG: u32 = 0xD734_C2CF;
/// `fe_hash("Slider_End")`
pub(crate) const SLIDER_END: u32 = 0xD734_D0B8;

// ---- screens/tuning.rs
/// `fe_hash("Front")`
pub(crate) const FRONT: u32 = 0x02CB_DC28;
/// seq at screens/tuning.rs:41
pub(crate) const LOW_0: u32 = 0x2603_3125;
/// seq at screens/tuning.rs:41
pub(crate) const LOW_1: u32 = 0xE667_08F3;
/// role not stated where it is used (screens/tuning.rs:460); needs the binary
pub(crate) const OBJ_01183698: u32 = 0x0118_3698;
/// role not stated where it is used (screens/tuning.rs:459); needs the binary
pub(crate) const OBJ_D35A7923: u32 = 0xD35A_7923;
/// `fe_hash("Rear")`
pub(crate) const REAR: u32 = 0x001C_0CC9;
/// seq at screens/tuning.rs:42
pub(crate) const SOFT_0: u32 = 0xE66D_2A8F;
/// seq at screens/tuning.rs:42
pub(crate) const SOFT_1: u32 = 0xE666_E852;
/// comment at screens/tuning.rs:289
pub(crate) const TIP: u32 = 0xCBBC_B1C5;
/// seq at screens/tuning.rs:7
pub(crate) const TUNING_NAMES_0: u32 = 0xFE22_B6A2;
/// seq at screens/tuning.rs:8
pub(crate) const TUNING_NAMES_1: u32 = 0xEFDB_2567;
/// seq at screens/tuning.rs:17
pub(crate) const TUNING_NAMES_10: u32 = 0x7D82_2B64;
/// seq at screens/tuning.rs:18
pub(crate) const TUNING_NAMES_11: u32 = 0x7D82_2B65;
/// seq at screens/tuning.rs:19
pub(crate) const TUNING_NAMES_12: u32 = 0x7D82_2B66;
/// seq at screens/tuning.rs:20
pub(crate) const TUNING_NAMES_13: u32 = 0x7D82_2B67;
/// seq at screens/tuning.rs:21
pub(crate) const TUNING_NAMES_14: u32 = 0x7D82_2B68;
/// seq at screens/tuning.rs:22
pub(crate) const TUNING_NAMES_15: u32 = 0x7D82_2B69;
/// seq at screens/tuning.rs:23
pub(crate) const TUNING_NAMES_16: u32 = 0x55C6_0293;
/// seq at screens/tuning.rs:24
pub(crate) const TUNING_NAMES_17: u32 = 0xFBBD_D223;
/// seq at screens/tuning.rs:25
pub(crate) const TUNING_NAMES_18: u32 = 0x2308_F06C;
/// seq at screens/tuning.rs:26
pub(crate) const TUNING_NAMES_19: u32 = 0x3988_FAEF;
/// seq at screens/tuning.rs:9
pub(crate) const TUNING_NAMES_2: u32 = 0xB6D4_44B4;
/// seq at screens/tuning.rs:27
pub(crate) const TUNING_NAMES_20: u32 = 0xE4EE_BD70;
/// seq at screens/tuning.rs:28
pub(crate) const TUNING_NAMES_21: u32 = 0xD99F_C2E4;
/// seq at screens/tuning.rs:29
pub(crate) const TUNING_NAMES_22: u32 = 0x21D8_3E05;
/// seq at screens/tuning.rs:30
pub(crate) const TUNING_NAMES_23: u32 = 0xC8B8_FBF1;
/// seq at screens/tuning.rs:31
pub(crate) const TUNING_NAMES_24: u32 = 0xFE82_FDE0;
/// seq at screens/tuning.rs:10
pub(crate) const TUNING_NAMES_3: u32 = 0x0B16_1EC3;
/// seq at screens/tuning.rs:11
pub(crate) const TUNING_NAMES_4: u32 = 0x93B7_D1A3;
/// seq at screens/tuning.rs:12
pub(crate) const TUNING_NAMES_5: u32 = 0x87C8_6448;
/// seq at screens/tuning.rs:13
pub(crate) const TUNING_NAMES_6: u32 = 0xFF0C_BFD5;
/// seq at screens/tuning.rs:14
pub(crate) const TUNING_NAMES_7: u32 = 0x534E_99E4;
/// seq at screens/tuning.rs:15
pub(crate) const TUNING_NAMES_8: u32 = 0x2430_E6BE;
/// seq at screens/tuning.rs:16
pub(crate) const TUNING_NAMES_9: u32 = 0x6A71_E798;
/// seq at screens/tuning.rs:43
pub(crate) const WEAK_0: u32 = 0xE66F_30DB;
/// seq at screens/tuning.rs:43
pub(crate) const WEAK_1: u32 = 0x36C3_0E70;

// ---- screens/world_map.rs
/// `COMPLETED` in screens/world_map.rs
pub(crate) const COMPLETED: u32 = 0xB1EE_4CA9;
/// role not stated where it is used (screens/world_map.rs:183); needs the binary
pub(crate) const OBJ_4D2F5C64: u32 = 0x4D2F_5C64;
/// `STAGE` in screens/world_map.rs
pub(crate) const STAGE: u32 = 0xC8E9_7C9C;
/// `STATUS` in screens/world_map.rs
pub(crate) const STATUS: u32 = 0x23EA_E8B9;
/// `TIME` in screens/world_map.rs
pub(crate) const TIME: u32 = 0x23EA_E8BC;
/// `DISTANCE` in screens/world_map.rs
pub(crate) const WORLD_MAP_DISTANCE: u32 = 0x23EA_E8BB;
/// `TYPE` in screens/world_map.rs
pub(crate) const WORLD_MAP_TYPE: u32 = 0x23EA_E8BA;

// ---- widgets/help_bar.rs
/// comment at widgets/help_bar.rs:312 ("Cancel Changes")
pub(crate) const CANCEL_CHANGES: u32 = 0x0181_27BD;
/// comment at widgets/help_bar.rs:326 ("Decal Color")
pub(crate) const DECAL_COLOR: u32 = 0x889B_ACB6;
/// comment at widgets/help_bar.rs:314 ("Delete Tuned Car")
pub(crate) const DELETE_TUNED_CAR: u32 = 0xEFDD_C126;
/// comment at widgets/help_bar.rs:319 ("EA Messenger")
pub(crate) const EA_MESSENGER: u32 = 0xD0EF_BEA3;
/// `fe_hash("Event_Results")`
pub(crate) const EVENT_RESULTS: u32 = 0xFCC0_20D2;
/// `FADE_IN` in widgets/help_bar.rs
pub(crate) const FADE_IN: u32 = 0xBCC0_0F05;
/// `FADE_OUT` in widgets/help_bar.rs
pub(crate) const FADE_OUT: u32 = 0x54C2_0A66;
/// comment at widgets/help_bar.rs:313 ("Back")
pub(crate) const HELP_BAR_BACK: u32 = 0x1C07_87E6;
/// comment at widgets/help_bar.rs:317 ("Customize")
pub(crate) const HELP_BAR_CUSTOMIZE: u32 = 0x8D5A_BF58;
/// comment at widgets/help_bar.rs:318 ("Delete")
pub(crate) const HELP_BAR_DELETE: u32 = 0x4100_ADA8;
/// comment at widgets/help_bar.rs:307 ("Hide on World Map")
pub(crate) const HIDE_ON_WORLD_MAP: u32 = 0x9DB5_4A80;
/// comment at widgets/help_bar.rs:298 ("Install Decal")
pub(crate) const INSTALL_DECAL: u32 = 0x5E0A_B884;
/// comment at widgets/help_bar.rs:303 ("Install Package")
pub(crate) const INSTALL_PACKAGE: u32 = 0x9E06_7677;
/// comment at widgets/help_bar.rs:304 ("Install paint")
pub(crate) const INSTALL_PAINT: u32 = 0x5EE1_C847;
/// comment at widgets/help_bar.rs:302 ("Install Part")
pub(crate) const INSTALL_PART: u32 = 0x7EFF_16C2;
/// comment at widgets/help_bar.rs:301 ("Install Vinyl")
pub(crate) const INSTALL_VINYL: u32 = 0x5F52_D4FD;
/// `fe_hash("In_Game")`
pub(crate) const IN_GAME: u32 = 0xC3EB_04AF;
/// comment at widgets/help_bar.rs:299 ("Logoff")
pub(crate) const LOGOFF: u32 = 0x545C_5292;
/// `OL_ICON_GROUP` in widgets/help_bar.rs
pub(crate) const OL_ICON_GROUP: u32 = 0x2BAC_0CEE;
/// comment at widgets/help_bar.rs:322 ("Open/Close Doors")
pub(crate) const OPEN_CLOSE_DOORS: u32 = 0x3E94_31DC;
/// comment at widgets/help_bar.rs:321 ("Open/Close Hood")
pub(crate) const OPEN_CLOSE_HOOD: u32 = 0xEAA1_D5DF;
/// `fe_hash("Pause_Menu_Main")`
pub(crate) const PAUSE_MENU_MAIN: u32 = 0xA95D_B9D5;
/// `fe_hash("Pause_Options")`
pub(crate) const PAUSE_OPTIONS: u32 = 0x2B5A_03A8;
/// comment at widgets/help_bar.rs:300 ("Quit Game")
pub(crate) const QUIT_GAME: u32 = 0x1C10_1778;
/// comment at widgets/help_bar.rs:305 ("Read Message")
pub(crate) const READ_MESSAGE: u32 = 0xE24B_4BD5;
/// `REAL_GROUP` in widgets/help_bar.rs
pub(crate) const REAL_GROUP: u32 = 0x3B94_9CD0;
/// comment at widgets/help_bar.rs:316 ("Reset Keys To Default")
pub(crate) const RESET_KEYS_TO_DEFAULT: u32 = 0xB7D0_D273;
/// comment at widgets/help_bar.rs:306 ("Select Event")
pub(crate) const SELECT_EVENT: u32 = 0xDAAC_95F9;
/// comment at widgets/help_bar.rs:308 ("Show on World Map")
pub(crate) const SHOW_ON_WORLD_MAP: u32 = 0xC6B7_EB02;
/// comment at widgets/help_bar.rs:320 ("Test N2O Purge")
pub(crate) const TEST_N2O_PURGE: u32 = 0xBA0E_1020;

// ---- widgets/rating.rs
/// role not stated where it is used (widgets/rating.rs:168); needs the binary
pub(crate) const OBJ_171D00CB: u32 = 0x171D_00CB;
/// role not stated where it is used (widgets/rating.rs:90); needs the binary
pub(crate) const OBJ_6EC2F0DE: u32 = 0x6EC2_F0DE;

// ---- widgets/stat_bar.rs
/// seq at widgets/stat_bar.rs:27
pub(crate) const PACKAGE_BARS_0: u32 = 0x7018_7FE5;
/// seq at widgets/stat_bar.rs:28
pub(crate) const PACKAGE_BARS_1: u32 = 0xDB1C_E262;
/// seq at widgets/stat_bar.rs:41
pub(crate) const PACKAGE_BARS_10: u32 = 0xAEEE_7572;
/// seq at widgets/stat_bar.rs:42
pub(crate) const PACKAGE_BARS_11: u32 = 0x1B13_DA4F;
/// seq at widgets/stat_bar.rs:43
pub(crate) const PACKAGE_BARS_12: u32 = 0xB44D_30CD;
/// seq at widgets/stat_bar.rs:44
pub(crate) const PACKAGE_BARS_13: u32 = 0x4FF6_6E8D;
/// seq at widgets/stat_bar.rs:45
pub(crate) const PACKAGE_BARS_14: u32 = 0x0C6F_B2C5;
/// seq at widgets/stat_bar.rs:29
pub(crate) const PACKAGE_BARS_2: u32 = 0x7577_3B40;
/// seq at widgets/stat_bar.rs:30
pub(crate) const PACKAGE_BARS_3: u32 = 0x30CF_3780;
/// seq at widgets/stat_bar.rs:31
pub(crate) const PACKAGE_BARS_4: u32 = 0xCD99_BD38;
/// seq at widgets/stat_bar.rs:34
pub(crate) const PACKAGE_BARS_5: u32 = 0x2C2F_6AB0;
/// seq at widgets/stat_bar.rs:35
pub(crate) const PACKAGE_BARS_6: u32 = 0xD14C_1FCD;
/// seq at widgets/stat_bar.rs:36
pub(crate) const PACKAGE_BARS_7: u32 = 0x318E_260B;
/// seq at widgets/stat_bar.rs:37
pub(crate) const PACKAGE_BARS_8: u32 = 0x1642_74CB;
/// seq at widgets/stat_bar.rs:38
pub(crate) const PACKAGE_BARS_9: u32 = 0x89B0_A803;
/// seq at widgets/stat_bar.rs:51
pub(crate) const PART_BARS_0: u32 = 0x0AED_16ED;
/// seq at widgets/stat_bar.rs:51
pub(crate) const PART_BARS_1: u32 = 0xDEB9_6FDC;
/// seq at widgets/stat_bar.rs:51
pub(crate) const PART_BARS_2: u32 = 0x44C7_C775;
/// seq at widgets/stat_bar.rs:51
pub(crate) const PART_BARS_3: u32 = 0xE433_ADB4;
/// seq at widgets/stat_bar.rs:52
pub(crate) const PART_BARS_4: u32 = 0x2B95_A91B;
/// seq at widgets/stat_bar.rs:52
pub(crate) const PART_BARS_5: u32 = 0x1474_47CA;
/// seq at widgets/stat_bar.rs:52
pub(crate) const PART_BARS_6: u32 = 0xCEA9_C0A3;
/// seq at widgets/stat_bar.rs:52
pub(crate) const PART_BARS_7: u32 = 0x6E15_A6E2;

/// How a name hashes: the front end's object names are upper-cased first
/// (`fe_hash`), file and texture names are not (`bin_hash`).
#[derive(Clone, Copy)]
enum Hash {
    Fe,
    Bin,
}

/// Every constant whose name (or the string the code hashed next to it) is
/// known, with the name it is the hash of.
const NAMED: &[(u32, &str, Hash)] = &[
    (AVUS, "Avus", Hash::Fe),
    (CAR_LOT_CUSTOMIZE, "Customize", Hash::Fe),
    (CONTINUE, "Continue", Hash::Fe),
    (CREATE, "Create", Hash::Fe),
    (DISABLED, "Disabled", Hash::Fe),
    (DONZ, "Donz", Hash::Fe),
    (ENKEI, "Enkei", Hash::Fe),
    (EVENT_RESULTS, "Event_Results", Hash::Fe),
    (FILTERTYPE, "FILTERTYPE", Hash::Fe),
    (FOXX, "Foxx", Hash::Fe),
    (FRONT, "Front", Hash::Fe),
    (GIANELLE, "Gianelle", Hash::Fe),
    (GIOVANNA, "Giovanna", Hash::Fe),
    (GRAB_ARROW_1, "Grab_Arrow_1", Hash::Fe),
    (HEADER_TEXT_3, "HEADER_TEXT_3", Hash::Fe),
    (INSTALL, "INSTALL", Hash::Fe),
    (INSTRUCTIONS_LABEL, "Instructions_Label", Hash::Fe),
    (IN_GAME, "In_Game", Hash::Fe),
    (KAIZER, "Kaizer", Hash::Fe),
    (KONIG, "Konig", Hash::Fe),
    (LEXANI, "Lexani", Hash::Fe),
    (LOAD, "Load", Hash::Fe),
    (LOWENHART, "Lowenhart", Hash::Fe),
    (MAGAZINECHICK, "MagazineChick", Hash::Fe),
    (MAGAZINEEXTRA, "MagazineExtra", Hash::Fe),
    (MAGAZINEMAST, "MagazineMast", Hash::Fe),
    (MAIN_ICON_CAREER, "MAIN_ICON_CAREER", Hash::Fe),
    (MAIN_ICON_CUSTOMIZE_CAR, "MAIN_ICON_CUSTOMIZE_CAR", Hash::Fe),
    (MAIN_ICON_GO_ONLINE, "MAIN_ICON_GO_ONLINE", Hash::Fe),
    (MAIN_ICON_LAN, "MAIN_ICON_LAN", Hash::Fe),
    (MAIN_ICON_OPTIONS, "MAIN_ICON_OPTIONS", Hash::Fe),
    (MAIN_ICON_QUICK_RACE, "MAIN_ICON_QUICK_RACE", Hash::Fe),
    (MEMCARD_DELETE, "Delete", Hash::Fe),
    (MOMO, "Momo", Hash::Fe),
    (MUFFLER, "Muffler", Hash::Fe),
    (NFSU, "NFSU", Hash::Fe),
    (ONLINE_CANCEL, "Cancel", Hash::Fe),
    (ONLINE_TITLE, "TITLE", Hash::Fe),
    (PAINT_ICON_1, "PAINT_ICON_1", Hash::Fe),
    (PAINT_TEXT, "Paint_text", Hash::Fe),
    (PAUSE_MENU_MAIN, "Pause_Menu_Main", Hash::Fe),
    (PAUSE_OPTIONS, "Pause_Options", Hash::Fe),
    (REAR, "Rear", Hash::Fe),
    (RETRY, "Retry", Hash::Fe),
    (ROTA, "Rota", Hash::Fe),
    (SAVE, "Save", Hash::Fe),
    (SCREEN_PLAYER, "PLAYER", Hash::Fe),
    (SIDESCROLLBAR_ARROW_1, "SideScrollBar_Arrow_1", Hash::Fe),
    (SIDESCROLLBAR_BACKING, "SideScrollBar_Backing", Hash::Fe),
    (SIDESCROLLBAR_HANDLE, "SideScrollBar_Handle", Hash::Fe),
    (SLIDER_BASE, "Slider_base", Hash::Fe),
    (SLIDER_BEG, "Slider_Beg", Hash::Fe),
    (SLIDER_END, "Slider_End", Hash::Fe),
    (START_CLICK, "start_click", Hash::Fe),
    (U2_MINI_ICON_BACKING, "U2_mini_icon_backing", Hash::Fe),
    (VOLK, "Volk", Hash::Fe),
    (WORK, "Work", Hash::Fe),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::hash::{bin_hash, fe_hash};

    #[test]
    fn constants_match_their_names() {
        assert!(!NAMED.is_empty());
        for &(value, name, kind) in NAMED {
            let hash = match kind {
                Hash::Fe => fe_hash(name),
                Hash::Bin => bin_hash(name),
            };
            assert_eq!(value, hash, "{name}");
        }
    }
}

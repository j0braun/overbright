//! The screens' code: what each package shows and what its buttons do.
//!
//! The packages hold the layout; the game's C++ fills in the dynamic
//! parts (option icons and labels, car names, event details). Option
//! lists are the game's own, read from the code that builds them
//! (0x4AEE30 main menu, 0x4AF150 career, 0x4AF840 crib): each option is
//! `0x51F680(icon texture hash, label hash, 0)`.

use crate::frontend::fe_ids;
use glam::Vec2;

use super::quick_race;
use super::career::{CareerData, Event, Profile};
use super::{Assets, Place, Quad, ScreenState, layout};
use crate::formats::hash::{bin_hash, fe_hash};

pub mod brand_select;
pub mod sms_mailbox;
pub mod status;
pub mod qr_options;

mod action;
mod input;

pub use action::Action;
pub use super::format::{fixed2, fixed3, signed_time};
pub(crate) use super::format::{fixed_grouped, grouped_1};
pub use input::Key;
pub use super::widgets::arrow_blink::{ARROW_BLINK_MS, ArrowBlink, SCROLLBAR_ARROWS};
pub(crate) use super::widgets::footer::footer;
pub use super::widgets::footer::total_cost;
pub use super::widgets::help_bar::*;
pub use super::widgets::icon_menu::{DIRECT_OPTIONS, FORWARD_SCRIPT, IconMenu, OptionState, accept_plays_forward, forward_ms};
pub(crate) use super::widgets::scroll_bar::scroll_bar_at;
pub(crate) use super::widgets::scroll_bar::scroll_bar_named;
pub use super::widgets::stat_bar::{PACKAGE_BARS, PART_BARS, StatBar, stat_bar};
pub(crate) use super::widgets::footer::retitle;
pub(crate) use super::widgets::icon_scroller::scroller_quads;
pub(crate) use super::widgets::scroll_bar::scroll_bar;

mod menus;
mod world_map;
mod ingame_map;
mod dialog;
mod memcard;
mod qr_track;
mod engage;
#[cfg(test)]
mod engage_wrap_tests;
mod post_race;
mod online;
mod credits;
mod controls;
pub mod jukebox;
mod splash;

pub use menus::{CUSTOMIZE_MENU, IconOption, MAIN_MENU, OPTIONS_MENU, PAUSE_OPTIONS_MENU, QR_MODE_MENU, career_menu, crib_menu, explore_pause_menu, explore_pause_menu_with, post_race_menu, profile_menu, race_pause_menu, race_pause_menu_with, reward_menu};
pub(crate) use menus::{ui_main};
pub use world_map::{MapView, MapWidget, WorldMap, map_window_uv, map_zoom, map_zoom_count, world_map_region, world_map_region_track};
pub use ingame_map::{InGameWorldMap, MAP_ICON_NAMES, MapIcon, WORLD_MAP_HELP, WORLD_MAP_ZOOMS, map_event_kind, map_group, map_label, map_shop_kind, world_map_max_zoom};
pub use dialog::{DIM_HANDLER, DIM_PAINT_SLOT, Dialog, Loading, dialog_package, dialog_size, help_dialog_package};
pub use memcard::{MC_MAIN_GROUPS, McList, McMain, McMessage, McOp, McOutcome, McStep, mc_background_quads, mc_list};
#[cfg(test)]
pub(crate) use memcard::caret_on;
pub use qr_track::{QuickRace};
pub use engage::{ChallengeDialog, ChallengeDialogKind, DialogFocus, Engage, ReplayBar, ShopEngage, challenge_message_range, message_set, replay_control_quads, reward_quads, sponsor_popup_quads};
pub use post_race::{PostRaceResults, PostRaceStats, RacerStats, ResultsKind, race_points, results_behind};
pub(crate) use post_race::{master_box};
pub use online::{EaLogin, isp_connect_quads, lan_servers_quads};
pub use credits::{Credits};
pub use controls::{CONTROL_ROWS, ControlsPage, ControlsScreen, controls_visible};
#[cfg(test)]
use controls::key_cell;
pub use jukebox::{EA_TRAX, Jukebox};
pub use splash::{Splash};

pub(crate) mod parts_browser;
mod shop_tables;
mod paint;
mod magazine;
mod dyno;
mod tuning;
mod perf_parts;
mod car_lot;

pub use parts_browser::{BODY_SHOP_CATEGORIES, BrowserCategory, BrowserKey, BrowserPart, CategoryAction, PartsBrowser, RIM_BRANDS, STOCK_RIMS, SUV_RIM_BRANDS, rim_brand_name};
pub use shop_tables::{NEON, SPECIALTIES, trunk_slots};
pub use paint::{GLOSS, METAL, PAINT_SLOTS, PEARL, PaintKey, PaintShop, Swatch, paint_finish, finish_arrows_shown};
pub use magazine::{CoverView, DVD_ICON, MAGAZINE_ICON};
pub use dyno::{DynoChart, DynoResults, DynoRun, DynoRuns};
pub use tuning::{TUNING_NAMES, TuneGraph, TuneSliders, tuning_rows};
pub use perf_parts::{PERF_PARTS_ROWS, PerfParts, PerfRow};
pub use car_lot::{CarChoice, CarLot, LotState, Placeholder, next_qr_filter, qr_filter_label, region_logos};
use car_lot::lot_bar;

/// Quick Race modes (UI_QRModeSelect).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QrMode {
    Circuit,
    Sprint,
    Drag,
    Drift,
    StreetX,
    Url,
    FreeRun,
}

/// A line one target pixel wide from `a` to `b` (canvas units), in front of
/// the package: the dyno graph's 0x50DCD0, a Direct3D line strip
/// (0x5C5760, DrawPrimitiveUP type 3).
fn hairline_quad(a: Vec2, b: Vec2, color: [f32; 4]) -> Quad {
    let (c, d) = ((a + b) * 0.5, b - a);
    let half = Vec2::new(d.length() * 0.5, 0.5);
    Quad { texture: None, rect: [c - half, c + half], uv: super::HAIRLINE_UV, color, z: 0.0, angle: d.y.atan2(d.x), place: Default::default() }
}

#[cfg(test)]
mod world_map_widget_tests {
    use super::*;

    #[test]
    fn the_widget_zooms_into_the_selected_track() {
        let w = [168.0, 194.0, 270.0];
        // Whole map for the first half second, then in to the window.
        assert_eq!(map_zoom(w, false, 0.2), 0.0);
        assert!((map_zoom(w, false, 0.75) - 0.5).abs() < 1e-5);
        assert_eq!(map_zoom(w, false, 2.0), 1.0);
        assert_eq!(map_zoom(w, true, 0.0), 1.0);
        assert_eq!(map_zoom([0.0; 3], false, 2.0), 0.0);
        assert_eq!(map_window_uv(w, 0.0), [Vec2::ZERO, Vec2::ONE]);
        let [a, b] = map_window_uv(w, 1.0);
        assert!((a - Vec2::new(168.0, 194.0) / 512.0).length() < 1e-5 && (b - Vec2::new(438.0, 464.0) / 512.0).length() < 1e-5);
        // The zoom from the widget's countdown (+0x28, 1/4000 s).
        assert_eq!(map_zoom_count(w, false, 4000), 0.0);
        assert_eq!(map_zoom_count(w, false, 2000), 0.0, "r = 0.5 s: t = 0");
        // 0.00025 is the f32 0.000250000012: just under a half.
        assert_eq!(map_zoom_count(w, false, 1000), 0.49999997);
        assert_eq!(map_zoom_count(w, false, 0), 1.0);
        assert_eq!(map_zoom_count(w, true, 4000), 1.0);
        assert_eq!(map_zoom_count([0.0; 3], false, 0), 0.0);
    }

    /// RACE ticks (30, 29, 31 of 1/1800 s) from the real-time clock `real`:
    /// each tick's (real clock after it, dt).
    fn race_ticks(real: u64, n: usize) -> Vec<(u64, f32)> {
        let mut r = real;
        (0..n)
            .map(|i| {
                let dt = [30u32, 29, 31][i % 3] as f32 * (1.0f32 / 1800.0);
                r += u64::from(crate::game_clock::clock_4000(dt));
                (r, dt)
            })
            .collect()
    }

    /// The hand-off 0.3 s of the real-time clock after a change (0x4E12DF /
    /// 0x4EFC30: (0x8651A8 − stamp) × 0.00025 ≥ 0.3, 0x78758C), 18 RACE
    /// ticks; then the widget's tick shows the track and counts the zoom
    /// down by _ftol(dt × 4000) (0x4CDD40 → 0x577320); a change pauses the
    /// countdown until its own hand-off; the same track keeps its zoom.
    #[test]
    fn the_map_widget_hands_off_and_zooms_on_ticks() {
        let mut m = MapWidget::changed_at(0);
        let ticks = race_ticks(0, 200);
        for (i, &(real, dt)) in ticks.iter().enumerate() {
            m.step(real, dt, Some(4001));
            match i + 1 {
                1..=17 => assert_eq!((m.handed, m.shown), (None, None), "tick {}", i + 1),
                18 => assert_eq!((m.changed, m.handed, m.shown), (None, Some(4001), None), "handed at real {real}"),
                19 => assert_eq!(m.view(), Some((4001, 4000 - 66))),
                _ => {}
            }
        }
        // 181 widget ticks of 66, 64, 68: down to 0.
        assert_eq!(m.view(), Some((4001, 0)));
        let mut m = MapWidget { changed: None, handed: Some(4001), shown: Some(4001), count: 3000 };
        let (real, _) = ticks[0];
        m.touch(real);
        for &(r, dt) in &ticks[1..10] {
            m.step(r, dt, Some(4001));
        }
        assert_eq!(m.count, 3000, "paused while the hand-off waits");
        for &(r, dt) in &ticks[10..20] {
            m.step(r, dt, Some(4001));
        }
        assert!(m.changed.is_none() && m.count < 3000 && m.shown == Some(4001), "the same track keeps its zoom");
        m.touch(ticks[20].0);
        for &(r, dt) in &ticks[21..40] {
            m.step(r, dt, Some(4002));
        }
        assert_eq!(m.shown, Some(4002));
        assert!(m.count > 3000, "a new track starts its zoom");
        // No track at the hand-off: the widget keeps its own.
        let mut m = MapWidget::showing(Some(4001));
        m.touch(0);
        for &(r, dt) in &race_ticks(0, 20) {
            m.step(r, dt, None);
        }
        assert_eq!((m.changed, m.handed, m.shown), (None, Some(4001), Some(4001)));
    }
}

#[cfg(test)]
mod qr_filter_tests {
    use super::*;

    #[test]
    fn triggers_step_the_filters_skipping_empty_ones() {
        // 0x4EED10, game mode 2: with every filter populated...
        let all = |_| true;
        let mut f = 3;
        let mut seen = Vec::new();
        for _ in 0..5 {
            f = next_qr_filter(f, true, all);
            seen.push(f);
        }
        assert_eq!(seen, [1, 2, 4, 8, 3]);
        assert_eq!(next_qr_filter(3, false, all), 8);
        assert_eq!(next_qr_filter(1, false, all), 3);
        // ...and with only stock cars, All and Stock alternate.
        assert_eq!(next_qr_filter(1, true, |_| false), 3);
        assert_eq!(next_qr_filter(3, false, |_| false), 1);
        // Career only (4): left from All goes to it, then to Stock.
        assert_eq!(next_qr_filter(3, false, |f| f == 4), 4);
        assert_eq!(next_qr_filter(4, false, |f| f == 4), 1);
        // 0x4B2746's names.
        assert_eq!(qr_filter_label(3), 0x6B68_362C);
        assert_eq!(qr_filter_label(1), 0xE798_A2F7);
        assert_eq!(qr_filter_label(8), 0x26FE_CEE7);
    }
}

#[cfg(test)]
mod controls_tests {
    use super::*;

    #[test]
    fn an_unbound_key_cell_draws_the_games_dashes() {
        // 0x4ACF40 draws the string at 0x790B94 ("---") when 0x5B8DA0
        // returns an empty name; a bound key is drawn as it is.
        assert_eq!(key_cell(""), "---");
        assert_eq!(key_cell("Left Shift"), "Left Shift");
    }
}

#[cfg(test)]
mod dyno_tests {
    use super::*;

    #[test]
    fn mc_main_types_after_the_starting_name() {
        // 0x551280 appends to the buffer 0x4FF3B0 filled with "NAME";
        // Backspace (8) is the only way to remove it.
        let mut m = McMain::new(false, 0.0);
        m.key(Key::Left, 0.0);
        m.key(Key::Accept, 0.0);
        "Ray-99".chars().for_each(|c| m.char(c));
        assert_eq!(m.name, "NAMERay");
        (0..7).for_each(|_| m.char('\u{8}'));
        "Ray".chars().for_each(|c| m.char(c));
        assert_eq!(m.name, "Ray");
    }

    #[test]
    fn mc_main_autosave_skips_the_save_question() {
        // 0x4E8DDD: with 0x844AD9 set, state 5 goes straight to 6 (save).
        let mut m = McMain::new(false, 0.0);
        m.autosave = true;
        m.key(Key::Left, 0.0);
        m.key(Key::Accept, 0.0);
        assert_eq!(m.step, McStep::Name);
        // 0x5512B9: an Enter as the editor's very first key is ignored.
        assert_eq!(m.key(Key::Accept, 0.0), None);
        assert_eq!(m.step, McStep::Name);
        assert_eq!(m.key(Key::Accept, 0.0), Some(McOutcome::Done { name: "NAME".into(), save: true }));
        let mut m = McMain::new(false, 0.0);
        m.key(Key::Left, 0.0);
        m.key(Key::Accept, 0.0);
        m.char('x');
        m.char('\u{8}');
        assert_eq!(m.key(Key::Accept, 0.0), None);
        assert_eq!(m.step, McStep::Confirm);
    }

    #[test]
    fn dyno_runs_keep_first_previous_and_current() {
        let run = |p: f32| DynoRun { top_speed: 50.0, torque: (200.0, 4000.0), power: (p, 6000.0), ..DynoRun::default() };
        let mut runs = DynoRuns::default();
        runs.record(run(100.0));
        assert_eq!((runs.first, runs.previous, runs.current), (Some(run(100.0)), None, Some(run(100.0))));
        runs.record(run(110.0));
        assert_eq!((runs.first, runs.previous, runs.current), (Some(run(100.0)), Some(run(100.0)), Some(run(110.0))));
        // A current run with no power is not moved to the previous (0x523487).
        let mut runs = DynoRuns { current: Some(run(0.0)), previous: None, first: Some(run(1.0)) };
        runs.record(run(120.0));
        assert_eq!(runs.previous, None);
    }

    #[test]
    fn dyno_results_columns() {
        let r = DynoRun { top_speed: 50.0, zero_60: 0, zero_100: 4 * 4000 + 2280, torque: (200.0, 4049.6), power: (100.0, 6000.0) };
        let c = DynoResults::column(Some(&r), false);
        assert_eq!(c[0], "n/a");
        assert_eq!(c[1], " 4.57 Sec.");
        assert_eq!(c[2], "111.8 MPH");
        assert_eq!(c[3], "147.5 ft-lbs^@4,050 rpm");
        assert_eq!(c[4], "134.1 bhp^@6,000 rpm");
        assert_eq!(DynoResults::column(Some(&r), true)[2], "180.0 KM/H");
        assert_eq!(fe_hash("CURRENT_DATA_1"), 0xED5B_ED0B);
        assert_eq!(fe_hash("FR_DATA_5"), 0xB755_4524);
    }
}

#[cfg(test)]
mod world_map_zoom_tests {
    use super::*;

    /// L1: the zoom records and the per-stage limit against the
    /// executable's bytes (0x7F42C8: 16-byte records {cx, cy, size, label};
    /// 0x7F4328: per stage a pointer to its last record). Under Wine
    /// (the finished-career save, 0x862838 = 6) `]` stepped Full Map → City Core → Beacon
    /// Hill → Jackson Heights → Coal Harbor East → Coal Harbor West → Full
    /// Map and `[` wrapped back, each window matching the clone's.
    #[test]
    #[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
    fn zoom_records_and_limits_match_the_executable() {
        let exe = crate::testkit::reference_exe();
        let u32_at = |va: usize| u32::from_le_bytes(exe[va - 0x400000..va - 0x400000 + 4].try_into().unwrap());
        for (i, z) in WORLD_MAP_ZOOMS.iter().enumerate() {
            let r = 0x7F42C8 + 16 * i;
            let f = |k: usize| f32::from_bits(u32_at(r + k));
            assert_eq!((f(0), f(4), f(8), u32_at(r + 12)), *z, "record {i}");
        }
        for stage in 0..7u8 {
            let last = (u32_at(0x7F4328 + 4 * usize::from(stage)) as usize - 0x7F42C8) / 16;
            assert_eq!(world_map_max_zoom(stage), last, "stage {stage}");
        }
    }
}

/// UI_ShowCase_Preview (the photo shoot of a milestone of kind 1 or 2:
/// 0x498E70 → 0x537990 "UI_ShowCase_Preview.fng"; constructor 0x4CE770,
/// vtable 0x78F998, handler 0x4CF170). The constructor ends a running world
/// challenge (0x541970(0)) and pays the reward (0x541940), hides
/// MagazineMast and MagazineExtra and selects the fourth option (+0x54 =
/// 3); its INIT (0x35F8620B) runs Highlight on the selected option's text
/// (0x95AD0949 + n) and box (0xC2EA8DE7 + n) and hands the keys to the
/// menu state 0x4CEA70: Down (0x911C0A4B) and Up (0x72619778) move the
/// highlight within 0..3 (UnHighlight on the old one, sounds 7 / 6), and
/// Accept (0x406415E3) runs the option — Control Car (0x4B5C70), Show Off
/// (0x4B5D80), Cycle Cameras (0x4B5EA0), Take Picture (0x4CF460). The
/// menu state's buttons (0x49A460): the first two (B8A7C6CC / 69BA0835,
/// B8A7C6CD / 69BA0836) hidden, the third (B8A7C6CE / 69BA0837) "Select"
/// (0x79AD657F). Take Picture hides the four options (0x49A1F0) and
/// 0xF9F1F5D0, shows "Back" (0x001335F0) on the second button and "Accept"
/// (0x4F876B6F) on the third, runs FadeIn on the frame 0x2253BD97 and
/// moves to the picture state 0x4CEDC0: Back (0x911AB364) returns to the
/// menu (0xF9F1F5D0 shown, the buttons as 0x49A460), Accept marks the
/// shoot done (0x533690) and asks "Do you want to save this magazine?"
/// (0x03312F33; "… DVD?" 0xE9641A65 when the reward flag 0x510C30 is not
/// 1: kind 2 sets 2 by 0x510C60) on GenericDialog_SMALL, Yes 0x11223344 /
/// No 0x11223345; both leave (0x499FA0, after Yes's screenshot).
///
/// The options (see [`ShowcaseMode`]): Control Car (0x4B5C70) and Show
/// Off (0x4B5D80) let the world run (0x529590 on 0x85EF50: +0x1C = 1,
/// flow event 2; 0x5295C0 stops it again) with only their own option
/// left shown (0x49A2E0 / 0x49A3A0 hide the other three and their
/// boxes), the first two buttons hidden and "Back" (0x001335F0) on the
/// third; Back (0x911AB364, or 0xB4623F67) returns to the menu (0x49A460).
/// Cycle Cameras (0x4B5EA0) stays in the menu: the set's next camera
/// (0x510710, wrapping) on the showcase camera (0x4B5730). Take Picture
/// streams the picks' masthead and extra (0x499E60 → 0x523BD0 on
/// 0x8364F4, from MAGAZINES_STREAMING_LOW) and runs FadeIn on
/// MagazineMast / MagazineExtra (0xB6638240 / 0x824FA72F); the picture
/// state's tick (0xC98356BA) shows each loaded one on its image (0x495400
/// / 0x495500: 0x836508 / 0x836510 once 0x836500 is set) and hides the
/// others. Its Left / Right / Up / Down (0x4B57B0) do nothing: 0x50D510
/// answers 0. Back there undoes the masthead (0x4B5880(0)).
// prov: open frontend.screens.quads-10
/// **Open**: UI_Showcase_DPAD (not referenced by name in the code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowcaseMode {
    /// The menu (0x4CEA70): the world stopped.
    Menu,
    /// Control Car (0x4B5C70, state 0x4B5A70): 0x499F10 shuts the hood,
    /// doors and trunk first; the player drives the car.
    Control,
    /// Show Off (0x4B5AD0): the car's controller off (0x5EBA40(0)), its
    // prov: open frontend.screens.quads-11
    /// input +0x214 = 0.8 (0x5EB900, meaning **open**), 0x836353 = 1;
    /// Up toggles the hood (+0xE9, channel 0), Down the trunk (+0xEB,
    /// channel 1), Left / Right the doors (+0xEA, channels 2 and 3), each
    /// by 0x61F830(car, channel, open, 1.0). Back gives the player the car
    /// again (0x5EBA40(1)) and clears 0x836353; the parts stay as they are.
    ShowOff,
    /// Take Picture (0x4CF460, state 0x4CEDC0).
    Picture,
}

#[derive(Clone, Debug)]
pub struct ShowcasePreview {
    /// The milestone (an index into `CareerData::milestones`).
    pub milestone: usize,
    pub dvd: bool,
    /// The selected option (+0x54), 0..3.
    pub option: usize,
    /// The picture state (0x4CEDC0) rather than the menu (0x4CEA70).
    pub picture: bool,
    /// The state (the handler +0x50).
    pub mode: ShowcaseMode,
    pub opened: f32,
    /// When the highlight last moved or the state changed, ms.
    pub changed: f32,
    /// The option highlighted before `option`, for its UnHighlight.
    pub previous: Option<usize>,
    /// The camera set (+0x78, an index into `Showcase::setups`) and the
    /// picks (+0x80..+0x90; 0x4B57B0(0, 0) at the constructor).
    pub setup: Option<usize>,
    pub picks: crate::formats::magazines::ShowcasePicks,
    /// Hood, doors, trunk (+0xE9, +0xEA, +0xEB): whether Show Off has them
    /// open.
    pub open: [bool; 3],
    /// Whether the masthead / extra textures are loaded (0x836500).
    pub streamed: bool,
    /// Yes arms five UI update ticks (0x4CEE18 / 0x4CF0ED), then waits
    /// for a completed world frame. None means no save was requested.
    pub save_ticks: Option<u8>,
}

/// What a showcase key does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowcaseKey {
    None,
    /// Accept in the picture state: the save question.
    Save,
    /// Control Car or Show Off began (the world runs) or ended (Back).
    Drive,
    Stop,
    /// Cycle Cameras: the set's next camera.
    Cycle,
    /// Take Picture: stream the masthead and the extra.
    Stream,
    /// Show Off's part toggles: (channel, open) for 0x61F830.
    Parts(&'static [usize], bool),
}

pub mod showcase {
    pub const OPTIONS: [u32; 4] = [crate::frontend::fe_ids::SCREENS_OPTIONS_0, crate::frontend::fe_ids::SCREENS_OPTIONS_1, crate::frontend::fe_ids::OPTIONS_2, crate::frontend::fe_ids::OPTIONS_3];
    pub const BOXES: [u32; 4] = [crate::frontend::fe_ids::BOXES_0, crate::frontend::fe_ids::BOXES_1, crate::frontend::fe_ids::BOXES_2, crate::frontend::fe_ids::BOXES_3];
    pub const BUTTON_TEXT: [u32; 3] = [crate::frontend::fe_ids::BUTTON_TEXT_0, crate::frontend::fe_ids::BUTTON_TEXT_1, crate::frontend::fe_ids::BUTTON];
    pub const BUTTONS: [u32; 3] = [crate::frontend::fe_ids::BUTTONS_0, crate::frontend::fe_ids::BUTTONS_1, crate::frontend::fe_ids::BUTTONS_2];
    pub const MENU_GROUP: u32 = crate::frontend::fe_ids::MENU_GROUP;
    /// The options' highlight frames (groups of U2_Highlight_box /
    /// U2_Highlight_Corner, top to bottom; the last is `MENU_GROUP`):
    /// INIT chains to HIDE, Highlight shows one, UnHighlight hides it.
    /// **Observed** under Wine (the DVD shoot): one frame, on the
    /// selected option, follows Up / Down. What runs their scripts is not
    /// traced (**inferred**: the option's highlight, as on the boxes).
    pub const FRAMES: [u32; 4] = [crate::frontend::fe_ids::FRAMES_0, crate::frontend::fe_ids::FRAMES_1, crate::frontend::fe_ids::FRAMES_2, crate::frontend::fe_ids::MENU_GROUP];
    pub const FRAME: u32 = crate::frontend::fe_ids::FRAME;
    pub const SELECT: u32 = crate::frontend::fe_ids::SELECT;
    pub const BACK: u32 = crate::frontend::fe_ids::SHOWCASE_BACK;
    pub const ACCEPT: u32 = crate::frontend::fe_ids::SHOWCASE_ACCEPT;
    /// 0x5C3130: dimensions from 0x510AD0/0x510AE0, scaled to the back
    /// buffer, then fixed pixel offsets. The capture is later resampled to 512².
    pub fn capture_rect((width, height): (u32, u32)) -> Option<((u32,u32),(u32,u32))> {
        let w = (320.0 * width as f64 * f32::from_bits(0x3acccccd) as f64) as i64 / 2;
        let h = (432.0 * height as f64 * f32::from_bits(0x3b088889) as f64) as i64 / 2;
        let cx = (width / 2) as i64; let cy = (height / 2) as i64;
        let x = (cx-w-10).max(0); let y = (cy-h-4).max(0);
        let right = (cx+w+3).min(width as i64); let bottom = (cy+h-6).min(height as i64);
        (right > x && bottom > y).then_some(((x as u32,y as u32),((right-x) as u32,(bottom-y) as u32)))
    }

    /// Crop an actual completed framebuffer; no replacement cover is composed.
    /// Saved native pictures are displayed opaque by the existing reader.
    pub fn capture_rgba(pixels: &[u8], size: (u32,u32), bgra: bool) -> Option<(Vec<u8>,u32,u32)> {
        let len = (size.0 as usize).checked_mul(size.1 as usize)?.checked_mul(4)?;
        if pixels.len() != len { return None; }
        let ((x,y),(width,height)) = capture_rect(size)?;
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for row in y..y+height {
            let start = (row as usize * size.0 as usize + x as usize) * 4;
            for p in pixels[start..start+width as usize*4].chunks_exact(4) {
                rgba.extend_from_slice(&if bgra { [p[2],p[1],p[0],255] } else { [p[0],p[1],p[2],255] });
            }
        }
        Some((rgba,width,height))
    }

}

impl ShowcasePreview {
    pub fn new(milestone: usize, dvd: bool, now: f32) -> Self {
        ShowcasePreview { milestone, dvd, option: 3, picture: false, mode: ShowcaseMode::Menu, opened: now, changed: now, previous: None, setup: None, picks: Default::default(), open: [false; 3], streamed: false, save_ticks: None }
    }

    pub fn save_tick(&mut self) {
        if let Some(left) = self.save_ticks.as_mut() { *left = left.saturating_sub(1); }
    }

    pub fn key(&mut self, key: Key, now: f32) -> ShowcaseKey {
        if self.save_ticks.is_some() { return ShowcaseKey::None; }
        match (self.mode, key) {
            (ShowcaseMode::Menu, Key::Down) if self.option < 3 => {
                self.previous = Some(self.option);
                self.option += 1;
                self.changed = now;
            }
            (ShowcaseMode::Menu, Key::Up) if self.option > 0 => {
                self.previous = Some(self.option);
                self.option -= 1;
                self.changed = now;
            }
            (ShowcaseMode::Menu, Key::Accept) => match self.option {
                0 => {
                    // 0x4B5C70 (the caller runs 0x499F10 on `open`).
                    self.mode = ShowcaseMode::Control;
                    self.changed = now;
                    return ShowcaseKey::Drive;
                }
                1 => {
                    self.mode = ShowcaseMode::ShowOff;
                    self.changed = now;
                    return ShowcaseKey::Drive;
                }
                2 => return ShowcaseKey::Cycle,
                _ => {
                    self.picture = true;
                    self.mode = ShowcaseMode::Picture;
                    self.changed = now;
                    return ShowcaseKey::Stream;
                }
            },
            (ShowcaseMode::Control | ShowcaseMode::ShowOff, Key::Back) => {
                self.mode = ShowcaseMode::Menu;
                self.previous = None;
                self.changed = now;
                return ShowcaseKey::Stop;
            }
            // 0x4B5AD0's toggles.
            (ShowcaseMode::ShowOff, Key::Up) => {
                self.open[0] = !self.open[0];
                return ShowcaseKey::Parts(&[0], self.open[0]);
            }
            (ShowcaseMode::ShowOff, Key::Down) => {
                self.open[2] = !self.open[2];
                return ShowcaseKey::Parts(&[1], self.open[2]);
            }
            (ShowcaseMode::ShowOff, Key::Left | Key::Right) => {
                self.open[1] = !self.open[1];
                return ShowcaseKey::Parts(&[2, 3], self.open[1]);
            }
            (ShowcaseMode::Picture, Key::Back) => {
                self.picture = false;
                self.mode = ShowcaseMode::Menu;
                self.previous = None;
                self.streamed = false;
                self.changed = now;
            }
            (ShowcaseMode::Picture, Key::Accept) => return ShowcaseKey::Save,
            _ => {}
        }
        ShowcaseKey::None
    }

    /// The save question's text (0x4CEDC0 on Accept).
    pub fn question(&self) -> u32 {
        if self.dvd { 0xE964_1A65 } else { 0x0331_2F33 }
    }

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        self.quads_hover(assets, now, None)
    }

    /// With the corner button under the cursor and since when: 0x4CF170's
    /// hover (0x49A110(k)) runs "Init_Pulse" (0x78F9C8) on button +0xC8[k]
    /// (BUTTONS[k]) and "Init" on the one before (+0xE4).
    pub fn quads_hover(&self, assets: &Assets, now: f32, hover: Option<(u32, f32)>) -> Vec<Quad> {
        use showcase::*;
        let Some(package) = assets.package("UI_ShowCase_Preview") else { return Vec::new() };
        let fe = crate::formats::hash::fe_hash;
        let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        let mut state = ScreenState::default();
        state.chained.extend(FRAMES);
        // The buttons' glows (UI_PC_Generic_Button_Glow_sm): INIT chains to
        // HIDE; **observed** under Wine: no glow on the menu or the
        // picture state.
        state.chained.extend([0xE0AF_7B8D, 0xE0AF_7B8E, 0xE0AF_7B8F]);
        state.hidden.insert(fe("MagazineMast"));
        state.hidden.insert(fe("MagazineExtra"));
        if self.picture {
            // The tick (0xC98356BA): each loaded layer on its image, run
            // in by 0x499E60's FadeIn.
            for (name, texture) in [("MagazineMast", self.picks.masthead), ("MagazineExtra", self.picks.extra)] {
                if self.streamed && texture != 0 {
                    state.hidden.remove(&fe(name));
                    state.image.insert(fe(name), texture);
                    state.run(fe(name), "FadeIn", self.changed - self.opened);
                }
            }
        }
        if matches!(self.mode, ShowcaseMode::Control | ShowcaseMode::ShowOff) {
            // 0x4B5C70 / 0x4B5D80: their option alone (0x49A2E0 /
            // 0x49A3A0), the first two buttons hidden, "Back" on the third.
            let keep = if self.mode == ShowcaseMode::Control { 0 } else { 1 };
            for i in (0..4).filter(|&i| i != keep) {
                state.hidden.extend([OPTIONS[i], BOXES[i]]);
            }
            state.hidden.extend([BUTTON_TEXT[0], BUTTONS[0], BUTTON_TEXT[1], BUTTONS[1]]);
            state.text.insert(BUTTON_TEXT[2], label(BACK));
            state.run(OPTIONS[self.option], "Highlight", 0.0);
            state.run(BOXES[self.option], "Highlight", 0.0);
            state.run(FRAMES[self.option], "Highlight", 0.0);
        } else if self.picture {
            // 0x4CF460 (as 0x4B5C70's buttons): the options hidden.
            for h in OPTIONS.iter().chain(BOXES.iter()).chain(FRAMES.iter()) {
                state.hidden.insert(*h);
            }
            state.hidden.insert(MENU_GROUP);
            state.hidden.extend([BUTTON_TEXT[0], BUTTONS[0]]);
            state.text.insert(BUTTON_TEXT[1], label(BACK));
            state.text.insert(BUTTON_TEXT[2], label(ACCEPT));
            state.run(FRAME, "FadeIn", self.changed - self.opened);
        } else {
            // 0x49A460: the first two buttons hidden, "Select" on the third.
            state.hidden.extend([BUTTON_TEXT[0], BUTTONS[0], BUTTON_TEXT[1], BUTTONS[1]]);
            state.text.insert(BUTTON_TEXT[2], label(SELECT));
            let at = if self.previous.is_some() { self.changed - self.opened } else { 0.0 };
            state.run(OPTIONS[self.option], "Highlight", at);
            state.run(BOXES[self.option], "Highlight", at);
            state.run(FRAMES[self.option], "Highlight", at);
            if let Some(p) = self.previous {
                state.run(OPTIONS[p], "UnHighlight", self.changed - self.opened);
                state.run(BOXES[p], "UnHighlight", self.changed - self.opened);
                state.run(FRAMES[p], "UnHighlight", self.changed - self.opened);
            }
        }
        for b in BUTTON_TEXT.iter().chain(BUTTONS.iter()) {
            if !state.hidden.contains(b) {
                state.shown.insert(*b);
            }
        }
        if let Some((button, since)) = hover.filter(|(b, _)| BUTTONS.contains(b) && !state.hidden.contains(b)) {
            state.scripts.insert(button, (INIT_PULSE, since - self.opened));
        }
        layout(assets, package, &state, now - self.opened)
    }
}

/// "Init_Pulse" (0xA3E4657B): the 1200 ms looping pulse the front end runs
/// on a hovered button.
pub const INIT_PULSE: u32 = fe_ids::INIT_PULSE;

#[cfg(test)]
mod dialog_size_tests {
    use super::{dialog_size, help_dialog_package};

    #[test]
    fn thresholds_of_0x526c50() {
        // strlen × line height: < 2561 SMALL, >= 5122 LARGE, else MED.
        assert_eq!(dialog_size(80, 32.0), "GenericDialog_SMALL");
        assert_eq!(dialog_size(81, 32.0), "GenericDialog_MED");
        assert_eq!(dialog_size(160, 32.0), "GenericDialog_MED");
        assert_eq!(dialog_size(161, 32.0), "GenericDialog_LARGE");
        assert_eq!(help_dialog_package("a^b^c^d"), "HelpDialog_SMALL");
        assert_eq!(help_dialog_package("a^b\nc^d^e"), "HelpDialog_MED");
    }
}

#[cfg(test)]
mod engage_text_tests {
    /// The engage dialog's distance as the original printed it under Wine
    /// (track 4013: 2202 m → "1.376 mi"): "%$f" takes 3 decimals.
    #[test]
    fn distance_takes_three_decimals() {
        let miles = 2202.0 * f64::from(f32::from_bits(0x3A23_D70A));
        assert_eq!(super::fixed3(miles), "1.376");
        assert_eq!(super::fixed3(1234.5), "1,234.500");
        // Track 4004 (5300 m) under Wine: "3.313 mi" (S2_SPON_CIRCUIT_2).
        let miles = f64::from(5300.0f32 * f32::from_bits(0x3A23_D70A));
        assert_eq!(super::fixed3(miles), "3.313");
    }
}

#[cfg(test)]
mod list_opening_tests {
    use super::*;

    fn browser(package: &'static str, slot: u8, installed: Option<usize>) -> PartsBrowser {
        let parts = (0..6u32).map(|i| (0x100 + i, format!("P{i}"), 0, true, installed == Some(i as usize))).collect();
        let categories = vec![BrowserCategory { slot, icon: 0, label: 0, parts, locked: false, action: CategoryAction::Parts, text: None }];
        PartsBrowser { package, title: 0, sub: None, categories, category: 0, part: None, opened: 0.0, total: None, arrows: ArrowBlink::default(), motion: Default::default() }
    }

    /// The parts lists open on the installed part (0x53A4C0 → 0x538710,
    /// 0x5606F0, 0x53BA10 neon, 0x560260 decals, 0x55F900 rim sizes), the
    /// first when none is listed (0x51F7B0 → the head).
    #[test]
    fn a_parts_list_opens_on_the_installed_part() {
        for (package, slot) in [("UI_GenericParts_Browser", 9u8), ("UI_ICEMAIN", 0x9F), ("UI_CustomNeonMain", 0x9A), ("UI_DecalMain", 0x61), ("UI_Rims_Browser", 0x1D)] {
            let mut b = browser(package, slot, Some(4));
            b.key(Key::Accept);
            assert_eq!(b.part, Some(4), "{package}");
            let mut b = browser(package, slot, None);
            b.key(Key::Accept);
            assert_eq!(b.part, Some(0), "{package} without an installed part");
        }
    }

    /// Custom Gauges (UI_CustomHUDOverlay 0x56C220): 0x53C200 refills the
    /// list and selects its head, so it opens on STOCK whatever is fitted
    /// (Wine, the finished-career save with GRIFFIN fitted: STOCK).
    #[test]
    fn the_custom_gauges_list_opens_on_the_top() {
        let mut b = browser("UI_ICEMAIN", 0xA3, Some(2));
        b.key(Key::Accept);
        assert_eq!(b.part, Some(0));
    }

    /// Lists Open on Installed (clone deviation): Custom Gauges opens on
    /// the fitted gauge, still STOCK (the head) with none fitted; off is
    /// the native STOCK; the other lists are unchanged either way.
    #[test]
    fn lists_open_on_installed_moves_the_gauges_cursor() {
        let b = browser("UI_ICEMAIN", 0xA3, Some(2));
        assert_eq!((b.opening_part(false), b.opening_part(true)), (Some(0), Some(2)));
        assert_eq!(browser("UI_ICEMAIN", 0xA3, None).opening_part(true), Some(0));
        for (package, slot) in [("UI_GenericParts_Browser", 9u8), ("UI_ICEMAIN", 0x9F), ("UI_CustomNeonMain", 0x9A)] {
            let b = browser(package, slot, Some(4));
            assert_eq!((b.opening_part(false), b.opening_part(true)), (Some(4), Some(4)), "{package}");
        }
    }
}

#[cfg(test)]
mod photo_mode_tests {
    use super::*;

    /// With the Enhancements row off the pause menus are the native
    /// lists; on, "Photo Mode" comes before "Quit to Main Menu" and nothing
    /// else moves.
    #[test]
    fn photo_mode_item_only_with_the_row_on() {
        for (career, garage) in [(true, true), (true, false), (false, false)] {
            let off = explore_pause_menu_with(career, garage, false);
            assert!(off.iter().all(|o| o.action != Action::PhotoMode));
            let on = explore_pause_menu_with(career, garage, true);
            assert_eq!(on.len(), off.len() + 1);
            assert_eq!(on[on.len() - 2].action, Action::PhotoMode);
            assert_eq!(on[on.len() - 2].label, super::super::options::PHOTO_MODE);
            let key = |v: &[IconOption]| v.iter().map(|o| (o.icon, o.label, o.action)).collect::<Vec<_>>();
            assert_eq!(key(&on[..on.len() - 2]), key(&off[..off.len() - 1]));
            assert_eq!(on.last().unwrap().action, Action::QuitToMainMenu);
        }
        for replay in [None, Some(false), Some(true)] {
            let off = race_pause_menu_with(true, true, replay, false);
            let on = race_pause_menu_with(true, true, replay, true);
            assert!(off.iter().all(|o| o.action != Action::PhotoMode));
            assert_eq!(on.len(), off.len() + 1);
            assert_eq!(on[on.len() - 2].action, Action::PhotoMode);
        }
        assert_eq!(super::super::options::clone_label(super::super::options::PHOTO_MODE), Some("Photo Mode"));
    }
}

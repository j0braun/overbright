use crate::frontend::fe_ids;
use super::*;

#[derive(Clone, Copy, Debug)]
pub struct IconOption {
    pub icon: u32,
    pub label: u32,
    pub action: Action,
}

const fn opt(icon: u32, label: u32, action: Action) -> IconOption {
    IconOption { icon, label, action }
}

/// The main menu (0x4AEE30).
pub const MAIN_MENU: [IconOption; 7] = [
    opt(fe_ids::MAIN_ICON_CAREER, fe_ids::TXT_CAREER, Action::Career), // MAIN_ICON_CAREER, "Career"
    opt(fe_ids::MAIN_ICON_QUICK_RACE, fe_ids::TXT_QUICK_RACE, Action::QuickRace), // MAIN_ICON_QUICK_RACE, "Quick Race"
    opt(fe_ids::MAIN_ICON_GO_ONLINE, fe_ids::TXT_ONLINE, Action::Online), // MAIN_ICON_GO_ONLINE, "Online"
    opt(fe_ids::MAIN_ICON_LAN, fe_ids::TXT_LAN, Action::Lan),    // MAIN_ICON_LAN, "LAN"
    opt(fe_ids::MAIN_ICON_CUSTOMIZE_CAR, fe_ids::TXT_CUSTOMIZE, Action::Customize), // MAIN_ICON_CUSTOMIZE_CAR, "Customize"
    opt(fe_ids::MAIN_ICON_PROFILE_MANAG, fe_ids::TXT_PROFILE_MANAGER, Action::ProfileManager), // MAIN_ICON_PROFILE_MANAG, "Profile Manager"
    opt(fe_ids::MAIN_ICON_OPTIONS, fe_ids::TXT_OPTIONS, Action::Options), // MAIN_ICON_OPTIONS, "Options"
];

/// UI_StartCareer's options (0x4AF110). With a career saved (career
/// +0x20, 0x502FE0): "Resume Career" when its intro movie has played
/// (0x86347C, career +0x873C, saved with it) beside a fresh "Start New
/// Career", else only "Start New Career" that resumes it; without one, a
/// fresh "Start New Career". "Load Game" always.
pub fn career_menu(saved: bool, under_way: bool) -> Vec<IconOption> {
    let mut v = Vec::new();
    if saved {
        if under_way {
            v.push(opt(fe_ids::EXPLORE_AND_RESUME_CAREER_ICON, fe_ids::TXT_RESUME_CAREER, Action::ResumeCareer)); // "Resume Career"
        } else {
            v.push(opt(fe_ids::NEW_CAREER_AND_RESUME_CAREER_ICON, fe_ids::TXT_START_NEW_CAREER, Action::ResumeCareer)); // "Start New Career" (resumes)
        }
    }
    if !saved || under_way {
        v.push(opt(fe_ids::NEW_CAREER_AND_RESUME_CAREER_ICON, fe_ids::TXT_START_NEW_CAREER, Action::NewCareer)); // "Start New Career"
    }
    v.push(opt(fe_ids::LOAD_CAREER_ICON, fe_ids::TXT_LOAD_GAME, Action::LoadCareer)); // "Load Game"
    v
}

/// The garage (UI_CareerCrib, 0x4AF820). World Map shows when the counter
/// at 0x862838 is above 1.
pub fn crib_menu(world_map: bool) -> Vec<IconOption> {
    let mut v = vec![opt(fe_ids::EXPLORE_AND_RESUME_CAREER_ICON, fe_ids::TXT_RETURN_TO_EXPLORE_MODE, Action::Explore)]; // "Return to Explore Mode"
    if world_map {
        v.push(opt(fe_ids::WORLD_MAP_ICON, fe_ids::TXT_WORLD_MAP, Action::WorldMap)); // "World Map"
    }
    v.extend([
        opt(fe_ids::CAR_SELECT_ICON, fe_ids::TXT_CAR_SELECT, Action::CarSelect),   // "Car Select"
        opt(fe_ids::CHANGE_PARTS_ICON, fe_ids::TXT_CHANGE_PARTS, Action::ChangeParts), // "Change Parts"
        opt(fe_ids::TUNING_ICON, fe_ids::TXT_PERFORMANCE_TUNING_DYNO, Action::Tuning),      // "Performance Tuning / Dyno"
        opt(fe_ids::REWARDS_ICON, fe_ids::TXT_VIEW_REWARDS, Action::Rewards),     // "View Rewards"
        opt(fe_ids::SAVE_ICON, fe_ids::TXT_SAVE, Action::Save),        // "Save"
        opt(fe_ids::MAIN_MENU_ICON, fe_ids::TXT_BACK_TO_MAIN_MENU, Action::MainMenu),    // "Back to Main Menu"
    ]);
    v
}

/// UI_CribRewardOptionsMain (0x4AFB70): an option locks (+0x22) with
/// nothing to show — no magazine covers earned (0x510D50(1)), no DVD
/// covers (0x510D50(2)), no sponsor signed (0x5009E0).
pub fn reward_menu(magazines: bool, dvds: bool, sponsors: bool) -> Vec<IconOption> {
    let or_locked = |open: bool, a: Action| if open { a } else { Action::Unavailable };
    vec![
        opt(fe_ids::MAGAZINE_ICON, 0xC799_3F9A, or_locked(magazines, Action::Magazines)), // "View Magazines"
        opt(fe_ids::DVD_ICON, 0x7647_B08C, or_locked(dvds, Action::DvdCovers)),      // "View DVD Covers"
        opt(0x0878_FFDF, 0xE000_4602, or_locked(sponsors, Action::Sponsors)),   // "View Signed Sponsors"
    ]
}

/// UI_QRModeSelect (built at 0x4B2D60; each option's accept, 0x4B2BE0…
/// 0x4B2D40, stores its race type through 0x4B2C00: Circuit 0, Sprint 1,
/// Drag 2, Drift 3, URL 4, Street X 10, Free Run 9). Outrun (0x086C74CB,
/// race type 8) is only added in game modes 8, 0x200 and 4 (0x850078, the
/// online / LAN / split-screen modes) and Free Run only outside them, so
/// Quick Race (game mode 2) offers Free Run and no Outrun.
pub const QR_MODE_MENU: [IconOption; 7] = [
    opt(fe_ids::QR_MODE_CIRCUIT_ICON, fe_ids::QR_MODE_CIRCUIT_LABEL, Action::QrMode(QrMode::Circuit)),
    opt(fe_ids::QR_MODE_SPRINT_ICON, fe_ids::QR_MODE_SPRINT_LABEL, Action::QrMode(QrMode::Sprint)),
    opt(fe_ids::QR_MODE_DRAG_ICON, fe_ids::QR_MODE_DRAG_LABEL, Action::QrMode(QrMode::Drag)),
    opt(fe_ids::QR_MODE_DRIFT_ICON, fe_ids::QR_MODE_DRIFT_LABEL, Action::QrMode(QrMode::Drift)),
    opt(fe_ids::QR_MODE_STREET_X_ICON, fe_ids::QR_MODE_STREET_X_LABEL, Action::QrMode(QrMode::StreetX)),
    opt(fe_ids::QR_MODE_URL_ICON, fe_ids::QR_MODE_URL_LABEL, Action::QrMode(QrMode::Url)),
    opt(fe_ids::QR_MODE_FREE_RUN_ICON, fe_ids::QR_MODE_FREE_RUN_LABEL, Action::QrMode(QrMode::FreeRun)),
];

/// UI_OptionsMain.
pub const OPTIONS_MENU: [IconOption; 7] = [
    opt(fe_ids::PAUSE_SETTING_0_AND_SETTING_0_ICON, fe_ids::TXT_AUDIO, Action::Setting(0)), // "Audio"
    opt(fe_ids::PAUSE_SETTING_1_AND_SETTING_1_ICON, fe_ids::TXT_DISPLAY, Action::Setting(1)), // "Display"
    opt(fe_ids::PAUSE_SETTING_2_AND_SETTING_2_ICON, fe_ids::TXT_GAMEPLAY, Action::Setting(2)), // "Gameplay"
    opt(fe_ids::PAUSE_SETTING_3_AND_SETTING_3_ICON, fe_ids::TXT_PLAYER, Action::Setting(3)), // "Player"
    opt(fe_ids::SETTING_4_ICON, fe_ids::TXT_CONTROLS, Action::Setting(4)), // "Controls"
    opt(fe_ids::SETTING_5_ICON, fe_ids::TXT_EA_GAMES_TRAX, Action::Setting(5)), // "EA GAMES TRAX"
    opt(fe_ids::SETTING_6_ICON, fe_ids::TXT_CREDITS, Action::Setting(6)), // "Credits"
];

/// UI_PauseOptionsMain (factory 0x4F47F0, screen 0x4EB580, vtable
/// 0x797CD4; options 0x4C3000): Audio, Display, Gameplay, Player, the
/// option classes 0x7918B8…0x7918D0 whose handlers are UI_OptionsMain's
/// (0x49A620…0x49A680, setting the category 0x83AA04). The pause's
/// Options (vtable 0x795510 → 0x4C2700, on 0x0C407210) opens it.
pub const PAUSE_OPTIONS_MENU: [IconOption; 4] = [
    opt(fe_ids::PAUSE_SETTING_0_AND_SETTING_0_ICON, fe_ids::TXT_AUDIO, Action::PauseSetting(0)),
    opt(fe_ids::PAUSE_SETTING_1_AND_SETTING_1_ICON, fe_ids::TXT_DISPLAY, Action::PauseSetting(1)),
    opt(fe_ids::PAUSE_SETTING_2_AND_SETTING_2_ICON, fe_ids::TXT_GAMEPLAY, Action::PauseSetting(2)),
    opt(fe_ids::PAUSE_SETTING_3_AND_SETTING_3_ICON, fe_ids::TXT_PLAYER, Action::PauseSetting(3)),
];

/// UI_ChooseCustomizeCategory (built by 0x520D70; class 0x556650): in
/// game mode 0x100 with a car first "Car Lot"; "Change Body Parts",
/// "Change Performance Parts", "Change Paint / Vinyls" (unless 0x83898C
/// is 1), "Change Interiors / Car Specialties", and "Performance Tuning /
/// Dyno" (0x520D20, vtable 0x79B580 → 0x55E7A0) when 0x83898C is 2, 3 or 4
/// outside game mode 0x200. The main menu's Customize opens it through
/// 0x552CD0(3, …), which sets 0x83898C to its first argument (0x552D32):
/// 3, so all five show.
pub const CUSTOMIZE_MENU: [IconOption; 5] = [
    opt(fe_ids::CUSTOMIZE_CATEGORY_0_ICON, fe_ids::TXT_CHANGE_BODY_PARTS, Action::CustomizeCategory(0)), // "Change Body Parts"
    opt(fe_ids::CUSTOMIZE_CATEGORY_1_ICON, fe_ids::TXT_CHANGE_PERFORMANCE_PARTS, Action::CustomizeCategory(1)), // "Change Performance Parts"
    opt(fe_ids::CUSTOMIZE_CATEGORY_2_ICON, fe_ids::TXT_CHANGE_PAINT_VINYLS, Action::CustomizeCategory(2)), // "Change Paint / Vinyls"
    opt(fe_ids::CUSTOMIZE_CATEGORY_3_ICON, fe_ids::TXT_CHANGE_INTERIORS_CAR_SPECIALTIES, Action::CustomizeCategory(3)), // "Change Interiors / Car Specialties"
    opt(fe_ids::TUNING_ICON, fe_ids::TXT_PERFORMANCE_TUNING_DYNO, Action::Tuning),               // "Performance Tuning / Dyno"
];

/// UI_ProfileManager (its options built by 0x4C58B0, **verified**, and as
/// the original shows them): Create (vtable 0x7959EC → 0x4C5880, MC_Main
/// mode 0x861), Load (0x7959DC → 0x4C5820, 0xC11), Save (0x7959D4 →
/// 0x4C57F0, 0x4A51) and Delete (0x7959E4 → 0x4C5850, 0x831), always all
/// four.
pub fn profile_menu(_has_profile: bool) -> Vec<IconOption> {
    vec![
        opt(fe_ids::PROFILE_CREATE_ICON, fe_ids::CREATE, Action::ProfileCreate),
        opt(fe_ids::PROFILE_LOAD_ICON, fe_ids::LOAD, Action::ProfileLoad),
        opt(fe_ids::PROFILE_SAVE_ICON, fe_ids::SAVE, Action::ProfileSave),
        opt(fe_ids::PROFILE_DELETE_ICON, fe_ids::MEMCARD_DELETE, Action::ProfileDelete),
    ]
}

/// UI_pause in explore mode (0x4C2900): career "Return to Explore Mode"
/// (and "Jump to Garage?" when 0x8635A8 is set), else "Return to Free
/// Run"; "Options"; "Quit to Main Menu".
pub fn explore_pause_menu(career: bool, garage: bool) -> Vec<IconOption> {
    explore_pause_menu_with(career, garage, crate::qol::photo_mode())
}

/// [`explore_pause_menu`] with the Photo Mode item listed or not.
pub fn explore_pause_menu_with(career: bool, garage: bool, photo: bool) -> Vec<IconOption> {
    let mut v = Vec::new();
    if career {
        v.push(opt(fe_ids::RETURN_TO_EXPLORE_ICON, fe_ids::MENUS_RETURN_TO_EXPLORE_LABEL_BCDC, Action::ReturnToExplore));
        if garage {
            v.push(opt(fe_ids::JUMP_TO_GARAGE_ICON, fe_ids::JUMP_TO_GARAGE_LABEL, Action::JumpToGarage));
        }
    } else {
        v.push(opt(fe_ids::RETURN_TO_EXPLORE_ICON, fe_ids::RETURN_TO_FREE_RUN_LABEL, Action::ReturnToFreeRun));
    }
    v.push(opt(fe_ids::OPTIONS_ICON, fe_ids::OPTIONS_LABEL, Action::Options));
    // "Replay" (0x4C2640, class 0x7954F0) out of game modes 4 / 8 / 0x200:
    // greyed (+0x20) unless the replay manager 0x8650B4 holds a recording
    // (+0x50EC set, +0xB8 > 0, 0x88F1A8 clear), which free roam does not;
    // its Accept (0x4C26B0) checks the same (**verified**, 2026-10-29; the
    // original's explore pause under Wine shows it greyed third).
    v.push(opt(fe_ids::REPLAY_AND_UNAVAILABLE_ICON, fe_ids::TXT_REPLAY, Action::Unavailable));
    photo_mode_item(&mut v, photo);
    v.push(opt(fe_ids::QUIT_TO_MAIN_MENU_ICON, fe_ids::QUIT_TO_MAIN_MENU_LABEL, Action::QuitToMainMenu));
    v
}

/// Photo mode (docs/playing/photo-mode.md, clone deviation): "Photo Mode" before "Quit
/// to Main Menu" when the Enhancements row "Photo Mode in Pause Menu" is
/// on (default Off: the native list, unchanged). The icon is the Replay
/// item's (the nearest the original has to a camera).
fn photo_mode_item(v: &mut Vec<IconOption>, photo: bool) {
    if photo {
        v.push(opt(fe_ids::REPLAY_AND_UNAVAILABLE_ICON, crate::frontend::options::PHOTO_MODE, Action::PhotoMode));
    }
}

/// UI_pause in a race (0x4C2BD0): career races from the World Map can go
/// back to it (0x85BA67); "Resume Race", "Restart", career "Return to
/// Explore Mode", "Options", "Replay", "Quit to Main Menu". Replay
/// (0x4C2640, class 0x7954F0) is added outside game modes 4 / 8 / 0x200
/// (`replay` None there) and greyed (+0x20) unless the replay manager
/// holds an INSTANT_REPLAY (+0xB8 > 0; +0x50EC is set by every manager,
/// 0x57A9AE) and the streamer holds no buffer (0x88F1A8 = 0)
/// (**verified**; **observed** under Wine: a Quick Race's pause lists
/// Resume Race, Restart, Options, Replay, Quit).
pub fn race_pause_menu(career: bool, from_map: bool, replay: Option<bool>) -> Vec<IconOption> {
    race_pause_menu_with(career, from_map, replay, crate::qol::photo_mode())
}

/// [`race_pause_menu`] with the Photo Mode item listed or not.
pub fn race_pause_menu_with(career: bool, from_map: bool, replay: Option<bool>, photo: bool) -> Vec<IconOption> {
    let mut v = Vec::new();
    if career && from_map {
        v.push(opt(fe_ids::RETURN_TO_WORLD_MAP_ICON, fe_ids::RETURN_TO_WORLD_MAP_LABEL, Action::ReturnToWorldMap));
    }
    v.push(opt(fe_ids::NEXT_RACE_AND_RESUME_RACE_ICON, fe_ids::RESUME_RACE_LABEL, Action::ResumeRace));
    v.push(opt(fe_ids::RESTART_ICON, fe_ids::RESTART_LABEL, Action::Restart));
    if career {
        v.push(opt(fe_ids::RETURN_TO_EXPLORE_ICON, fe_ids::MENUS_RETURN_TO_EXPLORE_LABEL_3C0D, Action::ReturnToExplore));
    }
    v.push(opt(fe_ids::OPTIONS_ICON, fe_ids::OPTIONS_LABEL, Action::Options));
    if let Some(on) = replay {
        v.push(opt(fe_ids::REPLAY_AND_UNAVAILABLE_ICON, fe_ids::TXT_REPLAY, if on { Action::Replay } else { Action::Unavailable }));
    }
    photo_mode_item(&mut v, photo);
    v.push(opt(fe_ids::QUIT_TO_MAIN_MENU_ICON, fe_ids::QUIT_TO_MAIN_MENU_LABEL, Action::QuitToMainMenu));
    v
}

/// UI_postrace (0x4BEE10): career "Next Race" (when 0x513FA0 offers
/// one), "Return to Explore Mode", "Return to World Map" (from the map);
/// then "Restart", "Race Stats", "Replay" (outside split screen, game mode
/// 4, when the replay manager 0x8650B4 holds a recording: +0x50EC set and
/// +0xBC > 0), "Quit to Main Menu".
pub fn post_race_menu(career: bool, next: bool, from_map: bool, replay: bool) -> Vec<IconOption> {
    let mut v = Vec::new();
    if career {
        if next {
            v.push(opt(fe_ids::NEXT_RACE_AND_RESUME_RACE_ICON, fe_ids::NEXT_RACE_LABEL, Action::NextRace));
        }
        v.push(opt(fe_ids::RETURN_TO_EXPLORE_ICON, fe_ids::MENUS_RETURN_TO_EXPLORE_LABEL_BCDC, Action::ReturnToExplore));
        if from_map {
            v.push(opt(fe_ids::RETURN_TO_WORLD_MAP_ICON, fe_ids::RETURN_TO_WORLD_MAP_LABEL, Action::ReturnToWorldMap));
        }
    }
    v.push(opt(fe_ids::RESTART_ICON, fe_ids::RESTART_LABEL, Action::Restart));
    v.push(opt(fe_ids::RACE_STATS_ICON, fe_ids::RACE_STATS_LABEL, Action::RaceStats));
    if replay {
        v.push(opt(fe_ids::REPLAY_AND_UNAVAILABLE_ICON, fe_ids::TXT_REPLAY, Action::Replay)); // "Replay"
    }
    v.push(opt(fe_ids::QUIT_TO_MAIN_MENU_ICON, fe_ids::QUIT_TO_MAIN_MENU_LABEL, Action::QuitToMainMenu));
    v
}

/// UI_Main's objects the menu code drives.
pub(crate) mod ui_main {
    /// The title at the top right ("Main Menu").
    pub const TITLE: u32 = crate::frontend::fe_ids::UI_MAIN_TITLE;
    /// The option name under the strip.
    pub const OPTION_TITLE: u32 = crate::frontend::fe_ids::UI_MAIN_OPTION_TITLE;
    /// The selected icon's slot (an untextured 118×59 image).
    pub const SLOT: u32 = crate::frontend::fe_ids::OPTION_MASTER;
    /// Player name and cash in the footer.
    pub const PLAYER_NAME: u32 = crate::frontend::fe_ids::PLAYER_NAME;
    pub const CASH: u32 = crate::frontend::fe_ids::UI_MAIN_CASH;
}

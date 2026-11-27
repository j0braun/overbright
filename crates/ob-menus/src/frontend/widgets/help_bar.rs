//! The PC help bar and the per-screen button masks.

use crate::frontend::fe_ids;
use crate::formats::hash::fe_hash;
use crate::frontend::{Assets, Quad, ScreenState, layout};

/// UI_PC_Help_Bar (screen record 0x7F7EA8, factory 0x552290 sets
/// 0x8384DC, constructor 0x551F90 sets the help bar 0x8384D8): its slots
/// are the objects `button%d` (a string) and `icon_%d` (an image) for 1..6,
/// appended in that order to the list at +0x4C (0x551FF1). 0x54E6F0(mask)
/// runs only while the bar is on (+0x6C, set by 0x54E6B0 — UI_Main's
/// constructor 0x4DF2ED among others — and cleared by 0x54E6C0): it sets
/// the player name (0x537B90: `EB406FEC` = "%s" of the profile name), hides
/// every slot, then gives each set bit, low to high (but 0x8000, 0x10000,
/// 0x20000 and 0x80000000), the next slot with the bit's label (0x52D340)
/// and shows it. A mask with neither 0x8000 nor 0x10000 shows "RealGroup"
/// (0x3B949CD0: the name, cash and their backing) and its children. Every
/// screen's base constructor (0x551DF0) calls it with its record's +0xC
/// when the package is up. `labels` are the labels in bit order (`help_labels`).
/// `career`: in game mode 1, the cash text and when the name / cash
/// alternation started (`HelpCycle`).
pub fn help_bar(assets: &Assets, labels: &[u32], mask: u32, name: Option<&str>, now: f32, opened: f32) -> Vec<Quad> {
    help_bar_in(assets, labels, mask, name, None, now, opened, None)
}

/// The career's name / cash alternation on the help bar (**verified**,
/// 2026-10-29). In game mode 1 the bar's constructor (0x551F90) and
/// 0x54E6F0 (for a mask with bit 1 and not bit 2) call 0x537E70: it finds
/// the name (EB406FEC) and the cash (EB3A688A, both string objects) and
/// 0x51DDA0(name, cash, 0.75, 5.0) starts the cycle once (only while +0x3C,
/// the object shown, is 0): +0x34 name, +0x38 cash, +0x3C = the name,
/// +0x40 0.75 s, +0x44 5.0 s, +0x48 the countdown 0.75 × 4000 + 0.5 (in
/// 0.25 ms ticks), Fade_In on the name and Fade_Out on the cash; then the
/// texts: the profile name ("%s") and the cash ("%$d" of 0x861E74, 0x537BF0).
/// The bar's tick (0x537F60 on 0xC98356BA, from its handler at 0x55BBDD)
/// counts +0x48 down by the frame's step (0x577320 with 0x865194:
/// `_ftol(dt × 4000)`, truncated: 66, 64, 68 a RACE tick — its own
/// countdown, not the real-time clock 0x8651A8, which rounds to 67, 64,
/// 69) and when it reaches 0 or less (0x577341..0x577358) sends Fade_Out
/// to the one shown and Fade_In to the other, which shows for its time:
/// the cash 5.0 s (+0x44), the name 0.75 s (+0x40), +0x48 =
/// `_ftol(time × 4000 + 0.5)` again (0x537FCA..0x537FE2); the cash text is
/// refreshed at each switch. Fade_In (0xBCC00F05) and Fade_Out
/// (0x54C20A66) are 320 ms scripts in the package. A mask with bit 2
/// (online) hides both and stops the cycle. (**traced**; before, the
/// switches were 750 / 5000 ms of the package clock.)
#[derive(Clone, Copy, Debug)]
pub struct HelpCycle {
    /// +0x3C: the cash is the one shown (else the name).
    pub cash: bool,
    /// When the shown one's Fade_In (and the other's Fade_Out) started
    /// (ms, the screens' clock).
    pub since: f32,
    /// +0x48: the countdown, 1/4000 s.
    pub count: i32,
}

impl HelpCycle {
    /// 0x51DDA0's times: the name's (+0x40) and the cash's (+0x44), ms.
    pub const NAME_MS: f32 = 750.0;
    pub const CASH_MS: f32 = 5000.0;

    /// `_ftol(t × 4000 + 0.5)` for a shown time of `ms`.
    fn count_for(ms: f32) -> i32 {
        (f64::from(ms / 1000.0) * f64::from(4000.0f32) + 0.5) as i32
    }

    /// 0x51DDA0 at `now` (ms): the name fades in, the cash out, +0x48 the
    /// name's 3000.
    pub fn new(now: f32) -> HelpCycle {
        HelpCycle { cash: false, since: now, count: Self::count_for(Self::NAME_MS) }
    }

    /// The cycle as if started at `start` and run on continuously to `now`
    /// (ms): the harness's shots set a moment of the cycle this way.
    pub fn started(start: f32, now: f32) -> HelpCycle {
        let period = Self::NAME_MS + Self::CASH_MS;
        let t = (now - start).max(0.0);
        let k = (t / period).floor();
        let base = start + k * period;
        let (cash, since, len) = if t - k * period < Self::NAME_MS {
            (false, base, Self::NAME_MS)
        } else {
            (true, base + Self::NAME_MS, Self::CASH_MS)
        };
        HelpCycle { cash, since, count: (((since + len - now) * 4.0).round() as i32).max(1) }
    }

    /// 0x537F60 on a tick of quantised dt `dt` at `now` (ms).
    pub fn step(&mut self, dt: f32, now: f32) {
        self.count -= crate::game_clock::countdown_step(dt);
        if self.count <= 0 {
            self.cash = !self.cash;
            self.since = now;
            self.count = Self::count_for(if self.cash { Self::CASH_MS } else { Self::NAME_MS });
        }
    }

    /// Whether the cash is the one shown and when the current one's
    /// Fade_In (and the other's Fade_Out) started.
    pub fn phase(&self) -> (bool, f32) {
        (self.cash, self.since)
    }
}

/// `hover`: the slot under the cursor and since when (`help_hover_script`).
pub fn help_bar_in(
    assets: &Assets,
    labels: &[u32],
    mask: u32,
    name: Option<&str>,
    career: Option<(&str, HelpCycle)>,
    now: f32,
    opened: f32,
    hover: Option<(usize, f32)>,
) -> Vec<Quad> {
    let Some(package) = assets.package("UI_PC_Help_Bar") else { return Vec::new() };
    // INIT's chains run (the cash's INIT → HIDE: out of the career no code
    // touches it, so it stays hidden; 0x537E70 fades it in the career).
    let mut state = ScreenState { chain_init: true, ..ScreenState::default() };
    for o in &package.objects {
        state.hidden.insert(o.hash);
    }
    if mask & 0x1_8000 == 0 {
        // RealGroup and everything under it (0x54E6F0 shows the group and
        // its children; the name's own scripts do the rest).
        let mut guids: std::collections::HashSet<u32> =
            package.objects.iter().filter(|o| o.hash == REAL_GROUP).map(|o| o.guid).collect();
        loop {
            let more: Vec<u32> = package
                .objects
                .iter()
                .filter(|o| o.parent.is_some_and(|p| guids.contains(&p)) && !guids.contains(&o.guid))
                .map(|o| o.guid)
                .collect();
            if more.is_empty() {
                break;
            }
            guids.extend(more);
        }
        for o in package.objects.iter().filter(|o| guids.contains(&o.guid)) {
            state.hidden.remove(&o.hash);
        }
        // Its parent group (A22ED3B7) is not hidden by any code either.
        let mut parent = package.objects.iter().find(|o| o.hash == REAL_GROUP).and_then(|o| o.parent);
        while let Some(g) = parent {
            let Some(o) = package.objects.iter().find(|o| o.guid == g) else { break };
            state.hidden.remove(&o.hash);
            parent = o.parent;
        }
        state.hidden.insert(OL_ICON_GROUP);
        // OL_ICON_GROUP (0x2BAC0CEE) is the online icons' (masks 1 / 2
        // with game modes 8 / 0x100; 0x54E6C0 hides it): not under
        // RealGroup, so it stays hidden.
    }
    if let Some(name) = name {
        state.text.insert(PLAYER_NAME, name.to_string());
    }
    if let Some((cash, cycle)) = career {
        if mask & 2 == 0 {
            // 0x51DDA0 / 0x537F60: one fades in, the other out (times
            // relative to the screen, as `layout` runs from `opened`).
            let (cash_shown, since) = cycle.phase();
            let (on, off) = if cash_shown { (HELP_CASH, PLAYER_NAME) } else { (PLAYER_NAME, HELP_CASH) };
            state.scripts.insert(on, (FADE_IN, since - opened));
            state.scripts.insert(off, (FADE_OUT, since - opened));
            state.text.insert(HELP_CASH, cash.to_string());
        } else {
            state.hidden.insert(PLAYER_NAME);
            state.hidden.insert(HELP_CASH);
        }
    }
    // 0x5428C0(slot, mask) for each slot given a label: with 0x10000 the
    // slot's text and icon run a layout script (0x51DBF0) — "In_Game" with
    // 0x8000, else "Event_Results" with 0x80000000, else "Pause_Options"
    // without 0x20000, else "Pause_Menu_Main" — and RealGroup goes
    // (0x4954E0); otherwise "Init" (**verified**, 2026-10-29). In_Game
    // puts slot 1 bottom left and slot 2 bottom right (the in-game World
    // Map's Back / Deactivate GPS, the SMS screens' buttons under Wine).
    let slot_script = (mask & 0x1_0000 != 0).then(|| match () {
        _ if mask & 0x8000 != 0 => fe_ids::IN_GAME,      // In_Game
        _ if mask & 0x8000_0000 != 0 => fe_ids::EVENT_RESULTS, // Event_Results
        _ if mask & 0x2_0000 == 0 => fe_ids::PAUSE_OPTIONS,    // Pause_Options
        _ => fe_ids::PAUSE_MENU_MAIN,                            // Pause_Menu_Main
    });
    for (i, &label) in labels.iter().take(6).enumerate() {
        let (text, icon) = (fe_hash(&format!("button{}", i + 1)), fe_hash(&format!("icon_{}", i + 1)));
        state.hidden.remove(&text);
        state.hidden.remove(&icon);
        state.shown.extend([text, icon]);
        state.text.insert(text, assets.strings.get(label).unwrap_or_default().to_string());
        if let Some(script) = slot_script {
            state.scripts.insert(text, (script, 0.0));
            state.scripts.insert(icon, (script, 0.0));
        }
        // The hovered slot's icon pulses (0x52D500, see `help_hover_script`);
        // leaving runs the slot's +0x20 script again ("Init", or the
        // layout script above), which is the state drawn without a hover.
        if let Some((_, since)) = hover.filter(|&(k, _)| k == i) {
            state.scripts.insert(icon, (help_hover_script(mask), since - opened));
        }
    }
    // Wide screens (clone deviation, `display::Place`): only the
    // screen-level bar (RealGroup's strip drawn) goes to the screen's
    // bottom corners; without it the buttons stay with the screen's
    // panel (`layout` → `element_place`, which the mouse's rectangles
    // follow).
    layout(assets, package, &state, now - opened)
}

/// The help bar's hover script (bar +0x60, **verified**): the bar's
/// handler 0x55DFB0 runs 0x52D500 each tick, which hit-tests each enabled
/// slot's icon (slot +0x14 = `icon_<k>`, 0x52D030) and, on the tick the
/// cursor enters, runs +0x60 on it (0x51CFF0; slot +0xC set); on leaving
/// it runs the slot's +0x20 script by name (0x51CF80). +0x60 is
/// "Init_Pulse" (0xA3E4657B; the constructor 0x5521FE and 0x5428C0 without
/// 0x10000, which also sets +0x20 = "Init"); with 0x10000 0x5428C0 gives
/// the layout's own pulse: In_Game 0x339F3277, Event_Results 0xDD0F9CDA,
/// Pause_Menu_Main 0x40AAAC1D, Pause_Options 0x1D39C830 (each icon's
/// 1200 ms looping script; +0x20 the layout's name).
pub fn help_hover_script(mask: u32) -> u32 {
    if mask & 0x1_0000 == 0 {
        fe_ids::INIT_PULSE
    } else if mask & 0x8000 != 0 {
        0x339F_3277
    } else if mask & 0x8000_0000 != 0 {
        0xDD0F_9CDA
    } else if mask & 0x2_0000 != 0 {
        0x40AA_AC1D
    } else {
        0x1D39_C830
    }
}

/// UI_PC_Help_Bar's "RealGroup" and the player name in it.
pub(crate) const REAL_GROUP: u32 = fe_ids::REAL_GROUP;
const PLAYER_NAME: u32 = fe_ids::PLAYER_NAME;
/// The help bar's cash (string object, INIT → HIDE).
const HELP_CASH: u32 = fe_ids::UI_MAIN_CASH;
/// The name's and cash's scripts "Fade_In" / "Fade_Out" (upper-case hashes).
const FADE_IN: u32 = fe_ids::FADE_IN;
const FADE_OUT: u32 = fe_ids::FADE_OUT;
const OL_ICON_GROUP: u32 = fe_ids::OL_ICON_GROUP;

/// The help-bar mask of a screen (its record's +0xC, records 0x7F7DC8 …
/// 0x7F915C, stride 0x1C: name, factory, help hash, mask, remembered
/// option), for the front-end screens the clone has.
pub fn help_mask(package: &str) -> Option<u32> {
    Some(match package {
        "UI_Main" | "UI_PerformanceBrandSelect" => 0x5,
        "UI_OptionsMain"
        | "UI_PC_LAN"
        | "UI_CareerCrib"
        | "UI_CribRewardOptionsMain"
        | "UI_CareerCarSelect"
        | "UI_StartCareer"
        | "UI_MagazineSelect"
        | "UI_CareerCarLot"
        | "UI_QRModeSelect"
        | "UI_QRModeOptions"
        | "UI_QRTrackSelect"
        | "UI_ChooseCustomizeCategory"
        | "UI_ChoosePerformanceCategory"
        | "UI_GenericParts_Browser"
        | "UI_Paint"
        | "UI_ChooseVinylLayer"
        | "UI_DecalMain"
        | "UI_DecalsOverlay"
        | "UI_ChooseCustomHUD"
        | "UI_ChooseRimBrand"
        | "UI_Rims_Browser"
        | "UI_ChooseSpinner"
        | "UI_ICEMAIN"
        | "UI_CustomNeonMain"
        | "UI_ProfileManager"
        | "UI_OLISPConnect"
        | "UI_OLEALogin" => 0x405,
        "UI_Options"
        | "UI_Wheel_Options"
        | "UI_PC_Customize_Options"
        | "UI_Trailers"
        | "Credits"
        | "UI_RewardsSponsor"
        | "EA_Trax_Jukebox" => 0x401,
        "UI_Options_PC_Controller" => 0xC01,
        "UI_PC_LAN_ServerSelect" => 0x4000_0405,
        "UI_CareerWorldMap" => 0x8401,
        "UI_Status_Master" | "UI_InGame_WorldMap" => 0x1_8400,
        "UI_QRCarSelect" | "UI_OLCarSelect" => 0x1405,
        "UI_ChoosePaintCategory" => 0x605,
        "UI_ChoosePerformancePackage" => 0x441,
        "UI_BuyPerformanceParts"
        | "UI_ChooseUniquePart"
        | "UI_IcePartsOverlay"
        | "UI_NeonPartsOverlay"
        | "UI_CustomHUDOverlay" => 0x409,
        "UI_PerformanceDyno_MAIN"
        | "UI_PerformanceTuning_Master"
        | "UI_PerformanceTuning_Setting"
        | "UI_PerformanceDyno_Results" => 0x404,
        "UI_PerformanceTuning_Graph" | "UI_PerformanceTuning_NOS" | "UI_PerformanceTuning_Drivetrain" => 0x40_0C00,
        "UI_PerformanceTuning_Sliders" => 0xC00,
        "UI_PerformanceDyno_Chart" => 0x4,
        "MC_List" | "MC_Main" => 0x1,
        // The in-game screens' records (0x7F81B8 …, 0x7F8A5C).
        "UI_pause" | "UI_Pause" | "UI_PauseOptionsMain" | "UI_postrace" | "UI_PostRace" => 0x3_0404,
        "UI_PauseOptions" | "UI_PostRaceStats" => 0x1_0400,
        "UI_EngageMessageDialog" => 0x1_8404,
        "UI_SMS_Mailbox" => 0x1_A500,
        "UI_PostRaceResults" | "UI_PostRace_TournResults" => 0x8001_0004,
        "UI_PostRace_TournStandings" => 0x4,
        "UI_PostRaceReward" => 0x404,
        "UI_SponsorPopup" | "UI_Sponsorship_new" => 0x400,
        _ => return None,
    })
}

/// Every package's init (0x54E4B0, "Init Package %s") hides the objects
/// flagged 0x40 (the visitor 0x51C870: 0x50CA00, and flag 0x400000):
/// on PC the console button glyphs and footers go, the help bar
/// (UI_PC_Help_Bar) standing in for them. The clone applies it where
/// noted.
pub fn package_init(state: &mut ScreenState, package: &crate::frontend::feng::Package) {
    for o in package.objects.iter().filter(|o| o.flags & 0x40 != 0) {
        state.hidden.insert(o.hash);
    }
}

/// The help bar's labels for a mask (0x52D340), in bit order; `dyno`
/// (g_IsDynoRun 0x89E7E6, read by 0x52D340 at 0x52D43F) picks 0x800's and `vinyl_layer` (UI_ChooseVinylLayer up)
/// 0x800000's.
pub fn help_labels(mask: u32) -> Vec<u32> {
    help_labels_in(mask, false, false)
}

pub fn help_labels_in(mask: u32, dyno: bool, vinyl_layer: bool) -> Vec<u32> {
    (0..32)
        .map(|b| 1u32 << b)
        .filter(|&b| mask & b != 0 && !matches!(b, 0x8000 | 0x1_0000 | 0x2_0000 | 0x8000_0000))
        .map(|b| match b {
            0x20 => fe_ids::INSTALL_DECAL,        // Install Decal
            0x2 => fe_ids::LOGOFF,         // Logoff
            0x1 => fe_ids::QUIT_GAME,         // Quit Game
            0x10 => fe_ids::INSTALL_VINYL,        // Install Vinyl
            0x8 => fe_ids::INSTALL_PART,         // Install Part
            0x40 => fe_ids::INSTALL_PACKAGE,        // Install Package
            0x80 => fe_ids::INSTALL_PAINT,        // Install paint
            0x100 => fe_ids::READ_MESSAGE,       // Read Message
            0x200_0000 => fe_ids::SELECT_EVENT,  // Select Event
            0x400_0000 => fe_ids::HIDE_ON_WORLD_MAP,  // Hide on World Map
            0x800_0000 => fe_ids::SHOW_ON_WORLD_MAP,  // Show on World Map
            0x1000_0000 => 0x4351_2519, // Activate GPS
            fe_ids::OBJ_20000000 => 0x1115_9F62, // Deactivate GPS
            0x4000_0000 => 0x80AF_4FC7, // Host LAN Server
            0x200 => fe_ids::CANCEL_CHANGES,       // Cancel Changes
            0x400 => fe_ids::HELP_BAR_BACK,       // Back
            0x4_0000 => fe_ids::DELETE_TUNED_CAR,    // Delete Tuned Car
            0x800 if dyno => 0xAD89_EA7B,
            0x800 => fe_ids::RESET_KEYS_TO_DEFAULT,      // Reset Keys To Default
            0x1000 => fe_ids::HELP_BAR_CUSTOMIZE,     // Customize
            0x2000 => fe_ids::HELP_BAR_DELETE,     // Delete
            0x4000 => fe_ids::EA_MESSENGER,     // EA Messenger
            0x8_0000 => fe_ids::TEST_N2O_PURGE,   // Test N2O Purge
            0x10_0000 => fe_ids::OPEN_CLOSE_HOOD,  // Open/Close Hood
            0x20_0000 => fe_ids::OPEN_CLOSE_DOORS,  // Open/Close Doors
            0x100_0000 => fe_ids::RESTART_LABEL, // Restart
            0x40_0000 => fe_ids::TIP,  // Tip
            0x80_0000 if vinyl_layer => 0xF667_659C,
            0x80_0000 => fe_ids::DECAL_COLOR, // Decal Color
            _ => 0x9F1B_2C7A,         // Continue (bit 4; the fallback)
        })
        .collect()
}

#[cfg(test)]
mod real_time_clock_tests {
    use super::HelpCycle;
    use crate::frame_clock::{RACE, TickClock, UNITS};

    /// 0x537F60 counts +0x48 down by `_ftol(dt × 4000)` (66, 64, 68
    /// a RACE tick) and switches when it reaches 0 or less, the name for
    /// 3000, the cash for 20 000: 46 and 304 ticks at 60 Hz (the package
    /// clock's 750 / 5000 ms took 46 and 306).
    #[test]
    fn help_cycle_counts_its_own_countdown() {
        let mut c = TickClock::default();
        let dts: Vec<f32> = (0..1200).flat_map(|_| c.frame_units(UNITS / 60, |_| RACE).0).collect();
        let mut h = HelpCycle::new(0.0);
        let mut switches = Vec::new();
        for (k, &dt) in dts.iter().enumerate().skip(3) {
            let was = h.cash;
            h.step(dt, k as f32);
            if h.cash != was {
                switches.push(k);
                assert_eq!(h.since, k as f32);
            }
        }
        let gaps: Vec<usize> = switches.windows(2).map(|w| w[1] - w[0]).collect();
        assert_eq!(switches[0] - 2, 46, "the name's 3000");
        assert!(gaps.iter().step_by(2).all(|&g| (303..=304).contains(&g)), "the cash's 20 000: {gaps:?}");
        assert!(gaps.iter().skip(1).step_by(2).all(|&g| (45..=46).contains(&g)), "the name's 3000: {gaps:?}");
        // The harness's continuous model: 1000 ms in, the cash shown since
        // 750, 4750 ms left (19 000 ticks).
        let s = HelpCycle::started(0.0, 1000.0);
        assert!(s.cash && s.since == 750.0 && s.count == 19_000);
    }
}

pub(crate) mod dialogs;
pub(crate) mod keys;
pub(crate) mod model;
pub(crate) mod icon_menus;
pub(crate) mod save_flow;
pub(crate) mod dialogs_impl;
pub(crate) mod car_select;
pub(crate) mod actions;
pub(crate) mod garage_view;
pub(crate) mod fe_tick;

use crate::*;

/// The front end's own state between screens: remembered options,
/// the help bar, dialogs' undim and the boot's flow (were `Game` fields).
pub(crate) struct FeState {
    /// UI_Main's remembered option, 1-based: the screen record's +0x10
    /// (0x7F7DD8, 0 at start), written by IconScrollerMenu_Handler on each
    /// move (0x543F47 / 0x543F99 → 0x50B8C0) and read by UI_Main_Options
    /// (0x4AEE30: 0 → 2, Quick Race).
    pub(crate) main_selection: usize,
    /// The help bar's player name when no profile is loaded (the shots
    /// that drop the profile keep its name here).
    pub(crate) help_name: Option<String>,
    /// The help bar's name / cash cycle (+0x34..+0x48, 0x51DDA0), once
    /// started in the career.
    pub(crate) help_cycle: Option<screens::HelpCycle>,
    /// UI_PC_LAN_ServerSelect's search timer +0x900: the real-time clock
    /// 0x8651A8 (1/4000 s) it ends after (`lan_search_until`).
    pub(crate) lan_search_until: u64,
    /// The help bar's slot under the cursor and since when (0x52D500's
    /// slot +0xC; `screens::help_hover_script`). Set by the tick's mouse
    /// (`mouse_help_bar`), read by the layout (a plain field, not
    /// a `Cell`).
    pub(crate) help_hover: Option<(usize, f32)>,
    /// UI_Menu_Asset_Reputation (the Visual Rating widget) is up: pushed by
    /// 0x557ED0 / 0x55B1B0 when a shop list shows it and left up when Back
    /// returns to a package 0x55BCA0 lists. A career shop entered from free
    /// roam starts without it (Wine, IND_BODYSHOP_1: no widget on the Body
    /// Shop strip until a parts list was opened, then kept on Back).
    pub(crate) rating_up: std::cell::Cell<bool>,
    /// A screen button under the cursor and since when (its hover pulse:
    /// UI_MagazineReward / UI_MagazineView / UI_ShowCase_Preview). Set by
    /// the tick's mouse (`set_fe_hover`; not a `Cell`).
    pub(crate) fe_hover: Option<(u32, f32)>,
    /// 0x836360: the save-prompts option as the Options opened (0x4AECE0).
    pub(crate) options_prompts: bool,
    /// The save-point snapshot with no profile (0x52A470 snapshots the
    /// game's memory state, not a profile): the options as UI_Main's
    /// Options opened (0x4AECE0), with save prompts on.
    pub(crate) options_settings: Option<options::Settings>,
    /// 0x836495: the splash's prompt has come up this session.
    pub(crate) splash_seen: bool,
    /// The cheat codes typed at the title screen (the manager 0x865930's
    /// ring; its input is on while UG_LS_Splash is up).
    pub(crate) cheat_input: nfsu2::frontend::cheats::Input,
    /// The bank of the career in memory while no profile is current
    /// (0x861E74): the boot's reset (0x60384F → 0x532840) leaves it 0, and
    /// a bonus code on the title pays into it (0x5760B0). A profile
    /// created with none current (MC_Main state 2) keeps that career
    /// (`cheats::creation_cash`).
    pub(crate) boot_cash: u32,
    /// A "quit to main menu" question is open.
    pub(crate) quitting: bool,
    /// The screen a dimmed dialog was over, once the dialog is answered
    /// back to it: its UNDIM runs from then (see [`Undim`]).
    pub(crate) undim: Option<Undim>,
    /// The screen an MC_Main operation was started from (0x4A3100 with
    /// mode & 0xF = 1: 0x96DDA9E6 to the caller, its StartDim, then the
    /// MC package pushed over it, 0x555EC0): drawn dimmed under the MC
    /// screens while they are up, from `.2` (ms).
    pub(crate) mc_under: Option<(Box<Mode>, &'static str, f32, (u32, u32))>,
    /// The screen a save point's MC_Main (mode & 0xF = 1) is pushed over
    /// (0x4A3100: 0x96DDA9E6 to it, then 0x555EC0(MC_Main, it, 0xFF, 0)),
    /// kept from the question answered just before: the shops' exit
    /// question (0x552C90, mode 0x41) and the dyno's save questions (0x51,
    /// 0x151); with its dim's package, start and scripts.
    pub(crate) save_under: Option<(Box<Mode>, &'static str, f32, (u32, u32))>,
    /// The Visual Rating widget pushed by a step in a neon / ICE parts
    /// list (0x55C180 → 0x55B1B0), with the screen it was pushed on; gone
    /// at the next screen change (`shop_rating_quads_of`).
    pub(crate) overlay_rating: Option<&'static str>,
    /// What the boot does with the profiles once the splash is passed
    /// (see `profiles::boot`); taken then.
    pub(crate) boot_profiles: Option<profiles::Boot>,
    /// The boot's list or create flow is under way (it ends on UI_Main).
    pub(crate) from_boot: bool,
    /// MC_Background (PC_load.tga, full screen) is the base package under
    /// the boot's memory-card screens (0x4C4F50 → 0x537990; seen under Wine).
    pub(crate) mc_background: bool,
    /// An in-game option's UI_InGameDialog was answered OK: run it.
    pub(crate) confirmed: bool,
    /// The Custom Gauges overlay (UI_CustomHUDOverlay, the specialties
    /// browser on Custom Gauges) under UI_ChooseCustomHUD: the picker is
    /// embedded in it (0x555D10 mode 4), so the overlay's header, strip,
    /// gauge list and name stay drawn (seen under Wine, menus112 h3).
    pub(crate) hud_overlay: Option<Box<Menu>>,
    /// A "Reset to default settings?" dialog is up (UI_Options, Display).
    pub(crate) resetting_display: bool,
}

impl FeState {
    /// The values `Game::new` starts with (`boot_profiles`: what the boot
    /// does with the profiles once the splash is passed).
    pub(crate) fn new(boot_profiles: profiles::Boot) -> FeState {
        FeState {
            options_prompts: false,
            options_settings: None,
            main_selection: 0,
            help_name: None,
            help_cycle: None,
            lan_search_until: 0,
            help_hover: None,
            rating_up: std::cell::Cell::new(true),
            fe_hover: None,
            splash_seen: false,
            cheat_input: Default::default(),
            boot_cash: 0,
            quitting: false,
            undim: None,
            mc_under: None,
            save_under: None,
            overlay_rating: None,
            boot_profiles: Some(boot_profiles),
            from_boot: false,
            mc_background: false,
            confirmed: false,
            hud_overlay: None,
            resetting_display: false,
        }
    }
}

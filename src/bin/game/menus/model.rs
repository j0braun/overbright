use crate::*;

pub(crate) enum Mode {
    /// A menu over a black background.
    Menu(Menu),
    /// Driving around Bayview.
    Explore,
    /// Racing an event.
    Race(Race),
}

pub(crate) enum Menu {
    Icons(IconMenu, MenuKind),
    CarLot(CarLot),
    /// What a car lot dialog's answer settles (never shown).
    LotDeal(LotDeal),
    WorldMap(WorldMap),
    /// UI_InGame_WorldMap over paused free roam (the World Map key).
    InGameMap(InGameWorldMap),
    QuickRace(QuickRace),
    /// UI_QRModeOptions, "Quick Race Game Options".
    QrOptions(nfsu2::frontend::quick_race::QrOptions),
    /// A dialog and what each of its buttons leads to.
    Dialog(Dialog, Vec<Mode>),
    /// MC_Main creating the profile for Start New Career.
    McMain(McMain),
    /// MC_List: the profiles to load or delete.
    McList(McList),
    /// An MC_Main message and what each button does.
    McMessage(McMessage, Vec<McAct>),
    /// UI_PostRaceReward, then the post-race menu.
    Reward(Reward),
    /// UI_EngageRaceDialog at an event's marker.
    Engage(Engage),
    /// UI_EngageShopDialog at a shop's marker.
    ShopEngage(ShopEngage),
    /// A world challenge's dialogs: UI_EngageEventDialog at its start
    /// marker, UI_EngageShopDialog's retry when its time ran out.
    Challenge(screens::ChallengeDialog),
    /// UI_GenericParts_Browser: a slot's parts.
    Parts(PartsBrowser),
    /// UI_Paint.
    Paint(screens::PaintShop),
    /// UI_ChooseCustomHUD's colour steps over the Custom Gauges overlay.
    ChooseHud(nfsu2::frontend::custom_hud::ChooseCustomHud),
    /// UI_PostRaceResults, then (a career win) the reward.
    Results(PostRaceResults, Option<Reward>),
    /// UI_MagazineView: a magazine or DVD cover.
    Cover(screens::CoverView),
    /// UI_ShowCase_Preview: a kind-1 / kind-2 photo shoot.
    Showcase(screens::ShowcasePreview),
    /// UI_PerformanceDyno_Chart: the dyno run.
    DynoChart(screens::DynoChart),
    TuneGraph(screens::TuneGraph),
    /// UI_PerformanceDyno_Results.
    DynoResults(screens::DynoResults),
    /// UI_PerformanceTuning_Sliders / _Drivetrain.
    TuneSliders(screens::TuneSliders),
    /// UI_PostRaceStats.
    Stats(screens::PostRaceStats),
    /// UI_Options: a category of options.
    Options(options::OptionsScreen),
    /// UI_SponsorPopup ("SELECT A SPONSOR!"), opened at this time.
    SponsorPopup(f32),
    /// UG_LS_Splash, the boot's title screen.
    Splash(screens::Splash),
    /// UI_OLISPConnect trying the network, opened at this time.
    IspConnect(f32),
    /// UI_PC_LAN_ServerSelect, opened at this time, and whether its
    /// "Looking for LAN servers ..." dialog is up.
    LanServers(f32, bool),
    /// UI_OLEALogin, and the connect helper's dialog up over it (0x4CFCB0
    /// / 0x4EFE40): its text and when it opened (NaN: open it now).
    EaLogin(screens::EaLogin, Option<(u32, f32)>),
    /// Credits.fng rolling.
    Credits(screens::Credits),
    /// UI_Options_PC_Controller.
    Controls(screens::ControlsScreen),
    /// EA_Trax_Jukebox.
    Jukebox(screens::Jukebox),
    /// UI_PerformanceBrandSelect, and the purchase it settles.
    BrandSelect(options::BrandSelect, PerfPending),
    /// IG_PlayMovie.fng playing the movie queue.
    Movie(MovieScreen),
    /// UI_SMS_Mailbox (game state 0xD, 0x4976B0).
    SmsMailbox(nfsu2::frontend::sms::Mailbox),
    /// UI_EngageMessageDialog: one message.
    SmsViewer(nfsu2::frontend::sms::Viewer),
    /// UI_Status_Master over paused free roam (the Career Statistics key).
    Status(Box<StatusMenu>),
}

/// UI_Status_Master with the statistics its factory filled (0x4E1B80 →
/// 0x528A70) and UI_Status_Career's rows (0x4E13C0).
#[derive(Clone)]
pub(crate) struct StatusMenu {
    pub(crate) screen: nfsu2::frontend::status::StatusScreen,
    pub(crate) stats: nfsu2::frontend::status::CareerStats,
    pub(crate) rows: Vec<nfsu2::frontend::status::Row>,
}

/// What the car lot's dialogs settle (the Yes messages of 0x4ED9E0).
#[derive(Clone, Debug)]
/// What a memory-card message's button does (MC_Main's states, 0x4E8D30).
pub(crate) enum McAct {
    /// Load the profile (state 4 → the read, 0x4A9750 / 0x4A9850 → the
    /// loader 0x534780), then "Load complete.".
    Load(String),
    /// Delete the profile (op 0x30), then "Save game deleted.".
    Delete(String),
    /// The overwrite question when the file exists, else the save.
    SaveAsk(String),
    /// Save the profile (state 6/7: 0x4D9AB0 / 0x4D9B80), then "Save
    /// complete.".
    Save(String),
    /// The list again.
    List(McOp),
    /// A menu.
    To(MenuKind),
    /// A save point's "Yes": "Do you want to overwrite previously saved
    /// game?" when the file exists, then the save and "Save complete.".
    SpOverwrite(AfterSave),
    /// A save point's save; with `true` "Save complete." (Continue) after.
    SpSave(AfterSave, bool),
    /// A save point's "Are you sure you want to save^the profile named
    /// %s?" (MC_Main state 5, 0x4A3C10; No focused): Yes → `SpOverwrite`,
    /// No → `Continue`.
    SpAsk(AfterSave),
    /// No at a save point's overwrite question: back to `SpAsk` (the
    /// library's result → 0xDC12AF2E → 0x4E8D30(2) → state 5); `true`
    /// when it was the first autosave's question (mode 0x100000), whose No
    /// also turns Autosave off (0x844AD9 = 0, the Save option reads
    /// Prompt) and clears the session's card +6 (observed under Wine).
    SpNo(AfterSave, bool),
    /// No at the Profile Manager's overwrite question: its "Are you
    /// sure…" again (Autosave kept: mode 0x4A51 has 0x4000, no 0x100000).
    SaveNo(String),
    /// Past a save point.
    Continue(AfterSave),
    /// Past the cover's load message: UI_MagazineView on milestone's
    /// loaded picture (`true`), else back on the covers list.
    Cover(usize, bool, bool),
}

/// Where the game goes on after a save point (the caller's message after
/// MC_Main: 0x461A18EE, 0x8867412D, or its callback).
#[derive(Clone, Debug)]
pub(crate) enum AfterSave {
    Menu(MenuKind),
    /// The city start at a place (0x5404B0).
    Explore(Option<(Vec3, f32)>),
    /// Out of the garage (0x9A3B571D on UI_CareerCrib, 0x4AF607): career
    /// +0x8744 = 1 (0x4AF616), after the save point, then the city start.
    LeaveGarage(Option<(Vec3, f32)>),
    /// The action again, the save point passed (UI_PostRace's
    /// 0x35E413DD / 0xD78D7BF1).
    Act(Action, MenuKind),
    /// The Quick Race car select kept in `Game::qr_customize`.
    CarSelect,
    /// The dyno's save question's Yes (0x5649E1, mode 0x51): the session
    /// ends after the save (0x53CC80; under Wine, menus14, the MC screens
    /// sit over the dimmed dyno menu and the car on the rollers), then
    /// the crib.
    /// Free roam again as it was (the photo shoot's save question).
    Roam,
    QuitTuning,
    /// Main-menu Customize's Dyno Yes past its save point (0x55E5D0, mode
    /// 0x1141; MC's result → 0x461A18EE): the session's profile back
    /// (`Game::dyno_scratch`), then the dyno.
    CustomizeDyno,
}

pub(crate) enum LotDeal {
    /// "Are you sure you want to quit?"'s Quit Game (0x919B1DD9): the
    /// game ends (**inferred**: the message's handler is not traced).
    QuitGame,
    /// UI_CareerCrib's "Return to Explore Mode?" Yes (0x451E768E →
    /// 0x4AF360): the save point (mode 0x151) when the profile changed
    /// since the garage opened, then the city start.
    LeaveCrib,
    /// The rims' / spinners' buy or install question Yes (0x09F00F07 /
    /// 0x51123592): `Game::fit_browser_part`.
    FitPart {
        package: &'static str,
        sub: Option<u8>,
        category: usize,
        i: usize,
        slot: u8,
        hash: u32,
        price: i32,
        owned: bool,
    },
    /// The carbon fibre convert / revert question's Yes (0x51123592 →
    /// 0x566F12): `Game::carbon_convert` on the strip's category.
    Convert { sub: Option<u8>, category: usize },
    /// The Custom Gauges purchase question's Yes (0x09F00F07 / 0x51123592
    /// in 0x563D10): `Game::fit_custom_gauge` with the picker's picks.
    FitGauge(Box<nfsu2::frontend::custom_hud::ChooseCustomHud>),
    /// A career shop's exit question Yes (0xD05FC3A3 → 0x552C90): the
    /// save point (mode 0x41) when the profile changed, then the city.
    LeaveShop,
    /// Delete Tuned Car's Yes (0x4EF090): My Cars record `record` cleared,
    /// the car select again near `at`.
    DeleteTuned { record: usize, at: usize },
    /// The stock car into the open slot, or in place of a slot's car
    /// (0x541550 → 0x5348F0; 0x496050 → 0x534960); then the save (state
    /// 6) and back to the city (0x9A3B571D → 0x5404B0).
    Take { slot: usize, car: String },
    /// Choose which of the player's cars to trade for this stock one
    /// (0x75D1E35E: state 5, 0x4DF9C0 in list mode 4).
    Browse { car: String },
    /// The crib's Car Select: drive the car in this slot (0xD05FC3A3 →
    /// 0x500BD0), then back to the crib.
    Select { slot: usize },
    /// The world map's "Would you like to race this event?" Yes
    /// (0x51123592): race the event (0x541170 / 0x5325F0).
    Race { event: usize },
    /// The dyno's Mode Select "store it?" Yes (0x2B5DFEF6): the tuning
    /// object's sliders to the mode's settings (0x50FC20), then
    /// UI_PerformanceTuning_Setting.
    StoreTuning,
    /// The mailbox's "delete this message?" Yes (0x01F23C75 → 0x4CCB10):
    /// the entry deleted, then the mailbox anew, or the game when it is
    /// empty (state 3 → 0x4977C0).
    SmsDelete { entry: usize, mailbox: nfsu2::frontend::sms::Mailbox },
    /// "Want to sign on with %s?" Yes (0xD05FC3A3): sign the offered
    /// sponsor `n` (0x532D00), then UI_PostRace (0x4BFF80).
    Sign(usize),
    /// "Quit to Garage?" Yes: commit and leave the dyno (`quit_tuning`).
    QuitTuning,
    /// The test run's Restart OK (0xE1A57D51): the run again.
    TestRunRestart,
    /// The test run's Return to Dyno Yes (0x526ADEFC in 0x5647F0): the car
    /// back on the dyno (0x83A644 = 1, 0x5F4250(10)).
    ReturnToDyno,
    /// "Do you want to enter the Performance Tuning / Dyno track?" (or
    /// the crib's question) Yes: the tuning session (0x5309F0) on the
    /// dyno track (`enter_dyno`).
    EnterDyno,
    /// Quit Tuning's "One of your custom-tuned setups has changed. Would
    /// you like to save your profile now?" (0x71851654): Yes 0x24DD40CF
    /// commits (0x50FAE0) and saves (0x4A3100 mode 0x151), No 0x1FAB5998
    /// commits and quits.
    QuitTuningSave(bool),
    /// The dyno's Save Profile question (0x0D7DE8AB) Yes (0x51123592 →
    /// 0x4A3100 mode 0x51): the save point, then the dyno menu.
    TuningSave,
    /// The in-game World Map's "Are you sure you want to engage the GPS
    /// system?" Yes (0x51123592): the GPS to the selected icon's entry
    /// (0x529490) and the map closes the next tick (+0x60, 0xC98356BA →
    /// 0x5F4250(1)).
    Gps { target: u32, trigger: career::Trigger },
    /// Its "Deactivating GPS" OK (0x34DC1BEC → 0x496CE0): the GPS cleared
    /// (0x5010E0), back on the map with "Activate GPS".
    GpsOff(Box<InGameWorldMap>),
    /// UI_InGameDialog's OK on an in-game option's question: the option
    /// itself (`Game::in_game_question`).
    Confirm(Action, MenuKind),
    /// UI_PauseOptions' save question after PAD_START, Yes (0x33333333 →
    /// 0x4A8164): resume the paused race (true) or free roam.
    Resume(bool),
    /// UI_MagazineReward's "Do you want to save this DVD?" Yes
    /// (0x11223344): the cover written as its own memory-card file
    /// (`profiles::cover_name`), then "Save complete." and free roam.
    SaveCover { milestone: usize, dvd: bool },
    /// In-world kind 1/2 shoot: return to its camera, then capture after
    /// the native five UI ticks instead of generating a frontend cover.
    SaveShowcase(screens::ShowcasePreview),
}

/// The menus, each the game's package with its option set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MenuKind {
    Main,
    Career,
    Crib,
    QrMode,
    Options,
    Rewards,
    Profile,
    ExplorePause,
    RacePause,
    PostRace,
    Sponsor,
    /// UI_ChooseRimBrand.
    RimBrand,
    /// UI_ChoosePaintCategory (the graphics shop).
    Graphics,
    /// UI_ChooseVinylLayer.
    VinylLayer,
    /// UI_ChoosePerformanceCategory.
    PerfCategory,
    /// UI_ChoosePerformancePackage for `perf_category`.
    PerfPackage,
    /// UI_MagazineSelect: the magazine (false) or DVD (true) covers.
    Covers(bool),
    /// UI_RewardsSponsor: the sponsors signed.
    SignedSponsors,
    /// UI_PerformanceDyno_MAIN.
    Dyno,
    /// UI_PerformanceTuning_Master.
    Tuning,
    /// UI_PerformanceTuning_Setting: the tuning mode select.
    TuneMode,
    /// UI_ChooseCustomizeCategory (main-menu Customize).
    CustomizeCategory,
    /// UI_PauseOptionsMain over the race's pause (true) or free roam's.
    PauseOptions(bool),
}

impl MenuKind {
    pub(crate) fn package(self) -> &'static str {
        match self {
            MenuKind::Main => "UI_Main",
            MenuKind::Career => "UI_StartCareer",
            MenuKind::Crib => "UI_CareerCrib",
            MenuKind::QrMode => "UI_QRModeSelect",
            MenuKind::Options => "UI_OptionsMain",
            MenuKind::Rewards => "UI_CribRewardOptionsMain",
            MenuKind::Profile => "UI_ProfileManager",
            MenuKind::ExplorePause | MenuKind::RacePause => "UI_pause",
            MenuKind::PostRace => "UI_postrace",
            MenuKind::Sponsor => "UI_Sponsorship_new",
            MenuKind::RimBrand => "UI_ChooseRimBrand",
            MenuKind::Graphics => "UI_ChoosePaintCategory",
            MenuKind::VinylLayer => "UI_ChooseVinylLayer",
            MenuKind::PerfCategory => "UI_ChoosePerformanceCategory",
            MenuKind::PerfPackage => "UI_ChoosePerformancePackage",
            MenuKind::Covers(_) => "UI_MagazineSelect",
            MenuKind::SignedSponsors => "UI_RewardsSponsor",
            MenuKind::Dyno => "UI_PerformanceDyno_MAIN",
            MenuKind::Tuning => "UI_PerformanceTuning_Master",
            MenuKind::TuneMode => "UI_PerformanceTuning_Setting",
            MenuKind::CustomizeCategory => "UI_ChooseCustomizeCategory",
            MenuKind::PauseOptions(_) => "UI_PauseOptionsMain",
        }
    }

    /// Menus drawn over the running game rather than the front end.
    pub(crate) fn in_game(self) -> bool {
        matches!(self, MenuKind::ExplorePause | MenuKind::RacePause | MenuKind::PostRace | MenuKind::PauseOptions(_))
    }
}

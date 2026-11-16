//! What choosing an option does.

use super::QrMode;

/// What choosing an option does (each option's vtable action; the
/// addresses are the actions' code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    // UI_Main (0x4AEE30)
    Career,
    QuickRace,
    Online,
    Lan,
    Customize,
    ProfileManager,
    Options,
    // UI_StartCareer (0x4AF110)
    ResumeCareer,
    NewCareer,
    LoadCareer,
    // UI_CareerCrib (0x4AF820)
    Explore,
    WorldMap,
    CarSelect,
    ChangeParts,
    Tuning,
    Rewards,
    Save,
    MainMenu,
    // UI_CribRewardOptionsMain
    Magazines,
    DvdCovers,
    Sponsors,
    // UI_MagazineSelect (0x53DE70): view the nth milestone's cover.
    Cover(usize),
    // UI_PerformanceDyno_MAIN (0x5227F0).
    TuneSettings,
    TestRun,
    DynoRun,
    ModeSelect,
    QuitTuning,
    /// The dyno's Save Profile (0x5226D0, react 0x564D70).
    TuningSave,
    /// The test run's pause (UI_PerformanceDyno_MAIN with 0x83A644 = 0,
    /// options 0x53C9F0): Restart (react 0x564C90) and Return to Dyno
    /// (0x564D00), each asking first.
    TestRunRestart,
    ReturnToDyno,
    // UI_PerformanceTuning_Master (0x557690): a tuning category.
    TuningCategory(usize),
    /// UI_PerformanceTuning_Setting's modes (0x557420, option +0x48).
    TuneMode(u8),
    // UI_QRModeSelect (0x4B2D60)
    QrMode(QrMode),
    // UI_OptionsMain (0x4B6240)
    Setting(u32),
    // UI_PauseOptionsMain (0x4C3000): the same handlers (0x49A620…) set
    // the category; the screen's 0xE1FDE1D1 opens UI_PauseOptions.
    PauseSetting(u32),
    /// The pause menu's "Photo Mode" (clone deviation, listed only
    /// with the Enhancements row "Photo Mode in Pause Menu" on).
    PhotoMode,
    // UI_ChooseCustomizeCategory (0x520D70): body 0, performance 1,
    // paint 2, specialties 3.
    CustomizeCategory(u8),
    // UI_ProfileManager (0x4C58C0)
    ProfileSave,
    ProfileCreate,
    ProfileLoad,
    ProfileDelete,
    // UI_pause (0x4C2900 explore, 0x4C2BD0 race)
    ReturnToExplore,
    ReturnToFreeRun,
    JumpToGarage,
    ResumeRace,
    Restart,
    ReturnToWorldMap,
    Replay,
    QuitToMainMenu,
    // UI_postrace (0x4BEE10)
    NextRace,
    RaceStats,
    // UI_Sponsorship_new: sign the nth offered sponsor.
    Sign(usize),
    // An option with nothing to offer.
    Unavailable,
    // An option the screen does nothing with (UI_RewardsSponsor's).
    Inert,
    // UI_ChooseRimBrand (0x546530): the nth brand's rims.
    RimBrand(usize),
    // UI_ChoosePaintCategory (0x548210): Paint, Vinyls, Decals and
    // Purchase / Install Setup (0x562BA0).
    Paint,
    Vinyls,
    Decals,
    PurchaseSetup,
    // UI_ChooseVinylLayer (0x548460, each 0x522140): a layer (0x563110).
    VinylLayer(u8),
    // UI_ChoosePerformanceCategory (0x530500): a category's packages.
    PerfCategory(u8),
    // UI_ChoosePerformancePackage (0x556E70): buy a level's package, or
    // take the category back to stock.
    PerfPackage(u8),
    PerfStock,
    /// Choose Individual Parts: UI_BuyPerformanceParts takes Accept.
    PerfParts,
}

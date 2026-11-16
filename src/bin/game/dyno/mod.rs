pub(crate) mod session;
pub(crate) mod session_impl;
pub(crate) mod tuning;
pub(crate) mod consts;

use crate::*;

/// The dyno and the tuning session (were `Game` fields).
pub(crate) struct DynoTuning {
    /// 0x83A640, the tuning session's way in (0x5309F0's third argument):
    /// 0 the garage (0x495B06), 1 the career performance shop (0x54FF7B),
    /// 2 Customize (0x55E67C). Quit Tuning asks and returns by it.
    pub(crate) from: u8,
    /// Customize's unsaved profile while its Dyno save point shows the
    /// loaded one (`AfterSave::CustomizeDyno`).
    pub(crate) scratch: Option<Profile>,
    /// The dyno camera's distance (+0x3C's product, 0x44F300).
    pub(crate) distance: f32,
    /// The first dyno run's engine (0x83A734) once a run has ended
    /// (0x523340 counts them at 0x83A658).
    pub(crate) first: Option<nfsu2::handling::Engine>,
    /// The dyno's run records (0x83A65C, 0x83A6C8, 0x83A734).
    pub(crate) runs: screens::DynoRuns,
    /// The car standing on the dyno's rollers (body origin and turn) while
    /// the dyno track is loaded: the place the pull's movement is drawn
    /// from and the dyno camera's fixed frame.
    pub(crate) rest: Option<(Vec3, glam::Quat)>,
    /// The dyno manager's rollers (0x56D1B0) while the dyno track is
    /// loaded.
    pub(crate) rollers: Option<nfsu2::dyno::Rollers>,
    /// A finished pull spinning down behind UI_PerformanceDyno_Results:
    /// the world runs on there (Wine 2026-11-01: the sim clock and the
    /// wheels' spin fall on under the results; it is the chart's pull).
    pub(crate) spin: Option<nfsu2::dyno::DynoPull>,
    /// The dyno's tuning mode (0x83A63C, the state 0x839C40 +0x9FC; 0xFF
    /// none chosen, set again by 0x53CD08).
    pub(crate) tuning_mode: u8,
    /// The dyno's tuning object's sliders (0x83A540 +0x14): None until the
    /// dyno menu makes it (0x50FE20) from the car object's settings 0.
    pub(crate) tuning_object: Option<nfsu2::tuning::Settings>,
    /// The tuning session (0x89E7E6, set by 0x526980 from 0x5309F0's
    /// entry to Quit Tuning): its menus are drawn over its track (the dyno
    /// track 4706, or a test run's) rather than the front end.
    pub(crate) tuning_session: bool,
    /// The tuning settings as the dyno found them (0x50F800 copies the car
    /// object's five to the working ones): 0x50FD10's "changed" compares
    /// the working settings (the clone's `Profile::tuning`) with the car
    /// object's.
    pub(crate) tuning_entry: [nfsu2::tuning::Settings; career::TUNING_MODES],
    /// `Game::tuned_gears` for the car and its performance parts (with
    /// the uniques) it was worked out for.
    pub(crate) gears: std::sync::Mutex<Option<((String, std::collections::BTreeSet<u8>), u32)>>,
}

impl Default for DynoTuning {
    fn default() -> DynoTuning {
        DynoTuning {
            from: 0,
            scratch: None,
            distance: DYNO_DISTANCE.1,
            first: None,
            rest: None,
            rollers: None,
            spin: None,
            runs: screens::DynoRuns::default(),
            tuning_mode: 0xFF,
            tuning_object: None,
            tuning_session: false,
            tuning_entry: Default::default(),
            gears: Default::default(),
        }
    }
}

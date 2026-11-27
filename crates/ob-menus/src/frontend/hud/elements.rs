/// One HUD element: its feature mask and the objects its constructor finds.
pub(super) struct Element {
    pub(super) lo: u32,
    pub(super) hi: u32,
    pub(super) objects: &'static [&'static str],
    /// Names with a number, `name` + 1, 2, … (the constructors' "%d"
    /// loops); every number the package has counts.
    pub(super) numbered: &'static [&'static str],
}

/// The drag tachometer (0x4F6500), built in place of 0x4F5100 in a drag
/// race (0x89E7D8 or 0x890108; 0x4F8CF2), same mask; its lights RPM_1..12
/// are only lit when 0x89E7C0 (the local human racer count; 0x4AB52E sums
/// 0x89E7C0..0x89E7CC against the racer count) is not 1, so never in the
/// clone's single-player races; the needle turns instead.
pub(super) const DRAG_TACH: Element = Element {
    lo: 0x1000_0060,
    hi: 0,
    objects: &[
        "RPM_fill",
        "Drag_Turbo_Backing",
        "Drag_Turbo_Lines",
        "3rdPersonTachRPMNumbers",
        "RPM_Redline",
        "3rdPersonNeedle",
        "3rdPersonGear",
        "ShiftMessage",
        "3rdPersonSpeedUnits",
        "3rdPersonSpeed",
        "Drag_Position_Backing_03",
        "Shift_light",
    ],
    numbered: &["RPM_"],
};

/// 0x4F5A70: the turbo gauge.
pub(super) const TURBO: Element = Element {
    lo: 0x1000_0060,
    hi: 0,
    objects: &[
        "3rdperson_Turbofill",
        "3rdperson_Turbolines",
        "3rdperson_TurboDial",
        "3rdperson_TurboRed_1",
        "3rdperson_TurboRed_2",
        "Drift_Angle_Backing2",
    ],
    numbered: &[],
};

/// The drift element's angle backing (0x4F6C00, while the player's slot is
/// in use, 0x8900AC): the car's turbo (0x457B00, the largest turbo curve
/// point, above 0) picks Drift_Angle_Backing1 (the plate left of the turbo
/// gauge, with Drift_Angle_Backing2 under the gauge) and runs "Turbo" on
/// Angle_Indicator_DATA and Angle_degrees; without a turbo it picks
/// Drift_Angle_Backing3 (the plate whose round end meets the tachometer)
/// and runs "NoTurbo", which moves the two texts 73 right onto it
/// (0x51CF80, flag 1). Only the picked backing joins the element, so the
/// other stays hidden as the HUD's constructor left it.
pub(super) const DRIFT_ANGLE_BACKING: [(&str, &str); 2] =
    [("Drift_Angle_Backing3", "NoTurbo"), ("Drift_Angle_Backing1", "Turbo")];

/// The angle readout's texts the turbo script moves (0x4F6C00).
pub(super) const DRIFT_ANGLE_TEXTS: [&str; 2] = ["Angle_Indicator_DATA", "Angle_degrees"];

/// The elements a single-player race or free roam builds (0x4F8BF0), with
/// the masks from their constructors and 0x4CA090.
pub(super) const ELEMENTS: &[Element] = &[
    // 0x4F5100: tachometer and speedometer (one local player: the needle
    // dial, tach_fill and Shift_light).
    Element {
        lo: 0x1000_0060,
        hi: 0,
        objects: &[
            "3rdperson_Turbolines",
            "3rdperson_Turbofill",
            "3rdperson_TurboDial",
            "3rdPersonNeedle",
            "3rdPersonTachRPMNumbers",
            "3rdPersonSpeed",
            "3rdPersonSpeedUnits",
            "3rdPersonShiftUp",
            "3rdPersonGear",
            "OptimalRPM",
            "GearBkg",
            "PerfectShift",
            "MissedShift",
            "Redline",
            "tach_fill",
            "Drift_Angle_Backing2",
            "Shift_light",
        ],
        numbered: &[],
    },
    TURBO,
    // 0x4F4AC0: race position.
    Element {
        lo: 0x200,
        hi: 0,
        objects: &[
            "RacePositionNumber",
            "RacePositionNth",
            "NumberRacers",
            "RaceTime",
            "Hurry_Text",
            "Hurry_Counter",
            "TimeText",
            "PositionBacking",
        ],
        numbered: &[],
    },
    // 0x4F69E0: engine heat (drag).
    Element { lo: 0x2000_0000, hi: 0, objects: &["HEAT_Bar"], numbered: &[] },
    // 0x4F6A90: nitrous.
    Element { lo: 0, hi: 0x4_0000, objects: &["NOS_Line", "NOS_GameBreaker", "NOS_Mask", "NOS_Fill"], numbered: &[] },
    // 0x4F6C00: drift and speed-trap readouts. The angle readout's backing
    // is one of two, by the car's turbo ([`DRIFT_ANGLE_BACKING`]).
    Element {
        lo: 0x8000_0000,
        hi: 0,
        objects: &[
            "SlowMessage",
            "WrongWay_CountDown",
            "Comment_DATA",
            "SpeedUnits",
            "SP_Drift_Point",
            "SP_Drift_Title",
            "DriftSpeed_Backing",
            "3rdperson_driftometer",
            "3rdperson_DriftAngle_Meter",
            "3rdperson_DriftAngle_Angle_degrees",
            "90Degrees",
            "45Degrees",
            "0Degrees",
            "Angle_Indicator_DATA",
            "Angle_degrees",
        ],
        numbered: &["CurrentSpeed_Trap_Backing_0"],
    },
    // 0x4F7170: race over.
    Element {
        lo: 0x4,
        hi: 0,
        objects: &[
            "RaceOverMessage",
            "RaceOverBacking",
            "TopVignette",
            "TopVignetteLine",
            "BottomVignette",
            "BottomVignetteLine",
        ],
        numbered: &[],
    },
    // 0x4F5C40: laps and lap times.
    Element {
        lo: 0x508,
        hi: 0,
        objects: &[
            "LapNumber",
            "NumberLaps",
            "BestLapTime",
            "CurrentLapTime",
            "LapText",
            "Lap_Time_Title",
            "Lap_Time_Text",
            "CurrentLapTimeText",
            "TimeAsLeaderBacking",
            "TimeAsLeaderText",
            "TimeAsLeader",
            "BestLapTimeText",
        ],
        numbered: &["LapCounterBacking_"],
    },
    // 0x4F6420: wrong way.
    Element { lo: 0x4000, hi: 0, objects: &["WrongWayImage", "WrongWay_CountDown"], numbered: &[] },
    // 0x4F6030: split times.
    Element { lo: 0x80, hi: 0, objects: &["SplitTimeText", "SplitTimeLapTime"], numbered: &[] },
    // 0x4F7AD0: style points.
    Element {
        lo: 0x2_0000,
        hi: 0,
        objects: &[
            "Style_Point_Backing",
            "Style_Points_Title",
            "SP_DATA",
            "SP_Trick_Point",
            "SP_Trick_Type",
            "Tach_NOS_Gamebreaker",
            "Style_Point_Backing_1",
            "SP_Drift_Point",
            "SP_Trick_Point_02",
            "SP_Drift_Title",
            "Style_Point_Backing_03",
            "SP_DRIFT_TOTAL",
            "Style_TotalDrift_Title",
            "CurrentSpeed_Trap_Backing_01",
            "NextSpeeed_Trap_Indicator",
            "drift_multiplier_text",
            "SpeedTrap_Speed_DATA",
            "BestDriftBacking",
            "BestLapTimeText",
            "BestDrift",
        ],
        numbered: &[],
    },
    // 0x4F6130: leader board.
    Element {
        lo: 0x80_0000,
        hi: 0,
        objects: &["LeaderBoardFill", "LeaderBoardLine"],
        numbered: &["LBData_", "LBTextBacking_", "LeaderText_", "LBBacking_"],
    },
    // 0x4F73E0: minimap.
    Element {
        lo: 0x1,
        hi: 0,
        objects: &[
            "Track2000_map",
            "TrackMapTargetRing",
            "Minimap_Mask",
            "StartLineIndicator",
            "PlayerCarIndicator_1",
            "PlayerCarIndicator_2",
            "Minimap_North_indicator",
            "GPS_Search_Group",
            "GPS_SEARCH_Text",
            "GPS_SEARCH_ERROR",
        ],
        numbered: &[
            "AIRACERINDICATOR_",
            "STARTLINEINDICATOR_",
            "CRIBINDICATOR_",
            "PAINTSHOPINDICATOR_",
            "PARTSSHOPINDICATOR_",
            "ICESHOPINDICATOR_",
            "PERFORMANCESHOPINDICATOR_",
            "CARLOTINDICATOR_",
            "SHOWCASEINDICATOR_",
            "CIRCUITEVENTINDICATOR_",
            "DRAGEVENTINDICATOR_",
            "STREETEVENTINDICATOR_",
            "DRIFTEVENTINDICATOR_",
            "SPRINTEVENTINDICATOR_",
            "URLEVENTINDICATOR_",
            "SUVEVENTINDICATOR_",
        ],
    },
    // 0x4F86D0: showcase timer.
    Element {
        lo: 0,
        hi: 0x4,
        objects: &["Show_case_backing", "CurrentLapTimeText_2", "CurrentLapTime_2"],
        numbered: &[],
    },
    // 0x4F87E0: the outrun meter (HUD_FEATURE_GETAWAY_METER; update
    // 0x4DABD0, see `quads`).
    Element {
        lo: 0,
        hi: 0x40,
        objects: &[
            "Getaway_Distance_Bar",
            "Distance_Number",
            "Out_Run_Message",
            "Distance_Text",
            "Getaway_Distance_Backing",
            "Getaway_Distance_Lines",
        ],
        numbered: &[],
    },
    // 0x4F89D0: dyno.
    Element { lo: 0x800, hi: 0, objects: &[], numbered: &["DYNO_HEADERBACKING0", "DYNO_CONTENTBACKING0"] },
    // 0x4F4E70 (career free roam): region names.
    Element { lo: 0x1, hi: 0, objects: &["Region_Message"], numbered: &[] },
    // 0x4F5060 (career free roam): cash reward.
    Element { lo: 0, hi: 0x80, objects: &["Cash_Reward"], numbered: &[] },
    // 0x4F4F10 (career free roam): bank.
    Element {
        lo: 0,
        hi: 0x8_0000,
        objects: &["Cash_Bank", "Cash_data", "Cash_header", "Show_case_backing"],
        numbered: &[],
    },
];

/// 0x4F7340, 0x4F83D0 and the minimap (0x4F73E0, its "Filter" group) find
/// objects by hash.
pub(super) const HASHED: &[(u32, u32, &[u32])] = &[
    (0, 0x2_0000, &[COUNTDOWN]),
    (0x1, 0, &[crate::frontend::fe_ids::HASHED_0]),
    (0, 0x2, &[crate::frontend::fe_ids::ENGAGE_DPAD, crate::frontend::fe_ids::ENGAGE_HIGHLIGHTS_0, crate::frontend::fe_ids::ENGAGE_HIGHLIGHTS_1, crate::frontend::fe_ids::ENGAGE_ICON, crate::frontend::fe_ids::ENGAGE_MAIL, crate::frontend::fe_ids::ENGAGE_COUNT, crate::frontend::fe_ids::HASHED_7, crate::frontend::fe_ids::HASHED_8]),
];

/// 0x4F7340's object: the start countdown.
pub(super) const COUNTDOWN: u32 = crate::frontend::fe_ids::COUNTDOWN;

/// Objects no element claims, which the game does not show in a race or
/// free roam (the "Circuit Stats" panel, the showcase "Preview" label).
/// Outside a drag race the constructor's hide of every object covers them
/// (see `Hud::quads`); in a drag race (no such hide) they are kept hidden
/// here (**inferred** there).
pub(super) const UNTRACED: &[u32] = &[crate::frontend::fe_ids::UNTRACED_0, crate::frontend::fe_ids::UNTRACED_1];

/// The style element's mask (0x4F7AD0, HUD +0x4C; update 0x4DA900).
pub(super) const STYLE: u32 = 0x2_0000;

/// Language labels the elements print.
pub(super) const MPH: u32 = crate::frontend::fe_ids::MPH;
pub(super) const KMH: u32 = crate::frontend::fe_ids::KMH;
pub(super) const NO_LAP_TIME: u32 = crate::frontend::fe_ids::NO_LAP_TIME; // "--:--.--"
pub(super) const LAP_TIME: u32 = crate::frontend::fe_ids::ELEMENTS_LAP_TIME;
pub(super) const BEST_LAP_TIME: u32 = crate::frontend::fe_ids::BEST_LAP_TIME;
pub(super) const COMPLETED: u32 = crate::frontend::fe_ids::COMPLETED;
pub(super) const YOUR_TIME: u32 = crate::frontend::fe_ids::YOUR_TIME;

/// Gear letters by the car's gear (0x7F66A4): reverse, neutral, 1st….
pub(super) const GEARS: [(i32, char); 10] =
    [(-1, 'A'), (0, 'R'), (1, 'N'), (2, '1'), (3, '2'), (4, '3'), (5, '4'), (6, '5'), (7, '6'), (8, '7')];

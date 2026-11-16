use super::*;

pub(super) const ROWS: &[GraphicsRow] = &[
    // Weather (docs/weather.md): only while it rains or the road and
    // cars are wet; the rain's schedule and amount stay the original's.
    GraphicsRow {
        id: RowId::WetCar,
        label: WET_CAR,
        group: GraphicsGroup::Weather,
        help: "Cars get wet with the rain and road spray: darker, glossier paint, beads and streaks running back with speed, slow drying.",
        tiers: &[
            t("Off", "Off: as the original (a dry car look in the rain)."),
            tc("On", "Only while wet; a dry car is untouched (integrated GPU: under 1 ms).", WET_CAR_COST_4K, 0.0),
        ],
        native: None,
        get: |s| usize::from(s.enh.wet_car),
        set: |s, i| s.enh.wet_car = i != 0,
        offered: None,
        legacy: false,
    },
    GraphicsRow {
        id: RowId::RainDetail,
        label: RAIN_DETAIL,
        group: GraphicsGroup::Weather,
        help: "Finer, denser rain streaks lit by lamps and headlights, splashes on the ground, a glow around lamps in the wet air.",
        tiers: &[
            t("Off", "Off: the original's rain only."),
            tc("On", "Fine lit rain, ground splashes, lamp glow (integrated GPU: ~0.7 ms).", RAIN_DETAIL_COST_4K, 0.0),
            tc("High", "Twice the drops and splashes (integrated GPU: ~2 ms).", RAIN_DETAIL_HIGH_COST_4K, 0.0),
            tc(
                "Next-Gen",
                "A simulated rain volume around you: streaks blurred by your speed, blown by gusts, bright in lamp light and headlight beams, hidden under bridges; splashes on every surface incl. car roofs; rain haze with halos around lamps; rain hitting the cars (integrated GPU: ~4 ms).",
                RAIN_DETAIL_NEXT_GEN_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.rain_detail.min(3)),
        set: |s, i| s.enh.rain_detail = i as u8,
        offered: None,
        legacy: false,
    },
    GraphicsRow {
        id: RowId::Spray,
        label: SPRAY,
        group: GraphicsGroup::Weather,
        help: "Wheel spray and mist behind cars on wet roads, lit by lamps and tail lights.",
        tiers: &[
            OFF_ON[0],
            tc("On", "Only on wet roads at speed (integrated GPU: ~2.3 ms behind a pack of cars).", SPRAY_COST_4K, 0.0),
        ],
        native: None,
        get: |s| usize::from(s.enh.spray),
        set: |s, i| s.enh.spray = i != 0,
        offered: None,
        legacy: false,
    },
    // docs/wet-road-car.md: only while the road is wet.
    GraphicsRow {
        id: RowId::WetInteraction,
        label: WET_INTERACTION,
        group: GraphicsGroup::Weather,
        help: "Cars interact with the wet road: volumetric rooster tails and mist behind them lit by the street lamps and the cars' lights, puddle splashes, dry tyre tracks and wakes, their lights mirrored in it, spray on the lens.",
        tiers: &[
            t("Off", "Off: as the original (the road and spray ignore the cars)."),
            tc(
                "On",
                "Only while the road is wet; includes Wet Roads' wet surface and the spray.",
                WET_INTERACTION_COST_4K,
                0.0,
            ),
            tc(
                "High",
                "Cinematic: denser spray in a finer volume, finer tracks over a wider area, more lights mirrored.",
                WET_INTERACTION_HIGH_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.wet_interaction.min(2)),
        set: |s, i| s.enh.wet_interaction = i as u8,
        offered: None,
        legacy: false,
    },
    GraphicsRow {
        id: RowId::LensDrops,
        label: LENS_DROPS,
        group: GraphicsGroup::Weather,
        help: "More rain drops on the screen that run off and are wiped away by the speed.",
        tiers: &[
            t("Off", "Off: the original's lens drops only."),
            tc("On", "Extra drops while it rains.", LENS_DROPS_COST_4K, 0.0),
        ],
        native: None,
        get: |s| usize::from(s.enh.lens_drops),
        set: |s, i| s.enh.lens_drops = i != 0,
        offered: None,
        legacy: false,
    },
];

use crate::frontend::fe_ids;
use super::*;

/// The feature mask a single-player race or free roam asks for (0x6000A0
/// with 0x5EA8F0, 0x5F33C0, 0x5EA8D0, 0x5F32B0), by the player's options
/// (0x83AA64 + player × 0x28): the minimap when the map mode is not Off
/// (0x5EA8F0, 1), the gauges (0x5F32B0: 0x5EA940's 0x40), the score
/// (0x5EA8D0, 0x2_0000); racing, the lap info (0x508), the split time
/// unless Off (0x80), the position with more than one racer (0x200) and
/// the leader board (0x80_0000) (0x5F33C0).
pub fn features(input: &Input) -> (u32, u32) {
    // A replay (**verified**, 0x6000A0): in race status 0 / 6 / 7 with the
    // race camera state 5 (0x600186) the low mask is 0 (0x60018B) and the
    // tail (0x60052C) adds the countdown element 0x20000 (0x4F7340) while
    // the state is not 7 or 6, a race is on (0x89E7B0) and the player's
    // +0x234 is 0. 0x4AB5B0 then shows "GO!" while 0x5EEA50 on the
    // restored race status gives 4: for +0xC (0.9 s) from the green tick.
    if input.replay {
        return (0, 0x2_0000);
    }
    if input.event_camera {
        return (0, 0);
    }
    if let Some(f) = input.script.filter(|_| !input.racing) {
        return f;
    }
    if input.finished.is_some() {
        // 0x5FFF60: the race-over element alone (0x40004 less the
        // 0x40000 that 0x4CA090 masks off), and the racing bit; after an
        // outrun the race state is back to 0 (0x5F84A0) and 0x5FFF60 holds
        // for 3 s from the finish tick, with no racing bit (0x60053E:
        // 0x89E7B0 = 0 and 0x83A9D0 → 0x457BA0, inferred clear).
        return (0x4, if input.outrun_result.is_some() && !input.racing { 0 } else { 0x2_0000 });
    }
    let o = &input.options;
    // 0x6000A0 in race status 3 (before the green light, one player):
    // the score (0x5EA8D0), the tach with the gauges on (0x5F32B0's bit),
    // the nitrous gauge for a tank outside drift / street X, the drag
    // heat bar, and the countdown's high 0x20000 (0x60052C) — no minimap,
    // lap info, position or leader board until the race runs (checked
    // against the original's Quick Race start, scenes75).
    if input.racing && input.clock < 0.0 && input.outrun.is_none() {
        let mut lo = 0;
        if o.score {
            lo |= 0x2_0000;
        }
        let mut hi = 0x2_0000;
        if o.gauges {
            lo |= 0x40;
            if input.nitrous.is_some()
                && !matches!(input.race, crate::race::RaceMode::Drift | crate::race::RaceMode::StreetX)
            {
                hi |= 0x4_0000;
            }
            if input.race == crate::race::RaceMode::Drag {
                lo |= fe_ids::OBJ_20000000;
            }
        }
        return (lo, hi);
    }
    let mut lo = 0x10_0000;
    if o.map {
        lo |= 0x1;
    }
    if o.score {
        lo |= 0x2_0000;
    }
    if o.gauges {
        lo |= 0x40;
    }
    let mut hi = 0x80;
    if input.racing {
        lo |= 0x4000;
        if o.lap_info {
            lo |= 0x508;
        }
        if o.split {
            lo |= 0x80;
        }
        if o.leaderboard {
            lo |= 0x80_0000;
        }
        if o.position && input.racers > 1 {
            lo |= 0x200;
        }
        hi |= 0x2_0000;
        // 0x5F33C0: no split times in a drift race; 0x5F32B0: the drift
        // readouts in a drift race, the heat bar in a drag race.
        match input.race {
            crate::race::RaceMode::Drift => {
                lo &= !0x80;
                lo |= 0x8000_0000;
            }
            crate::race::RaceMode::Drag => lo |= fe_ids::OBJ_20000000,
            _ => {}
        }
        // 0x600231..0x600260: the nitrous gauge for a car with a tank,
        // outside drift (0x401910) and Street X (0x401930) races.
        if input.nitrous.is_some()
            && !matches!(input.race, crate::race::RaceMode::Drift | crate::race::RaceMode::StreetX)
        {
            hi |= 0x4_0000;
        }
        // 0x5F33C0 in race state 7: the getaway meter (high 0x40,
        // HUD_FEATURE_GETAWAY_METER) and no leader board.
        if input.outrun.is_some() {
            hi |= 0x40;
            lo &= !0x80_0000;
        }
    } else {
        hi |= 0x2;
        if input.career {
            hi |= 0x8_0000;
        }
        // 0x5F32B0 (free roam's branch of 0x6000A0, 0x600466): with the
        // gauges on (0x83AA64), the nitrous gauge for a car whose tank
        // size (car +0x454) is above 0 — its second test (0x5F337C) has no
        // drift / street X exclusion.
        if o.gauges && input.nitrous.is_some() {
            hi |= 0x4_0000;
        }
    }
    // 0x6004B7: HUD_FEATURE_RVM (0x1000), the rear-view mirror, in this
    // branch only (racer +0x28 = 0): not before the green light, not at
    // the race over, not in the D-Day script's branch.
    if input.rear_view {
        lo |= 0x1000;
    }
    // 0x6004BD: a world challenge in state 1 (0x501690, 0x501680) adds
    // high 0x4, the showcase timer.
    if input.challenge.is_some() {
        hi |= 0x4;
    }
    (lo, hi)
}

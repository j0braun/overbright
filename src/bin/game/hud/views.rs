//! The HUD's views of the simulation, built here by the HUD glue
//! from plain simulation state (they lived on the simulation's types,
//! `Race::drift_hud`, `Encounters::hud` and `Progress::status_label`,
//! which made `sim/**` name the HUD's view structs; docs/developing/sim-session.md).

use crate::*;

/// The drift element's values (0x4C85D0).
pub(crate) fn drift_hud(race: &Race) -> Option<hud::DriftHud> {
    use nfsu2::drift::{DRIFT, TrickState};
    let (style, drift) = (race.scoring.style.as_ref()?, race.scoring.drift.as_ref()?);
    let t = style.trick(DRIFT)?;
    Some(hud::DriftHud {
        running: matches!(t.state, TrickState::Running | TrickState::Pending),
        banked_now: t.state == TrickState::Banked,
        lost: t.state == TrickState::Lost,
        points: t.points as i32,
        banked: t.banked as i32,
        best: t.best as i32,
        level: drift.level,
        next: drift.next_level,
        announce: race.scoring.announce.map(|(name, at)| (name, race.timing.clock - at)),
        angle: race.scoring.drift_angle,
        falling: drift.falling,
        held: drift.held,
        bonus: drift.bonus(),
    })
}

/// The HUD's view of the outrun (0x4DABD0's inputs).
pub(crate) fn outrun_hud(encounters: &Encounters, metric: bool) -> Option<hud::OutrunHud> {
    let o = encounters.outrun.as_ref()?;
    Some(hud::OutrunHud {
        value: nfsu2::outrun::hud_value(o.player, &[o.rival], metric),
        leading: o.player_leads,
        racing: true,
    })
}

/// The leader board's label once the racer is done (0x4AA9F0).
pub(crate) fn status_label(progress: &Progress) -> u32 {
    hud::status_label(progress.dropped.unwrap_or(0))
}

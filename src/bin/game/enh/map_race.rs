//! Custom races of map mods (docs/map-editor.md): while one is being
//! raced, its own route replaces its base track's in `drive_to_grid`.
use crate::*;
use nfsu2::tools::map::{self as map, CustomRace, RaceKind};

impl Game {
    /// A custom race's route in place of the track's own
    /// (`drive_to_grid`), while one is being raced on that track.
    pub(crate) fn map_race_route(&self, track: &career::Track) -> Option<Result<career::Route>> {
        let race = self.sim.map_race.as_ref().filter(|r| r.track == track.number)?;
        Some(map::race::road_net(&self.game_data.root, track).and_then(|net| map::race::route(&net, race, &|_| None)))
    }
}

/// The career event a custom race borrows its set-up from: one on its
/// base track of the same race mode (circuit and sprint share theirs).
#[allow(dead_code, reason = "the start of a custom race from the command line uses it")]
pub(crate) fn base_event(career: &career::CareerData, race: &CustomRace) -> Option<usize> {
    let want = match race.kind {
        RaceKind::Circuit | RaceKind::Sprint => nfsu2::race::RaceMode::FreeRoam,
        RaceKind::StreetX => nfsu2::race::RaceMode::StreetX,
        RaceKind::Drift => nfsu2::race::RaceMode::Drift,
    };
    // The race's first round is the one raced (`start_round`), on its
    // track and in its direction.
    let fits = |e: &&career::Event| {
        let (track, variant) = e.races.first().map_or((e.track, e.variant), |r| (r.0, r.1));
        track == race.track && (variant & 1 != 0) == race.backward && e.race_mode() == want
    };
    let same_kind = |e: &&career::Event| match race.kind {
        RaceKind::Circuit => e.kind == career::EventKind::Circuit,
        RaceKind::Sprint => e.kind == career::EventKind::Sprint,
        _ => true,
    };
    career.events.iter().filter(fits).find(same_kind).or_else(|| career.events.iter().find(fits)).map(|e| e.index)
}

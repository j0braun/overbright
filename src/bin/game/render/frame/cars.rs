//! The cars a frame draws besides the player's: one view,
//! [`DrawnCars`], in place of the frame's hand-written copies. The copies
//! do not all agree; each difference is an explicit [`CarSet`] value at
//! its use, with today's value (a later logic change fixes them):
//! - the headlight pools take free roam's traffic only (no race traffic);
//! - the shots take no paused race and leave the traffic out of the
//!   pools and the glows;
//! - the window's car pass ([`car_pass_set`]) takes free-roam traffic in a
//!   menu too and every encounter car, active or not.

use crate::*;

/// Which traffic a car set takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TrafficSet {
    /// Free roam's traffic, or the race's while not paused; none in a menu.
    Live,
    /// Free roam's traffic only (the headlight pools' copy).
    ExploreOnly,
    /// No traffic (the shots' pools and glows).
    #[cfg_attr(not(feature = "dev-harness"), allow(dead_code))]
    None,
}

/// The rules of one car set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CarSet {
    /// The paused race's cars when the mode is not a race (the window's
    /// frame: the race under the pause and post-race menus).
    pub(crate) paused_race: bool,
    pub(crate) traffic: TrafficSet,
}

/// The game state the world's scene reads (shared borrows of [`Game`];
/// the renderers it wrote are in the render state).
pub(crate) struct SceneGame<'a> {
    /// The frame's own values between the tick snapshots (the
    /// cars as drawn: [`render::snapshot::FramePoses::car`]).
    pub(crate) poses: &'a render::snapshot::FramePoses,
    pub(crate) mode: &'a Mode,
    pub(crate) paused_race: Option<&'a Race>,
    pub(crate) traffic: Option<&'a Traffic>,
    pub(crate) encounters: Option<&'a Encounters>,
    pub(crate) shop_scene: Option<&'a ShopRun>,
    pub(crate) scene_crowd: Option<&'a SceneCrowd>,
    pub(crate) settings: &'a options::Settings,
}

/// What a kind of drawn car is (its LOD table and the traffic's fade).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DrawnKind {
    Companion,
    Rival,
    Encounter,
    Traffic,
}

impl DrawnKind {
    /// The LOD table the frame draws the kind with.
    pub(crate) fn lod(self) -> LodDraw {
        match self {
            DrawnKind::Traffic => LodDraw::FrontEnd,
            _ => LodDraw::InGame,
        }
    }
}

/// The cars drawn besides the player's: the shop scene's companion, the
/// race's rivals, free roam's encounter racers and the traffic.
pub(crate) struct DrawnCars<'a> {
    /// The frame's cars as drawn: every state read goes through
    /// [`render::snapshot::FramePoses::car`].
    pub(crate) poses: &'a render::snapshot::FramePoses,
    pub(crate) race: Option<&'a Race>,
    pub(crate) traffic: Option<&'a Traffic>,
    pub(crate) encounters: Option<&'a Encounters>,
    pub(crate) companion: Option<(&'a Car, &'a CarState)>,
}

impl<'a> DrawnCars<'a> {
    /// The set under `set`'s rules.
    pub(crate) fn of(g: &'a SceneGame<'_>, set: CarSet) -> Self {
        Self::of_parts(
            g.mode,
            g.paused_race,
            g.traffic,
            g.encounters,
            g.shop_scene,
            g.poses,
            set,
        )
    }

    /// The set under `set`'s rules from the game's fields.
    pub(crate) fn of_parts(
        mode: &'a Mode,
        paused_race: Option<&'a Race>,
        traffic: Option<&'a Traffic>,
        encounters: Option<&'a Encounters>,
        shop_scene: Option<&'a ShopRun>,
        poses: &'a render::snapshot::FramePoses,
        set: CarSet,
    ) -> Self {
        let race = match mode {
            Mode::Race(r) => Some(r),
            _ if set.paused_race => paused_race,
            _ => None,
        };
        let traffic = match set.traffic {
            TrafficSet::Live => match mode {
                Mode::Explore => traffic,
                Mode::Race(race) => race.racers.traffic.as_ref().filter(|t| !t.paused),
                _ => None,
            },
            TrafficSet::ExploreOnly => {
                if matches!(mode, Mode::Explore) {
                    traffic
                } else {
                    None
                }
            }
            TrafficSet::None => None,
        };
        let encounters = if matches!(mode, Mode::Explore) { encounters } else { None };
        let companion = shop_scene.and_then(|run| run.companion.as_ref()).map(|c| (&c.car, poses.car(&c.id, &c.state)));
        DrawnCars { poses, race, traffic, encounters, companion }
    }

    /// The four parts as the frame's helpers take them.
    pub(crate) fn parts(
        &self,
    ) -> (Option<(&'a Car, &'a CarState)>, Option<&'a Race>, Option<&'a Encounters>, Option<&'a Traffic>) {
        (self.companion, self.race, self.encounters, self.traffic)
    }

    /// The cars on the road in drawing order: the rivals, the active
    /// encounter racers, the traffic placed in a lane.
    pub(crate) fn on_road(&self) -> impl Iterator<Item = (&'a Car, &'a CarState)> + 'a {
        let p = self.poses;
        let rivals = self.race.into_iter().flat_map(|race| &race.racers.rivals).map(move |rv| (&rv.car, p.car(&rv.id, &rv.state)));
        let encounters = self
            .encounters
            .into_iter()
            .flat_map(|e| e.cars.iter().filter(|c| c.active))
            .map(move |c| (&*c.car, p.car(&c.id, &c.state)));
        let traffic = self
            .traffic
            .into_iter()
            .flat_map(|t| t.cars.iter().filter(|c| traffic_drawn(c)))
            .map(move |c| (&*c.car, p.car(&c.id, &c.state)));
        rivals.chain(encounters).chain(traffic)
    }
}

/// Each drawn car besides the player's with its renderer, in drawing
/// order: the companion, the rivals (`paused_race`: the paused race's
/// under a menu), the active encounter racers in free roam, the traffic
/// placed in a lane ([`TrafficSet::Live`]).
pub(crate) fn for_each_drawn_car(
    g: &SceneGame<'_>,
    cars: &mut render::state::Keyed<CarRenderer>,
    paused_race: bool,
    mut each: impl FnMut(DrawnKind, u64, &mut Option<CarRenderer>, &Car, &CarState),
) {
    let p = g.poses;
    if let Some(c) = g.shop_scene.and_then(|run| run.companion.as_ref()) {
        each(DrawnKind::Companion, c.id.key(), cars.slot(&c.id), &c.car, p.car(&c.id, &c.state));
    }
    let race = match g.mode {
        Mode::Race(r) => Some(r),
        _ if paused_race => g.paused_race,
        _ => None,
    };
    if let Some(race) = race {
        for r in &race.racers.rivals {
            each(DrawnKind::Rival, r.id.key(), cars.slot(&r.id), &r.car, p.car(&r.id, &r.state));
        }
    }
    if let (Mode::Explore, Some(e)) = (g.mode, g.encounters) {
        for c in e.cars.iter().filter(|c| c.active) {
            each(DrawnKind::Encounter, c.id.key(), cars.slot(&c.id), &c.car, p.car(&c.id, &c.state));
        }
    }
    let traffic = match g.mode {
        Mode::Explore => g.traffic,
        Mode::Race(race) => race.racers.traffic.as_ref().filter(|t| !t.paused),
        _ => None,
    };
    if let Some(traffic) = traffic {
        for c in traffic.cars.iter().filter(|c| traffic_drawn(c)) {
            each(DrawnKind::Traffic, c.id.key(), cars.slot(&c.id), &c.car, p.car(&c.id, &c.state));
        }
    }
}

/// The window's car pass: every queued renderer, in the order queued.
/// It differs from [`DrawnCars`] (kept as it is): the
/// race's cars with the paused race's, the race's traffic unless paused
/// and otherwise free roam's (in a menu too), and every encounter car
/// (no free-roam or `active` filter). A renderer that was not queued
/// this frame records nothing (`CarRenderer::record_queued`).
pub(crate) fn car_pass_set<'a>(
    renderers: &'a render::state::Keyed<CarRenderer>,
    player: Option<&'a CarRenderer>,
    g: &'a SceneGame<'_>,
) -> Vec<&'a CarRenderer> {
    let mode: &Mode = g.mode;
    let race = match mode {
        Mode::Race(r) => Some(r),
        _ => g.paused_race,
    };
    let traffic = match mode {
        Mode::Race(race) => race.racers.traffic.as_ref().filter(|t| !t.paused),
        _ => g.traffic,
    };
    player
        .into_iter()
        .chain(g.shop_scene.iter().filter_map(|run| run.companion.as_ref()).filter_map(|c| renderers.get(&c.id)))
        .chain(race.into_iter().flat_map(|r| r.racers.rivals.iter().filter_map(|c| renderers.get(&c.id))))
        .chain(g.encounters.iter().flat_map(|e| e.cars.iter().filter_map(|c| renderers.get(&c.id))))
        .chain(traffic.into_iter().flat_map(|t| t.cars.iter().filter_map(|c| renderers.get(&c.id))))
        .collect::<Vec<_>>()
}

/// A traffic car World_DrawCars draws (0x6317A2: a car whose +0x550,
/// "active", is clear is skipped): placed in a lane and
/// active. A car placed over unloaded world from an unloaded parking place
/// runs inactive on the BeltMover until the world under it loads, and a
/// paused car (no lane) is never drawn, coasting or not.
pub(crate) fn traffic_drawn(c: &TrafficCar) -> bool {
    drawn(c.lane.is_some(), c.active)
}

fn drawn(placed: bool, active: bool) -> bool {
    placed && active
}

#[cfg(test)]
mod tests {
    /// 0x6317A2 (World_DrawCars): `mov al,[esi+0x550]; test al,al; je skip`
    /// — an inactive car is not drawn, placed or not.
    #[test]
    fn world_draw_cars_skips_an_inactive_car() {
        assert!(super::drawn(true, true));
        assert!(!super::drawn(true, false));
        assert!(!super::drawn(false, true));
        assert!(!super::drawn(false, false));
    }
}

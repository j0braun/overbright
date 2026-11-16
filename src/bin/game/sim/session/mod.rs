//! The simulation's boundary (docs/sim-session.md). A tick of the
//! world's simulation is one call, [`Session::step`]: what it reads from
//! outside itself comes in as a [`TickInput`], what the rest of the game
//! takes from it goes out as [`TickEvents`]. The session is a view of the
//! state the simulation owns (the drive, the race, the scene holding the
//! cars, the shared generator), lent by the game's update glue
//! (`update::Game::update_world`) for the tick; it cannot name the game,
//! the renderer, the audio, the window, the HUD or the front end
//! (`scripts/checks/layering.sh`, rule 4).

mod countdown;
mod live;
mod race_scene;
mod replay;
mod roam;
mod shop_scene;

pub(crate) use roam::{EncounterTick, RoamInput};
pub(crate) use shop_scene::SceneTick;

use crate::sim::race::{SceneCrowd, ShopRun};
use crate::{AiData, Controls, Drive, Encounters, Race, Traffic};
use nfsu2::frontend::career::RouteTriggers;
use nfsu2::opponents::GameRng;

/// Which world the tick runs in (`Mode` as the simulation reads it: it
/// never names the front end's menus).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum WorldMode {
    /// A menu (over a loaded world or none).
    Menus,
    /// Free roam.
    Explore,
    /// A race (the session's `race` is set).
    Race,
}

/// Everything a tick of the simulation reads from outside itself. The
/// settings are copied (they do not change within a tick); the data
/// handles are shared and immutable.
pub(crate) struct TickInput<'a> {
    /// The tick's dt (one of the tick clock's, `Game::update`).
    pub(crate) dt: f32,
    /// The mode the tick runs in.
    pub(crate) world: WorldMode,
    /// The tick's driving controls (the input queue applied, the
    /// script's hold and the hydraulics: `update::Game::tick_input`).
    pub(crate) controls: Controls,
    /// The player's Lap Info option (the race's lap snapshots).
    pub(crate) lap_info: bool,
    /// The player's camera option (`VIEW_ORDER` index before `min(4)`).
    pub(crate) camera: u8,
    /// The tick's settings (`sim::TickSettings`, taken once per tick).
    /// No reader inside the session yet (the chase camera's FOV offset
    /// still reads the store, `chase::ChaseCamera::update`).
    #[allow(dead_code)]
    pub(crate) settings: crate::sim::TickSettings,
    /// The AI racers' data.
    pub(crate) ai: Option<&'a AiData>,
    /// The region's route triggers.
    pub(crate) route_triggers: &'a RouteTriggers,
    /// What free roam reads from the career and the data.
    pub(crate) roam: Option<&'a RoamInput<'a>>,
}

/// A sound the simulation asks for (the audio glue posts it on the tick).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SimSound {
    /// The crash camera started (0x5A0105 → 0x6022F0 → 0x486130): the
    /// CollisionStream request (`SoundFxGate::crash_camera`).
    CrashCamera,
}

/// How a race's tick ended: the update goes on (`Go`: the camera's
/// velocity at the update's end), stops there (`Stop`), or the flow takes
/// over (the race is over, a replay was left).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum RaceTick {
    Go,
    Stop,
    /// Every racer has finished and the race-over hold has run: the
    /// player's finish time (`Game::finish_race`).
    Finished {
        time: f32,
    },
    /// A replay's skip asked to leave it (`Game::end_replay`).
    ReplayLeft,
}

/// Everything the rest of the game takes from a tick, in the order the
/// simulation produced it.
#[derive(Default, Debug)]
pub(crate) struct TickEvents {
    /// The sounds asked for, in order.
    pub(crate) sounds: Vec<SimSound>,
    /// A race's tick: how it ended (`None` outside a race).
    pub(crate) race: Option<RaceTick>,
    /// A shop scene's frame in free roam or the race's ending scene in
    /// the menus: it ran, or it ended (the flow's).
    pub(crate) scene: Option<SceneTick>,
    /// Free roam's encounter frame: what the career's rules act on
    /// before the session's second phase (`Session::step_roam`).
    pub(crate) encounters: Option<EncounterTick>,
    /// Free roam's world step: the tick's dt under the slow motion
    /// of free roam's event cameras (0x5EA370), for the session's second
    /// phase (`Session::step_roam`).
    pub(crate) roam_dt: Option<f32>,
    /// The contact sounds the tick's collision steps posted, in their
    /// order (the glue adds them to the session's queues right after
    /// the step, `SimSession::contact_sounds`).
    pub(crate) contacts: nfsu2::drive::ContactSounds,
}

/// The state the simulation owns, lent for one tick.
pub(crate) struct Session<'a> {
    /// The drive (the player's car, the world, the camera).
    pub(crate) drive: &'a mut Option<Drive>,
    /// The race running (`Mode::Race`'s).
    pub(crate) race: Option<&'a mut Race>,
    /// The scene holding the cars (a race's start or ending, a shop's
    /// drive-in or drive-out, the career's start, the dyno).
    pub(crate) scene: &'a mut Option<ShopRun>,
    /// The race start scene's characters.
    pub(crate) crowd: &'a mut Option<SceneCrowd>,
    /// Free roam's random encounters and outrun.
    pub(crate) encounters: &'a mut Option<Encounters>,
    /// Free roam's traffic.
    pub(crate) traffic: &'a mut Option<Traffic>,
    /// The free-roam racer's style manager (`SimSession::roam_style`).
    pub(crate) roam_style: &'a mut Option<(nfsu2::drift::StyleManager, f32, f32, nfsu2::style::Misc)>,
    /// Free roam's event cameras (`SimSession::roam_camera`).
    pub(crate) roam_camera: &'a mut Option<crate::RaceCamera>,
    /// The game's shared generator (0x7EA624; generator 1 in docs/rng.md).
    pub(crate) rng: &'a mut GameRng,
}

impl Session<'_> {
    /// One tick of the world's simulation: in the order
    /// `Game::update_world` ran it, the ending scene dropped outside the
    /// menus and the world's animations, the camera director's draw, then
    /// in a race the race's tick. What the rest of the game takes from it
    /// comes back in the [`TickEvents`], in the order it was produced.
    pub(crate) fn step(&mut self, input: &TickInput) -> TickEvents {
        let mut events = TickEvents::default();
        // The chase camera's Field Of View offset, as the tick took it.
        if let Some(d) = self.drive.as_mut() {
            d.fov_offset = input.settings.fov_offset;
        }
        self.world_anims(input.dt, input.world);
        self.director_draw(input.world);
        match input.world {
            WorldMode::Race => events.race = Some(self.update_race(input.dt, input, &mut events)),
            // Free roam's shop scene (drive-in / drive-out, the
            // career's start, the dyno's), in place of free roam's update.
            WorldMode::Explore if self.scene.is_some() => {
                // (A scene with no drive is dropped and still counts as
                // the tick's scene frame, as before.)
                events.scene = Some(self.scene_frame(input.dt, input.camera).unwrap_or(SceneTick::Ran));
            }
            // Free roam's first phase, the encounter manager's
            // frame; the career's rules act on it, then `step_roam`.
            WorldMode::Explore => {
                let dt = self.roam_world_dt(input.dt);
                events.roam_dt = Some(dt);
                events.encounters = Some(self.encounters_frame(dt, input));
            }
            // The race's ending scene behind the post-race screens.
            WorldMode::Menus => events.scene = self.ending_scene(input.dt, input.camera),
        }
        events
    }

    /// The ending scene left behind the post-race screens, and the world's
    /// animations.
    fn world_anims(&mut self, dt: f32, world: WorldMode) {
        // A race's ending scene ends with the post-race screens (a replay,
        // a restart, free roam or the front end take over).
        if world != WorldMode::Menus
            && self.scene.as_ref().is_some_and(|r| matches!(r.scene.kind, nfsu2::shop_scene::SceneKind::Ending(_)))
        {
            *self.scene = None;
            *self.crowd = None;
        }
        // The world animations (searchlights, planes, cranes, trains, sails)
        // play wherever the world is driven in.
        if matches!(world, WorldMode::Explore | WorldMode::Race)
            && let Some(d) = self.drive.as_mut()
        {
            let moved = d.world.step_loops(dt);
            d.posed.extend(moved);
        }
    }

    /// The camera director's draw (0x5FB060 → 0x5FB2EA) for this world
    /// update, while the player's NIS id (player +0x234) is set: 0x437910
    /// sets it when a scene starts, the scene's end 0x439070 (0x43BB10 once
    /// the scene clock passes its length) clears it. **Measured**
    /// (pinned c1, `[0x8900AC] +0x234` read every frame): set from
    /// the scene's first frame (race status 2) through the countdown and
    /// 325 frames past the green, one draw on every one of those frames and
    /// none after. The race's start scene runs past the green to its length
    /// (`Race::scene_end`; the clone drops its scene run at the green mark,
    /// so the time after it counts on the race clock). Here also a shop's
    /// drive-in (s2: 140 draws). The drive-out, the career start and the
    /// race's ending scene are left out (**open**: not measured; the gate's
    /// mover-type test 2–4 may hold there).
    fn director_draw(&mut self, world: WorldMode) {
        use nfsu2::shop_scene::SceneKind;
        if self.drive.is_none() {
            return;
        }
        let playing = match (world, self.scene.as_ref(), self.race.as_deref()) {
            (WorldMode::Race | WorldMode::Explore, Some(r), _) => {
                matches!(r.scene.kind, SceneKind::Intro(_) | SceneKind::Enter) && r.frame < r.scene.length
            }
            (WorldMode::Race, None, Some(race)) => {
                race.scene_start
                    && race.replay.playback.is_none()
                    && race.timing.clock >= 0.0
                    && race.timing.clock < race.scene_end
            }
            _ => false,
        };
        if playing {
            nfsu2::rng_sites::director(&mut *self.rng);
        }
    }

    /// A race's update: the start scene, the traffic, then the countdown,
    /// a replay's frame or the race itself, and its end (the flow's
    /// `finish_race` / `end_replay` as the [`RaceTick`] the glue acts on).
    fn update_race(&mut self, dt: f32, input: &TickInput, events: &mut TickEvents) -> RaceTick {
        if self.drive.is_none() {
            return RaceTick::Stop;
        }
        if self.update_race_scene(dt, input, events) == RaceTick::Stop {
            // The start scene runs the countdown and the green itself.
            let (race, d) = race_drive(&mut self.race, &mut *self.drive);
            race.status_clock_update(d.clock);
            return RaceTick::Stop;
        }
        self.race_traffic(dt);
        let (race, _) = race_drive(&mut self.race, &mut *self.drive);
        if race.timing.clock < 0.0 {
            self.update_countdown(dt, input, events);
        } else if race.replay.playback.is_some() {
            return self.update_replay(dt, input, events);
        } else {
            self.update_race_live(dt, input, events);
        }
        // The race status's clock +0x14 (0x6076E0).
        let (race, d) = race_drive(&mut self.race, &mut *self.drive);
        race.status_clock_update(d.clock);
        // The race is over when every racer has finished (0x5F85C0)
        // and the last one's hold has run (RACE_OVER); the HUD shows
        // the player's race-over message meanwhile.
        // The race is over (0x60B3B0 case 3) once every human
        // racer has finished (0x5F85C0: each player's finish tick
        // +0x50, set by 0x5EA540) and the last one's hold (+0x54,
        // RACE_OVER) has run; the AI racers still running are then
        // ended where they are (0x60AAB0 → 0x608CB0).
        let (race, _) = race_drive(&mut self.race, &mut *self.drive);
        let Some(time) = race.racers.me.finished else { return RaceTick::Go };
        if self.race_over_hold(dt) == RaceTick::Stop {
            return RaceTick::Stop;
        }
        RaceTick::Finished { time }
    }
}

/// A race phase's race and player drive (a race with a drive, as
/// [`Session::update_race`] checked), borrowing only those two fields.
pub(super) fn race_drive<'s>(
    race: &'s mut Option<&mut Race>,
    drive: &'s mut Option<Drive>,
) -> (&'s mut Race, &'s mut Drive) {
    let (Some(race), Some(d)) = (race.as_deref_mut(), drive.as_mut()) else {
        unreachable!("a race phase outside a race")
    };
    (race, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two types stay small: the input is a few copied values and
    /// shared handles, the events a few words and flags (an empty `Vec`
    /// allocates nothing; a scene's end is boxed).
    #[test]
    fn tick_input_and_events_are_small() {
        eprintln!(
            "TickInput {} B, TickEvents {} B, TickSettings {} B",
            std::mem::size_of::<TickInput>(),
            std::mem::size_of::<TickEvents>(),
            std::mem::size_of::<crate::sim::TickSettings>()
        );
        // + the tick's settings (`sim::TickSettings`, 28 B: the switches,
        // the rows the tick's phases and the tick-stepped effects read,
        // the section distances of the tick views, the chase camera's
        // Field Of View offset; 15 B with the draw's reads alone, which put
        // the input at 104 B) and free roam's career input (`RoamInput`, a
        // shared handle).
        assert!(std::mem::size_of::<TickInput>() <= 112);
        // + a scene's tick (its end boxed), free roam's encounter
        // tick, the tick's contact sounds (two queues, 48 B) and free
        // roam's world step under the slow motion (8 B).
        assert!(std::mem::size_of::<TickEvents>() <= 120);
    }
}

use super::{Session, SimSound, TickEvents, TickInput};
use crate::{
    CareerData, Profile, ShotRoad, crash_route, event_shot, jump_flight, road_car, roam_style_frame, step_explore,
};
use glam::Vec3;

/// What free roam's simulation reads from the career and the data
/// (shared and immutable for the tick, built by the glue).
pub(crate) struct RoamInput<'a> {
    /// The career profile, if any (the encounters read its stage and
    /// intro flags).
    pub(crate) profile: Option<&'a Profile>,
    /// The career's data.
    pub(crate) career: &'a CareerData,
    /// The style manager's trick definitions (the free-roam style
    /// manager's set-up).
    pub(crate) tricks: &'a [nfsu2::drift::TrickDef],
    /// The player's units option.
    pub(crate) metric: bool,
    /// A world challenge runs in a career's free roam (career +0xC08 =
    /// 0x85B948 set): no outrun is offered or engaged.
    pub(crate) challenge: bool,
}

/// The encounter manager's frame in free roam: what the career's
/// rules act on between the session's two free-roam phases.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub(crate) struct EncounterTick {
    /// The frame ran (encounters, a drive and the AI data): the outrun
    /// timer it left (the profile's `outrun_timer`).
    pub(crate) timer: Option<f32>,
    /// An outrun started this frame (0x607190 → 0x532E40).
    pub(crate) started: bool,
    /// An outrun ended this frame: whether the player won (0x5F84A0).
    pub(crate) ended: Option<bool>,
}

impl Session<'_> {
    /// Free roam's first phase (the step's part of the old
    /// `Game::explore_encounters`): the race status's frame in free roam
    /// (0x6076E0), the encounter manager and an outrun, before the cars
    /// move. The outrun's career consequences (the inbox, Rachel's hint
    /// lines and their draws on the shared generator, the bank) are the glue's, between
    /// this and [`Session::step_roam`].
    pub(super) fn encounters_frame(&mut self, dt: f32, input: &TickInput) -> EncounterTick {
        let Some(roam) = input.roam else { return EncounterTick::default() };
        let had_outrun = self.encounters.as_ref().is_some_and(|e| e.outrun.is_some());
        let mut timer = None;
        let ended = match (self.encounters.as_mut(), self.drive.as_ref(), input.ai) {
            (Some(e), Some(d), Some(ai)) => {
                let traffic: Vec<_> = self.traffic.as_ref().filter(|t| t.region == d.region).map_or(Vec::new(), |t| {
                    t.cars
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| c.road_object())
                        .map(|(i, c)| {
                            let mut rc = road_car(
                                1000 + i,
                                &c.car,
                                &c.state,
                                e.road_prev.get(&(1000 + i)).copied(),
                                false,
                                false,
                                e.road_dt.unwrap_or(dt),
                            );
                            crate::sim::ai::racer::traffic_road_path(&mut rc, c, &t.routes);
                            rc
                        })
                        .collect()
                });
                let ended = e.frame(
                    d,
                    &traffic,
                    roam.profile,
                    roam.career,
                    ai,
                    roam.metric,
                    roam.challenge,
                    dt,
                    self.traffic.as_mut().filter(|t| t.region == d.region).map(|t| &mut t.field),
                );
                timer = Some(e.timer);
                ended
            }
            _ => None,
        };
        let started = !had_outrun && self.encounters.as_ref().is_some_and(|e| e.outrun.is_some());
        EncounterTick { timer, started, ended }
    }

    /// Free roam's second phase (the old `Game::explore_drive`,
    /// after the outrun's career consequences): a won outrun's bank on
    /// the race-over message (`result_bank`, the career's answer), then
    /// the player's car, the traffic and the free-roam style manager.
    pub(crate) fn step_roam(&mut self, input: &TickInput, result_bank: Option<i32>) -> TickEvents {
        let mut events = TickEvents::default();
        let dt = input.dt;
        let controls = input.controls;
        // A win's race-over message shows the stage's bank (0x5F84A0 →
        // 0x541330).
        if let Some(bank) = result_bank
            && let Some(r) = self.encounters.as_mut().and_then(|e| e.result.as_mut())
        {
            r.1 = Some(bank);
        }
        let Some(roam) = input.roam else { return events };
        let mut before = Vec3::ZERO;
        if let Some(d) = self.drive.as_mut() {
            let traffic = self.traffic.as_mut().filter(|t| t.region == d.region);
            let tank_before = d.state.nitrous.map(|t| t.running.is_some());
            // The car's velocity before the update (`event_shot`).
            before = d.state.velocity;
            step_explore(d, traffic, self.encounters.as_mut(), controls, dt, &mut self.rng, &mut events.contacts);
            let roam_style = self.roam_style.get_or_insert_with(|| {
                (nfsu2::drift::StyleManager::new(roam.tricks.to_vec(), 1), 0.0, 0.0, Default::default())
            });
            let mut others: Vec<(Vec3, Vec3)> =
                self.traffic.as_ref().filter(|t| t.region == d.region).map_or(Vec::new(), |t| {
                    t.cars.iter().filter(|c| c.lane.is_some()).map(|c| (c.state.position, c.state.velocity)).collect()
                });
            if let Some(e) = self.encounters.as_ref() {
                others.extend(e.cars.iter().filter(|c| c.active).map(|c| (c.state.position, c.state.velocity)));
            }
            let shortcut = input.route_triggers.shortcut_at(d.state.position.truncate());
            roam_style_frame(roam_style, d, &others, shortcut, tank_before, dt);
        }
        self.roam_events(dt, before, &mut events);
        events
    }

    /// The world's step in free roam (0x5EA370): the tick's dt times the
    /// slow motion's rate while an event camera runs; its end runs the
    /// event's end (0x6022E0).
    pub(super) fn roam_world_dt(&mut self, dt: f32) -> f32 {
        let Some(cam) = self.roam_camera.as_mut() else { return dt };
        let (dt, expired) = cam.events.slow.advance(dt);
        if expired {
            cam.events.end();
        }
        if let Some(d) = self.drive.as_mut() {
            d.impact.follow(&cam.events);
        }
        dt
    }

    /// Free roam's event cameras after the cars moved, as a race's
    /// (`update_race_live`): free roam runs with race status 4 (observed
    /// under Wine), so Car_Crash's camera test (0x5A0095..0x5A0105) and
    /// Car_UpdateRaceEvents (0x604750, from the world update's car loop at
    /// 0x60A086, free roam included) start the crash and jump cameras;
    /// the placement walks the human car's route on the free-roam routes,
    /// the record's progress is 0 and its end 1.0 (no race road).
    /// Observed under Wine: a head-on hit on traffic in career free roam
    /// started kind 6 (slow motion 0.4 for 8 s, mover 0x0F, the HUD gone).
    fn roam_events(&mut self, dt: f32, before: Vec3, events: &mut TickEvents) {
        let (Some(d), Some(cam)) = (self.drive.as_mut(), self.roam_camera.as_mut()) else { return };
        let routes = self.encounters.as_ref().map(|e| &e.routes);
        let centre = d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
        let s = nfsu2::event_camera::RaceState {
            racing: true,
            drift: false,
            crash_mode: d.state.crash,
            wheels_down: d.state.wheels_down(),
            speed: d.state.speed(),
            climb: d.state.velocity.z,
            height: centre.z,
        };
        let mut crash_velocity = None;
        if std::mem::take(&mut d.state.crash_hit) {
            let placed = crash_route(d, routes).is_some();
            if cam.events.crash(&s, placed) {
                events.sounds.push(SimSound::CrashCamera);
                crash_velocity = Some(before);
            }
        }
        {
            let dd = &*d;
            cam.events
                .update_with(dt, &s, &mut |window| jump_flight(dd, window), &mut || crash_route(dd, routes).is_some());
        }
        let clock = cam.events.clock;
        // The views after the event rules read their kind and rate.
        d.impact.follow(&cam.events);
        event_shot(d, cam, routes, None::<ShotRoad>, clock, dt, crash_velocity);
        d.state.crash_camera = cam.events.kind == 6;
    }
}

use super::{RaceTick, Session, SimSound, TickEvents, TickInput, race_drive};
use crate::{
    Controls, RACE_OVER, RACE_OVER_EVENT, Recording, ReplayFrame, crash_camera, crash_route, jump_flight, race_frame,
};
use glam::Vec3;

impl Session<'_> {
    /// The race after the green light: the recorded frame, the race's
    /// step and its event cameras.
    pub(super) fn update_race_live(&mut self, dt: f32, input: &TickInput, events: &mut TickEvents) {
        let controls = input.controls;
        // The race's human input object (car +0x30, new at the countdown)
        // holds the analog brake byte +0x2D9 = 1 (event 0x95; its writers
        // are only the handler 0x407EB0 and the reset 0x4076E8), and its
        // brake is max(+0x2D9, +0x2DA) × 1/255 (0x407F24) while the brake
        // key's +0x2D8 is 0: so from the green the idle brake pedal +0x214
        // is 1/255, kept while no key (**measured**; through
        // the countdown the start's hold gives 1.0). Which device state
        // sends 0x95 = 1 at the handlers' registration (0x579420 →
        // 0x579370) is **open**.
        // prov: open game.race_idle_brake
        let controls = Controls { brake: controls.brake.max(0.003921569), ..controls };
        let (race, d) = race_drive(&mut self.race, &mut *self.drive);
        // The green light: the replay's snapshot, then each
        // frame's inputs, with a LAP_SNAPSHOT when one is due
        // (not on a frame with a shift or reset, which the
        // key made before the frame: inferred).
        // The green light releases the race start's hold.
        if d.state.held {
            d.state.release_hold(d.car.car_type.handling.race);
        }
        for r in race.racers.rivals.iter_mut().filter(|r| r.state.held) {
            r.state.release_hold(r.car.car_type.handling.race);
        }
        // Race state 4 (0x5FC910 → the AI racer controller's
        // notice 0x429B70, vtable 0x785370 slot 5): the
        // driver's vt+0x1C(state ≠ 4) clears the DriftDriver's
        // hold +0x23C (0x405B10).
        for d in race.racers.rivals.iter_mut().filter_map(|r| r.drift.as_mut()) {
            d.hold = false;
        }
        let mut rec = race.replay.recording.take().unwrap_or_else(|| Recording::new(d, race));
        // The world's step under the slow motion (0x5EA370); its
        // end runs the event's end (0x6022E0).
        let (sim_dt, expired) = race.camera.events.slow.advance(dt);
        if expired {
            race.camera.events.end();
        }
        d.impact.follow(&race.camera.events);
        let frame = ReplayFrame {
            controls,
            dt: sim_dt,
            shift: rec.shift.take(),
            reset: std::mem::take(&mut rec.reset),
            stability: rec.stability.take(),
        };
        if frame.shift.is_none() && !frame.reset {
            rec.instant_replay(d, race);
            rec.lap_snapshot(d, race);
        }
        rec.frames.push(frame);
        rec.elapsed += sim_dt;
        rec.tick(sim_dt);
        race.replay.recording = Some(rec);
        // The car's velocity before the update (car +0x70 when Car_Crash
        // runs inside a contact: `event_shot`).
        let before = d.state.velocity;
        race_frame(d, race, &frame, input.ai, input.route_triggers, input.lap_info, false, &mut self.rng, &mut events.contacts);
        // The crash camera (0x59FB80 → 0x6022F0) and the event
        // rules of 0x604750 (nfsu2::event_camera).
        let centre = d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
        let s = nfsu2::event_camera::RaceState {
            racing: race.timing.clock >= 0.0 && race.racers.me.finished.is_none(),
            drift: race.scoring.drift.is_some(),
            crash_mode: d.state.crash,
            wheels_down: d.state.wheels_down(),
            speed: d.state.speed(),
            climb: d.state.velocity.z,
            height: centre.z,
        };
        let mut crash_velocity = None;
        if std::mem::take(&mut d.state.crash_hit) {
            let placed = crash_route(d, race.racers.ai_routes.as_ref()).is_some();
            // 0x5A0105: the crash camera started (0x6022F0) →
            // 0x486130, the CollisionStream request (an event the
            // audio glue sets on the tick, `SimSound::CrashCamera`).
            if race.camera.events.crash(&s, placed) {
                events.sounds.push(SimSound::CrashCamera);
                crash_velocity = Some(before);
            }
        }
        {
            let (dd, routes) = (&*d, race.racers.ai_routes.as_ref());
            race.camera.events.update_with(sim_dt, &s, &mut |window| jump_flight(dd, window), &mut || {
                crash_route(dd, routes).is_some()
            });
        }
        // The views after the event rules read their kind and rate.
        d.impact.follow(&race.camera.events);
        crash_camera(d, race, sim_dt, crash_velocity);
        d.state.crash_camera = race.camera.events.kind == 6;
    }

    /// The player has finished: the race-over hold runs; `RaceTick::Stop`
    /// until it has.
    pub(super) fn race_over_hold(&mut self, dt: f32) -> RaceTick {
        let (race, _) = race_drive(&mut self.race, &mut *self.drive);
        let hold =
            *race.timing.hold.get_or_insert(if matches!(race.camera.events.kind, 6 | 7) { RACE_OVER_EVENT } else { RACE_OVER });
        race.over += dt;
        if race.over < hold {
            return RaceTick::Stop;
        }
        RaceTick::Go
    }
}
